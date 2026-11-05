//! Class diagram layout and SVG. Ported from pele's `diagrams/class/render.ts`, without math.

use std::collections::HashMap;
use std::fmt::Write;

use super::db::{ClassDb, ClassMember, ClassNode};
use super::graph::{GraphEdge, GraphNode, Pattern, Shape, build_class_graph};
use super::markers::{class_marker, class_marker_trim};
use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{runs_across, tighten, turn_to_fit};
use crate::diagrams::common::generics::parse_generic_types;
use crate::diagrams::common::ordered::Ordered;
use crate::layout::components::wrap_components;
use crate::layout::compound::{CEdge, CNode, Dir, cedge, cnode, compound_layout, direction};
use crate::layout::layered::LayeredOptions;
use crate::preprocess::{Config, Value};
use crate::svg::cluster::{
    Crossings, cluster_title_x, mark_crossings, struck_title, title_crossed,
};
use crate::svg::edges::{EdgePath, edge_label_svg, route_path};
use crate::svg::shapes::end_side;
use crate::svg::{
    Accessible, ROUNDED, ResolvedStyle, class_names, esc, esc_text, label_svg, num, resolve_style,
    rounded, svg_document, with_title,
};
use crate::text::entities::decode_entities;
use crate::text::label::{Label, has_math, layout_label};
use crate::text::{Style, TextMeasurer};
use crate::url::linked;
use crate::util::{round, trim};

const NODE_SEP: f64 = 40.0;
const EDGE_SEP: f64 = 16.0;
const RANK_SEP: f64 = 56.0;
const GROUP_PAD: f64 = 20.0;
const LOOP: f64 = 26.0;
const PAD_X: f64 = 12.0;
const TITLE_PAD: f64 = 8.0;
const SECTION_PAD: f64 = 6.0;
const EMPTY_SECTION: f64 = 10.0;
const MIN_WIDTH: f64 = 64.0;
const FOLD: f64 = 8.0;
const LINE: &str = "var(--_l)";
const TARGETS: [&str; 4] = ["_self", "_blank", "_parent", "_top"];

fn number_option(section: &Config, key: &str, fallback: f64) -> f64 {
    match section.get(key) {
        Some(&Value::Number(v)) if v > 0.0 => v.min(2000.0),
        _ => fallback,
    }
}

struct Line {
    text: String,
    width: f64,
    classifier: Option<char>,
}

struct View<'a> {
    node: &'a GraphNode,
    class: Option<&'a ClassNode>,
    style: ResolvedStyle,
    label: Label,
    w: f64,
    h: f64,
    annotations: Vec<Annotation>,
    attributes: Vec<Line>,
    methods: Vec<Line>,
    title_h: f64,
    attributes_h: f64,
    font_size: f64,
}

/// One end of an edge on a side of a node.
struct End {
    edge: usize,
    at: usize,
    toward: f64,
    order: usize,
    room: f64,
    /// Whether the text of this end goes on the right of, or below, its line.
    after: bool,
}

struct Card {
    text: String,
    /// The text typeset by the host, when it has math for it.
    math: Option<Label>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    parent: i64,
}

/// The line of an annotation in the title of a class: text, or math the host typeset.
enum Annotation {
    Text(String),
    Math(Label),
}

/// Text that holds math the host typeset, or `None`, for text that stays as written. Annotations
/// and cardinalities are literal text unless there is math in them.
fn math_text(raw: &str, measurer: &dyn TextMeasurer, size: f64) -> Option<Label> {
    if !raw.contains("$$") {
        return None;
    }
    let label = layout_label(Some(raw), false, measurer, size, 4000.0, Style::NONE);
    label
        .lines
        .iter()
        .any(|line| has_math(line))
        .then_some(label)
}

fn base(node: &GraphNode, label: Label, w: f64, h: f64, font_size: f64) -> View<'_> {
    View {
        node,
        class: None,
        style: ResolvedStyle::default(),
        label,
        w,
        h,
        annotations: Vec::new(),
        attributes: Vec::new(),
        methods: Vec::new(),
        title_h: 0.0,
        attributes_h: 0.0,
        font_size,
    }
}

fn member_line(member: &ClassMember, measurer: &dyn TextMeasurer, size: f64, mono: bool) -> Line {
    let decoded = decode_entities(&member.display_text());
    let text = trim(&decoded).to_string();
    let style = if member.classifier == Some('*') {
        Style::ITALIC
    } else {
        Style::NONE
    };
    Line {
        width: measurer.width(
            &text,
            size,
            style | if mono { Style::MONO } else { Style::NONE },
        ),
        text,
        classifier: member.classifier,
    }
}

pub(crate) fn render_class(db: &ClassDb, input: &Input<'_>) -> Output {
    let fitted = |down: bool| tighten(input.options, |tight| draw(db, input, down, tight));
    if runs_across(Some(&db.direction)) {
        turn_to_fit(input.options, fitted)
    } else {
        fitted(false)
    }
}

