//! Drawing a Wardley map.
//!
//! The map is a grid: the evolution axis runs across it and the visibility axis runs up it.
//! Every component stands where its coordinates name, the stages of the evolution axis are
//! written under the map, and the links between the components are drawn as straight lines.
//!
//! The layout is pele's: the axes, the stages, the pipelines, the notes, the annotations and
//! the forces are placed as pele places them.

use std::collections::HashMap;

use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::fit_width;
use crate::diagrams::wardley::db::{Flow, WardleyAnnotation, WardleyModel, WardleyNode};
use crate::error::Error;
use crate::preprocess::{Config, Value};
use crate::svg::markers::{marker, marker_trim};
use crate::svg::{
    Accessible, ROUNDED, esc, esc_text, label_svg, num, shared_style, span_style, svg_document,
    text_style,
};
use crate::text::label::{Label, layout_label};
use crate::text::{Style, TextMeasurer};
use crate::util::{js_number, round};

/// The stages of the evolution axis when the diagram names none.
const DEFAULT_STAGES: [&str; 4] = ["Genesis", "Custom Built", "Product", "Commodity"];
/// The room Mermaid's canvas leaves around the plot on every side.
const CANVAS_MARGIN: f64 = 48.0;
/// The radius of a component.
const R: f64 = 6.0;
/// The side of the square of a pipeline.
const SQUARE: f64 = 10.0;
/// How far the label of a component stands from it.
const LABEL_OFFSET: f64 = 8.0;
/// The room around the components of a pipeline.
const PIPELINE_PAD: f64 = 15.0;
/// The length of an accelerator.
const FORCE_WIDTH: f64 = 60.0;
/// The height of an accelerator.
const FORCE_HEIGHT: f64 = 30.0;
/// The length of the head of an accelerator.
const FORCE_HEAD: f64 = 20.0;
/// The width a label may take when nothing else is said about it.
const WIDE: f64 = 4000.0;

const ACCENT: &str = "var(--_c)";
const LINE: &str = "var(--_l)";

/// A node of the map, and where it stands.
struct Placed {
    x: f64,
    y: f64,
    /// The place of the node in the model.
    node: usize,
}

/// The box the drawing takes up.
#[derive(Clone, Copy)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn new(plot_w: f64, plot_h: f64) -> Self {
        Self {
            min_x: 0.0,
            min_y: 0.0,
            max_x: plot_w,
            max_y: plot_h,
        }
    }

    fn grow(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        self.min_x = self.min_x.min(x0);
        self.min_y = self.min_y.min(y0);
        self.max_x = self.max_x.max(x1);
        self.max_y = self.max_y.max(y1);
    }
}

/// A value of the settings of the diagram.
fn setting(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    let Some(Value::Map(section)) = config.get("wardley-beta") else {
        return fallback;
    };
    match section.get(key) {
        Some(Value::Number(n)) if n.is_finite() && *n >= min && *n <= max => *n,
        _ => fallback,
    }
}

/// Whether the settings of the diagram ask for a background grid.
fn show_grid(config: &Config) -> bool {
    matches!(
        config.get("wardley-beta"),
        Some(Value::Map(section)) if matches!(section.get("showGrid"), Some(Value::Bool(true)))
    )
}

/// The size of the plot, less the room Mermaid's canvas leaves around it.
fn dimension(config: &Config, given: Option<f64>, key: &str, fallback: f64) -> f64 {
    let value = given
        .filter(|value| value.is_finite())
        .unwrap_or_else(|| setting(config, key, fallback, 1.0, 1e6));
    value.clamp(200.0, 10000.0) - 2.0 * CANVAS_MARGIN
}

/// The place of the node with that id.
fn place_of(positions: &[Placed], nodes: &[&WardleyNode], id: &str) -> Option<usize> {
    positions.iter().position(|at| nodes[at.node].id == id)
}

/// A measured label.
fn text(
    raw: &str,
    measurer: &dyn TextMeasurer,
    font_size: f64,
    max_width: f64,
    style: Style,
) -> Label {
    layout_label(Some(raw), false, measurer, font_size, max_width, style)
}

