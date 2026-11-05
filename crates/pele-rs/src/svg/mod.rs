//! Building blocks for SVG output: escaping, number formatting, labels and the document root.
//! Ported from pele's `svg/` modules.

#[cfg(any(
    feature = "flowchart",
    feature = "class",
    feature = "er",
    feature = "usecase",
    feature = "agentflow",
))]
pub(crate) mod cluster;
mod document;
#[cfg(any(
    feature = "flowchart",
    feature = "state",
    feature = "class",
    feature = "er",
    feature = "requirement",
    feature = "block",
    feature = "usecase",
    feature = "agentflow",
    feature = "c4",
))]
pub(crate) mod edges;
#[cfg(any(
    feature = "flowchart",
    feature = "state",
    feature = "class",
    feature = "er",
    feature = "requirement",
    feature = "block",
    feature = "usecase",
    feature = "agentflow",
    feature = "c4"
))]
pub(crate) mod fills;
pub(crate) mod ink;
#[cfg(any(
    feature = "flowchart",
    feature = "sequence",
    feature = "state",
    feature = "class",
    feature = "er",
    feature = "requirement",
    feature = "timeline",
    feature = "block",
    feature = "ishikawa",
    feature = "usecase",
    feature = "agentflow",
    feature = "c4",
    feature = "wardley",
    feature = "eventmodeling",
))]
pub(crate) mod markers;
mod resolve;
#[cfg(any(
    feature = "flowchart",
    feature = "state",
    feature = "class",
    feature = "er",
    feature = "mindmap",
    feature = "block",
    feature = "c4",
    feature = "usecase",
    feature = "agentflow",
))]
#[cfg_attr(
    not(any(
        feature = "flowchart",
        feature = "state",
        feature = "class",
        feature = "er"
    )),
    allow(dead_code, reason = "mind maps use only part of it")
)]
pub(crate) mod shapes;
mod theme;
mod title;

pub(crate) use document::{Accessible, svg_document};
pub(crate) use resolve::resolve_vars;
pub(crate) use theme::{
    CRITICAL, FONT_MONO, ROOT_STYLE, ROUNDED, ResolvedStyle, class_names, font_style, parse_float,
    resolve_style, rounded, series_color, sets_fill,
};
pub(crate) use title::{title_label, with_title};

use std::fmt::Write;

use crate::text::entities::decode_entities;
use crate::text::label::Label;
use crate::text::{MathMarkup, Style};

/// Escapes text for XML and drops the control characters XML does not allow.
pub(crate) fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\0'..='\x08'
            | '\x0b'
            | '\x0c'
            | '\x0e'..='\x1f'
            | '\x7f'
            | '\u{fffe}'
            | '\u{ffff}' => {}
            c => out.push(c),
        }
    }
    out
}

/// Escapes text that is only ever read as text: a tooltip, an accessible name, a `data-id`.
/// Entity codes are turned into their characters first. A value that was checked before it is
/// written, such as a style, a colour or a URL, must go through [`esc`] instead, so that what
/// was checked is what is written.
pub(crate) fn esc_text(text: &str) -> String {
    esc(&decode_entities(text))
}

/// Writes a coordinate with at most two decimals and no trailing zeros.
pub(crate) fn num(value: f64) -> String {
    if !value.is_finite() {
        NUMERIC_ERRORS.with(|s| {
            if let Some(invalid) = s.borrow_mut().last_mut() {
                *invalid = true;
            }
        });
    }
    let n = crate::util::round(value * 100.0);
    if n == 0.0 || !n.is_finite() {
        return "0".to_string();
    }
    if n.abs() >= 1e11 {
        return crate::util::js_number(n / 100.0);
    }
    let neg = n < 0.0;
    let m = n.abs() as u64;
    let (q, r) = (m / 100, m % 100);
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    let _ = write!(out, "{q}");
    if r != 0 {
        if r % 10 == 0 {
            let _ = write!(out, ".{}", r / 10);
        } else {
            let _ = write!(out, ".{r:02}");
        }
    }
    out
}

const BOLD: &str = " font-weight=\"var(--_w)\"";
const ITALIC: &str = " font-style=\"italic\"";
const UNDERLINE: &str = " text-decoration=\"underline\"";
const MONO: &str = " font-family=\"var(--_fm)\"";
const SUB: &str = " baseline-shift=\"sub\" font-size=\"smaller\"";
const SUP: &str = " baseline-shift=\"super\" font-size=\"smaller\"";

pub(crate) fn span_style(style: Style) -> String {
    let mut out = String::new();
    for (flag, attrs) in [
        (Style::BOLD, BOLD),
        (Style::ITALIC, ITALIC),
        (Style::MONO, MONO),
        (Style::UNDERLINE, UNDERLINE),
    ] {
        if style.has(flag) {
            out.push_str(attrs);
        }
    }
    if style.has(Style::SUB) {
        out.push_str(SUB);
    } else if style.has(Style::SUP) {
        out.push_str(SUP);
    }
    out
}