/// Resolves the styles of a class: those of its classDefs and its own, keeping only the last
/// value of each property that has an effect. A class may carry the declarations of many
/// classDefs many times over, so each distinct declaration is checked once.
struct Styler<'a> {
    db: &'a ClassDb,
    effect: HashMap<String, String>,
    from_defs: HashMap<String, Vec<(String, String)>>,
}

impl Styler<'_> {
    fn keep(&mut self, kept: &mut Ordered<String>, decl: &str) {
        let prop = self
            .effect
            .entry(decl.to_string())
            .or_insert_with(|| {
                let one = resolve_style(&[decl]);
                if one.shape.is_empty() && one.text.is_empty() {
                    String::new()
                } else {
                    let name = decl.find(':').map_or(decl, |at| &decl[..at]);
                    trim(name).to_lowercase()
                }
            })
            .clone();
        if !prop.is_empty() {
            kept.set(&prop, decl.to_string());
        }
    }

    fn style(&mut self, cls: &ClassNode) -> ResolvedStyle {
        let mut kept: Ordered<String> = Ordered::default();
        if let Some(base) = self.from_defs.get(&cls.css_classes) {
            for (prop, decl) in base {
                kept.set(prop, decl.clone());
            }
        } else {
            for name in cls.css_classes.split(' ') {
                if let Some(def) = self.db.style_classes.get(name) {
                    for s in &def.styles {
                        for part in s.split(',') {
                            self.keep(&mut kept, part);
                        }
                    }
                }
            }
            self.from_defs
                .insert(cls.css_classes.clone(), kept.items.clone());
        }
        for s in &cls.styles {
            self.keep(&mut kept, s);
        }
        let decls: Vec<&str> = kept.values().map(String::as_str).collect();
        resolve_style(&decls)
    }
}

