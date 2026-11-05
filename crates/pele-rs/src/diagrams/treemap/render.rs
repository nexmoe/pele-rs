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
use super::layout::{Insets, layout_treemap};

/// The space a section keeps for its header.
const HEADER: f64 = 28.0;
/// The space a section keeps around its children on the other three sides.
const SIDE: f64 = 6.0;
/// The height of a line of text, as a multiple of its size.
const LINE: f64 = 1.25;

/// One line of text fitted into a width.
struct Fitted {
    text: String,
    size: f64,
}

fn option(input: &Input<'_>, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    match input.config.get("treemap") {
        Some(crate::preprocess::Value::Map(v)) => match v.get(key) {
            Some(crate::preprocess::Value::Number(v)) if *v >= min => v.min(max),
            _ => fallback,
        },
        _ => fallback,
    }
}
fn corners(attrs: &str, b: &super::layout::Box) -> String {
    let cap = b.w.min(b.h) / 4.0;
    let attrs = rounded(attrs);
    if cap < 4.0 {
        attrs.replace("rx:var(--_r)", &format!("rx:min(var(--_r),{}px)", num(cap)))
    } else {
        attrs
    }
}
fn title_part(raw: &str) -> String {
    crate::util::trim(&esc(crate::util::trim(&decode_entities(raw)))).to_string()
}
/// Draws a treemap with the official adaptive gaps, headers, labels and tooltips.
pub(crate) fn render_treemap(db: &TreemapDb, input: &Input<'_>) -> Result<Output, Error> {
    use crate::preprocess::Value;
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let tiny = round(size * 0.75);
    let least = round(size * 0.5625);
    let conf = match input.config.get("treemap") {
        Some(Value::Map(v)) => Some(v),
        _ => None,
    };
    let pad = options
        .padding
        .unwrap_or_else(|| option(input, "diagramPadding", 8.0, 0.0, 2000.0));
    let width = round(cap_width(
        options,
        option(input, "nodeWidth", 64.0, 1.0, 400.0) * 10.0,
        200.0,
        2.0 * pad,
    ));
    let height = round(option(input, "nodeHeight", 40.0, 1.0, 400.0) * 10.0);
    let show_values = conf.and_then(|v| v.get("showValues")) != Some(&Value::Bool(false));
    let spec = match conf.and_then(|v| v.get("valueFormat")) {
        Some(Value::String(s)) => s.as_str(),
        _ => ",",
    };
    let count = db
        .tree
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Leaf && n.value.is_some_and(|v| v.is_finite() && v > 0.0))
        .count()
        .max(1);
    let crowd = ((width * height) / count as f64 / 800.0)
        .log10()
        .clamp(0.0, 1.0);
    let boxes = layout_treemap(
        &db.tree,
        width,
        height,
        Insets {
            gap: option(input, "padding", round(2.0 + 2.0 * crowd), 0.0, 200.0),
            header: HEADER,
            short: 18.0,
            side: round(3.0 + 3.0 * crowd),
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
        let (x, y) = (left + b.x, top + b.y);
        let name = crate::util::trim(&decode_entities(&node.name)).to_string();
        let value = if show_values {
            super::format::format_spec(b.value, spec)
        } else {
            String::new()
        };
        let group = format!("data-id=\"{}\">", esc_text(&node.name));
        let own = title_part(&node.name);
        let mut path = if own.is_empty() {
            Vec::new()
        } else {
            vec![own.clone()]
        };
        let mut parent = b.parent;
        while let Some(p) = parent {
            if boxes[p].headed {
                break;
            }
            let section = title_part(&db.tree.nodes[boxes[p].node].name);
            if !section.is_empty() {
                path.insert(0, section);
            }
            parent = boxes[p].parent;
        }
        let full = path.join(" › ");
        let tooltip = |drawn: Option<&str>| {
            if full.is_empty() || (drawn == Some(name.as_str()) && full == own) {
                String::new()
            } else {
                format!("<title>{full}</title>")
            }
        };
        let rect = format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{color}\"",
            num(x),
            num(y),
            num(b.w),
            num(b.h)
        );
        let classes = format!(
            "pele-node pele-{}{}",
            node.kind.as_str().to_lowercase(),
            class_names(node.class_selector.as_deref().unwrap_or(""))
        );
        let fixed = style.font_size;
        let text_style = (if style.bold { Style::BOLD } else { Style::NONE })
            | if style.italic {
                Style::ITALIC
            } else {
                Style::NONE
            };
        let ink = if crate::svg::sets_fill(&style.shape) {
            crate::svg::ink::ink_fill(&style, None)
        } else {
            crate::palette::series_ink_fill(b.series)
        };
        if node.kind == NodeKind::Section {
            let mut text = String::new();
            let mut drawn = None;
            if b.headed {
                let short = b.header < HEADER;
                let label_size = fixed.unwrap_or(if short { tiny } else { small });
                let value_size = fixed.unwrap_or(tiny);
                let room = b.w - 2.0 * SIDE - 4.0;
                let value_width = if value.is_empty() {
                    0.0
                } else {
                    measurer.width(&value, value_size, text_style)
                };
                let with_value =
                    !short && value_width > 0.0 && room - value_width - 12.0 >= 3.0 * label_size;
                let label = if label_size * LINE < b.header {
                    fit_text(
                        &name,
                        if with_value {
                            room - value_width - 12.0
                        } else {
                            room
                        },
                        label_size,
                        label_size,
                        measurer,
                        text_style | Style::BOLD,
                    )
                } else {
                    None
                };
                let baseline = y + b.header / 2.0 + 1.0;
                if let Some(label) = label {
                    text += &format!(
                        "<text x=\"{}\" y=\"{}\" font-weight=\"var(--_hw)\"{}{ink}{}>{}</text>",
                        num(x + SIDE + 2.0),
                        num(baseline + label_size * 0.35),
                        if short {
                            format!(" font-size=\"{}\"", num(label_size))
                        } else {
                            String::new()
                        },
                        style.text,
                        esc(&label.text)
                    );
                    drawn = Some(label.text);
                }
                if with_value && value_size * LINE <= HEADER {
                    text += &format!(
                        "<text class=\"pele-value\" x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"end\"{}{}>{}</text>",
                        num(x + b.w - SIDE - 2.0),
                        num(baseline + value_size * 0.35),
                        num(tiny),
                        if ink.is_empty() {
                            " fill=\"var(--_m)\""
                        } else {
                            &ink
                        },
                        style.text,
                        esc(&value)
                    );
                }
            }
            let tint = if has_fill(&style.shape) {
                ""
            } else {
                " fill-opacity=\"0.12\""
            };
            let shape = if b.headed {
                format!(
                    "{rect}{}/>",
                    corners(&format!("{tint} stroke=\"{color}\"{}", style.shape), b)
                )
            } else {
                format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\"/>",
                    num(x),
                    num(y),
                    num(b.w),
                    num(b.h)
                )
            };
            sections += &format!(
                "<g class=\"{classes}\" {group}{}{shape}{text}</g>",
                tooltip(drawn.as_deref())
            );
            continue;
        }
        let mut text = String::new();
        let room = b.w - 8.0;
        let smallest = fixed.unwrap_or(least);
        let tallest = match fixed {
            None => size.min(((b.h - 4.0) / LINE).floor()),
            Some(fixed) if fixed * LINE <= b.h - 4.0 => fixed,
            _ => 0.0,
        };
        let label = if tallest >= smallest {
            fit_text(&name, room, tallest, smallest, measurer, text_style)
        } else {
            None
        };
        if let Some(label) = &label {
            let cx = num(x + b.w / 2.0);
            let cy = y + b.h / 2.0;
            let value_size = fixed.unwrap_or_else(|| least.max(round(label.size * 0.8)));
            let lines = (label.size + value_size) * LINE;
            if !value.is_empty()
                && lines <= b.h - 4.0
                && measurer.width(&value, value_size, text_style) <= room
            {
                let first = cy - lines / 2.0 + (label.size * LINE) / 2.0;
                let second = cy + lines / 2.0 - (value_size * LINE) / 2.0;
                text = format!(
                    "<text x=\"{cx}\" y=\"{}\" font-size=\"{}\"{ink}{}>{}</text><text class=\"pele-value\" x=\"{cx}\" y=\"{}\" font-size=\"{}\"{ink}{}>{}</text>",
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
                    "<text x=\"{cx}\" y=\"{}\" font-size=\"{}\"{ink}{}>{}</text>",
                    num(cy + label.size * 0.35),
                    num(label.size),
                    style.text,
                    esc(&label.text)
                );
            }
        }
        leaves += &format!(
            "<g class=\"{classes}\" {group}{}{rect}{}/>{text}</g>",
            tooltip(label.as_ref().map(|l| l.text.as_str())),
            corners(&style.shape, b)
        );
    }
    let total_width = (width.max(title.width) + 2.0 * pad).ceil();
    let total_height = (height + title_height + 2.0 * pad).ceil();
    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &if sections.is_empty() {
        String::new()
    } else {
        format!("<g class=\"pele-sections\" font-size=\"{small}\">{sections}</g>")
    } + &if leaves.is_empty() {
        String::new()
    } else {
        format!("<g class=\"pele-leaves\" text-anchor=\"middle\" fill=\"var(--_bg)\">{leaves}</g>")
    };
    let acc = Accessible {
        title: db.acc_title.clone(),
        descr: db.acc_descr.clone(),
    };
    Ok(Output {
        svg: svg_document(
            DiagramKind::Treemap,
            total_width,
            total_height,
            size,
            options,
            &acc,
            &content,
        ),
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