/// Draws a Wardley map.
pub(crate) fn render_wardley(model: &WardleyModel, input: &Input<'_>) -> Result<Output, Error> {
    let natural = dimension(
        input.config,
        model.size.map(|size| size.width),
        "width",
        900.0,
    );
    Ok(fit_width(input.options, natural, 160.0, |plot_w, tight| {
        draw(model, input, plot_w, natural, tight)
    }))
}

/// Draws the map at a width.
fn draw(
    model: &WardleyModel,
    input: &Input<'_>,
    plot_w: f64,
    natural_w: f64,
    tight: bool,
) -> Output {
    let options = input.options;
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.8125);
    let tiny = round(size * 0.6875);
    let measurer = input.measurer;
    let pad = options.padding.unwrap_or(8.0);

    // A narrower map is not as much shorter, so that components keep some room between them.
    let plot_h = dimension(
        input.config,
        model.size.map(|size| size.height),
        "height",
        600.0,
    ) * (plot_w / natural_w).max(0.75);

    let mut bounds = Bounds::new(plot_w, plot_h);
    let project_x = |value: f64| (value / 100.0) * plot_w;
    let project_y = |value: f64| plot_h - (value / 100.0) * plot_h;

    let mut frame = String::new();
    let mut stage_labels = String::new();
    let mut stage_bottom = plot_h;
    let mut previous = 0.0;

    let stages: Vec<String> = match &model.axes.stages {
        Some(stages) if !stages.is_empty() => stages.clone(),
        _ => DEFAULT_STAGES
            .iter()
            .map(|stage| (*stage).to_string())
            .collect(),
    };
    let boundaries = model.axes.stage_boundaries.as_ref();
    let custom = boundaries.is_some_and(|boundaries| boundaries.len() == stages.len());

    for (index, stage) in stages.iter().enumerate() {
        let start = if custom {
            previous
        } else {
            index as f64 / stages.len() as f64
        };
        let end = if custom {
            boundaries.unwrap()[index].clamp(0.0, 1.0)
        } else {
            (index + 1) as f64 / stages.len() as f64
        };
        previous = end;
        if index > 0 {
            frame.push_str(&format!(
                "<path class=\"pele-wardley-stage\" d=\"M{},0V{}\" stroke=\"var(--_b)\" \
                 stroke-dasharray=\"4 4\"/>",
                num(start * plot_w),
                num(plot_h)
            ));
        }
        let label = text(
            stage,
            measurer,
            small,
            ((end - start).abs() * plot_w - 12.0).max(48.0),
            Style::NONE,
        );
        let (cx, cy) = (
            (start + end) / 2.0 * plot_w,
            plot_h + 8.0 + label.height / 2.0,
        );
        bounds.grow(
            cx - label.width / 2.0,
            cy - label.height / 2.0,
            cx + label.width / 2.0,
            cy + label.height / 2.0,
        );
        stage_labels.push_str(&label_svg(
            &label,
            cx,
            cy,
            &format!(
                " class=\"pele-wardley-stage-label\" font-size=\"{}\" fill=\"var(--_m)\"",
                num(small)
            ),
        ));
        stage_bottom = stage_bottom.max(plot_h + 8.0 + label.height);
    }

    if show_grid(input.config) {
        for i in 1..4 {
            frame.push_str(&format!(
                "<path class=\"pele-wardley-grid\" d=\"M{},0V{}M0,{}H{}\" stroke=\"var(--_b)\" \
                 stroke-dasharray=\"2 6\"/>",
                num(plot_w * i as f64 / 4.0),
                num(plot_h),
                num(plot_h * i as f64 / 4.0),
                num(plot_w)
            ));
        }
    }
    frame.push_str(&format!(
        "<path class=\"pele-wardley-axis\" d=\"M0,0V{}H{}\" stroke=\"{LINE}\"/>",
        num(plot_h),
        num(plot_w)
    ));

    let x_title = text(
        model.axes.x_label.as_deref().unwrap_or("Evolution"),
        measurer,
        small,
        WIDE,
        Style::NONE,
    );
    let y_title = text(
        model.axes.y_label.as_deref().unwrap_or("Visibility"),
        measurer,
        small,
        WIDE,
        Style::NONE,
    );
    let axis_attrs = format!(
        " class=\"pele-wardley-axis-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
        num(small)
    );
    let axis_cx = plot_w / 2.0;
    let axis_cy = stage_bottom + 6.0 + x_title.height / 2.0;
    bounds.grow(
        axis_cx - x_title.width / 2.0,
        axis_cy - x_title.height / 2.0,
        axis_cx + x_title.width / 2.0,
        axis_cy + x_title.height / 2.0,
    );
    let mut axis_labels = label_svg(&x_title, axis_cx, axis_cy, &axis_attrs);
    axis_labels.push_str(&label_svg(
        &y_title,
        -16.0,
        plot_h / 2.0,
        &format!(
            "{axis_attrs} transform=\"rotate(-90 -16 {})\"",
            num(plot_h / 2.0)
        ),
    ));
    bounds.grow(-16.0 - y_title.height / 2.0, 0.0, 0.0, 0.0);
    let legend_top = stage_bottom + 6.0 + x_title.height + 10.0;

    let nodes: Vec<&WardleyNode> = model.nodes.items.iter().map(|(_, node)| node).collect();
    let mut positions: Vec<Placed> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| Placed {
            x: project_x(node.x),
            y: project_y(node.y),
            node: index,
        })
        .collect();

    // Pipelines: a box around the components, with the parent as a square on its top edge.
    let mut pipelines = String::new();
    let mut members: HashMap<String, Vec<String>> = HashMap::new();
    for pipeline in &model.pipelines {
        members.insert(pipeline.node_id.clone(), pipeline.component_ids.clone());
        let mut parts: Vec<usize> = pipeline
            .component_ids
            .iter()
            .filter_map(|id| place_of(&positions, &nodes, id))
            .collect();
        if parts.is_empty() {
            continue;
        }
        parts.sort_by(|&a, &b| {
            positions[a]
                .x
                .partial_cmp(&positions[b].x)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let left = positions[parts[0]].x;
        let right = positions[parts[parts.len() - 1]].x;
        let y = positions[parts[0]].y;
        let top = y - 2.0 * R;
        if let Some(parent) = place_of(&positions, &nodes, &pipeline.node_id) {
            positions[parent].x = (left + right) / 2.0;
            positions[parent].y = top - SQUARE / 6.0;
        }
        bounds.grow(
            left - PIPELINE_PAD,
            top,
            right + PIPELINE_PAD,
            top + 4.0 * R,
        );
        pipelines.push_str(&format!(
            "<g class=\"pele-wardley-pipeline\" data-id=\"{}\">",
            esc_text(&pipeline.node_id)
        ));
        pipelines.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\" \
             stroke=\"var(--_b)\"{ROUNDED}/>",
            num(left - PIPELINE_PAD),
            num(top),
            num(right - left + 2.0 * PIPELINE_PAD),
            num(4.0 * R)
        ));
        if parts.len() > 1 {
            pipelines.push_str(&format!(
                "<path d=\"M{},{}H{}\" stroke=\"{LINE}\" stroke-dasharray=\"4 4\"/>",
                num(left),
                num(y),
                num(right)
            ));
        }
        pipelines.push_str("</g>");
    }

    // How far from a node's center a line to it stops.
    let mut anchor_labels: HashMap<usize, Label> = HashMap::new();
    let reach = |at: &Placed, ux: f64, uy: f64, anchor_labels: &mut HashMap<usize, Label>| -> f64 {
        let node = nodes[at.node];
        if node.class_name == "anchor" {
            let width = match anchor_labels.get(&at.node) {
                Some(label) => label.width,
                None => {
                    let label = text(
                        &node.label,
                        measurer,
                        small,
                        if tight {
                            (plot_w / 2.0).max(64.0)
                        } else {
                            WIDE
                        },
                        Style::BOLD,
                    );
                    anchor_labels.insert(at.node, label);
                    anchor_labels[&at.node].width
                }
            };
            return 1.0
                / ((ux / (width / 2.0 + 5.0)).powi(2) + (uy / (small * 0.8)).powi(2)).sqrt();
        }
        if node.is_pipeline_parent.unwrap_or(false) {
            return SQUARE / std::f64::consts::SQRT_2;
        }
        if node.source_strategy.is_some() {
            return 2.0 * R;
        }
        R
    };

    // Quadrants around a node that a link runs through: 1 up and right, 2 down and right,
    // 4 up and left, 8 down and left.
    let mut taken: HashMap<usize, u8> = HashMap::new();

    let mut links = String::new();
    for link in &model.links {
        let Some(from) = place_of(&positions, &nodes, &link.source) else {
            continue;
        };
        let Some(to) = place_of(&positions, &nodes, &link.target) else {
            continue;
        };
        if from == to {
            continue;
        }
        // A pipeline component belongs to its parent already.
        if members
            .get(&link.target)
            .is_some_and(|parts| parts.contains(&link.source))
        {
            continue;
        }
        let dx = positions[to].x - positions[from].x;
        let dy = positions[to].y - positions[from].y;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance < 0.01 {
            continue;
        }
        let ux = dx / distance;
        let uy = dy / distance;
        let start = reach(&positions[from], ux, uy, &mut anchor_labels);
        let end = reach(&positions[to], ux, uy, &mut anchor_labels);
        if start + end >= distance {
            continue;
        }
        for (at, sx, sy) in [(from, ux, uy), (to, -ux, -uy)] {
            if sx.abs() < 0.25 || sy.abs() < 0.25 {
                continue;
            }
            let quadrant = if sx > 0.0 {
                if sy < 0.0 { 1 } else { 2 }
            } else if sy < 0.0 {
                4
            } else {
                8
            };
            *taken.entry(at).or_insert(0) |= quadrant;
        }
        let mut x1 = positions[from].x + ux * start;
        let mut y1 = positions[from].y + uy * start;
        let mut x2 = positions[to].x - ux * end;
        let mut y2 = positions[to].y - uy * end;
        let mut heads = String::new();
        if matches!(link.flow, Some(Flow::Forward | Flow::Bidirectional)) {
            heads.push_str(&marker("arrow_point", x2, y2, ux, uy, LINE));
            x2 -= ux * marker_trim("arrow_point");
            y2 -= uy * marker_trim("arrow_point");
        }
        if matches!(link.flow, Some(Flow::Backward | Flow::Bidirectional)) {
            heads.push_str(&marker("arrow_point", x1, y1, -ux, -uy, LINE));
            x1 += ux * marker_trim("arrow_point");
            y1 += uy * marker_trim("arrow_point");
        }
        let mut caption = String::new();
        if let Some(caption_text) = &link.label {
            let label = text(caption_text, measurer, tiny + 1.0, 200.0, Style::NONE);
            // Beside the middle of the line, on its upper side.
            let side = if ux >= 0.0 { 1.0 } else { -1.0 };
            let px = uy * side;
            let py = -ux * side;
            let away = 5.0 + (px.abs() * label.width) / 2.0 + (py.abs() * label.height) / 2.0;
            let cx = (positions[from].x + positions[to].x) / 2.0 + px * away;
            let cy = (positions[from].y + positions[to].y) / 2.0 + py * away;
            bounds.grow(
                cx - label.width / 2.0,
                cy - label.height / 2.0,
                cx + label.width / 2.0,
                cy + label.height / 2.0,
            );
            caption = label_svg(
                &label,
                cx,
                cy,
                &format!(
                    " class=\"pele-edge-label\" font-size=\"{}\" fill=\"var(--_m)\"",
                    num(tiny + 1.0)
                ),
            );
        }
        links.push_str(&format!(
            "<g class=\"pele-edge{}\" data-id=\"{}-{}\">",
            if link.dashed {
                " pele-wardley-dashed"
            } else {
                ""
            },
            esc_text(&link.source),
            esc_text(&link.target)
        ));
        links.push_str(&format!(
            "<path d=\"M{},{}L{},{}\" stroke=\"{LINE}\"{}/>",
            num(x1),
            num(y1),
            num(x2),
            num(y2),
            if link.dashed {
                " stroke-dasharray=\"6 6\""
            } else {
                ""
            }
        ));
        links.push_str(&heads);
        links.push_str(&caption);
        links.push_str("</g>");
    }

    // Evolution: a dashed arrow to where the component is heading, drawn as an open circle.
    let mut trends = String::new();
    for trend in &model.trends {
        let Some(origin) = place_of(&positions, &nodes, &trend.node_id) else {
            continue;
        };
        let tx = project_x(trend.target_x);
        let ty = project_y(trend.target_y);
        let dx = tx - positions[origin].x;
        let dy = ty - positions[origin].y;
        let distance = (dx * dx + dy * dy).sqrt();
        let mut arrow = String::new();
        let start = if distance > 0.0 {
            reach(
                &positions[origin],
                dx / distance,
                dy / distance,
                &mut anchor_labels,
            )
        } else {
            0.0
        };
        if distance > start + R + 4.0 {
            let ux = dx / distance;
            let uy = dy / distance;
            let ex = tx - ux * (R + 2.0);
            let ey = ty - uy * (R + 2.0);
            arrow.push_str(&format!(
                "<path d=\"M{},{}L{},{}\" stroke=\"{ACCENT}\" stroke-dasharray=\"4 4\"/>",
                num(positions[origin].x + ux * start),
                num(positions[origin].y + uy * start),
                num(ex - ux * marker_trim("arrow_point")),
                num(ey - uy * marker_trim("arrow_point"))
            ));
            arrow.push_str(&marker("arrow_point", ex, ey, ux, uy, ACCENT));
        }
        bounds.grow(tx - R, ty - R, tx + R, ty + R);
        trends.push_str(&format!(
            "<g class=\"pele-wardley-trend\" data-id=\"{}\">",
            esc_text(&trend.node_id)
        ));
        trends.push_str(&arrow);
        trends.push_str(&format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_bg)\" stroke=\"{ACCENT}\"/>",
            num(tx),
            num(ty),
            num(R)
        ));
        trends.push_str("</g>");
    }

    let mut node_svg = String::new();
    let tri = 1.2 * R;
    let dot = 0.7 * R;
    for at in &positions {
        let node = nodes[at.node];
        let (x, y) = (at.x, at.y);
        let anchor = node.class_name == "anchor";
        let strategy = node.source_strategy.as_deref();
        let mut shape = String::new();
        if let Some(strategy) = strategy {
            let fill = match strategy {
                "outsource" => "var(--_m)",
                "buy" => "var(--_a)",
                "build" => "var(--_s)",
                _ => "var(--_bg)",
            };
            shape.push_str(&format!(
                "<circle class=\"pele-wardley-{strategy}\" cx=\"{}\" cy=\"{}\" r=\"{}\" \
                 fill=\"{fill}\" stroke=\"{LINE}\"/>",
                num(x),
                num(y),
                num(2.0 * R)
            ));
        }
        if strategy == Some("market") {
            let bx = tri * (std::f64::consts::PI / 6.0).cos();
            let by = y + tri * (std::f64::consts::PI / 6.0).sin();
            shape.push_str(&format!(
                "<path d=\"M{},{}L{},{}H{}Z\" fill=\"none\" stroke=\"{LINE}\"/>",
                num(x),
                num(y - tri),
                num(x - bx),
                num(by),
                num(x + bx)
            ));
            for (cx, cy) in [(x, y - tri), (x - bx, by), (x + bx, by)] {
                shape.push_str(&format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_bg)\" stroke=\"{LINE}\"/>",
                    num(cx),
                    num(cy),
                    num(dot)
                ));
            }
        } else if node.is_pipeline_parent.unwrap_or(false) {
            shape.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_bg)\" \
                 stroke=\"{LINE}\"/>",
                num(x - SQUARE / 2.0),
                num(y - SQUARE / 2.0),
                num(SQUARE),
                num(SQUARE)
            ));
        } else if !anchor {
            shape.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_bg)\" stroke=\"{LINE}\"/>",
                num(x),
                num(y),
                num(R)
            ));
        }
        if node.inertia.unwrap_or(false) {
            let offset = (if node.is_pipeline_parent.unwrap_or(false) {
                SQUARE / 2.0
            } else {
                R
            }) + 10.0
                + if strategy.is_some() { R + 4.0 } else { 0.0 };
            shape.push_str(&format!(
                "<rect class=\"pele-wardley-inertia\" x=\"{}\" y=\"{}\" width=\"4\" \
                 height=\"{}\" fill=\"{LINE}\"/>",
                num(x + offset),
                num(y - R),
                num(2.0 * R)
            ));
        }
        let ox = node
            .label_offset_x
            .map(|offset| offset.clamp(-5000.0, 5000.0));
        let oy = node
            .label_offset_y
            .map(|offset| offset.clamp(-5000.0, 5000.0));
        let caption;
        if anchor {
            let label = match anchor_labels.get(&at.node) {
                Some(label) => label.clone(),
                None => {
                    let label = text(
                        &node.label,
                        measurer,
                        small,
                        if tight {
                            (plot_w / 2.0).max(64.0)
                        } else {
                            WIDE
                        },
                        Style::BOLD,
                    );
                    anchor_labels.insert(at.node, label.clone());
                    label
                }
            };
            let cx = x + ox.unwrap_or(0.0);
            let cy = y + oy.unwrap_or(-3.0);
            bounds.grow(
                cx - label.width / 2.0,
                cy - label.height / 2.0,
                cx + label.width / 2.0,
                cy + label.height / 2.0,
            );
            caption = label_svg(
                &label,
                cx,
                cy,
                &format!(
                    " class=\"pele-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
                    num(small)
                ),
            );
        } else {
            let label = text(
                &node.label,
                measurer,
                small,
                if tight {
                    (plot_w / 2.0).clamp(64.0, 120.0)
                } else {
                    WIDE
                },
                Style::NONE,
            );
            let attrs = format!(" class=\"pele-label\" font-size=\"{}\"", num(small));
            let lift = LABEL_OFFSET + if strategy.is_some() { 10.0 } else { 0.0 };
            if ox.is_some() || oy.is_some() {
                let lx = x + ox.unwrap_or(lift);
                let ly = y + oy.unwrap_or(-lift);
                bounds.grow(
                    lx,
                    ly - label.size,
                    lx + label.width,
                    ly - label.size + label.height + 4.0,
                );
                caption = start_label(&label, lx, ly, &attrs);
            } else if node.in_pipeline.unwrap_or(false) {
                let cy = y + 2.0 * R + 4.0 + label.height / 2.0;
                bounds.grow(
                    x - label.width / 2.0,
                    cy - label.height / 2.0,
                    x + label.width / 2.0,
                    cy + label.height / 2.0,
                );
                caption = label_svg(&label, x, cy, &attrs);
            } else {
                // The label goes up and to the right unless a link leaves that way; then the
                // next free corner.
                let busy = taken.get(&at.node).copied().unwrap_or(0);
                let corner = if busy & 1 == 0 {
                    1
                } else if busy & 2 == 0 {
                    2
                } else if busy & 4 == 0 {
                    4
                } else if busy & 8 == 0 {
                    8
                } else {
                    1
                };
                let right = corner == 1 || corner == 2;
                let up = corner == 1 || corner == 4;
                let lx = if right {
                    x + lift
                } else {
                    x - lift - label.width
                };
                let ly = if up {
                    y - lift
                } else {
                    y + lift + small * 0.75
                };
                bounds.grow(
                    lx,
                    ly - label.size,
                    lx + label.width,
                    ly - label.size + label.height + 4.0,
                );
                caption = start_label(&label, lx, ly, &attrs);
            }
        }
        bounds.grow(x - 2.0 * R, y - 2.0 * R, x + 2.0 * R + 20.0, y + 2.0 * R);
        node_svg.push_str(&format!(
            "<g class=\"pele-node {}\" data-id=\"{}\">",
            if anchor {
                "pele-wardley-anchor"
            } else if node.is_pipeline_parent.unwrap_or(false) {
                "pele-wardley-pipeline-parent"
            } else if node.in_pipeline.unwrap_or(false) {
                "pele-wardley-pipeline-component"
            } else {
                "pele-wardley-component"
            },
            esc_text(&node.id)
        ));
        node_svg.push_str(&shape);
        node_svg.push_str(&caption);
        node_svg.push_str("</g>");
    }

    let mut annotations = String::new();
    for annotation in &model.annotations {
        let label = text(
            &js_number(annotation.number),
            measurer,
            tiny,
            WIDE,
            Style::BOLD,
        );
        let mut marks = String::new();
        for (cx, cy) in &annotation.coordinates {
            let x = project_x(*cx);
            let y = project_y(*cy);
            let r = (label.width / 2.0 + 4.0).max(10.0);
            bounds.grow(x - r, y - r, x + r, y + r);
            marks.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_bg)\" stroke=\"{LINE}\"/>",
                num(x),
                num(y),
                num(r)
            ));
            marks.push_str(&label_svg(
                &label,
                x,
                y,
                &format!(" font-size=\"{}\" font-weight=\"var(--_hw)\"", num(tiny)),
            ));
        }
        annotations.push_str(&format!(
            "<g class=\"pele-wardley-annotation\" data-id=\"{}\">",
            esc_text(&js_number(annotation.number))
        ));
        annotations.push_str(&marks);
        annotations.push_str("</g>");
    }
    let mut listed: Vec<&WardleyAnnotation> = model
        .annotations
        .iter()
        .filter(|a| a.text.is_some())
        .collect();
    listed.sort_by(|a, b| {
        a.number
            .partial_cmp(&b.number)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if !listed.is_empty() {
        let lines: Vec<Label> = listed
            .iter()
            .map(|annotation| {
                let line = format!(
                    "{}. {}",
                    js_number(annotation.number),
                    annotation.text.as_deref().unwrap_or_default()
                );
                text(
                    &line,
                    measurer,
                    small,
                    if tight {
                        (plot_w - 20.0).max(80.0)
                    } else {
                        320.0
                    },
                    Style::NONE,
                )
            })
            .collect();
        let mut width = 0.0_f64;
        let mut height = 0.0_f64;
        for line in &lines {
            width = width.max(line.width);
            height += line.height;
        }
        width += 20.0;
        height += 14.0;
        // Without a position the list goes under the map, where it covers nothing.
        let bx = match model.annotations_box {
            Some((x, _)) => project_x(x).clamp(0.0, (plot_w - width).max(0.0)),
            None => 0.0,
        };
        let by = match model.annotations_box {
            Some((_, y)) => project_y(y).clamp(0.0, (plot_h - height).max(0.0)),
            None => legend_top,
        };
        bounds.grow(bx, by, bx + width, by + height);
        let mut rows = String::new();
        let mut y = by + 7.0;
        for line in &lines {
            rows.push_str(&start_label(
                line,
                bx + 10.0,
                y + line.line_height / 2.0 + small * 0.35,
                &format!(" font-size=\"{}\"", num(small)),
            ));
            y += line.height;
        }
        annotations.push_str(&format!(
            "<g class=\"pele-wardley-annotations\"><rect x=\"{}\" y=\"{}\" width=\"{}\" \
             height=\"{}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{ROUNDED}/>{rows}</g>",
            num(bx),
            num(by),
            num(width),
            num(height)
        ));
    }

    let mut notes = String::new();
    for note in &model.notes {
        let label = text(
            &note.text,
            measurer,
            small,
            (plot_w * 0.6).min(360.0),
            Style::BOLD,
        );
        let x = project_x(note.x);
        let y = project_y(note.y);
        bounds.grow(
            x,
            y - label.size,
            x + label.width,
            y - label.size + label.height + 4.0,
        );
        notes.push_str(&start_label(
            &label,
            x,
            y,
            &format!(
                " class=\"pele-wardley-note\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
                num(small)
            ),
        ));
    }

    // Accelerators point right, deaccelerators left.
    let mut forces = String::new();
    for (name, px, py, forward) in model
        .accelerators
        .iter()
        .map(|force| (force.name.as_str(), force.x, force.y, true))
        .chain(
            model
                .deaccelerators
                .iter()
                .map(|force| (force.name.as_str(), force.x, force.y, false)),
        )
    {
        let x = project_x(px);
        let y = project_y(py);
        let half = FORCE_HEIGHT / 2.0;
        let tip = if forward { x + FORCE_WIDTH } else { x };
        let tail = if forward { x } else { x + FORCE_WIDTH };
        let neck = if forward {
            x + FORCE_WIDTH - FORCE_HEAD
        } else {
            x + FORCE_HEAD
        };
        let d = format!(
            "M{},{}H{}V{}L{},{}L{},{}V{}H{}Z",
            num(tail),
            num(y - half),
            num(neck),
            num(y - half - 8.0),
            num(tip),
            num(y),
            num(neck),
            num(y + half + 8.0),
            num(y + half),
            num(tail)
        );
        bounds.grow(x, y - half - 8.0, x + FORCE_WIDTH, y + half + 8.0);
        let label = text(name, measurer, tiny + 1.0, 160.0, Style::BOLD);
        let cx = x + FORCE_WIDTH / 2.0;
        let cy = y + half + 8.0 + 12.0;
        bounds.grow(
            cx - label.width / 2.0,
            cy - label.height / 2.0,
            cx + label.width / 2.0,
            cy + label.height / 2.0,
        );
        forces.push_str(&format!(
            "<g class=\"pele-wardley-{}\" data-id=\"{}\"><path d=\"{}\" fill=\"var(--_s)\" \
             stroke=\"{LINE}\"/>{}</g>",
            if forward {
                "accelerator"
            } else {
                "deaccelerator"
            },
            esc_text(name),
            d,
            label_svg(
                &label,
                cx,
                cy,
                &format!(
                    " font-size=\"{}\" font-weight=\"var(--_hw)\"",
                    num(tiny + 1.0)
                )
            )
        ));
    }

    let title = layout_label(
        model.title.as_deref(),
        false,
        measurer,
        size,
        if tight { plot_w.max(80.0) } else { WIDE },
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let left = bounds.min_x.min(plot_w / 2.0 - title.width / 2.0);
    let right = bounds.max_x.max(plot_w / 2.0 + title.width / 2.0);
    let tx = pad - left;
    let ty = pad + title_height - bounds.min_y;
    let total_width = (right - left + 2.0 * pad).ceil();
    let total_height = (bounds.max_y - bounds.min_y + title_height + 2.0 * pad).ceil();

    let mut content = label_svg(
        &title,
        tx + plot_w / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    content.push_str(&format!(
        "<g transform=\"translate({},{})\">",
        num(tx),
        num(ty)
    ));
    content.push_str(&format!(
        "<g class=\"pele-wardley-frame\" fill=\"none\">{frame}</g>"
    ));
    content.push_str(&stage_labels);
    content.push_str(&axis_labels);
    content.push_str(&pipelines);
    if !links.is_empty() {
        content.push_str(&format!(
            "<g class=\"pele-edges\" fill=\"none\">{links}</g>"
        ));
    }
    if !trends.is_empty() {
        content.push_str(&format!(
            "<g class=\"pele-wardley-trends\" fill=\"none\">{trends}</g>"
        ));
    }
    content.push_str(&node_svg);
    content.push_str(&annotations);
    content.push_str(&notes);
    content.push_str(&forces);
    content.push_str("</g>");

    let acc = Accessible {
        title: model.acc_title.clone(),
        descr: model.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            crate::DiagramKind::Wardley,
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

/// Writes a label with its lines starting at x and its first baseline at y.
fn start_label(label: &Label, x: f64, y: f64, attrs: &str) -> String {
    let shared = shared_style(label);
    let mut body = String::new();
    let mut baseline = y;
    for line in &label.lines {
        let mut first = true;
        for span in line {
            let style = span_style(span.style & !shared);
            let position = if first {
                format!(" x=\"{}\" y=\"{}\"", num(x), num(baseline))
            } else {
                String::new()
            };
            body.push_str(&format!(
                "<tspan{position}{style}>{}</tspan>",
                esc(&span.text)
            ));
            first = false;
        }
        baseline += label.line_height;
    }
    if body.is_empty() {
        return String::new();
    }
    let attrs = text_style(attrs, shared);
    format!("<text{attrs} xml:space=\"preserve\">{body}</text>")
}
