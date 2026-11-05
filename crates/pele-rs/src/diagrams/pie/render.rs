//! Pie chart layout and SVG. Ported from pele's `diagrams/pie/render.ts`.

use std::f64::consts::PI;
use std::fmt::Write;

use super::model::PieModel;
use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{fit_width, title_room};
use crate::preprocess::{Config, Value};
use crate::svg::{esc_text, label_svg, num, series_color, svg_document};
use crate::text::Style;
use crate::text::label::{Label, layout_label};
use crate::util::{js_number, round, to_fixed0};

const RADIUS: f64 = 120.0;
const SWATCH: f64 = 12.0;
const ROW: f64 = 26.0;
const GAP: f64 = 32.0;
const OUTSIDE: f64 = 28.0;

fn option<'a>(config: &'a Config, key: &str) -> Option<&'a Value> {
    match config.get("pie") {
        Some(Value::Map(pie)) => pie.get(key),
        _ => None,
    }
}

fn number_in(value: Option<&Value>, valid: impl Fn(f64) -> bool) -> Option<f64> {
    match value {
        Some(&Value::Number(n)) if valid(n) => Some(n),
        _ => None,
    }
}

fn point(angle: f64, radius: f64) -> String {
    format!(
        "{},{}",
        num(angle.sin() * radius),
        num(-angle.cos() * radius)
    )
}

fn slice(start: f64, end: f64, outer: f64, inner: f64) -> String {
    if end - start >= PI * 2.0 - 1e-9 {
        let ring = |r: f64, sweep: u8| {
            let (r, n) = (num(r), num(-r));
            format!("M0,{n}A{r},{r} 0 1 {sweep} 0,{r}A{r},{r} 0 1 {sweep} 0,{n}Z")
        };
        let hole = if inner > 0.0 {
            ring(inner, 0)
        } else {
            String::new()
        };
        return ring(outer, 1) + &hole;
    }
    let large = u8::from(end - start > PI);
    let (o, i) = (num(outer), num(inner));
    if inner <= 0.0 {
        return format!(
            "M0,0L{}A{o},{o} 0 {large} 1 {}Z",
            point(start, outer),
            point(end, outer)
        );
    }
    format!(
        "M{}A{o},{o} 0 {large} 1 {}L{}A{i},{i} 0 {large} 0 {}Z",
        point(start, outer),
        point(end, outer),
        point(end, inner),
        point(start, inner)
    )
}

pub(crate) fn render_pie(model: &PieModel, input: &Input<'_>) -> Output {
    fit_width(input.options, RADIUS * 2.0, 140.0, |diameter, tight| {
        draw(model, input, diameter / 2.0, tight)
    })
}

struct Placed {
    x: f64,
    y: f64,
    width: f64,
}

