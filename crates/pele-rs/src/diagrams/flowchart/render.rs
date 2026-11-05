//! Flowchart layout and SVG. Ported from pele's `diagrams/flowchart/render.ts`.

use std::collections::HashMap;
use std::fmt::Write;

use super::db::{FlowDb, LabelType};
use super::graph::{GraphNode, build_flow_graph};
use super::shapes::canonical_shape;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{runs_across, tighten, title_room, turn_to_fit};
use crate::layout::compound::{
    CEdge, CNode, Dir, cedge, cnode, compound_layout, direction, shift_layout,
};
use crate::layout::layered::LayeredOptions;
use crate::preprocess::{Config, Value};
use crate::svg::cluster::{
    Crossings, cluster_title_x, mark_crossings, struck_title, title_crossed,
};
use crate::svg::edges::{
    EdgePath, SideReach, edge_label_svg, loop_path, marker, marker_trim, route_path,
};
use crate::svg::shapes::{
    draw_shape, end_side, inset_route, shape_has_label, shape_size, shape_span,
};
use crate::svg::{
    Accessible, ResolvedStyle, class_names, esc, esc_text, icon_svg, label_svg, num, resolve_style,
    rounded, svg_document, title_label,
};
use crate::text::entities::decode_entities;
use crate::text::label::{Label, layout_label};
use crate::text::{Style, icon_markup};
use crate::url::{image_url, linked, rel_attr, safe_url};
use crate::util::round;
use crate::{DiagramKind, Link};

const NODE_SEP: f64 = 40.0;
const EDGE_SEP: f64 = 16.0;
const RANK_SEP: f64 = 48.0;
const GROUP_PAD: f64 = 20.0;
const LOOP: f64 = 26.0;
const SHAPE_ATTRS: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";
const LINE_ATTRS: &str = " stroke=\"var(--_b)\"";

fn number_option(section: &Config, key: &str, fallback: f64) -> f64 {
    match section.get(key) {
        Some(&Value::Number(v)) if v > 0.0 => v.min(2000.0),
        _ => fallback,
    }
}

fn asset_size(value: Option<f64>, fallback: f64) -> f64 {
    value
        .filter(|&v| v > 0.0 && v <= 4000.0)
        .unwrap_or(fallback)
}

struct NodeView<'a> {
    node: &'a GraphNode,
    style: ResolvedStyle,
    label: Label,
    shape: &'static str,
    w: f64,
    h: f64,
    dy: f64,
}

pub(crate) fn render_flowchart(db: &FlowDb, input: &Input<'_>) -> Output {
    let options = input.options;
    let fitted = |down: bool| tighten(options, |tight| draw(db, input, down, tight));
    if runs_across(db.direction.as_deref()) {
        turn_to_fit(options, fitted)
    } else {
        fitted(false)
    }
}

