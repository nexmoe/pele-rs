//! Requirement diagram layout and SVG. Ported from pele's `diagrams/requirement/render.ts`,
//! without math.

use std::collections::HashMap;
use std::fmt::Write;

use super::db::RequirementDb;
use super::graph::build_requirement_graph;
use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{runs_across, turn_to_fit};
use crate::layout::compound::{CEdge, CNode, Dir, cedge, cnode, compound_layout, direction};
use crate::layout::layered::LayeredOptions;
use crate::svg::edges::{EdgePath, edge_label_svg, route_path};
use crate::svg::{
    Accessible, ResolvedStyle, class_names, esc, esc_text, label_svg, num, resolve_style, rounded,
    shared_style, span_style, svg_document, text_style, title_label,
};
use crate::text::label::{Label, has_math, label_line, layout_label, line_middle};
use crate::text::{Style, TextMeasurer};
use crate::util::{is_space, round};

const NODE_SEP: f64 = 40.0;
const EDGE_SEP: f64 = 16.0;
const RANK_SEP: f64 = 56.0;
const PAD_X: f64 = 14.0;
const PAD_Y: f64 = 10.0;
const ROW_GAP: f64 = 2.0;
const COLUMN_GAP: f64 = 12.0;
const MIN_WIDTH: f64 = 120.0;
const NAME_WRAP: f64 = 280.0;
const VALUE_WRAP: f64 = 240.0;
const LOOP: f64 = 26.0;
const CONTAINS_RADIUS: f64 = 6.0;
const ARROW_LENGTH: f64 = 9.0;
const ARROW_HALF: f64 = 4.5;
const END_STEP: f64 = 28.0;
const END_MARGIN: f64 = 14.0;

struct Row {
    key: Label,
    value: Label,
    height: f64,
}

struct View {
    style: ResolvedStyle,
    kind: Label,
    name: Label,
    rows: Vec<Row>,
    key_width: f64,
    head_height: f64,
    w: f64,
    h: f64,
}

/// Adds bold or italic to a whole label after its markdown is read, since emphasis markers
/// only toggle styles the label does not start with.
fn restyle(mut label: Label, bits: Style, measurer: &dyn TextMeasurer) -> Label {
    if bits.is_empty() || label.lines.is_empty() {
        return label;
    }
    let mut width: f64 = 0.0;
    label.widths.clear();
    for line in &mut label.lines {
        let mut total = 0.0;
        for span in line.iter_mut() {
            span.style |= bits;
            // An icon or math keeps the width it was measured with.
            if span.is_plain() {
                span.width = measurer.width(&span.text, label.size, span.style);
            }
            total += span.width;
        }
        label.widths.push(total);
        width = width.max(total);
    }
    label.width = width;
    label
}

/// Writes a measured label with its lines starting at x, the first line's box starting at top.
fn start_text(label: &Label, x: f64, top: f64, attrs: &str) -> String {
    let shared = shared_style(label);
    let mut body = String::new();
    let mut math = String::new();
    for (i, line) in label.lines.iter().enumerate() {
        let middle = top + line_middle(label, i);
        // A line that holds math is written as a label of its own.
        if has_math(line) {
            math += &label_svg(
                &label_line(label, i),
                x + label.widths[i] / 2.0,
                middle,
                attrs,
            );
            continue;
        }
        let y = middle + label.size * 0.35;
        let mut first = true;
        for span in line {
            if span.icon.is_some() || span.text.is_empty() {
                continue;
            }
            let pos = if first {
                format!(" x=\"{}\" y=\"{}\"", num(x), num(y))
            } else {
                String::new()
            };
            let _ = write!(
                body,
                "<tspan{pos}{}>{}</tspan>",
                span_style(span.style & !shared),
                esc(&span.text)
            );
            first = false;
        }
    }
    if body.is_empty() {
        return math;
    }
    format!(
        "<text{} text-anchor=\"start\" xml:space=\"preserve\">{body}</text>{math}",
        text_style(attrs, shared)
    )
}

