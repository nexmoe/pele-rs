//! The drawing of a quadrant chart: four boxes with the axes around them, and the points of
//! the campaigns in them.

use super::db::{QuadrantDb, QuadrantText};
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::cap_width;
use crate::error::Error;
use crate::preprocess::{Config, Value};
use crate::svg::{Accessible, esc_text, label_svg, num, svg_document};
use crate::svg::{ROUNDED, class_names, resolve_style, series_color};
use crate::text::Style;
use crate::text::label::{Label, layout_label};
use crate::util::{js_to_number, round};

/// The room between the quadrants.
const GAP: f64 = 4.0;
/// The number of points up to which the labels look for a free place.
const CAREFUL: f64 = 300.0;

/// A point of the chart, with the place its label has to keep clear of.
struct Dot {
    cx: f64,
    cy: f64,
    reach: f64,
    text: Label,
}

/// The value of a setting of the chart.
fn setting<'a>(config: &'a Config, key: &str) -> Option<&'a Value> {
    match config.get("quadrantChart") {
        Some(Value::Map(m)) => m.get(key),
        _ => None,
    }
}

/// A length from the settings: a value that is not a number, or is negative, falls back to the
/// default, and one that is too large is cut down to the maximum.
fn length(config: &Config, key: &str, fallback: f64, max: f64) -> f64 {
    match setting(config, key) {
        Some(Value::Number(n)) if *n >= 0.0 => n.min(max),
        _ => fallback,
    }
}

/// A color with its hash, as the styles of a point need.
fn hex(color: &str) -> String {
    if color.starts_with('#') {
        color.to_string()
    } else {
        format!("#{color}")
    }
}

/// Whether a box of the chart is free of the boxes that were taken before it.
fn is_clear(taken: &[f64], from: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
    let mut k = from;
    while k < taken.len() {
        if x0 < taken[k + 2] && x1 > taken[k] && y0 < taken[k + 3] && y1 > taken[k + 1] {
            return false;
        }
        k += 4;
    }
    true
}

