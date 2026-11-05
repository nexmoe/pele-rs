//! The drawing of an agentflow diagram: the flows that hold the agents, the agents
//! themselves, and the edges between them. Ported from pele's `diagrams/agentflow/render.ts`.

use std::collections::HashMap;
use std::fmt::Write;

use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::title_room;
use crate::error::Error;
use crate::layout::compound::{
    CEdge, CNode, Dir, cedge, cnode, compound_layout, direction, shift_layout,
};
use crate::layout::layered::LayeredOptions;
use crate::options::Link;
use crate::svg::cluster::{Crossings, cluster_title_x, mark_crossings};
use crate::svg::edges::{EdgePath, edge_label_svg, loop_path, marker, marker_trim, route_path};
use crate::svg::shapes::{draw_shape, inset_route, shape_size};
use crate::svg::{
    Accessible, class_names, esc, esc_text, label_svg, num, resolve_style, rounded, series_color,
    svg_document, title_label,
};
use crate::text::Style;
use crate::text::label::{Label, layout_label};
use crate::url::{linked, rel_attr};

use super::db::{AgentflowDb, LabelType};
use super::graph::{AgentGraph, GraphEdge, GraphNode, PALETTE};

const NODE_SEP: f64 = 40.0;
const EDGE_SEP: f64 = 16.0;
const RANK_SEP: f64 = 48.0;
const GROUP_PAD: f64 = 20.0;
const LOOP: f64 = 26.0;
const DOTS: f64 = 14.0;
/// The shape a collapsed flow is drawn with.
const COLLAPSED: &str = "collapsed";

/// A node as it is drawn.
struct NodeView {
    node: GraphNode,
    label: Label,
    style: crate::svg::ResolvedStyle,
    shape: String,
    w: f64,
    h: f64,
    dy: f64,
}

/// The palette slot of each kind of agent.
const KIND_SLOT: &[(&str, usize)] = &[
    ("tool", 0),
    ("task", 1),
    ("decision", 2),
    ("input", 3),
    ("refdoc", 4),
    ("connector", 5),
    ("action", 6),
];

/// A palette colour as an outline with a light wash of the same colour inside. A fill the
/// author set replaces the wash and stays opaque.
fn tint(slot: usize, wash: &str, style: &crate::svg::ResolvedStyle) -> String {
    let color = series_color(slot);
    if crate::svg::sets_fill(&style.shape) {
        format!(" fill=\"{color}\" stroke=\"{color}\"")
    } else {
        format!(" fill=\"{color}\" fill-opacity=\"{wash}\" stroke=\"{color}\"")
    }
}

/// The shape of an agent, as the drawing names it.
fn canonical_shape(shape: &str) -> &'static str {
    match shape {
        "roundedRect" => "rounded",
        "subroutine" | "subprocess" | "subproc" | "framed-rectangle" => "fr-rect",
        "lean-right" | "lean_right" => "lean-r",
        "diamond" => "diam",
        "hexagon" | "hex" => "hex",
        "lin-doc" | "lined-document" => "lin-doc",
        "stadium" => "stadium",
        "circle" => "circle",
        "doublecircle" => "dbl-circ",
        "cylinder" => "cyl",
        "ellipse" => "ellipse",
        "odd" => "odd",
        "trapezoid" => "trap-b",
        "inv_trapezoid" => "trap-t",
        "lean_left" => "lean-l",
        _ => "rect",
    }
}

/// A number from the `agentflow` section of the configuration.
fn number_option(input: &Input<'_>, key: &str, fallback: f64) -> f64 {
    let Some(value) = input.config.get("agentflow") else {
        return fallback;
    };
    let crate::preprocess::Value::Map(map) = value else {
        return fallback;
    };
    match map.get(key) {
        Some(crate::preprocess::Value::Number(n)) if *n > 0.0 => n.min(2000.0),
        _ => fallback,
    }
}

/// Draws an agentflow diagram.
pub(crate) fn render_agentflow(db: &mut AgentflowDb, input: &Input<'_>) -> Result<Output, Error> {
    let graph = super::graph::build_agent_graph(db);
    let options = input.options;
    // A diagram that does not fit the width the host has is drawn tightly instead.
    Ok(crate::diagrams::common::fit::turn_to_fit(
        options,
        |turned| draw(&graph, db, input, false, turned),
    ))
}

