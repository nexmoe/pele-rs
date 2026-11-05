//! The drawing of a treemap: nested boxes whose areas stand for the values.
//! Ported from pele's `diagrams/treemap/render.ts`.

use crate::DiagramKind;
use crate::RenderOptions;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::title_room;
use crate::error::Error;
use crate::svg::{Accessible, esc, esc_text, label_svg, num, svg_document};
use crate::svg::{class_names, resolve_style, rounded, series_color};
use crate::text::Style;
use crate::text::entities::decode_entities;
use crate::text::label::layout_label;
use crate::util::round;

use super::db::{NodeKind, TreemapDb};
use super::format::format_value;
use super::layout::{Insets, layout_treemap};

/// The space a section keeps for its header.
const HEADER: f64 = 28.0;
/// The space a section keeps around its children on the other three sides.
const SIDE: f64 = 6.0;
/// The space between two boxes.
const GAP: f64 = 4.0;
/// The height of a line of text, as a multiple of its size.
const LINE: f64 = 1.25;
/// Mermaid sizes the diagram as ten times `nodeWidth` by ten times `nodeHeight`, which are 64
/// and 40 by default. The type reads no configuration, so these are always the sizes.
const WIDTH: f64 = 640.0;
const HEIGHT: f64 = 400.0;
/// The space around the drawing.
const PADDING: f64 = 8.0;

/// One line of text fitted into a width.
struct Fitted {
    text: String,
    size: f64,
}

/// Draws a treemap.
pub(crate) fn render_treemap(db: &TreemapDb, input: &Input<'_>) -> Result<Output, Error> {
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let tiny = round(size * 0.75);
    let least = round(size * 0.5625);
    let pad = options.padding.unwrap_or(PADDING);
    let width = round(cap_width(options, WIDTH, 200.0, 2.0 * pad));
    let height = HEIGHT;

    let boxes = layout_treemap(
        &db.tree,
        width,
        height,
        Insets {
            gap: GAP,
            header: HEADER,
            side: SIDE,
        },
    );

    let title = layout_label(
        db.title.as_deref(),
        false,
        measurer,
        size,
        title_room(options),
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 8.0
    } else {
        0.0
    };
    let left = pad + (title.width - width).max(0.0) / 2.0;
    let top = pad + title_height;

    let mut sections = String::new();
    let mut leaves = String::new();
    for b in &boxes {
        let node = &db.tree.nodes[b.node];
        let style = resolve_style(&node.styles);
        let color = series_color(b.series);
        let x = left + b.x;
        let y = top + b.y;
        let name = decode_entities(&node.name).trim().to_string();
        let value = format_value(b.value);
        let group = format!("data-id=\"{}\">", esc_text(&node.name));
        let rect = format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"",
            num(x),
            num(y),
            num(b.w),
            num(b.h),
            color
        );
        let classes = format!(
            "pele-rs-node pele-rs-{}{}",
            node.kind.as_str().to_lowercase(),
            class_names(node.class_selector.as_deref().unwrap_or(""))
        );

        if node.kind == NodeKind::Section {
            let mut text = String::new();
            if b.h >= HEADER {
                let room = b.w - 2.0 * SIDE - 4.0;
                let value_width = if value.is_empty() {
                    0.0
                } else {
                    measurer.width(&value, tiny, Style::NONE)
                };
                let with_value = value_width > 0.0 && room - value_width - 12.0 >= 3.0 * small;
                let label = fit_text(
                    &name,
                    if with_value {
                        room - value_width - 12.0
                    } else {
                        room
                    },
                    small,
                    small,
                    measurer,
                    Style::BOLD,
                );
                let baseline = y + HEADER / 2.0 + 1.0;
                if let Some(label) = label {
                    text += &format!(
                        "<text x=\"{}\" y=\"{}\" font-weight=\"var(--_hw)\"{}>{}</text>",
                        num(x + SIDE + 2.0),
                        num(baseline + small * 0.35),
                        style.text,
                        esc(&label.text)
                    );
                }
                if with_value {
                    text += &format!(
                        "<text class=\"pele-rs-value\" x=\"{}\" y=\"{}\" font-size=\"{}\" \
                         text-anchor=\"end\" fill=\"var(--_m)\"{}>{}</text>",
                        num(x + b.w - SIDE - 2.0),
                        num(baseline + tiny * 0.35),
                        num(tiny),
                        style.text,
                        esc(&value)
                    );
                }
            }
            // A section is a tint of its color, unless a class gives it a fill of its own.
            let tint = if has_fill(&style.shape) {
                ""
            } else {
                " fill-opacity=\"0.12\""
            };
            sections += &format!(
                "<g class=\"{classes}\" {group}{rect}{}/>{text}</g>",
                rounded(&format!("{tint} stroke=\"{color}\"{}", style.shape))
            );
            continue;
        }

        let mut text = String::new();
        let room = b.w - 8.0;
        let tallest = size.min(((b.h - 4.0) / LINE).floor());
        if tallest >= least {
            if let Some(label) = fit_text(&name, room, tallest, least, measurer, Style::NONE) {
                let cx = num(x + b.w / 2.0);
                let cy = y + b.h / 2.0;
                let value_size = least.max(round(label.size * 0.8));
                let lines = (label.size + value_size) * LINE;
                if !value.is_empty()
                    && lines <= b.h - 4.0
                    && measurer.width(&value, value_size, Style::NONE) <= room
                {
                    let first = cy - lines / 2.0 + (label.size * LINE) / 2.0;
                    let second = cy + lines / 2.0 - (value_size * LINE) / 2.0;
                    text = format!(
                        "<text x=\"{cx}\" y=\"{}\" font-size=\"{}\"{}>{}</text>\
                         <text class=\"pele-rs-value\" x=\"{cx}\" y=\"{}\" font-size=\"{}\"{}>{}</text>",
                        num(first + label.size * 0.35),
                        num(label.size),
                        style.text,
                        esc(&label.text),
                        num(second + value_size * 0.35),
                        num(value_size),
                        style.text,
                        esc(&value)
                    );
                } else {
                    text = format!(
                        "<text x=\"{cx}\" y=\"{}\" font-size=\"{}\"{}>{}</text>",
                        num(cy + label.size * 0.35),
                        num(label.size),
                        style.text,
                        esc(&label.text)
                    );
                }
            }
        }
        leaves += &format!(
            "<g class=\"{classes}\" {group}{rect}{}/>{text}</g>",
            rounded(&style.shape)
        );
    }

    let total_width = (width.max(title.width) + 2.0 * pad).ceil();
    let total_height = (height + title_height + 2.0 * pad).ceil();
    let content = format!(
        "{}{}{}",
        label_svg(
            &title,
            total_width / 2.0,
            pad + title.height / 2.0,
            " class=\"pele-rs-title\" font-weight=\"var(--_tw)\""
        ),
        if sections.is_empty() {
            String::new()
        } else {
            format!("<g class=\"pele-rs-sections\" font-size=\"{small}\">{sections}</g>")
        },
        if leaves.is_empty() {
            String::new()
        } else {
            format!(
                "<g class=\"pele-rs-leaves\" text-anchor=\"middle\" fill=\"var(--_bg)\">{leaves}</g>"
            )
        }
    );
    let acc = Accessible {
        title: db.acc_title.clone(),
        descr: db.acc_descr.clone(),
    };
    let svg = svg_document(
        DiagramKind::Treemap,
        total_width,
        total_height,
        size,
        options,
        &acc,
        &content,
    );
    Ok(Output {
        svg,
        width: total_width,
        height: total_height,
        links: Vec::new(),
    })
}

