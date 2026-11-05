//! The drawing of a use case diagram: the actors, the use cases, the boundaries and the notes,
//! and the relationships between them. The layout is pele's own, as the reference of the project
//! is.

use std::collections::HashMap;

use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{runs_across, turn_to_fit};
use crate::layout::compound::{
    CEdge, CNode, cedge, cnode, compound_layout, direction, shift_layout,
};
use crate::layout::layered::LayeredOptions;
use crate::preprocess::{Config, Value};
use crate::svg::cluster::{Crossings, mark_crossings};
use crate::svg::edges::{EdgePath, edge_label_svg, marker, marker_trim, route_path};
use crate::svg::{Accessible, esc, esc_text, icon_svg, label_svg, num, svg_document, title_label};
use crate::svg::{ResolvedStyle, class_names, resolve_style, rounded};
use crate::text::label::{Label, label_line, layout_label, line_middle};
use crate::text::{Style, TextMeasurer, icon_markup};

use super::db::{ActorType, BoundaryType, UsecaseModel};
use super::graph::{GraphShape, Pattern, UsecaseGraphEdge, UsecaseGraphNode, build_usecase_graph};

const NODE_SEP: f64 = 32.0;
const EDGE_SEP: f64 = 16.0;
const RANK_SEP: f64 = 72.0;
const GROUP_PAD: f64 = 20.0;
const LOOP: f64 = 26.0;
const FIGURE_W: f64 = 40.0;
const FIGURE_H: f64 = 60.0;
const FIGURE_GAP: f64 = 6.0;
const TAB_PAD: f64 = 6.0;
const CELL_PAD: f64 = 10.0;
const ROW_PAD: f64 = 8.0;
const FOLD: f64 = 9.0;
const EXTENSION: f64 = 12.0;
const PORT_GAP: f64 = 14.0;
const SHAPE_ATTRS: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";

/// What a node is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Actor,
    Ellipse,
    Box,
    Json,
    Boundary,
}

/// What is drawn of one node.
struct View<'a> {
    node: &'a UsecaseGraphNode,
    kind: Kind,
    style: ResolvedStyle,
    label: Label,
    stereotype: Label,
    /// The size of what is drawn; the layout box is larger when the node carries a
    /// self-relationship.
    w: f64,
    h: f64,
    rows: Vec<(Label, Label)>,
    key_w: f64,
    head_h: f64,
    loops: usize,
}

fn number_option(config: &Config, key: &str, fallback: f64) -> f64 {
    match config.get("usecase") {
        Some(Value::Map(section)) => match section.get(key) {
            Some(&Value::Number(value)) if value > 0.0 => value.min(2000.0),
            _ => fallback,
        },
        _ => fallback,
    }
}

/// A plain label is shown as written: no tags, no markdown, no `\n` breaks. Its markup
/// characters are handed to the label parser as character references, which it turns back into
/// characters.
fn literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' | '<' | '>' | '\\' => {
                out.push_str(&format!("&#{};", c as u32));
            }
            _ => out.push(c),
        }
    }
    out
}

/// Lays out a label of the diagram.
fn text(
    raw: Option<&str>,
    markdown: bool,
    measurer: &dyn TextMeasurer,
    size: f64,
    wrap: f64,
    base: Style,
) -> Label {
    let shown = match raw {
        Some(raw) if !markdown => Some(literal(raw)),
        other => other.map(str::to_string),
    };
    layout_label(shown.as_deref(), markdown, measurer, size, wrap, base)
}

/// The point of a node's outline on one of its four sides (0 top, 1 right, 2 bottom, 3 left),
/// `along` away from the middle of that side. Lines meet an actor at its figure, not at its
/// label.
fn anchor(view: &View<'_>, c: &CNode, side: usize, along: f64) -> (f64, f64) {
    let sign = if side == 1 || side == 2 { 1.0 } else { -1.0 };
    let upright = side == 1 || side == 3;
    let mut reach = if upright { view.w } else { view.h } / 2.0;
    let mut middle = 0.0;
    if view.kind == Kind::Actor {
        if upright {
            reach = FIGURE_W / 2.0 + 3.0;
            middle = (FIGURE_H - view.h) / 2.0;
        } else {
            reach += 2.0;
        }
    } else if view.kind == Kind::Ellipse {
        let span = if upright { view.h } else { view.w } / 2.0;
        reach *= (1.0 - (along / span).powi(2)).max(0.0).sqrt();
    }
    if upright {
        (c.x + sign * reach, c.y + middle + along)
    } else {
        (c.x + along, c.y + sign * reach)
    }
}

