//! Labels as Mermaid's journey and timeline draw them. Ported from pele's
//! `diagrams/common/fit.ts`.

use crate::text::label::{Label, layout_label};
use crate::text::{Style, TextMeasurer};
use crate::util::{is_space, trim};

/// Lays out text as plain text in which only `<br>` is markup, so other tags and icon names
/// show as written instead of being read as formatting.
pub(crate) fn plain_label(
    raw: Option<&str>,
    measurer: &dyn TextMeasurer,
    size: f64,
    max: f64,
    base: Style,
) -> Label {
    let text = raw.map(|raw| {
        let mut text = escape_tags(raw);
        if text.contains(":fa-") {
            text = text.replace(":fa-", "&colon;fa-");
        }
        text
    });
    layout_label(text.as_deref(), false, measurer, size, max, base)
}

/// The length of a `<br>` tag at the start of `s`, matching `/<br\s*\/?>/i`.
fn br_at(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.len() < 3 || b[0] != b'<' || !b[1..3].eq_ignore_ascii_case(b"br") {
        return None;
    }
    let mut i = 3;
    while let Some(c) = s[i..].chars().next().filter(|&c| is_space(c)) {
        i += c.len_utf8();
    }
    if s[i..].starts_with('/') {
        i += 1;
    }
    s[i..].starts_with('>').then_some(i + 1)
}

/// `text.replace(/<(?!br\s*\/?>)/gi, '&lt;')`.
fn escape_tags(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for (i, c) in raw.char_indices() {
        if c == '<' && br_at(&raw[i..]).is_none() {
            out.push_str("&lt;");
        } else {
            out.push(c);
        }
    }
    out
}

/// The length of the unit at the start of `s` (`s` not empty): a line break, an encoded
/// entity, or one character, the pieces a word can be cut between. Whether it is one character
/// is returned too.
fn unit(s: &str) -> (usize, bool) {
    if let Some(n) = br_at(s) {
        return (n, false);
    }
    let words = |from: usize| {
        s[from..]
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
            .count()
    };
    // pele's encoded entities: `ﬂ°°` or `ﬂ°` (as `°?` makes the second optional), a word, `¶ß`.
    if let Some(rest) = s.strip_prefix("ﬂ°") {
        let skip = s.len() - rest.len()
            + if rest.starts_with('°') {
                '°'.len_utf8()
            } else {
                0
            };
        let k = words(skip);
        if k > 0 && s[skip + k..].starts_with("¶ß") {
            return (skip + k + "¶ß".len(), false);
        }
    }
    if let Some(rest) = s.strip_prefix('&') {
        let skip = 1 + usize::from(rest.starts_with('#'));
        let k = words(skip);
        if k > 0 && s[skip + k..].starts_with(';') {
            return (skip + k + 1, false);
        }
    }
    let c = s.chars().next().expect("not empty");
    (c.len_utf8(), true)
}

/// `layout_label` wraps at spaces only. For a box of fixed width, this also cuts any word too
/// long to fit.
#[cfg_attr(
    not(any(feature = "timeline", feature = "journey")),
    allow(dead_code, reason = "read by the timeline and journey diagrams")
)]
pub(crate) fn fit_label(
    raw: Option<&str>,
    measurer: &dyn TextMeasurer,
    size: f64,
    max: f64,
    base: Style,
) -> Label {
    let label = plain_label(raw, measurer, size, max, base);
    let Some(raw) = raw else {
        return label;
    };
    if label.width <= max {
        return label;
    }
    let mut out = String::with_capacity(raw.len() + 16);
    let mut width = 0.0;
    let mut i = 0;
    while i < raw.len() {
        let (n, single) = unit(&raw[i..]);
        let piece = &raw[i..i + n];
        if trim(piece).is_empty() || (!single && piece.starts_with('<')) {
            width = 0.0;
        } else {
            let w = measurer.width(if single { piece } else { "x" }, size, base);
            if width > 0.0 && width + w > max {
                out.push('\n');
                width = 0.0;
            }
            width += w;
        }
        out.push_str(piece);
        i += n;
    }
    plain_label(Some(&out), measurer, size, max, base)
}
