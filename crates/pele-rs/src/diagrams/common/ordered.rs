//! An insertion-ordered map, as a JavaScript `Map` iterates.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) struct Ordered<V> {
    pub items: Vec<(String, V)>,
    index: HashMap<String, usize>,
}

impl<V> Default for Ordered<V> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            index: HashMap::new(),
        }
    }
}

impl<V> Ordered<V> {
    pub(crate) fn get(&self, key: &str) -> Option<&V> {
        self.index.get(key).map(|&i| &self.items[i].1)
    }

    #[cfg_attr(
        not(any(feature = "class", feature = "er", feature = "block")),
        allow(dead_code, reason = "used by the types that change a value in place")
    )]
    pub(crate) fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        self.index.get(key).map(|&i| &mut self.items[i].1)
    }

    #[cfg_attr(not(feature = "class"), allow(dead_code))]
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    #[cfg_attr(not(feature = "class"), allow(dead_code))]
    pub(crate) fn values(&self) -> impl Iterator<Item = &V> {
        self.items.iter().map(|(_, v)| v)
    }

    /// The place of a key in insertion order.
    #[cfg_attr(not(feature = "class"), allow(dead_code))]
    pub(crate) fn position(&self, key: &str) -> Option<usize> {
        self.index.get(key).copied()
    }

    #[cfg_attr(
        not(any(feature = "class", feature = "er", feature = "requirement")),
        allow(
            dead_code,
            reason = "used by the types that ask whether a key is there"
        )
    )]
    pub(crate) fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    /// Sets a value; a key already there keeps its place.
    pub(crate) fn set(&mut self, key: &str, value: V) {
        match self.index.get(key) {
            Some(&i) => self.items[i].1 = value,
            None => {
                self.index.insert(key.to_string(), self.items.len());
                self.items.push((key.to_string(), value));
            }
        }
    }
}