/// The containment mark: a circled plus whose rim touches (x, y), where (dx, dy) points at the
/// node.
fn contains_marker(x: f64, y: f64, dx: f64, dy: f64) -> String {
    let r = CONTAINS_RADIUS;
    let cx = x - dx * r;
    let cy = y - dy * r;
    format!(
        "<circle class=\"pele-rs-marker\" cx=\"{}\" cy=\"{}\" r=\"{}\"/><path class=\"pele-rs-marker\" d=\"M{},{}L{},{}M{},{}L{},{}\"/>",
        num(cx),
        num(cy),
        num(r),
        num(cx - dx * r),
        num(cy - dy * r),
        num(cx + dx * r),
        num(cy + dy * r),
        num(cx + dy * r),
        num(cy - dx * r),
        num(cx - dy * r),
        num(cy + dx * r)
    )
}

/// An open arrowhead with its tip at (x, y), pointing along (dx, dy).
fn arrow_marker(x: f64, y: f64, dx: f64, dy: f64) -> String {
    let bx = x - dx * ARROW_LENGTH;
    let by = y - dy * ARROW_LENGTH;
    format!(
        "<path class=\"pele-rs-marker\" d=\"M{},{}L{},{}L{},{}\" stroke-linejoin=\"round\"/>",
        num(bx - dy * ARROW_HALF),
        num(by + dx * ARROW_HALF),
        num(x),
        num(y),
        num(bx + dy * ARROW_HALF),
        num(by - dx * ARROW_HALF)
    )
}

