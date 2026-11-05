//! Text measurement and label layout.

mod cache;
pub(crate) mod entities;
mod host;
pub use cache::CachingMeasurer;
#[allow(
    dead_code,
    reason = "parts are used only by diagram types that are not compiled in or not written yet"
)]
pub(crate) mod label;

use std::ops::{BitAnd, BitOr, BitOrAssign, Not};

pub(crate) use host::{Host, HostMeasurer, PlainMeasurer};
pub use host::{IconResolver, MathMarkup, MathResolver};

/// The emphasis a run of text is drawn with, as a set of flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Style(u8);

impl Style {
    pub const NONE: Style = Style(0);
    pub const BOLD: Style = Style(1);
    pub const ITALIC: Style = Style(2);
    pub const MONO: Style = Style(4);
    pub const SUB: Style = Style(8);
    pub const SUP: Style = Style(16);
    pub const UNDERLINE: Style = Style(32);
    const ALL: u8 = 63;

    /// Whether any flag of `other` is set.
    pub fn has(self, other: Style) -> bool {
        self.0 & other.0 != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl BitOr for Style {
    type Output = Style;
    fn bitor(self, rhs: Style) -> Style {
        Style(self.0 | rhs.0)
    }
}

impl BitOrAssign for Style {
    fn bitor_assign(&mut self, rhs: Style) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for Style {
    type Output = Style;
    fn bitand(self, rhs: Style) -> Style {
        Style(self.0 & rhs.0)
    }
}

impl Not for Style {
    type Output = Style;
    fn not(self) -> Style {
        Style(!self.0 & Style::ALL)
    }
}

/// The markup of the icon the host has by that name, if it has one.
///
/// Only the types that draw icons call it, so a build without them would not use it.
#[cfg(any(
    feature = "flowchart",
    feature = "usecase",
    feature = "mindmap",
    feature = "architecture",
    feature = "kanban",
))]
pub(crate) fn icon_markup(measurer: &dyn TextMeasurer, name: &str) -> Option<std::rc::Rc<str>> {
    measurer.host()?.icon(name)
}

/// Measures how wide text is drawn, in pixels.
///
/// The default, [`MetricsMeasurer`], estimates widths from font tables. A host that knows the
/// fonts the SVG will be shown in can measure them instead.
pub trait TextMeasurer {
    fn width(&self, text: &str, size: f64, style: Style) -> f64;

    /// The icons and the math the host of the render draws. A measurer has none of its own;
    /// the library wraps it in one that has, when [`RenderOptions`](crate::RenderOptions) gives
    /// resolvers.
    #[doc(hidden)]
    fn host(&self) -> Option<&Host<'_>> {
        None
    }
}

/// Approximate divisor for CSS `font-size="smaller"`.
const SMALLER: f64 = 1.2;

// Advance widths of printable ASCII in thousandths of an em, two base-36 digits each. Taken
// from pele's `text/metrics.ts`, which generates them from Arial.
const REGULAR: [u16; 95] = unpack(
    b"7q7q9vfgfgopij5b9999atg87q997q7qfgfgfgfgfgfgfgfgfgfg7q7qg8g8g8fgs7ijijk2k2ijgzlmk27qdwijfgn5k2lmijlmk2ijgzk2ijq8ijijgz7q7q7qd1fg99fgfgdwfgfg7qfgfg6666dw66n5fgfgfgfg99dw7qfgdwk2dwdwdw9a789ag8",
);
const BOLD: [u16; 95] = unpack(
    b"7q99d6fgfgopk26m9999atg87q997q7qfgfgfgfgfgfgfgfgfgfg9999g8g8g8gzr3k2k2k2k2ijgzlmk27qfgk2gzn5k2lmijlmk2ijgzk2ijq8ijijgz997q99g8fg99fggzfggzfg99gzgz7q7qfg7qopgzgzgzgzatfg99gzfglmfgfgdwat7satg8",
);

const fn digit36(c: u8) -> u16 {
    match c {
        b'0'..=b'9' => (c - b'0') as u16,
        b'a'..=b'z' => (c - b'a') as u16 + 10,
        _ => panic!("not a base-36 digit"),
    }
}

const fn unpack(packed: &[u8; 190]) -> [u16; 95] {
    let mut out = [0; 95];
    let mut i = 0;
    while i < 95 {
        out[i] = digit36(packed[i * 2]) * 36 + digit36(packed[i * 2 + 1]);
        i += 1;
    }
    out
}

fn is_wide(c: u32) -> bool {
    matches!(
        c,
        0x1100..=0x115f
            | 0x2e80..=0xa4cf
            | 0xac00..=0xd7a3
            | 0xf900..=0xfaff
            | 0xfe30..=0xfe4f
            | 0xff00..=0xff60
            | 0xffe0..=0xffe6
            // Beyond the BMP. pele sees a surrogate pair here: a wide high half and a
            // zero-width low half.
            | 0x10000..
    )
}

fn is_zero_width(c: u32) -> bool {
    matches!(c, 0xad | 0x300..=0x36f | 0x200b..=0x200f | 0xfe00..=0xfe0f | 0xfeff)
}

/// Estimates widths from tables of Arial advances, the way pele does where no canvas is
/// available. Results are deterministic and need no font files.
#[derive(Debug, Clone, Copy, Default)]
pub struct MetricsMeasurer;

impl TextMeasurer for MetricsMeasurer {
    fn width(&self, text: &str, size: f64, style: Style) -> f64 {
        let mono = style.has(Style::MONO);
        let eff = if style.has(Style::SUB | Style::SUP) {
            size / SMALLER
        } else {
            size
        };
        let table = if style.has(Style::BOLD) {
            &BOLD
        } else {
            &REGULAR
        };
        let mut units: u32 = 0;
        for c in text.chars() {
            let c = c as u32;
            if c < 32 || is_zero_width(c) {
                continue;
            }
            units += if c < 127 {
                if mono {
                    600
                } else {
                    u32::from(table[c as usize - 32])
                }
            } else if is_wide(c) {
                if mono { 1200 } else { 1000 }
            } else if mono {
                600
            } else {
                580
            };
        }
        f64::from(units) * eff / 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, size: f64, style: Style) -> f64 {
        MetricsMeasurer.width(text, size, style)
    }

    // Expected values come from running pele's metricsMeasurer on the same input.
    #[test]
    fn matches_pele_metrics() {
        let cases: &[(&str, f64, Style, f64)] = &[
            ("Hello, World!", 16.0, Style::NONE, 91.568),
            ("Hello, World!", 16.0, Style::BOLD, 98.672),
            ("Hello", 14.0, Style::MONO, 42.0),
            ("x²", 16.0, Style::SUB, 14.4),
            ("中文 abc", 16.0, Style::NONE, 62.24),
            ("中文", 16.0, Style::MONO, 38.4),
            ("e\u{301}\u{200b}", 10.0, Style::NONE, 5.56),
            ("😀a", 16.0, Style::NONE, 24.896),
            ("tab\there", 16.0, Style::NONE, 54.256),
        ];
        for &(text, size, style, expected) in cases {
            let got = w(text, size, style);
            assert!(
                (got - expected).abs() < 1e-3,
                "{text:?} {style:?}: got {got}, expected {expected}"
            );
        }
    }

    #[test]
    fn style_operations() {
        let s = Style::BOLD | Style::ITALIC;
        assert!(s.has(Style::BOLD));
        assert!(!s.has(Style::MONO));
        assert_eq!(s & !Style::BOLD, Style::ITALIC);
        assert!((s & Style::MONO).is_empty());
    }
}
