//! The drawing of a radar chart: axes around a centre, with the values of every curve drawn as a
//! polygon. The layout is pele's own, ported from `diagrams/radar/render.ts`.

use super::db::{Graticule, RadarModel};
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{fit_width, title_room};
use crate::preprocess::{Config, Value};
use crate::svg::{Accessible, esc_text, label_svg, num, series_color, svg_document};
use crate::text::Style;
use crate::text::label::{Label, layout_label};
use crate::util::round;

/// The size of the mark of a curve in the legend.
const SWATCH: f64 = 12.0;
/// The room one entry of the legend takes.
const ROW: f64 = 26.0;
/// The room between the chart and the legend.
const GAP: f64 = 24.0;

/// A place on the chart.
#[derive(Debug, Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

/// A number from the configuration of the chart: one that is not a number, or that is under the
/// smallest one allowed, falls back to the default, and one that is over the largest is cut down
/// to it.
fn option(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    let chart = match config.get("radar") {
        Some(Value::Map(chart)) => chart,
        _ => return fallback,
    };
    match chart.get(key) {
        Some(Value::Number(value)) if *value >= min => value.min(max),
        _ => fallback,
    }
}

/// How far from the centre a value reaches.
fn relative_radius(value: f64, min: f64, max: f64, radius: f64) -> f64 {
    let clipped = value.max(min).min(max);
    radius * (clipped - min) / (max - min)
}

/// A closed Catmull-Rom spline through the points, as cubic Bézier segments.
fn closed_round_curve(points: &[Point], tension: f64) -> String {
    let n = points.len();
    if n == 0 {
        return String::new();
    }
    let mut d = format!("M{},{}", num(points[0].x), num(points[0].y));
    for i in 0..n {
        let p0 = points[(i + n - 1) % n];
        let p1 = points[i];
        let p2 = points[(i + 1) % n];
        let p3 = points[(i + 2) % n];
        d += &format!(
            " C{},{} {},{} {},{}",
            num(p1.x + (p2.x - p0.x) * tension),
            num(p1.y + (p2.y - p0.y) * tension),
            num(p2.x - (p3.x - p1.x) * tension),
            num(p2.y - (p3.y - p1.y) * tension),
            num(p2.x),
            num(p2.y)
        );
    }
    d + " Z"
}

/// The points of a polygon, as the attribute of an SVG polygon.
fn polygon(points: &[Point]) -> String {
    let mut out = String::new();
    for point in points {
        if !out.is_empty() {
            out.push(' ');
        }
        out += &format!("{},{}", num(point.x), num(point.y));
    }
    out
}

/// Draws a radar chart.
pub(crate) fn render_radar(model: &RadarModel, input: &Input<'_>) -> Output {
    let natural = option(input.config, "width", 320.0, 1.0, 4000.0);
    let height = option(input.config, "height", 320.0, 1.0, 4000.0);
    let mut draw = |width: f64, tight: bool| {
        draw_radar(model, input, width, (height * width) / natural, tight)
    };
    fit_width(input.options, natural, 160.0, &mut draw)
}