/// Draws a quadrant chart.
pub(crate) fn render_quadrant(db: &QuadrantDb, input: &Input<'_>) -> Result<Output, Error> {
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let axis_size = round(size * 0.875);
    let point_size = round(size * 0.75);
    let config = &input.config;
    let natural = round(f64::max(100.0, length(config, "chartWidth", 500.0, 4000.0)));
    let width = round(cap_width(options, natural, 280.0));
    let height =
        round((f64::max(100.0, length(config, "chartHeight", 500.0, 4000.0)) * width) / natural);
    let outer = length(
        config,
        "quadrantPadding",
        options.padding.unwrap_or(8.0),
        200.0,
    );
    let x_pad = length(config, "xAxisLabelPadding", 5.0, 200.0);
    let y_pad = length(config, "yAxisLabelPadding", 5.0, 200.0);
    let has_points = !db.points.is_empty();

    let label = |text: Option<&QuadrantText>, font_size: f64, max: f64| -> Label {
        layout_label(
            text.map(|t| t.text.as_str()),
            text.is_some_and(|t| t.markdown),
            measurer,
            font_size,
            f64::max(max, 40.0),
            Style::NONE,
        )
    };

    let title = layout_label(
        db.title.as_deref(),
        false,
        measurer,
        size,
        f64::max(width - 2.0 * outer, 40.0),
        Style::BOLD,
    );
    let title_pad = length(config, "titlePadding", 10.0, 200.0);
    let title_space = if title.height > 0.0 {
        title.height + title_pad
    } else {
        0.0
    };

    let has_x = db.x_axis_left.as_ref().is_some_and(|t| !t.text.is_empty())
        || db.x_axis_right.as_ref().is_some_and(|t| !t.text.is_empty());
    let has_y = db
        .y_axis_bottom
        .as_ref()
        .is_some_and(|t| !t.text.is_empty())
        || db.y_axis_top.as_ref().is_some_and(|t| !t.text.is_empty());
    // Mermaid moves the x axis labels below the chart as soon as there are points.
    let x_below = has_points
        || matches!(setting(config, "xAxisPosition"), Some(Value::String(s)) if s == "bottom");
    let y_right =
        matches!(setting(config, "yAxisPosition"), Some(Value::String(s)) if s == "right");

    let side = width - 2.0 * outer;
    let x_left = label(
        db.x_axis_left.as_ref(),
        axis_size,
        if db.x_axis_right.is_some() {
            side / 2.0 - 16.0
        } else {
            side
        },
    );
    let x_right = label(db.x_axis_right.as_ref(), axis_size, side / 2.0 - 16.0);
    let x_height = f64::max(x_left.height, x_right.height);
    let x_space = if has_x { x_height + x_pad } else { 0.0 };
    let tall = height - 2.0 * outer - title_space - x_space;
    let y_bottom = label(
        db.y_axis_bottom.as_ref(),
        axis_size,
        if db.y_axis_top.is_some() {
            tall / 2.0 - 16.0
        } else {
            tall
        },
    );
    let y_top = label(db.y_axis_top.as_ref(), axis_size, tall / 2.0 - 16.0);
    let y_height = f64::max(y_bottom.height, y_top.height);
    let y_space = if has_y { y_height + y_pad } else { 0.0 };

    let left = outer + if y_right { 0.0 } else { y_space };
    let top = outer + title_space + if x_below { 0.0 } else { x_space };
    let w = f64::max(0.0, side - y_space);
    let h = f64::max(0.0, tall);
    let tile_w = f64::max(0.0, (w - GAP) / 2.0);
    let tile_h = f64::max(0.0, (h - GAP) / 2.0);

    // Boxes a point label keeps clear of: the gaps between quadrants, quadrant labels, points,
    // and the labels placed before it.
    let mut taken: Vec<f64> = vec![
        left + tile_w,
        top,
        left + w - tile_w,
        top + h,
        left,
        top + tile_h,
        left + w,
        top + h - tile_h,
    ];

    let text_top = length(config, "quadrantTextTopPadding", 5.0, 200.0);
    let mut quadrants = String::new();
    for k in 0..4 {
        let x = if k == 0 || k == 3 {
            left + w - tile_w
        } else {
            left
        };
        let y = if k < 2 { top } else { top + h - tile_h };
        let text = label(db.quadrants[k].as_ref(), size, tile_w - 16.0);
        let cx = x + tile_w / 2.0;
        let cy = if has_points {
            y + text_top + text.height / 2.0
        } else {
            y + tile_h / 2.0
        };
        taken.push(cx - text.width / 2.0);
        taken.push(cy - text.height / 2.0);
        taken.push(cx + text.width / 2.0);
        taken.push(cy + text.height / 2.0);
        quadrants += &format!(
            "<g class=\"pele-rs-quadrant\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" \
             height=\"{}\"{ROUNDED}/>{}</g>",
            k + 1,
            num(x),
            num(y),
            num(tile_w),
            num(tile_h),
            label_svg(
                &text,
                cx,
                cy,
                " class=\"pele-rs-label\" fill=\"var(--_fg)\" stroke=\"none\""
            )
        );
    }

    let mut axes = String::new();
    if has_x {
        let y = if x_below {
            top + h + x_pad + x_height / 2.0
        } else {
            top - x_pad - x_height / 2.0
        };
        axes += &label_svg(
            &x_left,
            if x_right.height > 0.0 {
                left + w / 4.0
            } else {
                left + x_left.width / 2.0
            },
            y,
            "",
        );
        axes += &label_svg(&x_right, left + (w * 3.0) / 4.0, y, "");
    }
    if has_y {
        let x = if y_right {
            left + w + y_pad + y_height / 2.0
        } else {
            left - y_pad - y_height / 2.0
        };
        let turned = |text: &Label, y: f64| -> String {
            if text.height > 0.0 {
                format!(
                    "<g transform=\"translate({},{}) rotate(-90)\">{}</g>",
                    num(x),
                    num(y),
                    label_svg(text, 0.0, 0.0, "")
                )
            } else {
                String::new()
            }
        };
        axes += &turned(
            &y_bottom,
            if y_top.height > 0.0 {
                top + (h * 3.0) / 4.0
            } else {
                top + h - y_bottom.width / 2.0
            },
        );
        axes += &turned(&y_top, top + h / 4.0);
    }

    let radius = length(config, "pointRadius", 5.0, 200.0);
    let text_pad = length(config, "pointTextPadding", 5.0, 200.0);
    let fill = series_color(0);
    let mut dots: Vec<Dot> = Vec::new();
    let mut circles: Vec<String> = Vec::new();
    for point in &db.points {
        if !(point.x >= 0.0 && point.x <= 1.0 && point.y >= 0.0 && point.y <= 1.0) {
            continue;
        }
        let shared = db.classes.get(&point.class_name);
        let r = f64::min(
            point
                .style
                .radius
                .or_else(|| shared.and_then(|s| s.radius))
                .unwrap_or(radius),
            2000.0,
        );
        let color = point
            .style
            .color
            .clone()
            .or_else(|| shared.and_then(|s| s.color.clone()));
        let stroke_color = point
            .style
            .stroke_color
            .clone()
            .or_else(|| shared.and_then(|s| s.stroke_color.clone()));
        let stroke_width = point
            .style
            .stroke_width
            .clone()
            .or_else(|| shared.and_then(|s| s.stroke_width.clone()));
        let mut styles: Vec<String> = Vec::new();
        if let Some(color) = &color {
            styles.push(format!("fill:{}", hex(color)));
        }
        if let Some(stroke_width) = &stroke_width {
            if let Some(stroke_color) = &stroke_color {
                styles.push(format!("stroke:{}", hex(stroke_color)));
            }
            styles.push(format!("stroke-width:{stroke_width}"));
        }
        let cx = left + point.x * w;
        let cy = top + (1.0 - point.y) * h;
        dots.push(Dot {
            cx,
            cy,
            reach: r + if stroke_width.is_some() {
                f64::min(
                    js_to_number(
                        stroke_width
                            .as_deref()
                            .unwrap_or_default()
                            .trim_end_matches("px"),
                    ),
                    400.0,
                ) / 2.0
            } else {
                0.0
            },
            text: label(Some(&point.text), point_size, 200.0),
        });
        circles.push(format!(
            "<circle class=\"pele-rs-point{}\" data-id=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{}\"{}{}/>",
            class_names(&point.class_name),
            esc_text(&point.text.text),
            num(cx),
            num(cy),
            num(r),
            if stroke_width.is_some() && stroke_color.is_none() {
                format!(" stroke=\"{fill}\"")
            } else {
                String::new()
            },
            resolve_style(&styles).shape,
        ));
    }

    // A label goes below its point, or above, right or left of it, or below its quadrant's
    // title, when that is the first place that is inside the chart and free. A second round
    // lets it cross the gaps between quadrants. Large charts skip the search for a free place.
    let careful = dots.len() as f64 <= CAREFUL;
    if careful {
        for dot in &dots {
            taken.push(dot.cx - dot.reach);
            taken.push(dot.cy - dot.reach);
            taken.push(dot.cx + dot.reach);
            taken.push(dot.cy + dot.reach);
        }
    }
    let mut labels = String::new();
    for dot in &dots {
        let half_w = dot.text.width / 2.0;
        let half_h = dot.text.size * 0.6 + (dot.text.height - dot.text.line_height) / 2.0;
        let away = dot.reach + text_pad;
        let inset = f64::min(away, f64::max(0.0, (tile_w - dot.text.width) / 2.0));
        let heading = 8.0
            + 4.0
                * (if dot.cx < left + w / 2.0 {
                    if dot.cy < top + h / 2.0 { 1.0 } else { 2.0 }
                } else if dot.cy < top + h / 2.0 {
                    0.0
                } else {
                    3.0
                });
        let below = f64::max(dot.cy + away, taken[heading as usize + 3] + text_pad) + half_h;
        let spots = [
            dot.cx,
            dot.cy + away + half_h,
            dot.cx,
            dot.cy - away - half_h,
            dot.cx + away + half_w,
            dot.cy,
            dot.cx - away - half_w,
            dot.cy,
            dot.cx,
            below,
        ];
        let mut tx = dot.cx;
        let mut ty = spots[1];
        let mut found = false;
        let mut k = 0;
        while k < 20 {
            let spot = k % 10;
            // A label moved below a title keeps clear of the plot's edge.
            let margin = if spot == 8 { inset } else { 0.0 };
            let x = if (spot < 4 || spot == 8) && dot.text.width < w {
                f64::min(
                    f64::max(spots[spot], left + half_w + margin),
                    left + w - half_w - margin,
                )
            } else {
                spots[spot]
            };
            let y = spots[spot + 1];
            if x - half_w < left
                || x + half_w > left + w
                || y - half_h < top
                || y + half_h > top + h
            {
                k += 2;
                continue;
            }
            let free = !careful
                || is_clear(
                    &taken,
                    if k < 10 { 0 } else { 8 },
                    x - half_w,
                    y - half_h,
                    x + half_w,
                    y + half_h,
                );
            if !found || free {
                tx = x;
                ty = y;
                found = true;
            }
            if free {
                break;
            }
            k += 2;
        }
        if careful {
            taken.push(tx - half_w);
            taken.push(ty - half_h);
            taken.push(tx + half_w);
            taken.push(ty + half_h);
        }
        labels += &label_svg(&dot.text, tx, ty, " class=\"pele-rs-label\"");
    }

    // Mermaid puts each new point under the ones before it.
    let title_svg = label_svg(
        &title,
        width / 2.0,
        title_pad / 2.0 + title.height / 2.0,
        " class=\"pele-rs-title\" font-weight=\"var(--_tw)\"",
    );
    let quadrants_svg = format!(
        "<g class=\"pele-rs-quadrants\" fill=\"var(--_s)\" stroke=\"var(--_b)\">{quadrants}</g>"
    );
    let axes_svg = if axes.is_empty() {
        String::new()
    } else {
        format!(
            "<g class=\"pele-rs-axis-labels\" font-size=\"{axis_size}\" \
             fill=\"var(--_m)\">{axes}</g>"
        )
    };
    let points_svg = if dots.is_empty() {
        String::new()
    } else {
        format!(
            "<g class=\"pele-rs-points\" fill=\"{fill}\">{}</g><g \
             class=\"pele-rs-point-labels\" font-size=\"{point_size}\">{labels}</g>",
            circles.iter().rev().cloned().collect::<Vec<_>>().join("")
        )
    };
    let content = format!("{title_svg}{quadrants_svg}{axes_svg}{points_svg}");

    let acc = Accessible {
        title: db.acc_title.clone(),
        descr: db.acc_descr.clone(),
    };
    Ok(Output {
        svg: svg_document(
            crate::DiagramKind::QuadrantChart,
            width,
            height,
            size,
            options,
            &acc,
            &content,
        ),
        width,
        height,
        links: Vec::new(),
    })
}