/// The width a diagram is drawn at: its own, or the host's when that is smaller, but not under
/// `least`. `around` is what the diagram adds outside that width.
fn cap_width(options: &RenderOptions, natural: f64, least: f64, around: f64) -> f64 {
    let Some(available) = options.max_width.filter(|&w| w > 0.0) else {
        return natural;
    };
    least.min(natural).max(natural.min(available - around))
}

/// Whether the style gives a shape a fill of its own.
fn has_fill(shape: &str) -> bool {
    shape.contains("\"fill:") || shape.contains(";fill:")
}

/// Fits one line of text into a width: first by shrinking the font down to `min`, then by cutting
/// the text and adding an ellipsis. Nothing when not even one character fits.
/// Ported from pele's `diagrams/common/fit-text.ts`.
fn fit_text(
    text: &str,
    max: f64,
    size: f64,
    min: f64,
    measurer: &dyn crate::text::TextMeasurer,
    style: Style,
) -> Option<Fitted> {
    if text.is_empty() || max <= 0.0 {
        return None;
    }
    let mut width = measurer.width(text, size, style);
    if width <= max {
        return Some(Fitted {
            text: text.to_string(),
            size,
        });
    }
    // The width grows with the size, so one division lands on the right size or just above it.
    let mut fitted = min.max((size * max / width).floor());
    while fitted >= min {
        width = measurer.width(text, fitted, style);
        if width <= max {
            return Some(Fitted {
                text: text.to_string(),
                size: fitted,
            });
        }
        fitted -= 1.0;
    }

    let chars: Vec<char> = text.chars().collect();
    let mut low = 0i64;
    let mut high = chars.len() as i64 - 1;
    while low < high {
        let mid = (low + high + 1) / 2;
        let cut: String = chars[..mid as usize].iter().collect();
        let cut = format!("{}…", cut.trim_end());
        if measurer.width(&cut, min, style) <= max {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    if low == 0 {
        return None;
    }
    let cut: String = chars[..low as usize].iter().collect();
    let cut = format!("{}…", cut.trim_end());
    let width = measurer.width(&cut, min, style);
    (width <= max).then_some(Fitted {
        text: cut,
        size: min,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::MetricsMeasurer;

    #[test]
    fn fits_a_text_into_a_width() {
        let measurer = MetricsMeasurer;
        let fitted = fit_text("hello", 100.0, 16.0, 16.0, &measurer, Style::NONE).expect("fits");
        assert_eq!(fitted.text, "hello");
        assert_eq!(fitted.size, 16.0);
    }

    #[test]
    fn cuts_a_text_that_does_not_fit() {
        let measurer = MetricsMeasurer;
        let fitted =
            fit_text("hello there", 20.0, 16.0, 16.0, &measurer, Style::NONE).expect("fits");
        assert!(fitted.text.ends_with('…'));
        assert!(fitted.text.len() < "hello there".len());
    }

    #[test]
    fn fits_nothing_when_the_room_is_too_small() {
        let measurer = MetricsMeasurer;
        assert!(fit_text("hello", 1.0, 16.0, 16.0, &measurer, Style::NONE).is_none());
        assert!(fit_text("", 100.0, 16.0, 16.0, &measurer, Style::NONE).is_none());
    }

    #[test]
    fn tells_a_fill_from_another_style() {
        assert!(has_fill(" style=\"fill:red;\""));
        assert!(has_fill(" style=\"stroke:red;fill:blue;\""));
        assert!(!has_fill(" style=\"stroke:red;\""));
        assert!(!has_fill(""));
    }
}