/// Draws a radar chart at a width. `tight` is set when the chart does not fit the width it has
/// with its legend beside it.
fn draw_radar(
    model: &RadarModel,
    input: &Input<'_>,
    width: f64,
    height: f64,
    tight: bool,
) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let pad = options.padding.unwrap_or(8.0);

    let axis_scale = option(input.config, "axisScaleFactor", 1.0, 0.0, 10.0);
    let label_factor = option(input.config, "axisLabelFactor", 1.05, 0.0, 10.0);
    let tension = option(input.config, "curveTension", 0.17, 0.0, 1.0);
    let radius = width.min(height) / 2.0;

    let axes = &model.axes;
    let curves = &model.curves;
    let ticks = model.options.ticks;
    let graticule = model.options.graticule;
    let min = model.options.min;
    let count = axes.len();
    // The direction of every axis, the first one straight up.
    let unit: Vec<Point> = (0..count)
        .map(|i| {
            let angle =
                (2.0 * std::f64::consts::PI * i as f64) / count as f64 - std::f64::consts::PI / 2.0;
            Point {
                x: angle.cos(),
                y: angle.sin(),
            }
        })
        .collect();

    // The largest value of the chart, when the chart does not name one.
    let max = match model.options.max {
        Some(max) => max,
        None => {
            let mut max = f64::NEG_INFINITY;
            for curve in curves {
                for value in &curve.entries {
                    if *value > max {
                        max = *value;
                    }
                }
            }
            max
        }
    };

    // The chart is drawn around the origin. The margins reserve room; the labels and the legend
    // widen it.
    let mut x0 = -width / 2.0 - option(input.config, "marginLeft", 16.0, 0.0, 2000.0);
    let mut x1 = width / 2.0 + option(input.config, "marginRight", 16.0, 0.0, 2000.0);
    let mut y0 = -height / 2.0 - option(input.config, "marginTop", 16.0, 0.0, 2000.0);
    let mut y1 = height / 2.0 + option(input.config, "marginBottom", 16.0, 0.0, 2000.0);

    let mut grid = String::new();
    let mut i = 0.0;
    while i < ticks {
        let r = radius.min((radius * (i + 1.0)) / ticks);
        if graticule == Graticule::Circle {
            grid += &format!("<circle r=\"{}\"/>", num(r));
        } else {
            let points: Vec<Point> = unit
                .iter()
                .map(|u| Point {
                    x: u.x * r,
                    y: u.y * r,
                })
                .collect();
            grid += &format!("<polygon points=\"{}\"/>", polygon(&points));
        }
        i += 1.0;
    }

    let mut spokes = String::new();
    let mut labels = String::new();
    for (i, u) in unit.iter().enumerate() {
        let reach = radius * axis_scale;
        spokes += &format!("M0,0L{},{}", num(u.x * reach), num(u.y * reach));
        let label = layout_label(
            Some(&axes[i].label),
            false,
            measurer,
            small,
            4000.0,
            Style::NONE,
        );
        let at = radius * label_factor + 6.0;
        // Every label hangs off its axis, on the side away from the centre.
        let cx = u.x * at
            + if u.x > 0.01 {
                label.width / 2.0
            } else if u.x < -0.01 {
                -label.width / 2.0
            } else {
                0.0
            };
        let cy = u.y * at
            + if u.y > 0.01 {
                label.height / 2.0
            } else if u.y < -0.01 {
                -label.height / 2.0
            } else {
                0.0
            };
        labels += &format!(
            "<g class=\"pele-rs-radar-axis-label\" data-id=\"{}\">{}</g>",
            esc_text(&axes[i].name),
            label_svg(&label, cx, cy, "")
        );
        x0 = x0.min(cx - label.width / 2.0);
        x1 = x1.max(cx + label.width / 2.0);
        y0 = y0.min(cy - label.height / 2.0);
        y1 = y1.max(cy + label.height / 2.0);
    }

    let mut shapes = String::new();
    for (index, curve) in curves.iter().enumerate() {
        // A curve that does not have a value for every axis is left out.
        if curve.entries.len() != count {
            continue;
        }
        let points: Vec<Point> = curve
            .entries
            .iter()
            .zip(unit.iter())
            .map(|(value, u)| {
                let r = relative_radius(*value, min, max, radius);
                let safe = if r.is_finite() { r } else { 0.0 };
                Point {
                    x: u.x * safe,
                    y: u.y * safe,
                }
            })
            .collect();
        let color = series_color(index);
        let attrs = format!(
            " class=\"pele-rs-radar-curve\" data-id=\"{}\" fill=\"{color}\" stroke=\"{color}\"",
            esc_text(&curve.name)
        );
        shapes += &if graticule == Graticule::Circle {
            let path = closed_round_curve(&points, tension);
            format!("<path{} d=\"{path}\"/>", attrs)
        } else {
            let points = polygon(&points);
            format!("<polygon{} points=\"{points}\"/>", attrs)
        };
    }

    let legend: Vec<Label> = if model.options.show_legend {
        curves
            .iter()
            .map(|curve| {
                layout_label(
                    Some(&curve.label),
                    false,
                    measurer,
                    small,
                    4000.0,
                    Style::NONE,
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut legend_width: f64 = 0.0;
    for label in &legend {
        legend_width = legend_width.max(label.width);
    }
    let legend_height = legend.len() as f64 * ROW;
    let legend_x = if tight {
        -(SWATCH + 8.0 + legend_width) / 2.0
    } else {
        x1 + GAP
    };
    let legend_top = if tight {
        y1 + GAP / 2.0
    } else {
        -legend_height / 2.0
    };
    if !legend.is_empty() && tight {
        x0 = x0.min(legend_x);
        x1 = x1.max(-legend_x);
        y1 = legend_top + legend_height;
    } else if !legend.is_empty() {
        x1 = legend_x + SWATCH + 8.0 + legend_width;
        y0 = y0.min(-legend_height / 2.0);
        y1 = y1.max(legend_height / 2.0);
    }
    let mut legend_out = String::new();
    for (index, label) in legend.iter().enumerate() {
        let y = legend_top + index as f64 * ROW + ROW / 2.0;
        let color = series_color(index);
        legend_out += &format!(
            "<g class=\"pele-rs-radar-legend-item\" data-id=\"{}\">\
             <rect x=\"{}\" y=\"{}\" width=\"{SWATCH}\" height=\"{SWATCH}\" rx=\"2\" \
             fill=\"{color}\" fill-opacity=\"0.4\" stroke=\"{color}\"/>{}</g>",
            esc_text(&curves[index].name),
            num(legend_x),
            num(y - SWATCH / 2.0),
            label_svg(label, legend_x + SWATCH + 8.0 + label.width / 2.0, y, "")
        );
    }

    let title = layout_label(
        model.title.as_deref(),
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
    let body_width = (x1 - x0).max(title.width);
    let total_width = (body_width + 2.0 * pad).ceil();
    let total_height = (y1 - y0 + title_height + 2.0 * pad).ceil();
    let origin = format!(
        "translate({},{})",
        num(pad + (body_width - (x1 - x0)) / 2.0 - x0),
        num(pad + title_height - y0)
    );

    let content = format!(
        "{}<g transform=\"{origin}\">{}{}{}{}{}</g>",
        label_svg(
            &title,
            total_width / 2.0,
            pad + title.height / 2.0,
            " class=\"pele-rs-title\" font-weight=\"var(--_tw)\""
        ),
        if grid.is_empty() {
            String::new()
        } else {
            format!(
                "<g class=\"pele-rs-radar-graticule\" fill=\"none\" stroke=\"var(--_a)\">{grid}</g>"
            )
        },
        if spokes.is_empty() {
            String::new()
        } else {
            format!(
                "<path class=\"pele-rs-radar-axes\" d=\"{spokes}\" fill=\"none\" stroke=\"var(--_b)\"/>"
            )
        },
        if shapes.is_empty() {
            String::new()
        } else {
            format!(
                "<g class=\"pele-rs-radar-curves\" fill-opacity=\"0.2\" stroke-width=\"1.5\" \
                 stroke-linejoin=\"round\">{shapes}</g>"
            )
        },
        if labels.is_empty() {
            String::new()
        } else {
            format!("<g class=\"pele-rs-radar-axis-labels\" font-size=\"{small}\">{labels}</g>")
        },
        if legend_out.is_empty() {
            String::new()
        } else {
            format!("<g class=\"pele-rs-radar-legend\" font-size=\"{small}\">{legend_out}</g>")
        },
    );

    let acc = Accessible {
        title: model.acc_title.clone(),
        descr: model.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            crate::DiagramKind::Radar,
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
    }
}