/// `tight` is set when the chart does not fit the width it has with its legend beside it.
fn draw(model: &PieModel, input: &Input<'_>, radius: f64, tight: bool) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let config = input.config;
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let pad = options.padding.unwrap_or(8.0);

    let text_position =
        number_in(option(config, "textPosition"), |p| (0.0..=1.0).contains(&p)).unwrap_or(0.75);
    let inner =
        number_in(option(config, "donutHole"), |h| h > 0.0 && h <= 0.9).map_or(0.0, |h| h * radius);
    let placed = match option(config, "legendPosition") {
        None | Some(Value::Null) => "right".to_string(),
        Some(value) => value.js_string(),
    };
    let legend_position = if tight && (placed == "left" || placed == "right") {
        "bottom"
    } else {
        placed.as_str()
    };

    let entries = &model.sections;
    let sum: f64 = entries.iter().map(|(_, value)| value).sum();

    let legend: Vec<Label> = entries
        .iter()
        .map(|(label, value)| {
            let text = if model.show_data {
                format!("{label} [{}]", js_number(*value))
            } else {
                label.clone()
            };
            layout_label(Some(&text), false, measurer, small, 4000.0, Style::NONE)
        })
        .collect();
    let mut legend_width = legend.iter().map(|l| l.width).fold(0.0, f64::max);
    if !legend.is_empty() {
        legend_width += SWATCH + 8.0;
    }
    let legend_height = legend.len() as f64 * ROW;

    let title = layout_label(
        model.title.as_deref(),
        false,
        measurer,
        size,
        title_room(options),
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 16.0
    } else {
        0.0
    };

    let percent = |share: f64| to_fixed0(share * 100.0) + "%";
    // A slice too thin for its percentage gets the label outside the rim, which needs a margin.
    let shares: Vec<f64> = entries
        .iter()
        .map(|(_, value)| {
            let share = if sum > 0.0 { value / sum } else { 0.0 };
            if share * 100.0 >= 1.0 && to_fixed0(share * 100.0) != "0" {
                share
            } else {
                0.0
            }
        })
        .collect();
    let narrow: Vec<bool> = shares
        .iter()
        .map(|&share| {
            if share == 0.0 || share >= 0.5 {
                return false;
            }
            let chord = 2.0 * radius * text_position * (share * PI).sin();
            chord < measurer.width(&percent(share), small, Style::NONE) + 6.0
        })
        .collect();
    let margin = if narrow.contains(&true) { OUTSIDE } else { 0.0 };

    let side = match legend_position {
        "left" | "right" | "center" => legend_position,
        _ => "",
    };
    let stacked = legend_position == "top" || legend_position == "bottom";
    let diameter = (radius + margin) * 2.0;
    let mut width = diameter;
    let mut height = diameter;
    if side == "left" || side == "right" {
        if legend_width > 0.0 {
            width += GAP + legend_width;
        }
        height = height.max(legend_height);
    } else if stacked {
        width = width.max(legend_width);
        if legend_height > 0.0 {
            height += 20.0 + legend_height;
        }
    }
    width = width.max(title.width);

    let cx = pad
        + match side {
            "left" => legend_width + GAP + diameter / 2.0,
            "right" => diameter / 2.0,
            _ => width / 2.0,
        };
    let body_top = pad + title_height;
    let cy = body_top
        + if legend_position == "top" {
            legend_height + 20.0 + diameter / 2.0
        } else if stacked {
            diameter / 2.0
        } else {
            height / 2.0
        };

    let mut slices = String::new();
    let mut labels = String::new();
    let mut outside = String::new();
    let mut angle = 0.0;
    let rim = radius + OUTSIDE / 2.0;
    let mut placed_outside: Vec<Placed> = Vec::new();
    for (index, (label, _)) in entries.iter().enumerate() {
        let share = shares[index];
        if share == 0.0 {
            continue;
        }
        let end = angle + share * PI * 2.0;
        let _ = write!(
            slices,
            "<path class=\"pele-rs-slice\" data-id=\"{}\" d=\"{}\" fill=\"{}\"/>",
            esc_text(label),
            slice(angle, end, radius, inner),
            series_color(index)
        );
        let mid = (angle + end) / 2.0;
        let text = percent(share);
        if narrow[index] {
            let width = measurer.width(&text, small, Style::NONE);
            let clear = |x: f64, y: f64| {
                placed_outside.iter().all(|other| {
                    (x - other.x).abs() >= (width + other.width) / 2.0 + 2.0
                        || (y - other.y).abs() >= small + 2.0
                })
            };
            let mut at = mid;
            for _ in 0..360 {
                if clear(at.sin() * rim, -at.cos() * rim) {
                    break;
                }
                at += 2.0 / rim;
            }
            let (x, y) = (at.sin() * rim, -at.cos() * rim);
            placed_outside.push(Placed { x, y, width });
            let _ = write!(
                outside,
                "<text x=\"{}\" y=\"{}\">{text}</text>",
                num(x),
                num(y + small * 0.35)
            );
        } else {
            let r = radius * text_position;
            let _ = write!(
                labels,
                "<text x=\"{}\" y=\"{}\">{text}</text>",
                num(mid.sin() * r),
                num(-mid.cos() * r + small * 0.35)
            );
        }
        angle = end;
    }
    if slices.is_empty() {
        slices = format!(
            "<circle class=\"pele-rs-slice\" r=\"{}\" fill=\"none\" stroke=\"var(--_b)\"/>",
            js_number(radius)
        );
    }

    let (lx, ly) = match side {
        "right" => (
            cx + diameter / 2.0 + GAP,
            body_top + (height - legend_height) / 2.0,
        ),
        "left" => (pad, body_top + (height - legend_height) / 2.0),
        "center" => (cx - legend_width / 2.0, cy - legend_height / 2.0),
        _ => (
            pad + (width - legend_width) / 2.0,
            if legend_position == "top" {
                body_top
            } else {
                body_top + diameter + 20.0
            },
        ),
    };
    let mut legend_out = String::new();
    for (index, label) in legend.iter().enumerate() {
        let y = ly + index as f64 * ROW + ROW / 2.0;
        let _ = write!(
            legend_out,
            "<g class=\"pele-rs-legend-item\" data-id=\"{}\">\
             <rect x=\"{}\" y=\"{}\" width=\"{SWATCH}\" height=\"{SWATCH}\" rx=\"2\" fill=\"{}\"/>{}</g>",
            esc_text(&entries[index].0),
            num(lx),
            num(y - SWATCH / 2.0),
            series_color(index),
            label_svg(label, lx + SWATCH + 8.0 + label.width / 2.0, y, "")
        );
    }

    let total_width = (width + 2.0 * pad).ceil();
    let total_height = (height + title_height + 2.0 * pad).ceil();
    let center = format!("translate({},{})", num(cx), num(cy));
    let small_size = num(small);
    let mut content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-rs-title\" font-weight=\"var(--_tw)\"",
    );
    let _ = write!(
        content,
        "<g class=\"pele-rs-slices\" transform=\"{center}\" stroke=\"var(--_bg)\" stroke-width=\"1\" \
         stroke-linejoin=\"round\">{slices}</g>"
    );
    if !labels.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-slice-labels\" transform=\"{center}\" font-size=\"{small_size}\" \
             text-anchor=\"middle\" fill=\"var(--_bg)\">{labels}</g>"
        );
    }
    if !outside.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-slice-labels\" transform=\"{center}\" font-size=\"{small_size}\" \
             text-anchor=\"middle\">{outside}</g>"
        );
    }
    if !legend_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-legend\" font-size=\"{small_size}\">{legend_out}</g>"
        );
    }

    Output {
        svg: svg_document(
            DiagramKind::Pie,
            total_width,
            total_height,
            size,
            options,
            &model.acc,
            &content,
        ),
        width: total_width,
        height: total_height,
        links: Vec::new(),
    }
}