/// `turned` draws a diagram that runs across as one that runs down. `tight` gives up the room
/// that keeps a namespace's name clear of an edge coming in beside it.
fn draw(db: &ClassDb, input: &Input<'_>, turned: bool, tight: bool) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let empty = Config::new();
    let conf = match input.config.get("class") {
        Some(Value::Map(m)) => m,
        _ => &empty,
    };
    let graph = build_class_graph(
        db,
        !matches!(conf.get("hierarchicalNamespaces"), Some(Value::Bool(false))),
    );
    let hide_empty = matches!(conf.get("hideEmptyMembersBox"), Some(Value::Bool(true)));
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let tiny = round(size * 0.8);
    let tiny_height = round(tiny * 1.4);
    let pad = options.padding.unwrap_or(8.0);
    let card_width = |raw: &str| {
        if raw.is_empty() {
            0.0
        } else if let Some(math) = math_text(raw, measurer, tiny) {
            math.width
        } else {
            measurer.width(&decode_entities(raw), tiny, Style::NONE)
        }
    };
    let root_dir = if turned {
        Dir::TB
    } else {
        direction(Some(&db.direction))
    };
    let mut styler = Styler {
        db,
        effect: HashMap::new(),
        from_defs: HashMap::new(),
    };

    let n = graph.nodes.len();
    let mut children = vec![0usize; n];
    for node in &graph.nodes {
        if node.parent >= 0 {
            children[node.parent as usize] += 1;
        }
    }

    // Namespaces that an edge may cross into are made wide enough for the title to sit beside
    // it.
    let mut crossed = vec![false; n];
    for edge in &graph.edges {
        if edge.source < 0 || edge.target < 0 {
            continue;
        }
        let a = graph.nodes[edge.source as usize].parent;
        let b = graph.nodes[edge.target as usize].parent;
        if a == b {
            continue;
        }
        for start in [a, b] {
            let mut g = start;
            while g >= 0 && !crossed[g as usize] {
                crossed[g as usize] = true;
                g = graph.nodes[g as usize].parent;
            }
        }
    }

    let mut views: Vec<View<'_>> = Vec::with_capacity(n);
    let mut cnodes: Vec<CNode> = Vec::with_capacity(n);
    for (i, node) in graph.nodes.iter().enumerate() {
        let (view, mut c) = match node.shape {
            Shape::Namespace => {
                let label = layout_label(
                    Some(&node.label),
                    false,
                    measurer,
                    size,
                    4000.0,
                    Style::NONE,
                );
                if children[i] > 0 {
                    let mut c = cnode(0.0, 0.0);
                    c.is_group = true;
                    c.pad_x = GROUP_PAD;
                    c.pad_top = if label.height > 0.0 {
                        label.height + 16.0
                    } else {
                        GROUP_PAD
                    };
                    c.pad_bottom = GROUP_PAD;
                    c.min_w = label.width + 2.0 * GROUP_PAD - 12.0
                        + if crossed[i] && !tight {
                            (label.width + 24.0).min(240.0)
                        } else {
                            0.0
                        };
                    (base(node, label, 0.0, 0.0, size), c)
                } else {
                    let w = (label.width + 2.0 * GROUP_PAD).max(80.0);
                    let h = label.height + 2.0 * GROUP_PAD;
                    (base(node, label, w, h, size), cnode(w, h))
                }
            }
            Shape::Class => {
                let cls = &db.classes.items[node.class.expect("class")].1;
                let view = class_view(
                    node,
                    cls,
                    styler.style(cls),
                    measurer,
                    size,
                    tiny,
                    tiny_height,
                    hide_empty,
                    options.monospace.contains(&crate::MonospacePart::Members),
                );
                let c = cnode(view.w, view.h);
                (view, c)
            }
            Shape::Note => {
                let label =
                    layout_label(Some(&node.label), true, measurer, small, 260.0, Style::NONE);
                let w = (label.width + 2.0 * PAD_X).max(40.0);
                let h = label.height + 2.0 * TITLE_PAD;
                (base(node, label, w, h, small), cnode(w, h))
            }
            Shape::Interface => {
                let label = layout_label(
                    Some(&node.label),
                    false,
                    measurer,
                    small,
                    200.0,
                    Style::NONE,
                );
                let w = label.width.max(12.0) + 4.0;
                let h = label.height.max(12.0);
                (base(node, label, w, h, small), cnode(w, h))
            }
        };
        c.parent = node.parent;
        c.seq = i as f64;
        views.push(view);
        cnodes.push(c);
    }

    let mut cedges: Vec<CEdge> = Vec::new();
    let mut drawn: Vec<&GraphEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut start_room: Vec<f64> = Vec::new();
    let mut end_room: Vec<f64> = Vec::new();
    // Edges leave one side of a class and arrive at the other, so a class with many of them is
    // widened until they, and the text at their ends, fit side by side.
    let mut crowd = vec![0.0f64; n * 4];
    let meet = |crowd: &mut Vec<f64>, item: usize, slot: usize, room: f64| {
        crowd[item * 4 + slot] += 1.0;
        if room > crowd[item * 4 + slot + 1] {
            crowd[item * 4 + slot + 1] = room;
        }
    };
    for edge in &graph.edges {
        if edge.source < 0 || edge.target < 0 {
            continue;
        }
        let (src, dst) = (edge.source as usize, edge.target as usize);
        let label = layout_label(
            edge.label.as_deref(),
            true,
            measurer,
            small,
            200.0,
            Style::NONE,
        );
        let start_w = card_width(&edge.start_label);
        let end_w = card_width(&edge.end_label);
        if src == dst {
            cnodes[src].w += 2.0
                * (LOOP
                    + if label.width > 0.0 {
                        label.width + 12.0
                    } else {
                        0.0
                    });
        } else {
            meet(&mut crowd, src, 0, start_w);
            meet(&mut crowd, dst, 2, end_w);
        }
        start_room.push(start_w);
        end_room.push(end_w);
        drawn.push(edge);
        cedges.push(cedge(
            src,
            dst,
            1,
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
        edge_labels.push(label);
    }

    if root_dir.vertical() {
        for i in 0..views.len() {
            if views[i].class.is_none() {
                continue;
            }
            let mut need = 0.0f64;
            for slot in [0, 2] {
                let count = crowd[i * 4 + slot];
                let room = crowd[i * 4 + slot + 1];
                if count > 1.0 {
                    need = need
                        .max((count - 1.0) * if room > 0.0 { room + 24.0 } else { 14.0 } + 24.0);
                }
            }
            need = need.min(480.0);
            if need > views[i].w {
                cnodes[i].w += need - views[i].w;
                views[i].w = need;
            }
        }
    }

    let lettered = (0..drawn.len()).any(|i| start_room[i] > 0.0 || end_room[i] > 0.0);
    // Cardinalities at both ends of a vertical edge need room for their height between the ranks.
    let mut tall_cards = 0.0f64;
    if root_dir.vertical() {
        let card_height = |raw: &str| {
            if raw.is_empty() {
                0.0
            } else {
                tiny_height.max(math_text(raw, measurer, tiny).map_or(0.0, |math| math.height))
            }
        };
        for edge in &drawn {
            tall_cards = tall_cards.max(
                card_height(&edge.start_label) + card_height(&edge.end_label) - 2.0 * tiny_height,
            );
        }
    }
    let layout = compound_layout(
        &mut cnodes,
        &mut cedges,
        root_dir,
        &LayeredOptions {
            node_sep: number_option(conf, "nodeSpacing", NODE_SEP),
            edge_sep: EDGE_SEP,
            rank_sep: number_option(
                conf,
                "rankSpacing",
                if lettered { RANK_SEP + 16.0 } else { RANK_SEP },
            ) + tall_cards,
            port_sep: 20.0,
            apart: true,
            ..LayeredOptions::default()
        },
    );

    // Every edge reaches the middle of a side. Where several share a side they are spread along
    // it, in the order of where they come from. An end with text runs straight for the length
    // of the text, which sits beside it on the side the line then bends toward, away from the
    // neighbors that bend the other way. Neighbors are set apart far enough for the text
    // between them.
    let mut sides: Vec<(usize, Vec<End>)> = Vec::new();
    let mut side_at: HashMap<usize, usize> = HashMap::new();
    let mut attach =
        |cedges: &mut [CEdge], e: usize, at: usize, item: usize, room: f64| -> Option<(u8, bool)> {
            let c = &cnodes[item];
            let route = &mut cedges[e].route;
            let side = end_side(route, at, c)?;
            let slack = (c.w - views[item].w) / 2.0;
            if side == 1 {
                route[at] -= slack;
            } else if side == 3 {
                route[at] += slack;
            }
            let across = usize::from(side & 1);
            let toward = route[if at == 0 { 3 } else { at - 3 } + across];
            let bend = toward - route[at + across];
            let after = if across == 1 { bend > 0.0 } else { bend >= 0.0 };
            let key = item * 4 + side as usize;
            let k = *side_at.entry(key).or_insert_with(|| {
                sides.push((key, Vec::new()));
                sides.len() - 1
            });
            sides[k].1.push(End {
                edge: e,
                at,
                toward,
                order: e,
                room,

                after,
            });
            Some((side, after))
        };
    let mut starts: Vec<Option<(u8, bool)>> = Vec::with_capacity(cedges.len());
    let mut ends: Vec<Option<(u8, bool)>> = Vec::with_capacity(cedges.len());
    for i in 0..cedges.len() {
        let (src, dst, len) = (cedges[i].src, cedges[i].dst, cedges[i].route.len());
        let routed = src != dst && len >= 6;
        starts.push(if routed {
            attach(&mut cedges, i, 0, src, start_room[i])
        } else {
            None
        });
        ends.push(if routed {
            attach(&mut cedges, i, len - 3, dst, end_room[i])
        } else {
            None
        });
    }
    // Whether a point lies on the border of one of the namespaces around a class.
    let on_border = |item: usize, x: f64, y: f64| {
        let mut g = cnodes[item].parent;
        let mut hops = 0;
        while g >= 0 && hops < 10000 {
            let c = &cnodes[g as usize];
            let dx = (x - c.x).abs();
            let dy = (y - c.y).abs();
            if ((dy - c.h / 2.0).abs() < 0.5 && dx <= c.w / 2.0 + 0.5)
                || ((dx - c.w / 2.0).abs() < 0.5 && dy <= c.h / 2.0 + 0.5)
            {
                return true;
            }
            g = c.parent;
            hops += 1;
        }
        false
    };
    let mut gaps: Vec<f64> = Vec::new();
    for (key, list) in &mut sides {
        if list.len() < 2 {
            continue;
        }
        let item = *key >> 2;
        let across = *key & 1;
        let length = if across == 1 {
            views[item].h
        } else {
            views[item].w
        };
        list.sort_by(|a, b| {
            (a.toward - b.toward)
                .partial_cmp(&0.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.order.cmp(&b.order))
        });
        let mut total = 0.0;
        gaps.resize(list.len(), 0.0);
        for j in 0..list.len() - 1 {
            let (a, b) = (&list[j], &list[j + 1]);
            let mut gap = if across == 1 { 20.0 } else { 24.0 };
            if a.room > 0.0 && a.after {
                gap += if across == 1 {
                    tiny_height + 6.0
                } else {
                    a.room
                };
            }
            if b.room > 0.0 && !b.after {
                gap += if across == 1 {
                    tiny_height + 6.0
                } else {
                    b.room
                };
            }
            gaps[j] = gap;
            total += gap;
        }
        let scale = ((length - 24.0) / total).min(1.0);
        if scale <= 0.0 {
            continue;
        }
        let mut at = if across == 1 {
            cnodes[item].y
        } else {
            cnodes[item].x
        } - (total * scale) / 2.0;
        for (j, end) in list.iter().enumerate() {
            let route = &mut cedges[end.edge].route;
            // An edge that crosses the border of the class's namespace close by runs straight
            // to it: the point where it crosses moves into line with its end.
            let step: isize = if end.at == 0 { 3 } else { -3 };
            let first = end.at as isize + step;
            let line = route[first as usize + across];
            let mut border: isize = -1;
            let mut k = first;
            while k >= 0
                && (k as usize) < route.len()
                && (route[k as usize + across] - line).abs() < 0.5
            {
                if on_border(item, route[k as usize], route[k as usize + 1]) {
                    border = k;
                    break;
                }
                k += step;
            }
            if border >= 0 && (line - at).abs() <= 24.0 {
                let mut k = first;
                while k != border + step {
                    route[k as usize + across] = at;
                    k += step;
                }
            }
            route[end.at + across] = at;
            at += gaps[j] * scale;
        }
    }

    let straighten = |route: &mut Vec<f64>, end: Option<(u8, bool)>, room: f64, first: bool| {
        let Some((side, _)) = end else {
            return;
        };
        if room <= 0.0 {
            return;
        }
        let across = usize::from(side & 1);
        let at = if first { 0 } else { route.len() - 3 };
        let along = at + 1 - across;
        let reach = (route[if first { 3 } else { at - 3 } + 1 - across] - route[along]).abs();
        let length = if across == 1 {
            room + 10.0
        } else {
            tiny_height + 6.0
        }
        .min(reach * 0.35);
        if length < 4.0 {
            return;
        }
        let mut point = [route[at], route[at + 1], route[at + 2]];
        point[1 - across] += if side == 0 || side == 3 { -1.0 } else { 1.0 } * length;
        let to = if first { 3 } else { at };
        route.splice(to..to, point);
    };
    for i in 0..cedges.len() {
        straighten(&mut cedges[i].route, starts[i], start_room[i], true);
        straighten(&mut cedges[i].route, ends[i], end_room[i], false);
    }

    let mut min_x = 0.0f64;
    let mut min_y = 0.0f64;
    let mut max_x = layout.width;
    let mut max_y = layout.height;
    let mut cards: Vec<Card> = Vec::new();
    let mut card = |parent: usize, raw: &str, width: f64, x: f64, y: f64, side: u8, after: bool| {
        if raw.is_empty() {
            return;
        }
        let math = math_text(raw, measurer, tiny);
        let h = math
            .as_ref()
            .map_or(tiny_height, |m| tiny_height.max(m.height));
        let (mut cx, mut cy) = (x, y);
        if side & 1 == 1 {
            cx += if side == 3 { -1.0 } else { 1.0 } * (5.0 + width / 2.0);
            cy += if after { 1.0 } else { -1.0 } * (7.0 + h / 2.0);
        } else {
            cx += if after { 1.0 } else { -1.0 } * (9.0 + width / 2.0);
            cy += if side == 0 { -1.0 } else { 1.0 } * (3.0 + h / 2.0);
        }
        min_x = min_x.min(cx - width / 2.0);
        max_x = max_x.max(cx + width / 2.0);
        min_y = min_y.min(cy - h / 2.0);
        max_y = max_y.max(cy + h / 2.0);
        cards.push(Card {
            text: decode_entities(raw),
            math,
            x: cx,
            y: cy,
            w: width,
            h,
            parent: parent as i64,
        });
    };
    let mut loop_seen: HashMap<usize, usize> = HashMap::new();
    let mut loop_at = vec![0usize; cedges.len()];
    for i in 0..cedges.len() {
        let edge = drawn[i];
        let ce = &mut cedges[i];
        if ce.src == ce.dst {
            let seen = loop_seen.entry(ce.src).or_insert(0);
            let k = *seen;
            *seen += 1;
            loop_at[i] = k;
            let c = &cnodes[ce.src];
            let view = &views[ce.src];
            let x = c.x + view.w / 2.0;
            let spread = (view.h / 2.0 - 4.0).min(8.0 + k as f64 * 6.0);
            card(
                ce.src,
                &edge.start_label,
                start_room[i],
                x,
                c.y - spread,
                1,
                false,
            );
            card(
                ce.dst,
                &edge.end_label,
                end_room[i],
                x,
                c.y + spread,
                1,
                true,
            );
            ce.label_x = c.x + view.w / 2.0 + LOOP + k as f64 * 8.0 + 4.0 + ce.label_w / 2.0;
            ce.label_y = c.y;
        } else if ce.route.len() >= 6 {
            let route = &ce.route;
            let last = route.len() - 3;
            let (from_side, from_after) = starts[i].unwrap_or((2, true));
            let (to_side, to_after) = ends[i].unwrap_or((0, true));
            card(
                ce.src,
                &edge.start_label,
                start_room[i],
                route[0],
                route[1],
                from_side,
                from_after,
            );
            card(
                ce.dst,
                &edge.end_label,
                end_room[i],
                route[last],
                route[last + 1],
                to_side,
                to_after,
            );
        }
    }

    if let Some(available) = options.max_width.filter(|&a| a > 0.0) {
        if max_x - min_x + 2.0 * pad > available {
            let count = cnodes.len();
            for item in &cards {
                let mut c = cnode(item.w, item.h);
                c.parent = item.parent;
                c.x = item.x;
                c.y = item.y;
                cnodes.push(c);
            }
            let wrapped = wrap_components(
                &mut cnodes,
                &mut cedges,
                (available - 2.0 * pad).max(1.0),
                number_option(conf, "nodeSpacing", NODE_SEP),
            );
            if let Some(wrapped) = wrapped {
                for (item, c) in cards.iter_mut().zip(&cnodes[count..]) {
                    item.x = c.x;
                    item.y = c.y;
                }
                min_x = 0.0;
                min_y = 0.0;
                max_x = wrapped.width;
                max_y = wrapped.height;
            }
            cnodes.truncate(count);
        }
    }

    let ox = pad - min_x;
    let oy = pad - min_y;
    let width = (max_x - min_x + 2.0 * pad).ceil();
    let height = (max_y - min_y + 2.0 * pad).ceil();
    let mut links = Vec::new();

    let mut crossings: Crossings = HashMap::new();
    if graph
        .nodes
        .first()
        .is_some_and(|n| n.shape == Shape::Namespace)
    {
        for e in &cedges {
            mark_crossings(&mut crossings, &e.route);
        }
    }

    let mut clusters = String::new();
    let mut struck_titles = String::new();
    let mut nodes_out = String::new();
    for (i, view) in views.iter().enumerate() {
        let c = &cnodes[i];
        let node = view.node;
        let x = c.x + ox;
        let y = c.y + oy;
        let id = esc_text(&node.id);
        let (w, h) = if c.is_group {
            (c.w, c.h)
        } else {
            (view.w, view.h)
        };
        if node.shape == Shape::Namespace {
            let title_x = if c.is_group {
                cluster_title_x(c, view.label.width, &crossings, GROUP_PAD)
            } else {
                c.x
            };
            let title_y = y - h / 2.0 + 8.0 + view.label.height / 2.0;
            let title_attrs = " class=\"pele-cluster-label\" fill=\"var(--_m)\"";
            let struck = c.is_group && title_crossed(c, view.label.width, title_x, &crossings);
            if struck {
                let mut depth = 1;
                let mut p = graph.nodes[i].parent;
                while p >= 0 && depth < 8 {
                    depth += 1;
                    p = graph.nodes[p as usize].parent;
                }
                struck_titles +=
                    &struck_title(&view.label, title_x + ox, title_y, title_attrs, &id, depth);
            }
            let _ = write!(
                clusters,
                "<g class=\"pele-cluster pele-namespace\" data-id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"{ROUNDED}/>{}</g>",
                num(x - w / 2.0),
                num(y - h / 2.0),
                num(w),
                num(h),
                if struck {
                    String::new()
                } else {
                    label_svg(&view.label, title_x + ox, title_y, title_attrs)
                }
            );
            continue;
        }
        let at = format!(" transform=\"translate({},{})\"", num(x), num(y));
        match view.class {
            Some(cls) => {
                nodes_out += &class_svg(
                    view,
                    cls,
                    &id,
                    &at,
                    size,
                    tiny,
                    tiny_height,
                    &mut links,
                    input,
                )
            }
            None if node.shape == Shape::Note => {
                let r = w / 2.0;
                let t = -h / 2.0;
                let _ = write!(
                    nodes_out,
                    "<g class=\"pele-node pele-note\" data-id=\"{id}\"{at}><path d=\"M{},{}H{}L{},{}V{}H{}Z\" fill=\"var(--_a)\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"/><path d=\"M{},{}V{}H{}\" fill=\"none\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"/>{}</g>",
                    num(-r),
                    num(t),
                    num(r - FOLD),
                    num(r),
                    num(t + FOLD),
                    num(-t),
                    num(-r),
                    num(r - FOLD),
                    num(t),
                    num(t + FOLD),
                    num(r),
                    label_svg(
                        &view.label,
                        0.0,
                        0.0,
                        &format!(" class=\"pele-label\" font-size=\"{}\"", num(small))
                    )
                );
            }
            None => {
                let _ = write!(
                    nodes_out,
                    "<g class=\"pele-node pele-interface\" data-id=\"{id}\"{at}>{}</g>",
                    label_svg(
                        &view.label,
                        0.0,
                        0.0,
                        &format!(" class=\"pele-label\" font-size=\"{}\"", num(small))
                    )
                );
            }
        }
    }

    let mut edges_out = String::new();
    let mut labels_out = String::new();
    for i in 0..cedges.len() {
        let edge = drawn[i];
        let label = &edge_labels[i];
        let start_trim = class_marker_trim(edge.start);
        let end_trim = class_marker_trim(edge.end);
        let ce = &mut cedges[i];
        let path = if ce.src == ce.dst {
            let c = &cnodes[ce.src];
            let view = &views[ce.src];
            let k = loop_at[i] as f64;
            let x = c.x + ox + view.w / 2.0;
            let y = c.y + oy;
            let spread = (view.h / 2.0 - 4.0).min(8.0 + k * 6.0);
            let reach = LOOP + k * 8.0;
            EdgePath {
                d: format!(
                    "M{},{}C{},{} {},{} {},{}",
                    num(x + start_trim),
                    num(y - spread),
                    num(x + reach + start_trim),
                    num(y - spread),
                    num(x + reach + end_trim),
                    num(y + spread),
                    num(x + end_trim),
                    num(y + spread)
                ),
                sx: x,
                sy: y - spread,
                sdx: -1.0,
                sdy: 0.0,
                ex: x,
                ey: y + spread,
                edx: -1.0,
                edy: 0.0,
            }
        } else {
            for k in (0..ce.route.len()).step_by(3) {
                ce.route[k] += ox;
                ce.route[k + 1] += oy;
            }
            route_path(&ce.route, None, start_trim, end_trim, None, None)
        };
        let attrs = match edge.pattern {
            Pattern::Dashed => " stroke-dasharray=\"6 4\"",
            Pattern::Dotted => " stroke-dasharray=\"2 4\"",
            Pattern::Solid => "",
        };
        let _ = write!(
            edges_out,
            "<g class=\"pele-edge{}\" data-id=\"{}\"><path d=\"{}\"{attrs}/>{}{}</g>",
            if edge.pattern == Pattern::Dotted {
                " pele-note-edge"
            } else {
                " pele-relation"
            },
            esc_text(&edge.id),
            path.d,
            class_marker(edge.start, path.sx, path.sy, path.sdx, path.sdy, LINE),
            class_marker(edge.end, path.ex, path.ey, path.edx, path.edy, LINE)
        );
        if label.width > 0.0 {
            labels_out += &edge_label_svg(&edge.id, label, ce.label_x + ox, ce.label_y + oy, "");
        }
    }

    let mut cards_out = String::new();
    for item in &cards {
        if let Some(math) = &item.math {
            cards_out += &label_svg(
                math,
                item.x + ox,
                item.y + oy,
                " class=\"pele-cardinality\"",
            );
            continue;
        }
        let _ = write!(
            cards_out,
            "<text class=\"pele-cardinality\" x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
            num(item.x + ox),
            num(item.y + oy + tiny * 0.35),
            esc(&item.text)
        );
    }

    let mut content = String::new();
    if !clusters.is_empty() {
        let _ = write!(content, "<g class=\"pele-clusters\">{clusters}</g>");
    }
    if !edges_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-edges\" fill=\"none\" stroke=\"{LINE}\" stroke-linecap=\"round\">{edges_out}</g>"
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
            num(small)
        );
    }
    if !cards_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-cardinalities\" font-size=\"{}\" fill=\"var(--_m)\">{cards_out}</g>",
            num(tiny)
        );
    }
    let _ = write!(content, "<g class=\"pele-nodes\">{nodes_out}</g>");

    let titled = with_title(
        db.title.as_deref(),
        measurer,
        size,
        pad,
        width,
        height,
        content,
    );
    let acc = Accessible {
        title: db.acc_title.clone(),
        descr: db.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            DiagramKind::Class,
            titled.width,
            titled.height,
            size,
            options,
            &acc,
            &titled.content,
        ),
        width: titled.width,
        height: titled.height,
        links,
    }
}