/// The emphasis every span of a label has. It is written once, on the text element, where a
/// CSS rule for the label reaches it.
pub(crate) fn shared_style(label: &Label) -> Style {
    let mut shared = Style::BOLD | Style::ITALIC | Style::MONO;
    let mut any = false;
    for span in label.lines.iter().flatten() {
        if span.icon.is_some() || span.text.is_empty() {
            continue;
        }
        shared = shared & span.style;
        any = true;
    }
    if any { shared } else { Style::NONE }
}

/// Adds that emphasis to a text element's attributes, unless they give a weight of their own.
pub(crate) fn text_style(attrs: &str, shared: Style) -> String {
    let mut out = attrs.to_string();
    if shared.has(Style::BOLD) && !attrs.contains(" font-weight=\"") {
        out.push_str(BOLD);
    }
    if shared.has(Style::ITALIC) && !attrs.contains(" font-style=\"") {
        out.push_str(ITALIC);
    }
    if shared.has(Style::MONO) && !attrs.contains(" font-family=\"") {
        out.push_str(MONO);
    }
    out
}

/// Writes a measured label as SVG text centered on (cx, cy). Icons and math are written beside
/// the text, each at the place its span has in the line.
pub(crate) fn label_svg(label: &Label, cx: f64, cy: f64, attrs: &str) -> String {
    let lines = &label.lines;
    if lines.is_empty() {
        return String::new();
    }
    let lh = label.line_height;
    let shift = label.size * 0.35;
    let mut top = cy - label.height / 2.0;

    if let [line] = lines.as_slice() {
        if let [only] = line.as_slice() {
            if only.is_plain() && only.style.is_empty() {
                return format!(
                    "<text{attrs} x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
                    num(cx),
                    num(top + lh / 2.0 + shift),
                    esc(&only.text)
                );
            }
        }
    }

    let shared = shared_style(label);
    let given = attrs;
    let attrs = text_style(attrs, shared);
    let mut math_look: Option<MathLook> = None;
    let mut body = String::new();
    let mut extra = String::new();
    for (i, line) in lines.iter().enumerate() {
        let h = label.heights.as_ref().map_or(lh, |heights| heights[i]);
        let mid = top + h / 2.0;
        let y = mid + shift;
        if line.iter().any(|span| !span.is_plain()) {
            // Text beside an icon or math is placed span by span.
            let mut x = cx - label.widths[i] / 2.0;
            for span in line {
                if let Some(math) = &span.math {
                    let boxed = format!(
                        "x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                        num(x),
                        num(mid - math.height() / 2.0),
                        num(math.width()),
                        num(math.height())
                    );
                    let look = math_look.get_or_insert_with(|| MathLook::of(given));
                    match &look.halo {
                        None => extra += &math_svg(math, &boxed, label.size, &look.attrs),
                        Some(halo) => {
                            let _ = write!(extra, "<rect {boxed} rx=\"2\" fill=\"{halo}\"/>");
                        }
                    }
                } else if let Some(icon) = &span.icon {
                    let side = label.size * 1.1;
                    extra += &icon_svg(&icon.name, x, mid - side / 2.0, side, &icon.markup);
                } else if !span.text.is_empty() {
                    let _ = write!(
                        body,
                        "<tspan x=\"{}\" y=\"{}\" text-anchor=\"start\"{}>{}</tspan>",
                        num(x),
                        num(y),
                        span_style(span.style & !shared),
                        esc(&span.text)
                    );
                }
                x += span.width;
            }
        } else {
            for (j, span) in line.iter().enumerate() {
                let pos = if j == 0 {
                    format!(" x=\"{}\" y=\"{}\"", num(cx), num(y))
                } else {
                    String::new()
                };
                let _ = write!(
                    body,
                    "<tspan{pos}{}>{}</tspan>",
                    span_style(span.style & !shared),
                    esc(&span.text)
                );
            }
            if line.is_empty() {
                let _ = write!(body, "<tspan x=\"{}\" y=\"{}\"> </tspan>", num(cx), num(y));
            }
        }
        top += h;
    }
    format!("<text{attrs} text-anchor=\"middle\" xml:space=\"preserve\">{body}</text>{extra}")
}

/// A square for a named icon, with the host's drawing of it. The markup is the host's, and is
/// trusted.
pub(crate) fn icon_svg(name: &str, x: f64, y: f64, side: f64, markup: &str) -> String {
    format!(
        "<svg class=\"pele-icon\" data-icon=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{markup}</svg>",
        esc_text(name),
        num(x),
        num(y),
        num(side),
        num(side)
    )
}

/// What typeset math takes from the text it stands in: its color and opacity only, so that the
/// size the host measured stays. A cluster title is drawn with an outline that hides the edges
/// behind it, and math there gets a solid patch of that color instead.
struct MathLook {
    attrs: String,
    halo: Option<String>,
}