/// `turned` draws a flowchart that runs across as one that runs down. `tight` sets nodes closer
/// together.
fn draw(db: &FlowDb, input: &Input<'_>, turned: bool, tight: bool) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let empty = Config::new();
    let flow = match input.config.get("flowchart") {
        Some(Value::Map(m)) => m,
        _ => &empty,
    };
    let curve = match flow.get("curve") {
        Some(Value::String(s)) => Some(s.as_str()),
        _ => None,
    };
    let group_pad = if tight { 12.0 } else { GROUP_PAD };
    let graph = build_flow_graph(db, curve);
    let size = options.font_size.unwrap_or(16.0);
    let edge_size = round(size * 0.875);
    let wrap_width = number_option(flow, "wrappingWidth", 200.0);
    let pad = options.padding.unwrap_or(8.0);

    let index: HashMap<&str, usize> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let mut views: Vec<NodeView<'_>> = Vec::new();
    let mut cnodes: Vec<CNode> = Vec::new();
    for node in &graph.nodes {
        let style = if node.css_styles.is_empty() {
            resolve_style(&node.css_compiled_styles)
        } else {
            let mut all = node.css_compiled_styles.clone();
            all.extend(node.css_styles.iter().cloned());
            resolve_style(&all)
        };
        let mut text_style = Style::NONE;
        if style.bold {
            text_style |= Style::BOLD;
        }
        if style.italic {
            text_style |= Style::ITALIC;
        }
        let markdown = node.label_type == LabelType::Markdown;
        let font_size = style.font_size.unwrap_or(size);
        let (view, mut c) = if node.is_group {
            let label = layout_label(
                node.label.as_deref(),
                markdown,
                measurer,
                font_size,
                4000.0,
                text_style,
            );
            let mut c = cnode(0.0, 0.0);
            c.is_group = true;
            c.dir = node.dir.as_deref().map(|d| direction(Some(d)));
            c.pad_x = group_pad;
            c.pad_top = if label.height > 0.0 {
                label.height + 16.0
            } else {
                GROUP_PAD
            };
            c.pad_bottom = group_pad;
            c.min_w = label.width + 2.0 * group_pad - 12.0;
            (
                NodeView {
                    node,
                    style,
                    label,
                    shape: "group",
                    w: 0.0,
                    h: 0.0,
                    dy: 0.0,
                },
                c,
            )
        } else {
            let shape = canonical_shape(Some(&node.shape));
            let asset = node.img.is_some() || node.icon.is_some();
            let text = if asset || shape_has_label(shape) {
                node.label.as_deref()
            } else {
                None
            };
            let label = layout_label(text, markdown, measurer, font_size, wrap_width, text_style);
            let (w, h, dy) = if asset {
                let bx = if node.img.is_some() { 80.0 } else { 48.0 };
                let aw = asset_size(node.asset_width, bx);
                let ah = asset_size(node.asset_height, bx);
                let w = aw.max(label.width + 8.0);
                let h = ah
                    + if label.height > 0.0 {
                        label.height + 6.0
                    } else {
                        0.0
                    };
                let dy = if label.height == 0.0 {
                    0.0
                } else if node.pos.as_deref() == Some("t") {
                    -ah / 2.0
                } else {
                    ah / 2.0
                };
                (w, h, dy)
            } else {
                let s = shape_size(shape, label.width, label.height);
                (s.w, s.h, s.dy)
            };
            (
                NodeView {
                    node,
                    style,
                    label,
                    shape,
                    w,
                    h,
                    dy,
                },
                cnode(w, h),
            )
        };
        c.seq = views.len() as f64;
        views.push(view);
        cnodes.push(c);
    }
    for (i, node) in graph.nodes.iter().enumerate() {
        cnodes[i].parent = node
            .parent_id
            .as_deref()
            .map_or(-1, |p| index.get(p).map_or(-1, |&i| i as i64));
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
        while p >= 0 && dir.is_none() && hops < 10000 {
            dir = cnodes[p as usize].dir;
            p = cnodes[p as usize].parent;
            hops += 1;
        }
        matches!(dir.unwrap_or(root_dir), Dir::LR | Dir::RL)
    };
    // The edges on one side of a node spread along it; a picture or an icon keeps them at its
    // middle.
    for i in 0..views.len() {
        let view = &views[i];
        if view.node.is_group {
            cnodes[i].span = f64::INFINITY;
        } else if view.node.img.is_none() && view.node.icon.is_none() {
            cnodes[i].span = shape_span(view.shape, view.w, view.h, flows_sideways(&cnodes, i));
        }
    }

    let mut cedges: Vec<CEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut drawn = Vec::new();
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
            edge.minlen.unwrap_or(1),
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
        node_sep: number_option(flow, "nodeSpacing", NODE_SEP) * if tight { 0.5 } else { 1.0 },
        edge_sep: EDGE_SEP,
        rank_sep: number_option(flow, "rankSpacing", RANK_SEP) * if tight { 0.75 } else { 1.0 },
        port_sep: 20.0,
        tracks: curve.is_some_and(|c| c.starts_with("step")),
        straight: curve == Some("linear"),
        head_room: marker_trim("arrow_point"),
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

    let mut crossings: Crossings = HashMap::new();
    if cnodes.iter().any(|c| c.is_group) {
        for e in &cedges {
            mark_crossings(&mut crossings, &e.route);
        }
    }

    let mut fills = Vec::new();
    let mut clusters = String::new();
    let mut struck_titles = String::new();
    let mut nodes_out = String::new();
    for (i, view) in views.iter().enumerate() {
        let c = &cnodes[i];
        let node = view.node;
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
            let title_x = cluster_title_x(c, view.label.width, &crossings, group_pad);
            let title_y = y - c.h / 2.0 + 8.0 + view.label.height / 2.0;
            let struck = title_crossed(c, view.label.width, title_x, &crossings);
            let ink = if struck {
                " fill=\"var(--_m)\"".to_string()
            } else {
                crate::svg::ink::ink_fill(&view.style, Some("var(--_m)"))
            };
            let title_attrs = format!(" class=\"pele-cluster-label\"{ink}{}", view.style.text);
            if struck {
                let mut depth = 1;
                let mut p = c.parent;
                while p >= 0 && depth < 8 {
                    depth += 1;
                    p = cnodes[p as usize].parent;
                }
                struck_titles +=
                    &struck_title(&view.label, title_x, title_y, &title_attrs, &id, depth);
            }
            let _ = write!(
                clusters,
                "<g class=\"pele-cluster{classes}\" data-id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" \
                 fill=\"var(--_a)\"{} stroke=\"var(--_b)\"{}/>{}</g>",
                num(x - c.w / 2.0),
                num(y - c.h / 2.0),
                num(c.w),
                num(c.h),
                if crate::svg::sets_fill(&view.style.shape) {
                    ""
                } else {
                    " fill-opacity=\"0.5\""
                },
                rounded(&view.style.shape),
                if struck {
                    String::new()
                } else {
                    label_svg(&view.label, title_x, title_y, &title_attrs)
                }
            );
            continue;
        }

        let (w, h) = (view.w, view.h);
        let inner = if let Some(img) = &node.img {
            let aw = asset_size(node.asset_width, 80.0);
            let ah = asset_size(node.asset_height, 80.0);
            let top = if view.label.height > 0.0 {
                if node.pos.as_deref() == Some("t") {
                    h / 2.0 - ah
                } else {
                    -h / 2.0
                }
            } else {
                -ah / 2.0
            };
            match image_url(&safe_url(img), options) {
                None => String::new(),
                Some(src) => format!(
                    "<image href=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"{}\"/>",
                    esc(&src),
                    num(-aw / 2.0),
                    num(top),
                    num(aw),
                    num(ah),
                    if node.constraint.as_deref() == Some("on") {
                        "xMidYMid meet"
                    } else {
                        "none"
                    }
                ),
            }
        } else if let Some(icon) = &node.icon {
            let bx = asset_size(node.asset_height, 48.0);
            let top = if view.label.height > 0.0 {
                if node.pos.as_deref() == Some("t") {
                    h / 2.0 - bx
                } else {
                    -h / 2.0
                }
            } else {
                -bx / 2.0
            };
            let frame = match node.shape.as_str() {
                "icon" => String::new(),
                "iconCircle" => format!(
                    "<circle cy=\"{}\" r=\"{}\"{SHAPE_ATTRS}{}/>",
                    num(top + bx / 2.0),
                    num(bx / 2.0),
                    view.style.shape
                ),
                shape => format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{SHAPE_ATTRS}{}/>",
                    num(-bx / 2.0),
                    num(top),
                    num(bx),
                    num(bx),
                    if shape == "iconRounded" { 8 } else { 0 },
                    view.style.shape
                ),
            };
            let glyph = bx * 0.6;
            let (x, y) = (-glyph / 2.0, top + (bx - glyph) / 2.0);
            format!(
                "{frame}{}",
                icon_svg(
                    icon,
                    x,
                    y,
                    glyph,
                    &icon_markup(measurer, icon).unwrap_or_default()
                )
            )
        } else {
            draw_shape(
                view.shape,
                w,
                h,
                &format!("{SHAPE_ATTRS}{}", view.style.shape),
                &format!("{LINE_ATTRS}{}", view.style.line),
            )
        };

        let ink = if node.img.is_none()
            && node.icon.is_none()
            && crate::svg::shapes::shape_filled(view.shape)
        {
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
                // The label is a note name, but it still ends up in an href, so it gets the same
                // check as a URL.
                let name = decode_entities(label);
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
        let mut body = inner + &text;
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
        let _ = write!(
            nodes_out,
            "<g class=\"pele-node pele-shape-{}{classes}\" data-id=\"{id}\" transform=\"translate({},{})\">{body}</g>",
            view.shape,
            num(x),
            num(y)
        );
    }

    let mut reach = SideReach::default();
    for (i, ce) in cedges.iter().enumerate() {
        if ce.src == ce.dst
            || drawn[i].thickness.as_deref() == Some("invisible")
            || ce.route.len() < 6
        {
            continue;
        }
        reach.add(
            ce.src,
            end_side(&ce.route, 0, &cnodes[ce.src]),
            &drawn[i].arrow_start,
        );
        reach.add(
            ce.dst,
            end_side(&ce.route, ce.route.len() - 3, &cnodes[ce.dst]),
            &drawn[i].arrow_end,
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
            let path: EdgePath = if ce.src == ce.dst {
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
                let leaves = reach.of(ce.src, end_side(&route, 0, &cnodes[ce.src]));
                let arrives = reach.of(ce.dst, end_side(&route, last, &cnodes[ce.dst]));
                let (sv, dv) = (&views[ce.src], &views[ce.dst]);
                inset_route(&mut route, 0, sv.shape, sv.w, sv.h, &cnodes[ce.src]);
                inset_route(&mut route, last, dv.shape, dv.w, dv.h, &cnodes[ce.dst]);
                route_path(
                    &route,
                    edge.curve.as_deref(),
                    marker_trim(&edge.arrow_start),
                    marker_trim(&edge.arrow_end),
                    Some(leaves),
                    Some(arrives),
                )
            };
            let line = style.as_ref().map_or("", |s| s.line.as_str());
            let over = crate::svg::fills::over_fills(
                &path.d,
                &[path.sx, path.sy, path.ex, path.ey],
                &fills,
                line,
            );
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
            let _ = write!(
                edges_out,
                "<g class=\"pele-edge{}\" data-id=\"{}\"{over_attr}{}><path d=\"{}\"{attrs}/>{}{}</g>",
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
    if !struck_titles.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-cluster-titles\">{struck_titles}</g>"
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
            DiagramKind::Flowchart,
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
