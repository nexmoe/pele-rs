//! What the host of the library draws for a label: icons and typeset math. Ported from pele's
//! `text/icons.ts` and `text/math.ts`.
//!
//! pele keeps the host's resolvers in module state for the render in progress. Rust has no
//! such state to borrow from, so the resolvers travel with the text measurer, which every label
//! layout is given already: [`TextMeasurer::host`](super::TextMeasurer::host).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::entities::decode_entities;
use super::{Style, TextMeasurer};

/// Gives the SVG markup of the icon with a name, such as `fa:fa-car` or `logos:aws`, or `None`
/// when the host has no such icon. The markup is drawn in a 24 by 24 box, with
/// `stroke="currentColor"` set on it, and a host that wants a filled icon sets `fill` itself.
///
/// The markup is put into the SVG as it is: it is the host's, and it is trusted.
pub type IconResolver = Box<dyn Fn(&str) -> Option<String>>;

/// Typesets TeX, given without its `$$` delimiters, for text of a font size in pixels. `None`
/// leaves the text as it was written, with its delimiters.
pub type MathResolver = Box<dyn Fn(&str, f64) -> Option<MathMarkup>>;

/// Typeset math. The sizes are in pixels. A size that is not positive and finite makes the
/// markup unusable, and the text is left as it was written.
///
/// The markup is put into the SVG as it is: it is the host's, and it is trusted. `Html` is
/// drawn in a `foreignObject`, and needs the CSS of the typesetter on the page.
#[derive(Debug, Clone, PartialEq)]
pub enum MathMarkup {
    Svg {
        svg: String,
        width: f64,
        height: f64,
    },
    Html {
        html: String,
        width: f64,
        height: f64,
    },
}

impl MathMarkup {
    pub fn width(&self) -> f64 {
        match self {
            MathMarkup::Svg { width, .. } | MathMarkup::Html { width, .. } => *width,
        }
    }

    pub fn height(&self) -> f64 {
        match self {
            MathMarkup::Svg { height, .. } | MathMarkup::Html { height, .. } => *height,
        }
    }

    fn is_usable(&self) -> bool {
        let valid = |size: f64| size.is_finite() && size > 0.0;
        valid(self.width()) && valid(self.height())
    }
}

/// The font size, by its bits, and the TeX.
type MathKey = (u64, String);

/// The resolvers of the render in progress, with what they have answered. A label leaves room
/// for an icon only when the host has one by that name, and layout has to know that before
/// anything is drawn, so every answer is kept for the rest of the render.
#[doc(hidden)]
pub struct Host<'a> {
    icons: Option<&'a IconResolver>,
    math: Option<&'a MathResolver>,
    found_icons: RefCell<HashMap<String, Option<Rc<str>>>>,
    found_math: RefCell<HashMap<MathKey, Option<Rc<MathMarkup>>>>,
}

impl<'a> Host<'a> {
    pub(crate) fn new(icons: Option<&'a IconResolver>, math: Option<&'a MathResolver>) -> Self {
        Host {
            icons,
            math,
            found_icons: RefCell::default(),
            found_math: RefCell::default(),
        }
    }

    /// Whether `$$…$$` in a label is typeset.
    pub(crate) fn math_active(&self) -> bool {
        self.math.is_some()
    }

    /// The markup of an icon. The name is read as text, so its entity codes are decoded.
    pub(crate) fn icon(&self, name: &str) -> Option<Rc<str>> {
        let resolve = self.icons?;
        let name = decode_entities(name);
        if let Some(found) = self.found_icons.borrow().get(&name) {
            return found.clone();
        }
        let markup = resolve(&name)
            .filter(|markup| !markup.is_empty())
            .map(Rc::from);
        self.found_icons.borrow_mut().insert(name, markup.clone());
        markup
    }

    /// The typeset form of `tex` at a font size, when the host has one.
    pub(crate) fn typeset(&self, tex: &str, size: f64) -> Option<Rc<MathMarkup>> {
        let resolve = self.math?;
        let key = (size.to_bits(), tex.to_string());
        if let Some(found) = self.found_math.borrow().get(&key) {
            return found.clone();
        }
        let markup = resolve(tex, size)
            .filter(MathMarkup::is_usable)
            .map(Rc::new);
        self.found_math.borrow_mut().insert(key, markup.clone());
        markup
    }
}

/// A measurer that brings the host's resolvers to the label layout.
pub(crate) struct HostMeasurer<'a> {
    inner: &'a dyn TextMeasurer,
    host: Host<'a>,
}

impl<'a> HostMeasurer<'a> {
    pub(crate) fn new(inner: &'a dyn TextMeasurer, host: Host<'a>) -> Self {
        HostMeasurer { inner, host }
    }
}

impl TextMeasurer for HostMeasurer<'_> {
    fn width(&self, text: &str, size: f64, style: Style) -> f64 {
        self.inner.width(text, size, style)
    }

    fn host(&self) -> Option<&Host<'_>> {
        Some(&self.host)
    }
}

/// A measurer for text that is not typeset, as the title of a diagram.
pub(crate) struct PlainMeasurer<'a> {
    inner: &'a dyn TextMeasurer,
    host: Option<Host<'a>>,
}

impl<'a> PlainMeasurer<'a> {
    pub(crate) fn new(inner: &'a dyn TextMeasurer) -> Self {
        let host = inner.host().map(|host| {
            // The icons are the host's, and the new host keeps asking them.
            Host::new(host.icons, None)
        });
        PlainMeasurer { inner, host }
    }
}

impl TextMeasurer for PlainMeasurer<'_> {
    fn width(&self, text: &str, size: f64, style: Style) -> f64 {
        self.inner.width(text, size, style)
    }

    fn host(&self) -> Option<&Host<'_>> {
        self.host.as_ref()
    }
}