/// The layout attaches every edge to the middle of a node's side. Ends that share a side are
/// spread along it, in the order of where they head, so their markers do not sit on each
/// other.
fn spread_ends(cnodes: &[CNode], cedges: &mut [CEdge], views: &[View]) {
    struct End {
        edge: usize,
        at: usize,
        toward: f64,
    }
    let mut sides: Vec<(usize, Vec<End>)> = Vec::new();
    let mut side_at: HashMap<usize, usize> = HashMap::new();
    for (j, edge) in cedges.iter().enumerate() {
        let route = &edge.route;
        if edge.src == edge.dst || route.len() < 6 {
            continue;
        }
        for at in [0, route.len() - 3] {
            let node = if at == 0 { edge.src } else { edge.dst };
            let c = &cnodes[node];
            let dx = route[at] - c.x;
            let dy = route[at + 1] - c.y;
            let next = if at == 0 { 3 } else { at - 3 };
            let side = if (dy.abs() - c.h / 2.0).abs() < 0.5 {
                if dy < 0.0 { 0 } else { 2 }
            } else if (dx.abs() - c.w / 2.0).abs() < 0.5 {
                if dx < 0.0 { 3 } else { 1 }
            } else {
                continue;
            };
            let key = node * 4 + side;
            let end = End {
                edge: j,
                at,
                toward: route[if side % 2 == 0 { next } else { next + 1 }],
            };
            match side_at.get(&key) {
                Some(&k) => sides[k].1.push(end),
                None => {
                    side_at.insert(key, sides.len());
                    sides.push((key, vec![end]));
                }
            }
        }
    }
    for (key, list) in &mut sides {
        if list.len() < 2 {
            continue;
        }
        let node = *key >> 2;
        let vertical = *key % 2 == 1;
        let span = if vertical {
            views[node].h
        } else {
            views[node].w
        } - 2.0 * END_MARGIN;
        let count = list.len() as f64;
        let step = END_STEP.min(span / (count - 1.0)).max(0.0);
        list.sort_by(|a, b| {
            a.toward
                .partial_cmp(&b.toward)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (j, end) in list.iter().enumerate() {
            let offset = (j as f64 - (count - 1.0) / 2.0) * step;
            if vertical {
                cedges[end.edge].route[end.at + 1] = cnodes[node].y + offset;
            } else {
                cedges[end.edge].route[end.at] = cnodes[node].x + offset;
            }
        }
    }
}

/// `classes.replace(/^default\s?/, '')`.
fn without_default(classes: &str) -> &str {
    match classes.strip_prefix("default") {
        Some(rest) => match rest.chars().next() {
            Some(c) if is_space(c) => &rest[c.len_utf8()..],
            _ => rest,
        },
        None => classes,
    }
}

pub(crate) fn render_requirement(db: &RequirementDb, input: &Input<'_>) -> Output {
    if runs_across(Some(db.direction)) {
        turn_to_fit(input.options, |down| draw(db, input, down))
    } else {
        draw(db, input, false)
    }
}

/// `turned` draws a diagram that runs across as one that runs down.
fn draw(db: &RequirementDb, input: &Input<'_>, turned: bool) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let (nodes, edges) = build_requirement_graph(db);
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let pad = options.padding.unwrap_or(8.0);

    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut views: Vec<View> = Vec::with_capacity(nodes.len());
    let mut cnodes: Vec<CNode> = Vec::with_capacity(nodes.len());
    for node in &nodes {
        let style = resolve_style(node.css_styles);
        let mut base = Style::NONE;
        if style.bold {
            base |= Style::BOLD;
        }
        if style.italic {
            base |= Style::ITALIC;
        }
        let name_size = style.font_size.unwrap_or(size);
        let body_size = style.font_size.unwrap_or(small);
        let kind_text = format!("«{}»", node.requirement.map_or("Element", |r| r.kind));
        let kind = restyle(
            layout_label(
                Some(&kind_text),
                false,
                measurer,
                body_size,
                4000.0,
                Style::NONE,
            ),
            base,
            measurer,
        );
        let name = restyle(
            layout_label(
                Some(node.id),
                true,
                measurer,
                name_size,
                NAME_WRAP,
                Style::NONE,
            ),
            base | Style::BOLD,
            measurer,
        );
        let fields: Vec<(&str, &str)> = match (node.requirement, node.element) {
            (Some(r), _) => vec![
                ("ID", &r.requirement_id),
                ("Text", &r.text),
                ("Risk", &r.risk),
                ("Verification", &r.verify_method),
            ],
            (None, Some(e)) => vec![("Type", &e.kind), ("Doc Ref", &e.doc_ref)],
            (None, None) => Vec::new(),
        };
        let mut rows = Vec::new();
        let mut key_width: f64 = 0.0;
        let mut value_width: f64 = 0.0;
        let mut body_height = 0.0;
        for (label, text) in fields {
            if text.is_empty() {
                continue;
            }
            let key = restyle(
                layout_label(Some(label), false, measurer, body_size, 4000.0, Style::NONE),
                base,
                measurer,
            );
            let value = restyle(
                layout_label(
                    Some(text),
                    true,
                    measurer,
                    body_size,
                    VALUE_WRAP,
                    Style::NONE,
                ),
                base,
                measurer,
            );
            let height = key.height.max(value.height);
            key_width = key_width.max(key.width);
            value_width = value_width.max(value.width);
            body_height += height + ROW_GAP;
            rows.push(Row { key, value, height });
        }
        if !rows.is_empty() {
            body_height += 2.0 * PAD_Y - ROW_GAP;
        }
        let head_height = 2.0 * PAD_Y + kind.height + name.height;
        let w = MIN_WIDTH
            .max(kind.width.max(name.width) + 2.0 * PAD_X)
            .max(if rows.is_empty() {
                0.0
            } else {
                key_width + COLUMN_GAP + value_width + 2.0 * PAD_X
            });
        // A requirement and an element may share a name; relationships then go to the
        // requirement.
        index.entry(node.id).or_insert(views.len());
        let mut c = cnode(w, head_height + body_height);
        c.seq = views.len() as f64;
        views.push(View {
            style,
            kind,
            name,
            rows,
            key_width,
            head_height,
            w,
            h: head_height + body_height,
        });
        cnodes.push(c);
    }

    let root_dir = if turned {
        Dir::TB
    } else {
        direction(Some(db.direction))
    };
    let sideways = matches!(root_dir, Dir::LR | Dir::RL);
    let mut cedges: Vec<CEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut drawn = Vec::new();
    for edge in &edges {
        let (Some(&src), Some(&dst)) = (
            edge.start.and_then(|s| index.get(s)),
            edge.end.and_then(|e| index.get(e)),
        ) else {
            continue;
        };
        let label = layout_label(
            Some(&format!("«{}»", edge.kind)),
            false,
            measurer,
            small,
            4000.0,
            Style::NONE,
        );
        if src == dst {
            // A self-loop sits beside the node, across the flow, and the node's box grows to
            // hold it.
            if sideways {
                cnodes[src].h += 2.0 * (LOOP + label.height + 8.0);
            } else {
                cnodes[src].w += 2.0 * (LOOP + label.width + 12.0);
            }
        }
        cedges.push(cedge(src, dst, 1, label.width + 12.0, label.height + 4.0));
        drawn.push(edge);
        edge_labels.push(label);
    }

    let layout_options = LayeredOptions {
        node_sep: NODE_SEP,
        edge_sep: EDGE_SEP,
        rank_sep: RANK_SEP,
        port_sep: 20.0,
        ..LayeredOptions::default()
    };
    let layout = compound_layout(&mut cnodes, &mut cedges, root_dir, &layout_options);

    spread_ends(&cnodes, &mut cedges, &views);

    let title = title_label(db.title.as_deref(), measurer, size, 4000.0);
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let inner = layout.width.max(title.width);
    let ox = pad + (inner - layout.width) / 2.0;
    let oy = pad + title_height;
    let width = (inner + 2.0 * pad).ceil();
    let height = (layout.height + title_height + 2.0 * pad).ceil();

    let mut nodes_out = String::new();
    for (i, view) in views.iter().enumerate() {
        let node = &nodes[i];
        let (w, h, style) = (view.w, view.h, &view.style);
        let left = -w / 2.0;
        let top = -h / 2.0;
        let mut body = format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{}/>",
            num(left),
            num(top),
            num(w),
            num(h),
            rounded(&style.shape)
        );
        body += &label_svg(
            &view.kind,
            0.0,
            top + PAD_Y + view.kind.height / 2.0,
            &format!(
                " class=\"pele-rs-stereotype\" font-size=\"{}\" fill=\"var(--_m)\"{}",
                num(view.kind.size),
                style.text
            ),
        );
        body += &label_svg(
            &view.name,
            0.0,
            top + PAD_Y + view.kind.height + view.name.height / 2.0,
            &format!(
                " class=\"pele-rs-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"{}",
                num(view.name.size),
                style.text
            ),
        );
        if !view.rows.is_empty() {
            let divider = top + view.head_height;
            let _ = write!(
                body,
                "<path class=\"pele-rs-divider\" d=\"M{},{}H{}\" stroke=\"var(--_b)\"{}/>",
                num(left),
                num(divider),
                num(-left),
                style.line
            );
            let mut y = divider + PAD_Y;
            let mut rows = String::new();
            for row in &view.rows {
                rows += &start_text(
                    &row.key,
                    left + PAD_X,
                    y,
                    &format!(" class=\"pele-rs-field\" fill=\"var(--_m)\"{}", style.text),
                );
                rows += &start_text(
                    &row.value,
                    left + PAD_X + view.key_width + COLUMN_GAP,
                    y,
                    &format!(" class=\"pele-rs-value\"{}", style.text),
                );
                y += row.height + ROW_GAP;
            }
            let _ = write!(
                body,
                "<g class=\"pele-rs-fields\" font-size=\"{}\">{rows}</g>",
                num(view.rows[0].key.size)
            );
        }
        let _ = write!(
            nodes_out,
            "<g class=\"pele-rs-node {}{}\" data-id=\"{}\" transform=\"translate({},{})\">{body}</g>",
            if node.requirement.is_some() {
                "pele-rs-requirement"
            } else {
                "pele-rs-element"
            },
            class_names(without_default(&node.css_classes)),
            esc_text(node.id),
            num(cnodes[i].x + ox),
            num(cnodes[i].y + oy)
        );
    }

    let mut edges_out = String::new();
    let mut labels_out = String::new();
    let mut loop_seen: HashMap<usize, usize> = HashMap::new();
    for (i, ce) in cedges.iter().enumerate() {
        let edge = drawn[i];
        let label = &edge_labels[i];
        let start_trim = if edge.contains {
            2.0 * CONTAINS_RADIUS
        } else {
            0.0
        };
        let mut label_x = ce.label_x + ox;
        let mut label_y = ce.label_y + oy;
        let path = if ce.src == ce.dst {
            let view = &views[ce.src];
            let cx = cnodes[ce.src].x + ox;
            let cy = cnodes[ce.src].y + oy;
            let seen = loop_seen.entry(ce.src).or_insert(0);
            let k = *seen as f64;
            *seen += 1;
            let half = if sideways { view.w } else { view.h } / 2.0;
            let spread = (half - 4.0).min(12.0 + k * 6.0);
            let reach = LOOP + k * 8.0;
            // Local frame: `out` points away from the node, `along` runs along its side.
            let at = |out: f64, along: f64| {
                if sideways {
                    format!("{},{}", num(cx + along), num(cy + view.h / 2.0 + out))
                } else {
                    format!("{},{}", num(cx + view.w / 2.0 + out), num(cy + along))
                }
            };
            // A cubic whose control points are this far out peaks at `reach`.
            let pull = reach / 0.75;
            if sideways {
                label_x = cx;
                label_y = cy + view.h / 2.0 + reach + 4.0 + ce.label_h / 2.0;
            } else {
                label_x = cx + view.w / 2.0 + reach + 4.0 + ce.label_w / 2.0;
                label_y = cy;
            }
            EdgePath {
                d: format!(
                    "M{}C{} {} {}",
                    at(start_trim, -spread),
                    at(pull, -spread),
                    at(pull, spread),
                    at(0.0, spread)
                ),
                sx: if sideways {
                    cx - spread
                } else {
                    cx + view.w / 2.0
                },
                sy: if sideways {
                    cy + view.h / 2.0
                } else {
                    cy - spread
                },
                sdx: if sideways { 0.0 } else { -1.0 },
                sdy: if sideways { -1.0 } else { 0.0 },
                ex: if sideways {
                    cx + spread
                } else {
                    cx + view.w / 2.0
                },
                ey: if sideways {
                    cy + view.h / 2.0
                } else {
                    cy + spread
                },
                edx: if sideways { 0.0 } else { -1.0 },
                edy: if sideways { -1.0 } else { 0.0 },
            }
        } else {
            let mut route = ce.route.clone();
            for k in (0..route.len()).step_by(3) {
                route[k] += ox;
                route[k + 1] += oy;
            }
            route_path(&route, None, start_trim, 0.0, None, None)
        };
        let _ = write!(
            edges_out,
            "<g class=\"pele-rs-edge pele-rs-relationship-{}\" data-id=\"{}\"><path d=\"{}\"{}/>{}</g>",
            if edge.contains { "contains" } else { "arrow" },
            esc_text(&edge.id),
            path.d,
            if edge.contains {
                ""
            } else {
                " stroke-dasharray=\"5 4\""
            },
            if edge.contains {
                contains_marker(path.sx, path.sy, path.sdx, path.sdy)
            } else {
                arrow_marker(path.ex, path.ey, path.edx, path.edy)
            }
        );
        labels_out += &edge_label_svg(&edge.id, label, label_x, label_y, "");
    }

    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-rs-title\" font-weight=\"var(--_tw)\"",
    );
    if !edges_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges_out}</g>"
        );
    }
    if !labels_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-edge-labels\" font-size=\"{}\" fill=\"var(--_m)\">{labels_out}</g>",
            num(small)
        );
    }
    let _ = write!(content, "<g class=\"pele-rs-nodes\">{nodes_out}</g>");

    let acc = Accessible {
        title: db.acc_title.clone(),
        descr: db.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            DiagramKind::Requirement,
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
    }
}