/// How much of a side edges may spread over.
fn side_length(view: &View<'_>, side: usize) -> f64 {
    let upright = side == 1 || side == 3;
    if view.kind == Kind::Actor {
        return if upright {
            FIGURE_H * 0.5
        } else {
            FIGURE_W * 0.6
        };
    }
    let length = if upright { view.h } else { view.w };
    if view.kind == Kind::Ellipse {
        length * 0.6
    } else {
        length - 16.0
    }
}

/// Where the layout brought an edge to a node.
struct End {
    route: usize,
    at: usize,
    toward: f64,
}

/// The layout brings every edge that ends on one side of a node to the middle of that side.
/// This spaces them along the side in the order they fan out, and puts each on the node's
/// outline.
fn attach_ends(cedges: &mut [CEdge], cnodes: &[CNode], views: &[View<'_>]) {
    let mut sides: HashMap<usize, Vec<End>> = HashMap::new();
    for (edge_index, edge) in cedges.iter().enumerate() {
        // The route of an edge holds its two ends and the points between them, so an edge
        // with fewer points than that has no end to put on the outline of a node.
        if edge.src == edge.dst || edge.route.len() < 6 {
            continue;
        }
        let last = edge.route.len() - 3;
        for (index, at, next) in [(edge.src, 0usize, 3usize), (edge.dst, last, last - 3)] {
            let c = &cnodes[index];
            if c.is_group {
                continue;
            }
            let dx = edge.route[at] - c.x;
            let dy = edge.route[at + 1] - c.y;
            let side = if (dy.abs() - c.h / 2.0).abs() < 0.5 {
                if dy < 0.0 { 0 } else { 2 }
            } else if (dx.abs() - c.w / 2.0).abs() < 0.5 {
                if dx < 0.0 { 3 } else { 1 }
            } else {
                continue;
            };
            let toward = edge.route[next + usize::from(side & 1 == 1)];
            sides.entry(index * 4 + side).or_default().push(End {
                route: edge_index,
                at,
                toward,
            });
        }
    }
    let mut keys: Vec<usize> = sides.keys().copied().collect();
    keys.sort_unstable();
    for key in keys {
        let index = key >> 2;
        let side = key & 3;
        let view = &views[index];
        let ends = sides.get_mut(&key).expect("the ends are there");
        let step = if ends.len() > 1 {
            PORT_GAP.min(side_length(view, side) / (ends.len() - 1) as f64)
        } else {
            0.0
        };
        ends.sort_by(|a, b| {
            a.toward
                .partial_cmp(&b.toward)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (i, end) in ends.iter().enumerate() {
            let (x, y) = anchor(
                view,
                &cnodes[index],
                side,
                (i as f64 - (ends.len() - 1) as f64 / 2.0) * step,
            );
            cedges[end.route].route[end.at] = x;
            cedges[end.route].route[end.at + 1] = y;
        }
    }
}

/// The hollow triangle of a generalization, with its tip at (x, y).
fn extension(x: f64, y: f64, dx: f64, dy: f64, color: &str) -> String {
    let bx = x - dx * EXTENSION;
    let by = y - dy * EXTENSION;
    let half = EXTENSION * 0.5;
    format!(
        "<path class=\"pele-marker\" d=\"M{},{}L{},{}L{},{}Z\" fill=\"var(--_bg)\" stroke=\"{color}\" stroke-linejoin=\"round\"/>",
        num(x),
        num(y),
        num(bx - dy * half),
        num(by + dx * half),
        num(bx + dy * half),
        num(by - dx * half)
    )
}

fn trim(kind: &str) -> f64 {
    if kind == "extension" {
        EXTENSION
    } else {
        marker_trim(kind)
    }
}

fn mark(kind: &str, x: f64, y: f64, dx: f64, dy: f64, color: &str) -> String {
    if kind == "extension" {
        extension(x, y, dx, dy, color)
    } else {
        marker(kind, x, y, dx, dy, color)
    }
}

/// The figure of an actor.
fn figure(view: &View<'_>, measurer: &dyn TextMeasurer) -> String {
    let node = view.node;
    let shape = &view.style.shape;
    let line = format!(" fill=\"none\" stroke=\"var(--_b)\"{}", view.style.line);
    let mut head = -21.0;
    let mut radius = 9.0;
    let mut out;
    match node.actor_type {
        Some(ActorType::Hollow) => {
            radius = 8.0;
            out = format!(
                "<circle cy=\"-21\" r=\"8\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{shape}/>\
                 <path d=\"M-20,-9H20V0H5.5L20,15.5L12,25.5L0,12L-12,25.5L-20,15.5L-5.5,0H-20Z\" \
                 fill=\"var(--_bg)\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"{shape}/>"
            );
        }
        Some(ActorType::Awesome) => {
            out = format!(
                "<path d=\"M0,-30A11,11 0 1 1 0,-8A11,11 0 1 1 0,-30ZM-20,25C-20,9 -12,-3 0,-3C12,-3 20,9 20,25C20,28 18,30 15,30H-15C-18,30 -20,28 -20,25Z\" \
                 fill=\"var(--_b)\"{shape}/>"
            );
        }
        Some(ActorType::Icon) => {
            head = -14.0;
            radius = 12.0;
            // The drawing is the host's; a host with no such icon leaves the frame empty.
            let name = node.icon.as_deref().unwrap_or("");
            let markup = icon_markup(measurer, name).unwrap_or_default();
            out = format!(
                "<rect x=\"-22\" y=\"-26\" width=\"44\" height=\"44\"{}/>{}",
                rounded(&format!("{SHAPE_ATTRS}{shape}")),
                icon_svg(name, -14.0, -18.0, 28.0, &markup)
            );
        }
        _ => {
            out = format!(
                "<circle cy=\"-21\" r=\"9\"{SHAPE_ATTRS}{shape}/>\
                 <path d=\"M0,-12V10M-14,-3H14M0,10L-12,30M0,10L12,30\"{line}/>"
            );
        }
    }
    if node.business == Some(true) {
        // The slash of a business actor: a chord across the lower right of the head.
        out.push_str(&format!(
            "<path class=\"pele-business\" d=\"M{},{}L{},{}\"{line}/>",
            num(0.12 * radius),
            num(head + 0.99 * radius),
            num(0.92 * radius),
            num(head - 0.39 * radius)
        ));
    }
    out
}

/// A label whose lines are drawn one by one, each in the middle of its own width.
fn left_label(label: &Label, x: f64, cy: f64, attrs: &str) -> String {
    if label.lines.len() < 2 {
        return label_svg(label, x + label.width / 2.0, cy, attrs);
    }
    let mut out = String::new();
    for i in 0..label.lines.len() {
        out.push_str(&label_svg(
            &label_line(label, i),
            x + label.widths[i] / 2.0,
            cy - label.height / 2.0 + line_middle(label, i),
            attrs,
        ));
    }
    out
}

/// A boundary's title sits in the middle of its top edge. Where edges enter through that edge it
/// moves into the stretch between them that is nearest the middle, or the widest if none has
/// room.
fn title_x(c: &CNode, width: f64, crossings: &Crossings) -> f64 {
    let top = crate::util::round(c.y - c.h / 2.0) as i64;
    let Some(xs) = crossings.get(&top) else {
        return c.x;
    };
    if width == 0.0 {
        return c.x;
    }
    let left = c.x - c.w / 2.0 + 6.0;
    let right = c.x + c.w / 2.0 - 6.0;
    let mut cuts: Vec<f64> = xs
        .iter()
        .copied()
        .filter(|x| *x > left && *x < right)
        .collect();
    cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    cuts.push(right);
    let need = width / 2.0 + 6.0;
    let mut best = c.x;
    let mut best_off = f64::INFINITY;
    let mut widest = 0.0;
    let mut from = left;
    for cut in cuts {
        if cut - from >= 2.0 * need {
            let x = c.x.max(from + need).min(cut - need);
            if (x - c.x).abs() < best_off {
                best_off = (x - c.x).abs();
                best = x;
            }
        } else if best_off == f64::INFINITY && cut - from > widest {
            widest = cut - from;
            best = (from + cut) / 2.0;
        }
        from = cut;
    }
    best
}

/// Draws a use case diagram.
pub(crate) fn render_usecase(model: &UsecaseModel, input: &Input<'_>) -> Output {
    let options = input.options;
    if !runs_across(Some(model.direction.as_str())) {
        return draw(model, input, false);
    }
    turn_to_fit(options, |down| draw(model, input, down))
}

/// `turned` draws a diagram that runs across as one that runs down.
fn draw(model: &UsecaseModel, input: &Input<'_>, turned: bool) -> Output {
    let options = input.options;
    let config = input.config;
    let measurer = input.measurer;
    let graph = build_usecase_graph(model);
    let size = options.font_size.unwrap_or(16.0);
    let small = (size * 0.875).round();
    let tiny = (size * 0.8125).round();
    let pad = options.padding.unwrap_or(8.0);
    let root_dir = if turned {
        crate::layout::compound::Dir::TB
    } else {
        direction(Some(model.direction.as_str()))
    };
    let sideways = matches!(
        root_dir,
        crate::layout::compound::Dir::LR | crate::layout::compound::Dir::RL
    );

    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, node) in graph.nodes.iter().enumerate() {
        index.insert(node.id.as_str(), i);
    }

    let mut views: Vec<View<'_>> = Vec::with_capacity(graph.nodes.len());
    let mut cnodes: Vec<CNode> = Vec::with_capacity(graph.nodes.len());
    for node in &graph.nodes {
        let mut declarations: Vec<&str> = node
            .css_compiled_styles
            .iter()
            .map(String::as_str)
            .collect();
        if !node.css_styles.is_empty() {
            declarations.extend(node.css_styles.iter().map(String::as_str));
        }
        let style = resolve_style(&declarations);
        let mut base = Style::NONE;
        if style.bold {
            base |= Style::BOLD;
        }
        if style.italic {
            base |= Style::ITALIC;
        }
        let font_size = style.font_size.unwrap_or(size);
        let markdown = node.label_type == super::db::LabelType::Markdown;
        let stereotype = text(
            node.stereotype
                .as_deref()
                .map(|stereotype| format!("\u{ab}{stereotype}\u{bb}"))
                .as_deref(),
            false,
            measurer,
            tiny,
            200.0,
            base,
        );
        let mut view = View {
            node,
            kind: Kind::Box,
            style,
            label: stereotype.clone(),
            stereotype,
            w: 0.0,
            h: 0.0,
            rows: Vec::new(),
            key_w: 0.0,
            head_h: 0.0,
            loops: 0,
        };
        let mut c;
        if node.is_group {
            view.kind = Kind::Boundary;
            view.label = text(
                Some(&node.label),
                markdown,
                measurer,
                font_size,
                4000.0,
                base,
            );
            let title_h = view.label.height;
            c = cnode(0.0, 0.0);
            c.is_group = true;
            c.pad_x = GROUP_PAD;
            c.pad_bottom = GROUP_PAD;
            c.pad_top = if node.boundary_type == Some(BoundaryType::Package) {
                title_h + 2.0 * TAB_PAD + 16.0
            } else if title_h > 0.0 {
                title_h + 16.0
            } else {
                GROUP_PAD
            };
            c.min_w = view.label.width + 2.0 * GROUP_PAD;
        } else if let Some(rows) = &node.json_rows {
            view.kind = Kind::Json;
            view.label = text(
                Some(&node.label),
                false,
                measurer,
                font_size,
                4000.0,
                base | Style::BOLD,
            );
            let mut value_w = 0.0f64;
            for row in rows {
                let key = text(Some(&row.key), false, measurer, small, 4000.0, base);
                let value = text(Some(&row.value), false, measurer, small, 260.0, base);
                view.key_w = view.key_w.max(key.width);
                value_w = value_w.max(value.width);
                view.h += key.height.max(value.height).max((small * 1.5).round()) + ROW_PAD;
                view.rows.push((key, value));
            }
            // The root of an empty object has a value and no key, and then there is no key
            // column.
            if view.key_w > 0.0 {
                view.key_w += 2.0 * CELL_PAD;
            }
            view.head_h = view.label.height.max((font_size * 1.5).round()) + ROW_PAD;
            view.w = (view.label.width + 2.0 * CELL_PAD)
                .max(if !view.rows.is_empty() {
                    view.key_w + value_w + 2.0 * CELL_PAD
                } else {
                    0.0
                })
                .max(72.0);
            view.h += view.head_h;
            c = cnode(view.w, view.h);
        } else if node.actor_type.is_some() {
            view.kind = Kind::Actor;
            view.label = text(
                Some(&node.label),
                markdown,
                measurer,
                font_size,
                140.0,
                base,
            );
            view.w = FIGURE_W.max(view.label.width).max(view.stereotype.width);
            view.h = FIGURE_H + FIGURE_GAP + view.stereotype.height + view.label.height;
            c = cnode(view.w, view.h);
        } else {
            view.label = text(
                Some(&node.label),
                markdown,
                measurer,
                if node.shape == GraphShape::Note {
                    small
                } else {
                    font_size
                },
                180.0,
                base,
            );
            let tw = view.label.width.max(view.stereotype.width);
            let th = view.label.height + view.stereotype.height;
            if node.shape == GraphShape::UsecaseEllipse || node.shape == GraphShape::UsecaseBusiness
            {
                view.kind = Kind::Ellipse;
                // An oval needs more room around its text than a box, and a business oval room
                // for its slash.
                view.w = (tw
                    + 2.0 * node.padding
                    + if node.business == Some(true) {
                        16.0
                    } else {
                        4.0
                    })
                .max(88.0);
                view.h = (th + 28.0).max(48.0);
            } else if node.shape == GraphShape::Note {
                view.w = tw + 2.0 * node.padding + 6.0;
                view.h = th + 18.0;
            } else {
                view.w = (tw + 2.0 * node.padding + 12.0).max(80.0);
                view.h = (th + 24.0).max(44.0);
            }
            c = cnode(view.w, view.h);
        }
        c.seq = views.len() as f64;
        views.push(view);
        cnodes.push(c);
    }
    for (i, node) in graph.nodes.iter().enumerate() {
        cnodes[i].parent = match &node.parent_id {
            Some(parent) => index.get(parent.as_str()).map(|i| *i as i64).unwrap_or(-1),
            None => -1,
        };
    }

    let mut cedges: Vec<CEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut drawn: Vec<&UsecaseGraphEdge> = Vec::new();
    for edge in &graph.edges {
        let (Some(&src), Some(&dst)) =
            (index.get(edge.start.as_str()), index.get(edge.end.as_str()))
        else {
            continue;
        };
        let semantic = edge.relationship_type == super::graph::EdgeKind::Include
            || edge.relationship_type == super::graph::EdgeKind::Extend;
        let shown = if semantic {
            edge.label
                .as_deref()
                .map(|label| format!("\u{ab}{label}\u{bb}"))
        } else {
            edge.label.clone()
        };
        let label = text(
            shown.as_deref(),
            edge.label_type == Some(super::db::LabelType::Markdown),
            measurer,
            small,
            200.0,
            Style::NONE,
        );
        if src == dst {
            // A self-relationship loops out across the flow, and the node's box grows to hold it.
            views[src].loops += 1;
            if sideways {
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
        drawn.push(edge);
        edge_labels.push(label);
        cedges.push(cedge(
            src,
            dst,
            edge.minlen,
            if edge_labels.last().map(|l| l.width).unwrap_or(0.0) > 0.0 {
                edge_labels.last().map(|l| l.width).unwrap_or(0.0) + 12.0
            } else {
                0.0
            },
            if edge_labels.last().map(|l| l.height).unwrap_or(0.0) > 0.0 {
                edge_labels.last().map(|l| l.height).unwrap_or(0.0) + 4.0
            } else {
                0.0
            },
        ));
    }

    let layout_options = LayeredOptions {
        node_sep: number_option(config, "nodeSpacing", NODE_SEP),
        edge_sep: EDGE_SEP,
        rank_sep: number_option(config, "rankSpacing", RANK_SEP),
        port_sep: 20.0,
        ..LayeredOptions::default()
    };
    let layout = compound_layout(&mut cnodes, &mut cedges, root_dir, &layout_options);

    let title = title_label(model.title.as_deref(), measurer, size, 4000.0);
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

    // Where edges cross each boundary's top border, so that a title can keep out of their way.
    let mut crossings: Crossings = HashMap::new();
    for edge in &cedges {
        mark_crossings(&mut crossings, &edge.route);
    }
    attach_ends(&mut cedges, &cnodes, &views);

    let mut fills = Vec::new();
    let mut clusters = String::new();
    let mut nodes_out = String::new();
    for (i, view) in views.iter().enumerate() {
        let c = &cnodes[i];
        let node = view.node;
        let classes = class_names(&node.css_classes.replacen("default ", "", 1));
        let id = esc_text(&node.id);
        let text_attrs = &view.style.text;
        let color = crate::svg::ink::ink_fill(&view.style, None);
        let muted = if color.is_empty() {
            " fill=\"var(--_m)\"".to_string()
        } else {
            color.clone()
        };
        let shape = &view.style.shape;
        let w = view.w;
        let h = view.h;

        if view.kind == Kind::Boundary {
            let x = c.x - c.w / 2.0;
            let y = c.y - c.h / 2.0;
            if crate::svg::ink::paints(shape) {
                fills.push(crate::svg::fills::Fill {
                    bounds: [x, y, x + c.w, y + c.h],
                    shape: shape.clone(),
                });
            }
            let opacity = if crate::svg::sets_fill(shape) {
                ""
            } else {
                " fill-opacity=\"0.5\""
            };
            let box_attrs = format!(" fill=\"var(--_a)\"{opacity} stroke=\"var(--_b)\"{shape}");
            let title_attrs = format!(" class=\"pele-cluster-label\"{muted}{text_attrs}");
            let body = if node.boundary_type == Some(BoundaryType::Package) {
                let tab_w = (view.label.width + 2.0 * TAB_PAD + 8.0).min(c.w);
                let tab_h = view.label.height + 2.0 * TAB_PAD;
                format!(
                    "<rect class=\"pele-boundary-tab\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{box_attrs}/>\
                     <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{box_attrs}/>",
                    num(x),
                    num(y),
                    num(tab_w),
                    num(tab_h),
                    num(x),
                    num(y + tab_h),
                    num(c.w),
                    num(c.h - tab_h),
                ) + &label_svg(&view.label, x + tab_w / 2.0, y + tab_h / 2.0, &title_attrs)
            } else {
                format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>",
                    num(x),
                    num(y),
                    num(c.w),
                    num(c.h),
                    rounded(&box_attrs)
                ) + &label_svg(
                    &view.label,
                    title_x(c, view.label.width, &crossings),
                    y + 8.0 + view.label.height / 2.0,
                    &title_attrs,
                )
            };
            clusters.push_str(&format!(
                "<g class=\"pele-cluster pele-boundary{classes}\" data-id=\"{id}\">{body}</g>"
            ));
            continue;
        }

        let (kind, body) = if view.kind == Kind::Actor {
            let top = -h / 2.0;
            let stereotype_y = top + FIGURE_H + FIGURE_GAP + view.stereotype.height / 2.0;
            let body = format!(
                "<g class=\"pele-actor-figure\" transform=\"translate(0,{})\">{}</g>",
                num(top + FIGURE_H / 2.0),
                figure(view, measurer)
            ) + &label_svg(
                &view.stereotype,
                0.0,
                stereotype_y,
                &format!(" class=\"pele-stereotype\" font-size=\"{tiny}\" fill=\"var(--_m)\""),
            ) + &label_svg(
                &view.label,
                0.0,
                h / 2.0 - view.label.height / 2.0,
                &format!(" class=\"pele-label\"{text_attrs}"),
            );
            ("pele-actor", body)
        } else if view.kind == Kind::Json {
            let x0 = -w / 2.0;
            let mut y = -h / 2.0 + view.head_h;
            let mut cells = String::new();
            let mut lines = format!("M{},{}H{}", num(x0), num(y), num(x0 + w));
            if view.key_w > 0.0 {
                lines.push_str(&format!(
                    "M{},{}V{}",
                    num(x0 + view.key_w),
                    num(y),
                    num(h / 2.0)
                ));
            }
            for (key, value) in &view.rows {
                let row_h = key.height.max(value.height).max((small * 1.5).round()) + ROW_PAD;
                cells.push_str(&label_svg(
                    key,
                    x0 + CELL_PAD + key.width / 2.0,
                    y + row_h / 2.0,
                    &format!(" class=\"pele-json-key\"{muted}"),
                ));
                cells.push_str(&left_label(
                    value,
                    x0 + view.key_w + CELL_PAD,
                    y + row_h / 2.0,
                    &format!(" class=\"pele-json-value\"{color}{text_attrs}"),
                ));
                y += row_h;
            }
            let body = format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>\
                 <path class=\"pele-json-lines\" d=\"{lines}\" fill=\"none\" stroke=\"var(--_b)\"{}/>{}\
                 <g font-size=\"{small}\">{cells}</g>",
                num(x0),
                num(-h / 2.0),
                num(w),
                num(h),
                rounded(&format!("{SHAPE_ATTRS}{shape}")),
                view.style.line,
                label_svg(
                    &view.label,
                    0.0,
                    -h / 2.0 + view.head_h / 2.0,
                    &format!(" class=\"pele-label\" font-weight=\"var(--_hw)\"{color}{text_attrs}"),
                ),
            );
            ("pele-json", body)
        } else {
            let label_y = view.stereotype.height / 2.0;
            let kind = if view.kind == Kind::Ellipse {
                "pele-usecase"
            } else if node.shape == GraphShape::Note {
                "pele-note"
            } else {
                "pele-usecase"
            };
            let outline = if view.kind == Kind::Ellipse {
                let mut outline = format!(
                    "<ellipse rx=\"{}\" ry=\"{}\"{SHAPE_ATTRS}{shape}/>",
                    num(w / 2.0),
                    num(h / 2.0)
                );
                if node.business == Some(true) {
                    // The slash of a business use case, cutting off the right end of the oval
                    // beside the text.
                    let from = w / 2.0 - 15.0;
                    let to = w / 2.0 - 5.0;
                    let arc = |at: f64| (h / 2.0) * (1.0 - (at / (w / 2.0)).powi(2)).sqrt();
                    outline.push_str(&format!(
                        "<path class=\"pele-business\" d=\"M{},{}L{},{}\" fill=\"none\" stroke=\"var(--_b)\"{}/>",
                        num(from),
                        num(arc(from)),
                        num(to),
                        num(-arc(to)),
                        view.style.line
                    ));
                }
                outline
            } else if node.shape == GraphShape::Note {
                let r = w / 2.0;
                let b = h / 2.0;
                format!(
                    "<path d=\"M{},{}H{}L{},{}V{}H{}Z\" fill=\"var(--_a)\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"{shape}/>\
                     <path d=\"M{},{}V{}H{}\" fill=\"none\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"{}/>",
                    num(-r),
                    num(-b),
                    num(r - FOLD),
                    num(r),
                    num(FOLD - b),
                    num(b),
                    num(-r),
                    num(r - FOLD),
                    num(-b),
                    num(FOLD - b),
                    num(r),
                    view.style.line
                )
            } else {
                format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>",
                    num(-w / 2.0),
                    num(-h / 2.0),
                    num(w),
                    num(h),
                    rounded(&format!("{SHAPE_ATTRS}{shape}"))
                )
            };
            let body = outline
                + &label_svg(
                    &view.stereotype,
                    0.0,
                    -view.label.height / 2.0,
                    &format!(" class=\"pele-stereotype\" font-size=\"{tiny}\"{muted}"),
                )
                + &label_svg(
                    &view.label,
                    0.0,
                    label_y,
                    &format!(
                        " class=\"pele-label\"{}{color}{text_attrs}",
                        if node.shape == GraphShape::Note {
                            format!(" font-size=\"{small}\"")
                        } else {
                            String::new()
                        }
                    ),
                );
            (kind, body)
        };
        nodes_out.push_str(&format!(
            "<g class=\"pele-node {kind}{classes}\" data-id=\"{id}\" transform=\"translate({},{})\">{body}</g>",
            num(c.x),
            num(c.y)
        ));
    }

    let mut casings = String::new();
    let mut edges_out = String::new();
    let mut labels_out = String::new();
    let mut loop_seen: HashMap<usize, usize> = HashMap::new();
    for (i, ce) in cedges.iter().enumerate() {
        let edge = drawn[i];
        let label = &edge_labels[i];
        let style = if edge.style.is_empty() && edge.css_compiled_styles.is_empty() {
            None
        } else {
            let mut declarations: Vec<&str> = edge
                .css_compiled_styles
                .iter()
                .map(String::as_str)
                .collect();
            declarations.extend(edge.style.iter().map(String::as_str));
            Some(resolve_style(&declarations))
        };
        let start_type = edge.arrow_type_start.as_str();
        let end_type = edge.arrow_type_end.as_str();
        let mut label_at: Option<(f64, f64)> = None;
        let path: EdgePath = if ce.src == ce.dst {
            let c = &cnodes[ce.src];
            let view = &views[ce.src];
            let k = loop_seen.entry(ce.src).or_insert(0);
            let count = *k;
            *k += 1;
            let side = if sideways { 2 } else { 1 };
            let spread = (side_length(view, side) / 2.0).min(8.0 + count as f64 * 6.0);
            let reach = LOOP + count as f64 * 8.0;
            let (ax, ay) = anchor(view, c, side, -spread);
            let (bx, by) = anchor(view, c, side, spread);
            let len = (reach * reach + spread * spread).sqrt();
            let len = if len == 0.0 { 1.0 } else { len };
            let ux = if sideways { spread / len } else { reach / len };
            let uy = if sideways { reach / len } else { spread / len };
            let st = trim(start_type);
            let et = trim(end_type);
            // Both ends leave the node leaning away from each other, and the curve joins them
            // outside it.
            let out = |x: f64, y: f64, sign: f64, far: f64| -> String {
                let reach = if far != 0.0 { 1.0 } else { 0.0 };
                if sideways {
                    format!("{},{}", num(x + sign * spread * reach), num(y + far))
                } else {
                    format!("{},{}", num(x + far), num(y + sign * spread * reach))
                }
            };
            let sx = if sideways { ax - ux * st } else { ax + ux * st };
            let sy = if sideways { ay + uy * st } else { ay - uy * st };
            let ex = bx + ux * et;
            let ey = by + uy * et;
            // The label of a self-relationship goes beside the loop.
            if sideways {
                label_at = Some((c.x, ay.max(by) + reach * 0.75 + 6.0 + label.height / 2.0));
            } else {
                label_at = Some((ax.max(bx) + reach * 0.75 + 8.0 + label.width / 2.0, c.y));
            }
            EdgePath {
                d: format!(
                    "M{},{}C{} {} {},{}",
                    num(sx),
                    num(sy),
                    out(ax, ay, -1.0, reach),
                    out(bx, by, 1.0, reach),
                    num(ex),
                    num(ey)
                ),
                sx: ax,
                sy: ay,
                sdx: if sideways { ux } else { -ux },
                sdy: if sideways { -uy } else { uy },
                ex: bx,
                ey: by,
                edx: -ux,
                edy: -uy,
            }
        } else {
            if ce.route.len() < 6 {
                continue;
            }
            route_path(
                &ce.route,
                None,
                trim(start_type),
                trim(end_type),
                None,
                None,
            )
        };
        let line = style.as_ref().map_or("", |s| s.line.as_str());
        let under =
            crate::svg::fills::fill_under(&path.d, &[path.sx, path.sy, path.ex, path.ey], &fills);
        let over = match &under {
            crate::svg::fills::Coverage::Outside => None,
            crate::svg::fills::Coverage::Crossed => Some(String::new()),
            crate::svg::fills::Coverage::Inside(fill) => {
                Some(crate::svg::fills::edge_ink(fill, line))
            }
        };
        let color = over.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| {
            style
                .as_ref()
                .and_then(|s| s.stroke.as_deref())
                .map_or("var(--_l)".to_string(), esc)
        });
        if over.as_deref() == Some("") {
            let marks = (mark(
                start_type,
                path.sx,
                path.sy,
                path.sdx,
                path.sdy,
                "var(--_bg)",
            ) + &mark(end_type, path.ex, path.ey, path.edx, path.edy, "var(--_bg)"))
                .replace("stroke=\"none\"", "stroke=\"var(--_bg)\"");
            casings += &crate::svg::fills::casing(&path.d, &marks, line, false);
        }
        let over_attr = over
            .as_ref()
            .filter(|s| !s.is_empty())
            .map_or_else(String::new, |s| format!(" stroke=\"{s}\""));
        // Mermaid animates these dashes. A still picture keeps the dashes.
        let dashes = if edge.internal {
            "2 3"
        } else if edge.pattern == Pattern::Dotted {
            "5 4"
        } else if edge.animate {
            "9 5"
        } else {
            ""
        };
        let prefix = format!(
            "default relationship relationship-{}",
            edge.relationship_type.as_str()
        );
        let own_classes = edge
            .classes
            .strip_prefix(prefix.as_str())
            .unwrap_or(&edge.classes)
            .trim_start();
        let edge_classes = class_names(own_classes);
        let dash_attr = if dashes.is_empty() {
            String::new()
        } else {
            format!(" stroke-dasharray=\"{dashes}\"")
        };
        let mut markers = format!(
            "{}{}",
            mark(start_type, path.sx, path.sy, path.sdx, path.sdy, &color),
            mark(end_type, path.ex, path.ey, path.edx, path.edy, &color)
        );
        if let crate::svg::fills::Coverage::Inside(fill) = under {
            if over.as_ref().is_some_and(|s| !s.is_empty()) {
                markers = crate::svg::fills::hollow_on(&markers, fill);
            }
        }
        edges_out.push_str(&format!(
            "<g class=\"pele-edge pele-{}{}\" data-id=\"{}\"{over_attr}{}>\
             <path d=\"{}\"{dash_attr}/>{markers}</g>",
            edge.relationship_type.as_str(),
            edge_classes,
            esc_text(&edge.id),
            style
                .as_ref()
                .map(|style| style.line.clone())
                .unwrap_or_default(),
            path.d,
        ));
        if label.width > 0.0 {
            let semantic = edge.relationship_type == super::graph::EdgeKind::Include
                || edge.relationship_type == super::graph::EdgeKind::Extend;
            let attrs = format!(
                "{}{}",
                if semantic { " fill=\"var(--_m)\"" } else { "" },
                style
                    .as_ref()
                    .map(|style| style.text.clone())
                    .unwrap_or_default()
            );
            let (label_x, label_y) = label_at.unwrap_or((ce.label_x, ce.label_y));
            labels_out.push_str(&edge_label_svg(&edge.id, label, label_x, label_y, &attrs));
        }
    }

    let acc = Accessible {
        title: if model.acc_title.is_empty() {
            None
        } else {
            Some(model.acc_title.clone())
        },
        descr: if model.acc_descr.is_empty() {
            None
        } else {
            Some(model.acc_descr.clone())
        },
    };
    let content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &if clusters.is_empty() {
        String::new()
    } else {
        format!("<g class=\"pele-clusters\">{clusters}</g>")
    } + &crate::svg::fills::casings_svg(&casings)
        + &if edges_out.is_empty() {
            String::new()
        } else {
            format!(
                "<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges_out}</g>"
            )
        }
        + &if labels_out.is_empty() {
            String::new()
        } else {
            format!("<g class=\"pele-edge-labels\" font-size=\"{small}\">{labels_out}</g>")
        }
        + &format!("<g class=\"pele-nodes\">{nodes_out}</g>");

    Output {
        svg: svg_document(
            crate::DiagramKind::Usecase,
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