#[allow(clippy::too_many_arguments)]
fn class_view<'a>(
    node: &'a GraphNode,
    cls: &'a ClassNode,
    style: ResolvedStyle,
    measurer: &dyn TextMeasurer,
    size: f64,
    tiny: f64,
    tiny_height: f64,
    hide_empty: bool,
    monospaced: bool,
) -> View<'a> {
    let mono = monospaced && !style.text.contains("font-family:");
    let font_size = style.font_size.unwrap_or(size);
    let member_size = round(font_size * 0.875);
    let line_height = round(member_size * 1.5);
    // The generic type is escaped so that `<T>` is not read as a tag.
    let generic = if cls.generic.is_empty() {
        String::new()
    } else {
        format!(
            "&lt;{}&gt;",
            parse_generic_types(&cls.generic)
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        )
    };
    let mut base = Style::BOLD;
    if style.italic {
        base |= Style::ITALIC;
    }
    let label = layout_label(
        Some(&format!("{}{generic}", cls.label)),
        false,
        measurer,
        font_size,
        4000.0,
        base,
    );
    let annotations: Vec<Annotation> = cls
        .annotations
        .iter()
        .map(|a| {
            math_text(&format!("«{}»", trim(a)), measurer, tiny)
                .map(Annotation::Math)
                .unwrap_or_else(|| Annotation::Text(format!("«{}»", trim(&decode_entities(a)))))
        })
        .collect();
    let attributes: Vec<Line> = cls
        .members
        .iter()
        .map(|m| member_line(m, measurer, member_size, mono))
        .collect();
    let methods: Vec<Line> = cls
        .methods
        .iter()
        .map(|m| member_line(m, measurer, member_size, mono))
        .collect();
    let mut inner = label.width;
    let mut annotations_h = 0.0;
    for a in &annotations {
        match a {
            Annotation::Text(text) => {
                inner = inner.max(measurer.width(text, tiny, Style::NONE));
                annotations_h += tiny_height;
            }
            Annotation::Math(label) => {
                inner = inner.max(label.width);
                annotations_h += tiny_height.max(label.height);
            }
        }
    }
    for line in attributes.iter().chain(&methods) {
        inner = inner.max(line.width);
    }
    let sections = !(hide_empty && attributes.is_empty() && methods.is_empty());
    let section = |count: usize| {
        if !sections {
            0.0
        } else if count > 0 {
            2.0 * SECTION_PAD + count as f64 * line_height
        } else {
            EMPTY_SECTION
        }
    };
    let title_h = 2.0 * TITLE_PAD + annotations_h + label.height.max(round(font_size * 1.5));
    let attributes_h = section(attributes.len());
    let h = title_h + attributes_h + section(methods.len());
    View {
        node,
        class: Some(cls),
        style,
        label,
        w: (inner + 2.0 * PAD_X).max(MIN_WIDTH),
        h,
        annotations,
        attributes,
        methods,
        title_h,
        attributes_h,
        font_size,
    }
}