/// The value of the attribute ` name="…"`, the first one in `attrs`.
fn attribute<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let at = attrs.find(&key)? + key.len();
    attrs[at..].split('"').next()
}

/// The value of a property in a `style` attribute.
fn property<'a>(style: &'a str, name: &str) -> Option<&'a str> {
    style.split(';').find_map(|part| {
        crate::util::trim_start(part)
            .strip_prefix(name)?
            .strip_prefix(':')
    })
}

impl MathLook {
    fn of(attrs: &str) -> MathLook {
        let style = attribute(attrs, "style").unwrap_or("");
        let fill = crate::util::trim(
            property(style, "fill")
                .or_else(|| attribute(attrs, "fill"))
                .unwrap_or(""),
        );
        if fill == "none" {
            return MathLook {
                attrs: String::new(),
                halo: Some(attribute(attrs, "stroke").unwrap_or("none").to_string()),
            };
        }
        let mut out = String::new();
        if !fill.is_empty() {
            let _ = write!(out, " color=\"{fill}\"");
        }
        if let Some(opacity) = property(style, "opacity").map(crate::util::trim) {
            if !opacity.is_empty() {
                let _ = write!(out, " opacity=\"{opacity}\"");
            }
        }
        MathLook {
            attrs: out,
            halo: None,
        }
    }
}

/// The markup of typeset math is the host's, and is trusted.
fn math_svg(math: &MathMarkup, boxed: &str, size: f64, attrs: &str) -> String {
    match math {
        MathMarkup::Svg { svg, .. } => {
            format!("<svg class=\"pele-math\" {boxed} overflow=\"visible\"{attrs}>{svg}</svg>")
        }
        MathMarkup::Html { html, .. } => format!(
            "<foreignObject class=\"pele-math\" {boxed} overflow=\"visible\"{attrs}><div xmlns=\"http://www.w3.org/1999/xhtml\" style=\"display:flex;align-items:center;justify-content:center;width:100%;height:100%;font-size:{}px;line-height:normal;white-space:nowrap\">{html}</div></foreignObject>",
            num(size)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::MetricsMeasurer;
    use crate::text::label::layout_label;

    #[test]
    fn formats_numbers() {
        assert_eq!(num(0.0), "0");
        assert_eq!(num(0.004), "0");
        assert_eq!(num(1.0), "1");
        assert_eq!(num(1.5), "1.5");
        assert_eq!(num(1.25), "1.25");
        assert_eq!(num(1.05), "1.05");
        // -2.345 * 100 is -234.50000000000003 in binary floating point, as in pele.
        assert_eq!(num(-2.345), "-2.35");
        assert_eq!(num(0.005), "0.01");
        assert_eq!(num(-0.005), "0");
        assert_eq!(num(-0.5), "-0.5");
        assert_eq!(num(123456.789), "123456.79");
        // pele writes "NaN" here; pele-rs writes a valid coordinate instead.
        assert_eq!(num(f64::NAN), "0");
    }

    #[test]
    fn escapes_markup_and_controls() {
        assert_eq!(
            esc("<a href=\"x\">&\u{1}\t\n</a>"),
            "&lt;a href=&quot;x&quot;&gt;&amp;\t\n&lt;/a&gt;"
        );
        assert_eq!(esc_text("#35;&lt;"), "#35;&lt;");
    }

    #[test]
    fn writes_labels() {
        let plain = layout_label(
            Some("A<B"),
            false,
            &MetricsMeasurer,
            16.0,
            1000.0,
            Style::NONE,
        );
        assert_eq!(
            label_svg(&plain, 50.0, 20.0, " class=\"t\""),
            "<text class=\"t\" x=\"50\" y=\"25.6\" text-anchor=\"middle\">A&lt;B</text>"
        );
        let styled = layout_label(
            Some("<b>x</b><br><b><i>y</i></b>"),
            false,
            &MetricsMeasurer,
            16.0,
            1000.0,
            Style::NONE,
        );
        assert_eq!(
            label_svg(&styled, 0.0, 0.0, ""),
            "<text font-weight=\"var(--_w)\" text-anchor=\"middle\" xml:space=\"preserve\">\
             <tspan x=\"0\" y=\"-4.9\">x</tspan><tspan x=\"0\" y=\"16.1\" font-style=\"italic\">y</tspan></text>"
        );
    }
}

// Numeric validation is scoped to a render, including nested renders from host callbacks.
thread_local! {
    static NUMERIC_ERRORS: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub(crate) struct NumericGuard;
impl NumericGuard {
    pub fn new() -> Self {
        NUMERIC_ERRORS.with(|s| s.borrow_mut().push(false));
        Self
    }
    pub fn invalid(&self) -> bool {
        NUMERIC_ERRORS.with(|s| s.borrow().last().copied().unwrap_or(false))
    }
}
impl Drop for NumericGuard {
    fn drop(&mut self) {
        NUMERIC_ERRORS.with(|s| {
            s.borrow_mut().pop();
        });
    }
}
