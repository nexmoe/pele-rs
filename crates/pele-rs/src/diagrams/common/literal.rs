//! Labels that keep every character. Ported from pele's `diagrams/common/literal-label.ts`.

use crate::text::TextMeasurer;
use crate::text::label::{Label, Span, with_heights};

/// Breaks the lines of a label that are wider than `room` between characters, keeping each
/// run of one style together.
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
        with_heights(broken)
    } else {
        broken
    }
}