#[allow(clippy::too_many_arguments)]
fn class_svg(
    view: &View<'_>,
    cls: &ClassNode,
    id: &str,
    at: &str,
    size: f64,
    tiny: f64,
    tiny_height: f64,
    links: &mut Vec<crate::Link>,
    input: &Input<'_>,
) -> String {
    let (w, h, style) = (view.w, view.h, &view.style);
    let ink = crate::svg::ink::ink_fill(style, None);
    let muted = crate::svg::ink::ink_fill(style, Some("var(--_m)"));
    let mono = input
        .options
        .monospace
        .contains(&crate::MonospacePart::Members)
        && !style.text.contains("font-family:");
    let family = if mono {
        format!(" font-family=\"{}\"", crate::svg::FONT_MONO)
    } else {
        String::new()
    };
    let left = -w / 2.0;
    let top = -h / 2.0;
    let member_size = round(view.font_size * 0.875);
    let line_height = round(member_size * 1.5);
    let mut body = format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{}/>",
        num(left),
        num(top),
        num(w),
        num(h),
        rounded(&style.shape)
    );
    let first = top + view.title_h;
    let second = first + view.attributes_h;
    if h > view.title_h {
        let stroke = style
            .stroke
            .as_deref()
            .map(|s| format!(" style=\"stroke:{}\"", esc(s)))
            .unwrap_or_default();
        let _ = write!(
            body,
            "<path class=\"pele-divider\" d=\"M{},{}H{}M{},{}H{}\" fill=\"none\" stroke=\"var(--_b)\"{stroke}/>",
            num(left),
            num(first),
            num(-left),
            num(left),
            num(second),
            num(-left)
        );
    }
    let mut y = top + TITLE_PAD;
    for annotation in &view.annotations {
        match annotation {
            Annotation::Text(text) => {
                let _ = write!(
                    body,
                    "<text class=\"pele-annotation\" x=\"0\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\"{muted}>{}</text>",
                    num(y + tiny_height / 2.0 + tiny * 0.35),
                    num(tiny),
                    esc(text)
                );
                y += tiny_height;
            }
            Annotation::Math(label) => {
                let h = tiny_height.max(label.height);
                body += &label_svg(
                    label,
                    0.0,
                    y + h / 2.0,
                    &format!(
                        " class=\"pele-annotation\" font-size=\"{}\"{muted}",
                        num(tiny)
                    ),
                );
                y += h;
            }
        }
    }
    let title_size = if view.font_size == size {
        String::new()
    } else {
        format!(" font-size=\"{}\"", num(view.font_size))
    };
    body += &label_svg(
        &view.label,
        0.0,
        (y + first - TITLE_PAD) / 2.0,
        &format!(
            " class=\"pele-label\"{title_size} font-weight=\"var(--_hw)\"{ink}{}",
            style.text
        ),
    );
    let lines = |list: &[Line], kind: &str, from: f64| {
        let mut out = String::new();
        let mut baseline = from + SECTION_PAD + line_height / 2.0 + member_size * 0.35;
        for line in list {
            let mark = match line.classifier {
                Some('$') => {
                    if style.text.is_empty() {
                        " pele-static\" style=\"text-decoration:underline\"".to_string()
                    } else {
                        format!(
                            " pele-static\"{}text-decoration:underline\"",
                            &style.text[..style.text.len() - 1]
                        )
                    }
                }
                Some('*') => format!(" pele-abstract\" font-style=\"italic\"{}", style.text),
                _ => format!("\"{}", style.text),
            };
            let _ = write!(
                out,
                "<text class=\"pele-member pele-{kind}{mark} x=\"{}\" y=\"{}\">{}</text>",
                num(left + PAD_X),
                num(baseline),
                esc(&line.text)
            );
            baseline += line_height;
        }
        out
    };
    if !view.attributes.is_empty() || !view.methods.is_empty() {
        let _ = write!(
            body,
            "<g class=\"pele-members\" font-size=\"{}\"{ink}{family}>{}{}</g>",
            num(member_size),
            lines(&view.attributes, "attribute", first),
            lines(&view.methods, "method", second)
        );
    }
    if let Some(tooltip) = cls.tooltip.as_deref().filter(|t| !t.is_empty()) {
        body = format!("<title>{}</title>{body}", esc_text(tooltip));
    }
    if let Some(link) = cls.link.as_deref().filter(|l| !l.is_empty()) {
        let target = cls
            .link_target
            .as_deref()
            .filter(|t| TARGETS.contains(t))
            .map(|t| format!(" target=\"{t}\""))
            .unwrap_or_default();
        body = linked(&body, link, &cls.id, links, input.options, &target);
    }
    let classes =
        cls.css_classes
            .strip_prefix("default")
            .map_or(cls.css_classes.as_str(), |rest| {
                rest.strip_prefix(|c: char| crate::util::is_space(c))
                    .unwrap_or(rest)
            });
    format!(
        "<g class=\"pele-node pele-class{}\" data-id=\"{id}\"{at}>{body}</g>",
        class_names(classes)
    )
}
