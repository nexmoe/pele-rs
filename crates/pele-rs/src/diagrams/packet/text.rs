//! The text of a packet field: a label that keeps every character, and a line of text that is
//! fitted into the room a field holds. Ported from pele's `diagrams/common/literal-label.ts` and
//! `diagrams/common/fit-text.ts`.

use crate::text::Style;
use crate::text::TextMeasurer;
use crate::text::label::{Label, Span};
use crate::util::{is_space, round, trim_end};

/// A line of text that was fitted into a width.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Fitted {
    pub text: String,
    /// The size the text is drawn at.
    pub size: f64,
    pub width: f64,
}

/// Lays out text that keeps every character, breaking the lines that are wider than `room`
/// between characters. Only the spaces a line breaks at are dropped.
pub(crate) fn literal_label(
    text: &str,
    measurer: &dyn TextMeasurer,
    size: f64,
    room: f64,
) -> Label {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let chars: Vec<char> = raw.chars().collect();
        if chars.is_empty() {
            lines.push(String::new());
        }
        let mut from = 0usize;
        while from < chars.len() {
            let fits = |length: usize| -> bool {
                let piece: String = chars[from..from + length].iter().collect();
                measurer.width(&piece, size, Style::NONE) <= room
            };
            let mut hi = 1usize;
            while hi < chars.len() - from && fits(hi) {
                hi = (chars.len() - from).min(hi * 2);
            }
            let mut lo = 1usize;
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
            if wraps && !is_space(chars[from + take]) {
                for i in (1..take).rev() {
                    if is_space(chars[from + i]) {
                        take = i + 1;
                        break;
                    }
                }
            }
            let line: String = chars[from..from + take].iter().collect();
            lines.push(if wraps {
                let trimmed = trim_end(&line);
                if trimmed.is_empty() {
                    line
                } else {
                    trimmed.to_string()
                }
            } else {
                line
            });
            from += take;
            while wraps && from < chars.len() && is_space(chars[from]) {
                from += 1;
            }
        }
    }
    if text.is_empty() {
        lines.clear();
    }
    let widths: Vec<f64> = lines
        .iter()
        .map(|line| measurer.width(line, size, Style::NONE))
        .collect();
    let width = widths.iter().copied().fold(0.0, f64::max);
    let line_height = round(size * 1.5);
    Label {
        lines: widths
            .iter()
            .zip(&lines)
            .map(|(width, line)| vec![Span::text(line.clone(), Style::NONE, *width)])
            .collect(),
        widths,
        width,
        height: lines.len() as f64 * line_height,
        size,
        line_height,
        heights: None,
    }
}

/// Fits one line of text into a width: first by shrinking the font down to `min`, then by cutting
/// the text and adding an ellipsis. Nothing when not even one character fits.
pub(crate) fn fit_text(
    text: &str,
    max: f64,
    size: f64,
    min: f64,
    measurer: &dyn TextMeasurer,
) -> Option<Fitted> {
    if text.is_empty() || max <= 0.0 {
        return None;
    }
    let mut width = measurer.width(text, size, Style::NONE);
    if width <= max {
        return Some(Fitted {
            text: text.to_string(),
            size,
            width,
        });
    }

    // The width grows with the font size, so one division lands on the right size or just above.
    let mut fitted = min.max(((size * max) / width).floor());
    while fitted >= min {
        width = measurer.width(text, fitted, Style::NONE);
        if width <= max {
            return Some(Fitted {
                text: text.to_string(),
                size: fitted,
                width,
            });
        }
        fitted -= 1.0;
    }

    let chars: Vec<char> = text.chars().collect();
    let mut low = 0usize;
    let mut high = chars.len() - 1;
    while low < high {
        let mid = (low + high + 1) >> 1;
        if measurer.width(&cut(&chars, mid), min, Style::NONE) <= max {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    if low == 0 {
        return None;
    }
    let cut = cut(&chars, low);
    let width = measurer.width(&cut, min, Style::NONE);
    if width <= max {
        Some(Fitted {
            text: cut,
            size: min,
            width,
        })
    } else {
        None
    }
}

/// The first `length` characters, cut at a word end and closed with an ellipsis.
fn cut(chars: &[char], length: usize) -> String {
    let piece: String = chars[..length].iter().collect();
    format!("{}…", trim_end(&piece))
}
