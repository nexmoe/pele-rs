//! A bounded cache around a host's font measurements. It owns no device or font database.

use super::{Host, Style, TextMeasurer};
use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
};

type Key = (String, u64, Style);
#[derive(Default)]
struct Cache {
    widths: HashMap<Key, f64>,
    order: VecDeque<Key>,
}

/// Reuse measurements across layouts with different widths. Create a new cache or call
/// `clear` when the host changes fonts or fallback rules. Capacity is a number of text runs.
pub struct CachingMeasurer<M> {
    inner: M,
    capacity: usize,
    cache: Mutex<Cache>,
}

impl<M: TextMeasurer> CachingMeasurer<M> {
    pub fn new(inner: M, capacity: usize) -> Self {
        Self {
            inner,
            capacity,
            cache: Mutex::default(),
        }
    }
    pub fn clear(&self) {
        *self.cache.lock().unwrap_or_else(|e| e.into_inner()) = Cache::default();
    }
    pub fn len(&self) -> usize {
        self.cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .widths
            .len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn inner(&self) -> &M {
        &self.inner
    }
}

impl<M: TextMeasurer> TextMeasurer for CachingMeasurer<M> {
    fn width(&self, text: &str, size: f64, style: Style) -> f64 {
        if self.capacity == 0 {
            return self.inner.width(text, size, style);
        }
        let key = (text.to_string(), size.to_bits(), style);
        if let Some(width) = self
            .cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .widths
            .get(&key)
            .copied()
        {
            return width;
        }
        // The host callback may be reentrant. Never hold the cache lock while measuring.
        let width = self.inner.width(text, size, style);
        if width.is_finite() && width >= 0.0 {
            let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            if !cache.widths.contains_key(&key) {
                if cache.widths.len() == self.capacity {
                    if let Some(old) = cache.order.pop_front() {
                        cache.widths.remove(&old);
                    }
                }
                cache.order.push_back(key.clone());
                cache.widths.insert(key, width);
            }
        }
        width
    }
    fn host(&self) -> Option<&Host<'_>> {
        self.inner.host()
    }
}