/// Draws the diagram of a model. `tight` sets the agents closer to each other.
fn draw(
    graph: &AgentGraph,
    db: &AgentflowDb,
    input: &Input<'_>,
    tight: bool,
    turned: bool,
) -> Output {
    let options = input.options;
    let size = options.font_size.unwrap_or(16.0);
    let edge_size = (size * 0.875).round();
    let measurer = input.measurer;
    let wrap_width = number_option(input, "wrappingWidth", 120.0);
    let min_width = number_option(input, "minNodeWidth", 120.0);
    let pad = options.padding.unwrap_or(8.0);

    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut views: Vec<NodeView> = Vec::new();
    let mut cnodes: Vec<CNode> = Vec::new();
    for (i, node) in graph.nodes.iter().enumerate() {
        index.insert(node.id.as_str(), i);
        let mut style = resolve_style(&node.css_compiled_styles);
        if !node.css_styles.is_empty() {
            let mut all = node.css_compiled_styles.clone();
            all.extend(node.css_styles.iter().cloned());
            style = resolve_style(&all);
        }
        let mut text_style = Style::NONE;
        if style.bold {
            text_style |= Style::BOLD;
        }
        if style.italic {
            text_style |= Style::ITALIC;
        }
        let markdown = node.label_type == LabelType::Markdown;
        let font_size = style.font_size.unwrap_or(size);
        let (shape, w, h, dy, label) = if node.is_group {
            let label = layout_label(
                node.label.as_deref(),
                markdown,
                measurer,
                font_size,
                4000.0,
                text_style,
            );
            ("group".to_string(), 0.0, 0.0, 0.0, label)
        } else if node.shape == "collapsedGroup" {
            let label = layout_label(
                node.label.as_deref(),
                markdown,
                measurer,
                font_size,
                wrap_width,
                text_style,
            );
            let w = label.width.max(min_width) + 2.0 * 18.0;
            let h = label.height + 2.0 * 12.0 + DOTS;
            (COLLAPSED.to_string(), w, h, -DOTS / 2.0, label)
        } else {
            let shape = canonical_shape(&node.shape);
            let label = layout_label(
                node.label.as_deref(),
                markdown,
                measurer,
                font_size,
                wrap_width,
                text_style,
            );
            // A diamond grows twice as fast as its label, so it keeps the label's own width.
            let width = if shape == "diam" {
                label.width
            } else {
                label.width.max(min_width)
            };
            let s = shape_size(shape, width, label.height);
            (shape.to_string(), s.w, s.h, s.dy, label)
        };
        let mut c = cnode(w, h);
        if node.is_group {
            c.is_group = true;
            c.dir = node.dir.as_deref().map(|d| direction(Some(d)));
            c.pad_x = GROUP_PAD;
            c.pad_top = if label.height > 0.0 {
                label.height + 16.0
            } else {
                GROUP_PAD
            };
            c.pad_bottom = GROUP_PAD;
            c.min_w = label.width + 2.0 * GROUP_PAD - 12.0;
        }
        c.seq = views.len() as f64;
        views.push(NodeView {
            node: node.clone(),
            label,
            style,
            shape,
            w,
            h,
            dy,
        });
        cnodes.push(c);
    }
    for (i, node) in graph.nodes.iter().enumerate() {
        cnodes[i].parent = node
            .parent_id
            .as_deref()
            .map_or(-1, |p| index.get(p).map_or(-1, |&at| at as i64));
    }

    crate::layout::compound::inherit_crossed_directions(
        &mut cnodes,
        graph
            .edges
            .iter()
            .filter_map(|e| Some((*index.get(e.start.as_str())?, *index.get(e.end.as_str())?))),
    );

    let root_dir = if turned {
        Dir::TB
    } else {
        direction(db.direction.as_deref())
    };
    let flows_sideways = |cnodes: &[CNode], i: usize| {
        let mut dir = None;
        let mut p = cnodes[i].parent;
        let mut hops = 0;
        while p >= 0 && dir.is_none() && hops < 10_000 {
            dir = cnodes[p as usize].dir;
            p = cnodes[p as usize].parent;
            hops += 1;
        }
        matches!(dir.unwrap_or(root_dir), Dir::LR | Dir::RL)
    };
    let mut cedges: Vec<CEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut drawn: Vec<&GraphEdge> = Vec::new();
    let mut held = Vec::new();
    let parents: Vec<_> = cnodes.iter().map(|c| c.parent).collect();
    let contains = |group: usize, node: usize| {
        let mut at = parents[node];
        for _ in 0..parents.len() {
            if at < 0 {
                break;
            }
            if at as usize == group {
                return true;
            }
            at = parents[at as usize];
        }
        false
    };
    for edge in &graph.edges {
        let (Some(&src), Some(&dst)) =
            (index.get(edge.start.as_str()), index.get(edge.end.as_str()))
        else {
            continue;
        };
        let label = layout_label(
            Some(&edge.label),
            edge.label_type == LabelType::Markdown,
            measurer,
            edge_size,
            wrap_width,
            Style::NONE,
        );
        if contains(src, dst) || contains(dst, src) {
            held.push((edge, label, src, dst));
            continue;
        }
        if src == dst {
            // A self-loop sits beside the node, across the flow, and the node's box grows to
            // hold it.
            if flows_sideways(&cnodes, src) {
                cnodes[src].h += 2.0
                    * (LOOP
                        + if label.height > 0.0 {
                            label.height + 8.0
                        } else {
                            0.0
                        });
            } else {
                cnodes[src].w += 2.0
                    * (LOOP
                        + if label.width > 0.0 {
                            label.width + 12.0
                        } else {
                            0.0
                        });
            }
        }
        cedges.push(cedge(
            src,
            dst,
            edge.minlen.unwrap_or(1).clamp(1, 10) as i32,
            if label.width > 0.0 {
                label.width + 12.0
            } else {
                0.0
            },
            if label.height > 0.0 {
                label.height + 4.0
            } else {
                0.0
            },
        ));
        drawn.push(edge);
        edge_labels.push(label);
    }

    let layout_options = LayeredOptions {
        node_sep: number_option(input, "nodeSpacing", NODE_SEP) * if tight { 0.5 } else { 1.0 },
        edge_sep: EDGE_SEP,
        rank_sep: number_option(input, "rankSpacing", RANK_SEP) * if tight { 0.75 } else { 1.0 },
        port_sep: 20.0,
        ..LayeredOptions::default()
    };
    let layout = compound_layout(&mut cnodes, &mut cedges, root_dir, &layout_options);

    let title = title_label(
        db.title.as_deref(),
        measurer,
        size,
        if tight { title_room(options) } else { 4000.0 },
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let inner = layout.width.max(title.width);
    let ox = pad + (inner - layout.width) / 2.0;
    let oy = pad + title_height;
    shift_layout(&mut cnodes, &mut cedges, ox, oy);
    let width = (inner + 2.0 * pad).ceil();
    let height = (layout.height + title_height + 2.0 * pad).ceil();
    let mut links: Vec<Link> = Vec::new();

    let mut held_paths = HashMap::new();
    let careful = held.len() * cnodes.len() < 2_000_000;
    for (edge, label, src, dst) in held {
        let out = contains(dst, src);
        let outer = if out { dst } else { src };
        let i = if out { src } else { dst };
        let g = &cnodes[outer];
        let n = &cnodes[i];
        let mut dir = g.dir;
        let mut p = g.parent;
        for _ in 0..cnodes.len() {
            if dir.is_some() || p < 0 {
                break;
            }
            dir = cnodes[p as usize].dir;
            p = cnodes[p as usize].parent;
        }
        let end = match dir.unwrap_or(root_dir) {
            Dir::TB => 2,
            Dir::BT => 0,
            Dir::LR => 1,
            Dir::RL => 3,
        };
        let first = if out { end } else { (end + 2) & 3 };
        let toward = |side| {
            let upright = side & 1 == 0;
            let sign = if side == 0 || side == 3 { -1.0 } else { 1.0 };
            let nx = if upright { n.x } else { n.x + sign * n.w / 2.0 };
            let ny = if upright { n.y + sign * n.h / 2.0 } else { n.y };
            let bx = if upright { n.x } else { g.x + sign * g.w / 2.0 };
            let by = if upright { g.y + sign * g.h / 2.0 } else { n.y };
            let axis = if upright { 0.0 } else { 1.0 };
            if out {
                vec![nx, ny, axis, bx, by, axis]
            } else {
                vec![bx, by, axis, nx, ny, axis]
            }
        };
        let blocked = |r: &[f64]| {
            cnodes.iter().enumerate().any(|(k, c)| {
                k != i
                    && !c.is_group
                    && c.x + c.w / 2.0 > r[0].min(r[3]) - 1.0
                    && c.x - c.w / 2.0 < r[0].max(r[3]) + 1.0
                    && c.y + c.h / 2.0 > r[1].min(r[4]) - 1.0
                    && c.y - c.h / 2.0 < r[1].max(r[4]) + 1.0
            })
        };
        let mut route = toward(first);
        if careful && blocked(&route) {
            for side in [(first + 1) & 3, (first + 3) & 3, (first + 2) & 3] {
                let r = toward(side);
                if !blocked(&r) {
                    route = r;
                    break;
                }
            }
        }
        let mut ce = cedge(src, dst, 1, 0.0, 0.0);
        ce.label_x = (route[0] + route[3]) / 2.0;
        ce.label_y = (route[1] + route[4]) / 2.0;
        ce.route = route.clone();
        inset_route(
            &mut route,
            if out { 0 } else { 3 },
            &views[i].shape,
            views[i].w,
            views[i].h,
            n,
        );
        held_paths.insert(
            cedges.len(),
            route_path(
                &route,
                Some("linear"),
                0.0,
                marker_trim(&edge.arrow_end),
                None,
                None,
            ),
        );
        cedges.push(ce);
        drawn.push(edge);
        edge_labels.push(label);
    }

    let mut crossings: Crossings = HashMap::new();
    if cnodes.iter().any(|c| c.is_group) {
        for e in &cedges {
            mark_crossings(&mut crossings, &e.route);
        }
    }

    let mut fills = Vec::new();
    let mut clusters = String::new();
    let mut nodes_out = String::new();
    for (i, view) in views.iter().enumerate() {
        let c = &cnodes[i];
        let node = &view.node;
        let (x, y) = (c.x, c.y);
        let own_classes = node
            .css_classes
            .strip_prefix("default ")
            .or_else(|| node.css_classes.strip_prefix("default"))
            .unwrap_or(&node.css_classes);
        let classes = class_names(own_classes);
        let id = esc_text(&node.id);
        if c.is_group {
            if crate::svg::ink::paints(&view.style.shape) {
                fills.push(crate::svg::fills::Fill {
                    bounds: [x - c.w / 2.0, y - c.h / 2.0, x + c.w / 2.0, y + c.h / 2.0],
                    shape: view.style.shape.clone(),
                });
            }
            let title_x = cluster_title_x(c, view.label.width, &crossings, GROUP_PAD);
            let title_y = y - c.h / 2.0 + 8.0 + view.label.height / 2.0;
            let slot = node.color_index.unwrap_or(PALETTE);
            let kind = db.subgraph(&node.id).map_or("flow", |sg| sg.kind.as_str());
            let _ = write!(
                clusters,
                "<g class=\"pele-cluster pele-{kind}{classes}\" data-id=\"{id}\"><rect x=\"{}\" y=\"{}\" \
                 width=\"{}\" height=\"{}\"{}/>{}</g>",
                num(x - c.w / 2.0),
                num(y - c.h / 2.0),
                num(c.w),
                num(c.h),
                rounded(&(tint(slot, "0.06", &view.style) + &view.style.shape)),
                label_svg(
                    &view.label,
                    title_x,
                    title_y,
                    &format!(
                        " class=\"pele-cluster-label\"{}{}",
                        crate::svg::ink::ink_fill(&view.style, Some("var(--_m)")),
                        view.style.text
                    )
                )
            );
            continue;
        }

        let (w, h) = (view.w, view.h);
        let shape = if view.shape == COLLAPSED {
            let color = series_color(node.color_index.unwrap_or(PALETTE));
            let line = h / 2.0 - DOTS - 2.0;
            let dots = line + (DOTS + 2.0) / 2.0;
            format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>\
                 <path d=\"M{},{}H{}\" fill=\"none\" stroke=\"{color}\" stroke-dasharray=\"3 3\"{}/>\
                 <path class=\"pele-collapsed-mark\" d=\"{}{}{}\" fill=\"{color}\" stroke=\"none\"/>",
                num(-w / 2.0),
                num(-h / 2.0),
                num(w),
                num(h),
                rounded(
                    &(tint(node.color_index.unwrap_or(PALETTE), "0.06", &view.style)
                        + &view.style.shape)
                ),
                num(-w / 2.0 + 10.0),
                num(line),
                num(w / 2.0 - 10.0),
                view.style.line,
                dot(-9.0, dots),
                dot(0.0, dots),
                dot(9.0, dots)
            )
        } else {
            let slot = KIND_SLOT
                .iter()
                .find(|(kind, _)| Some(*kind) == node.kind.as_deref())
                .map_or(1, |(_, slot)| *slot);
            draw_shape(
                &view.shape,
                w,
                h,
                &(tint(slot, "0.14", &view.style) + &view.style.shape),
                &format!(" stroke=\"{}\"{}", series_color(slot), view.style.line),
            )
        };

        let ink = if view.shape == COLLAPSED || crate::svg::shapes::shape_filled(&view.shape) {
            crate::svg::ink::ink_fill(&view.style, None)
        } else {
            String::new()
        };
        let mut text = label_svg(
            &view.label,
            0.0,
            view.dy,
            &format!(" class=\"pele-label\"{ink}{}", view.style.text),
        );
        let internal = format!("{classes} ").contains(" internal-link ");
        if internal && options.links {
            if let Some(label) = node.label.as_deref().filter(|l| !l.is_empty()) {
                // The label is a note name, but it still ends up in an href, so it gets the
                // same check as a URL.
                let name = crate::text::entities::decode_entities(label);
                let safe = crate::url::internal_link(&name, options);
                let target = esc(&name);
                let href = if safe {
                    format!(" href=\"{target}\"{}", rel_attr(options))
                } else {
                    String::new()
                };
                text =
                    format!("<a class=\"internal-link\"{href} data-href=\"{target}\">{text}</a>");
                links.push(Link {
                    id: node.id.clone(),
                    href: name,
                    internal: true,
                });
            }
        }
        let mut body = shape + &text;
        if let Some(tooltip) = node.tooltip.as_deref().filter(|t| !t.is_empty()) {
            body = format!("<title>{}</title>{body}", esc_text(tooltip));
        }
        if let Some(link) = &node.link {
            let attrs = node
                .link_target
                .as_deref()
                .map_or(String::new(), |t| format!(" target=\"{}\"", esc(t)));
            body = linked(&body, link, &node.id, &mut links, options, &attrs);
        }
        let kind = node
            .kind
            .as_deref()
            .map_or_else(String::new, |k| format!(" pele-kind-{k}"));
        let _ = write!(
            nodes_out,
            "<g class=\"pele-node pele-shape-{}{kind}{classes}\" data-id=\"{id}\" \
             transform=\"translate({},{})\">{body}</g>",
            view.shape,
            num(x),
            num(y)
        );
    }

    let mut casings = String::new();
    let mut edges_out = String::new();
    let mut labels_out = String::new();
    let mut loop_seen: HashMap<usize, usize> = HashMap::new();
    for i in 0..cedges.len() {
        let edge = drawn[i];
        let label = &edge_labels[i];
        let style = (!edge.style.is_empty() || !edge.css_compiled_styles.is_empty()).then(|| {
            let mut all = edge.css_compiled_styles.clone();
            all.extend(edge.style.iter().cloned());
            resolve_style(&all)
        });
        let (mut label_x, mut label_y) = (cedges[i].label_x, cedges[i].label_y);
        let invisible = edge.thickness.as_deref() == Some("invisible");
        if !invisible {
            let ce = &cedges[i];
            let path: EdgePath = if let Some(path) = held_paths.remove(&i) {
                path
            } else if ce.src == ce.dst {
                let view = &views[ce.src];
                let k = loop_seen.entry(ce.src).or_insert(0);
                let (path, lx, ly) = loop_path(
                    cnodes[ce.src].x,
                    cnodes[ce.src].y,
                    view.w,
                    view.h,
                    *k,
                    flows_sideways(&cnodes, ce.src),
                    LOOP,
                    marker_trim(&edge.arrow_end),
                    (ce.label_w, ce.label_h),
                );
                *k += 1;
                label_x = lx;
                label_y = ly;
                path
            } else {
                let mut route = ce.route.clone();
                let last = route.len() - 3;
                let (sv, dv) = (&views[ce.src], &views[ce.dst]);
                inset_route(&mut route, 0, &sv.shape, sv.w, sv.h, &cnodes[ce.src]);
                inset_route(&mut route, last, &dv.shape, dv.w, dv.h, &cnodes[ce.dst]);
                route_path(
                    &route,
                    edge.curve.as_deref(),
                    0.0,
                    marker_trim(&edge.arrow_end),
                    None,
                    None,
                )
            };
            let line = style.as_ref().map_or("", |s| s.line.as_str());
            let over = crate::svg::fills::over_fills(&path.d, &[path.ex, path.ey], &fills, line);
            if over.as_deref() == Some("") {
                casings += &crate::svg::fills::edge_casing(
                    &path,
                    &edge.arrow_start,
                    &edge.arrow_end,
                    line,
                    edge.thickness.as_deref() == Some("thick"),
                );
            }
            let color = over
                .as_ref()
                .filter(|s| !s.is_empty())
                .cloned()
                .unwrap_or_else(|| {
                    style
                        .as_ref()
                        .and_then(|s| s.stroke.as_deref())
                        .map_or("var(--_l)".to_string(), esc)
                });
            let over_attr = over
                .filter(|s| !s.is_empty())
                .map_or_else(String::new, |s| format!(" stroke=\"{s}\""));
            let attrs = match edge.thickness.as_deref() {
                Some("thick") => " stroke-width=\"2.5\"",
                Some("dotted") => " stroke-dasharray=\"3 4\"",
                _ => "",
            };
            let own = edge
                .classes
                .replace("edge-thickness-normal", "")
                .replace("edge-pattern-solid", "")
                .replace("flowchart-link", "");
            let kind = if edge.thickness.as_deref() == Some("dotted") {
                "reference"
            } else if edge.arrow_end == "arrow_cross" {
                "failure"
            } else {
                "sequence"
            };
            let _ = write!(
                edges_out,
                "<g class=\"pele-edge pele-edge-{kind}{}\" data-id=\"{}\"{over_attr}{}><path d=\"{}\"{attrs}/>{}{}</g>",
                class_names(&own),
                esc_text(&edge.id),
                style.as_ref().map_or("", |s| s.line.as_str()),
                path.d,
                marker(
                    &edge.arrow_start,
                    path.sx,
                    path.sy,
                    path.sdx,
                    path.sdy,
                    &color
                ),
                marker(
                    &edge.arrow_end,
                    path.ex,
                    path.ey,
                    path.edx,
                    path.edy,
                    &color
                )
            );
        }
        if label.width > 0.0 {
            labels_out += &edge_label_svg(
                &edge.id,
                label,
                label_x,
                label_y,
                style.as_ref().map_or("", |s| s.text.as_str()),
            );
        }
    }

    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    if !clusters.is_empty() {
        let _ = write!(content, "<g class=\"pele-clusters\">{clusters}</g>");
    }
    content += &crate::svg::fills::casings_svg(&casings);
    if !edges_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges_out}</g>"
        );
    }
    if !labels_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-edge-labels\" font-size=\"{}\">{labels_out}</g>",
            num(edge_size)
        );
    }
    let _ = write!(content, "<g class=\"pele-nodes\">{nodes_out}</g>");
    let acc = Accessible {
        title: db.acc_title.clone(),
        descr: db.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            DiagramKind::Agentflow,
            width,
            height,
            size,
            options,
            &acc,
            &content,
        ),
        width,
        height,
        links,
    }
}

/// A dot of the mark of a collapsed flow.
fn dot(x: f64, y: f64) -> String {
    format!(
        "M{},{}a1.75,1.75 0 1 0 3.5,0a1.75,1.75 0 1 0 -3.5,0Z",
        num(x - 1.75),
        num(y)
    )
}
