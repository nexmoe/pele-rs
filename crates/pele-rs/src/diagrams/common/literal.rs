//! Labels that keep every character. Ported from pele's `diagrams/common/literal-label.ts`.

use crate::text::TextMeasurer;
use crate::text::label::{Label, Span};

/// Breaks the lines of a label that are wider than `room` between characters, keeping each
/// run of one style together.
#[cfg(any(feature = "sequence", feature = "ishikawa", feature = "eventmodeling"))]
pub(crate) fn break_label(label: Label, measurer: &dyn TextMeasurer, room: f64) -> Label {
    if label.width <= room {
        return label;
    }
    let mut lines: Vec<Vec<Span>> = Vec::new();
    let mut widths: Vec<f64> = Vec::new();
    for (i, line) in label.lines.iter().enumerate() {
        if label.widths[i] <= room {
            lines.push(line.clone());
            widths.push(label.widths[i]);
            continue;
        }
        let mut current: Vec<Span> = Vec::new();
        let mut width = 0.0;
        for span in line {
            // An icon or math is not broken between characters.
            if !span.is_plain() {
                if !current.is_empty() && width + span.width > room {
                    lines.push(std::mem::take(&mut current));
                    widths.push(width);
                    width = 0.0;
                }
                current.push(span.clone());
                width += span.width;
                continue;
            }
            for ch in span.text.chars() {
                let joined = current
                    .last()
                    .is_some_and(|last| last.is_plain() && last.style == span.style);
                let piece = ch.to_string();
                let next_width = if joined {
                    let last = &current[current.len() - 1];
                    measurer.width(&(last.text.clone() + &piece), label.size, span.style)
                } else {
                    measurer.width(&piece, label.size, span.style)
                };
                let mut added = next_width
                    - if joined {
                        current[current.len() - 1].width
                    } else {
                        0.0
                    };
                let mut joined = joined;
                if !current.is_empty() && width + added > room {
                    lines.push(std::mem::take(&mut current));
                    widths.push(width);
                    width = 0.0;
                    joined = false;
                    added = measurer.width(&piece, label.size, span.style);
                }
                if joined {
                    let last = current.last_mut().expect("joined implies a last span");
                    last.text.push(ch);
                    last.width += added;
                } else {
                    current.push(Span::text(piece, span.style, added));
                }
                width += added;
            }
        }
        if !current.is_empty() || line.is_empty() {
            lines.push(current);
            widths.push(width);
        }
    }
    let width = widths.iter().copied().fold(0.0, f64::max);
    let height = lines.len() as f64 * label.line_height;
    let broken = Label {
        lines,
        widths,
        width,
        height,
        ..label
    };
    if broken.heights.is_some() {
        crate::text::label::with_heights(broken)
    } else {
        broken
    }
}

/// A label that keeps every character of a name, broken between characters when it does not
/// fit the room it has.
#[cfg(any(feature = "treeview", feature = "eventmodeling"))]
pub(crate) fn literal_label(
    text: &str,
    measurer: &dyn TextMeasurer,
    size: f64,
    room: f64,
    style: crate::text::Style,
) -> Label {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let chars: Vec<char> = raw.chars().collect();
        if chars.is_empty() {
            lines.push(String::new());
        }
        let mut from = 0;
        while from < chars.len() {
            let fits = |length: usize| {
                let piece: String = chars[from..from + length].iter().collect();
                measurer.width(&piece, size, style) <= room
            };
            let mut hi = 1;
            while hi < chars.len() - from && fits(hi) {
                hi = (chars.len() - from).min(hi * 2);
            }
            let mut lo = 1;
            while lo < hi {
                let mid = (lo + hi).div_ceil(2);
                if fits(mid) {
                    lo = mid;
                } else {
                    hi = mid - 1;
                }
            }
            let mut take = lo;
            let wraps = from + take < chars.len();
            if wraps && !chars[from + take].is_whitespace() {
                for i in (1..take).rev() {
                    if chars[from + i].is_whitespace() {
                        take = i + 1;
                        break;
                    }
                }
            }
            let line: String = chars[from..from + take].iter().collect();
            lines.push(if wraps {
                let trimmed = line.trim_end();
                if trimmed.is_empty() {
                    line
                } else {
                    trimmed.to_string()
                }
            } else {
                line
            });
            from += take;
            while wraps && from < chars.len() && chars[from].is_whitespace() {
                from += 1;
            }
        }
    }
    if text.is_empty() {
        lines.clear();
    }
    let line_height = crate::util::round(size * 1.5);
    let widths: Vec<f64> = lines
        .iter()
        .map(|line| measurer.width(line, size, style))
        .collect();
    let width = widths.iter().copied().fold(0.0f64, f64::max);
    let spans: Vec<Vec<Span>> = lines
        .into_iter()
        .zip(widths.iter())
        .map(|(line, &width)| vec![Span::text(line, style, width)])
        .collect();
    let height = spans.len() as f64 * line_height;
    Label {
        lines: spans,
        widths,
        width,
        height,
        size,
        line_height,
        heights: None,
    }
}

/// Every line starts at the same x coordinate; plain text retains its spaces.
#[cfg(feature = "eventmodeling")]
pub(crate) fn left_label_svg(label: &Label, x: f64, top: f64, attrs: &str) -> String {
    use crate::svg::{esc, num, shared_style, span_style, text_style};
    use std::fmt::Write;
    let shared = shared_style(label);
    let attrs = text_style(attrs, shared);
    let mut body = String::new();
    for (i, spans) in label.lines.iter().enumerate() {
        let cy = top + (i as f64 + 0.5) * label.line_height;
        let mut text = String::new();
        for span in spans {
            let own = span_style(span.style & !shared);
            if own.is_empty() {
                text += &esc(&span.text);
            } else {
                let _ = write!(text, "<tspan{own}>{}</tspan>", esc(&span.text));
            }
        }
        if !text.is_empty() {
            let _ = write!(
                body,
                "<text{attrs} x=\"{}\" y=\"{}\" text-anchor=\"start\" xml:space=\"preserve\">{text}</text>",
                num(x),
                num(cy + label.size * 0.35)
            );
        }
    }
    body
}
