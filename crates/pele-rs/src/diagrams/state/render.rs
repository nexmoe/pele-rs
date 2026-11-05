//! State diagram layout and SVG. Ported from pele's `diagrams/state/render.ts`.

use std::collections::HashMap;
use std::fmt::Write;

use super::db::StateDb;
use super::graph::StateNode;
use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{runs_across, turn_to_fit};
use crate::layout::compound::{CEdge, CNode, Dir, cedge, cnode, compound_layout, direction};
use crate::layout::layered::LayeredOptions;
use crate::preprocess::{Config, Value};
use crate::svg::edges::{EdgePath, SideReach, edge_label_svg, marker, marker_trim, route_path};
use crate::svg::shapes::end_side;
use crate::svg::{
    Accessible, ResolvedStyle, class_names, esc, esc_text, label_svg, num, resolve_style,
    svg_document, with_title,
};
use crate::text::Style;
use crate::text::entities::decode_entities;
use crate::text::label::{Label, layout_label};
use crate::url::{linked, safe_url};
use crate::util::round;

const NODE_SEP: f64 = 40.0;
const EDGE_SEP: f64 = 16.0;
const RANK_SEP: f64 = 44.0;
const GROUP_PAD: f64 = 16.0;
const REGION_PAD: f64 = 10.0;
const PAD_X: f64 = 16.0;
const PAD_Y: f64 = 10.0;
const LOOP: f64 = 26.0;
const FOLD: f64 = 8.0;
const NOTE_WRAP: f64 = 320.0;
const CORNER: f64 = 10.0;
const BAR: f64 = 8.0;
const SURFACE: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";
const RULE: &str = " fill=\"none\" stroke=\"var(--_b)\"";
/// The classes Mermaid gives states for its own styling, left out of the drawing.
const MERMAID_CLASSES: [&str; 4] = [
    "statediagram-state",
    "statediagram-cluster-alt",
    "statediagram-cluster",
    "statediagram-note",
];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum K {
    State,
    Described,
    Start,
    End,
    Choice,
    Fork,
    Join,
    Note,
    Composite,
    Region,
    Hidden,
}

impl K {
    fn of_shape(shape: Option<&str>) -> K {
        match shape {
            Some("rectWithTitle") => K::Described,
            Some("stateStart") => K::Start,
            Some("stateEnd") => K::End,
            Some("choice") => K::Choice,
            Some("fork") => K::Fork,
            Some("join") => K::Join,
            Some("note") => K::Note,
            Some("divider") => K::Hidden,
            _ => K::State,
        }
    }

    fn class(self) -> &'static str {
        match self {
            K::State | K::Described => "pele-node pele-state",
            K::Start => "pele-node pele-state-start",
            K::End => "pele-node pele-state-end",
            K::Choice => "pele-node pele-state-choice",
            K::Fork => "pele-node pele-state-fork",
            K::Join => "pele-node pele-state-join",
            K::Note => "pele-note",
            _ => "pele-cluster pele-state-composite",
        }
    }
}

struct View<'a> {
    /// Absent for the unseen helpers the layout needs.
    node: Option<&'a StateNode>,
    kind: K,
    style: Option<ResolvedStyle>,
    label: Label,
    lines: Vec<Label>,
    w: f64,
    h: f64,
    title_h: f64,
}

fn vertical(dir: Option<Dir>) -> bool {
    dir.is_some_and(Dir::vertical)
}

fn number_option(section: &Config, key: &str, fallback: f64) -> f64 {
    match section.get(key) {
        Some(&Value::Number(v)) if v > 0.0 => v.min(2000.0),
        _ => fallback,
    }
}

/// Mermaid keeps the quotes of a click statement's strings and strips them when it draws.
fn unquote(text: &str) -> &str {
    text.trim_matches('"')
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

fn rect(x: f64, y: f64, w: f64, h: f64, rest: &str) -> String {
    format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rest}/>",
        num(x),
        num(y),
        num(w),
        num(h)
    )
}

fn group(dir: Dir, pad_x: f64, pad_top: f64, pad_bottom: f64) -> CNode {
    let mut c = cnode(0.0, 0.0);
    c.is_group = true;
    c.dir = Some(dir);
    c.pad_x = pad_x;
    c.pad_top = pad_top;
    c.pad_bottom = pad_bottom;
    c
}

fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn or_one(x: f64) -> f64 {
    if x == 0.0 || x.is_nan() { 1.0 } else { x }
}

fn new_view(node: Option<&StateNode>, kind: K) -> View<'_> {
    View {
        node,
        kind,
        style: None,
        label: Label::default(),
        lines: Vec::new(),
        w: 0.0,
        h: 0.0,
        title_h: 0.0,
    }
}

fn node_of<'a>(views: &[View<'a>], i: usize) -> &'a StateNode {
    views[i].node.expect("state")
}

/// As in Mermaid, a composite that sets no direction runs top to bottom whatever surrounds it.
fn inner_dir(views: &[View<'_>], composite: usize) -> Dir {
    direction(node_of(views, composite).dir.as_deref())
}

/// Whether the flow around a state runs top to bottom or bottom to top.
fn across(views: &[View<'_>], logical: &[i64], root_dir: Dir, i: usize) -> bool {
    if logical[i] == -1 {
        root_dir
    } else {
        inner_dir(views, logical[i] as usize)
    }
    .vertical()
}

pub(crate) fn render_state(db: &StateDb, input: &Input<'_>) -> Output {
    if !runs_across(Some(db.direction())) {
        return draw(db, input, false);
    }
    turn_to_fit(input.options, |down| draw(db, input, down))
}

/// `turned` draws a diagram that runs across as one that runs down.
fn draw(db: &StateDb, input: &Input<'_>, turned: bool) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let graph = &db.graph;
    let empty = Config::new();
    let section = |name: &str| match input.config.get(name) {
        Some(Value::Map(m)) => m,
        _ => &empty,
    };
    let state_config = section("state");
    // State diagrams follow the flowchart curve.
    let curve = match section("flowchart").get("curve") {
        Some(Value::String(s)) => Some(s.as_str()),
        _ => None,
    };
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let wrap_width = number_option(state_config, "wrappingWidth", 200.0);
    let pad = options.padding.unwrap_or(8.0);
    let root_dir = if turned {
        Dir::TB
    } else {
        direction(Some(db.direction()))
    };
    let text = |raw: Option<&str>, font_size: f64, wrap: f64, style: Style| {
        layout_label(raw, true, measurer, font_size, wrap, style)
    };

    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut views: Vec<View<'_>> = Vec::new();
    let mut cnodes: Vec<CNode> = Vec::new();
    for node in &graph.nodes {
        if node.shape.as_deref() == Some("noteGroup") {
            continue;
        }
        let kind = if node.is_group {
            if node.shape.as_deref() == Some("divider") {
                K::Region
            } else {
                K::Composite
            }
        } else {
            K::of_shape(node.shape.as_deref())
        };
        index.insert(node.id.as_str(), views.len());
        views.push(new_view(Some(node), kind));
    }
    let states = views.len();

    let mut logical: Vec<i64> = (0..states)
        .map(|i| {
            let parent = node_of(&views, i)
                .parent_id
                .as_deref()
                .and_then(|p| index.get(p).copied());
            match parent {
                Some(p) if p != i && node_of(&views, p).is_group => p as i64,
                _ => -1,
            }
        })
        .collect();
    // Composites that are redeclared inside each other form a ring with no way in from the
    // root. One member of each ring moves to the root, which leaves a tree.
    {
        let mut kids: Vec<Vec<usize>> = vec![Vec::new(); states];
        let mut queue: Vec<usize> = Vec::new();
        let mut placed = vec![false; states];
        for i in 0..states {
            if logical[i] == -1 {
                placed[i] = true;
                queue.push(i);
            } else {
                kids[logical[i] as usize].push(i);
            }
        }
        let mut done = 0;
        let settle = |queue: &mut Vec<usize>, placed: &mut Vec<bool>, done: &mut usize| {
            while *done < queue.len() {
                for &kid in &kids[queue[*done]] {
                    if !placed[kid] {
                        placed[kid] = true;
                        queue.push(kid);
                    }
                }
                *done += 1;
            }
        };
        settle(&mut queue, &mut placed, &mut done);
        let mut seen = vec![usize::MAX; states];
        for i in 0..states {
            if placed[i] {
                continue;
            }
            let mut ring = i;
            while seen[ring] != i {
                seen[ring] = i;
                ring = logical[ring] as usize;
            }
            logical[ring] = -1;
            placed[ring] = true;
            queue.push(ring);
            settle(&mut queue, &mut placed, &mut done);
        }
    }
    let mut owners: Vec<i64> = vec![-1; states];
    for i in 0..states {
        let owner = node_of(&views, i)
            .note_for
            .as_deref()
            .and_then(|n| index.get(n).copied());
        if let Some(owner) = owner {
            owners[i] = owner as i64;
            logical[i] = logical[owner];
        }
    }

    let across =
        |views: &[View<'_>], logical: &[i64], i: usize| across(views, logical, root_dir, i);

    let mut degree = vec![0i64; states];
    let mut noted = vec![false; states];
    // When each state first takes part in a transition. Layout follows this order rather than
    // the order of declaration, so a state declared up front does not become the top of the
    // flow.
    let mut appear: Vec<f64> = vec![f64::INFINITY; states];
    let mut appeared = 0.0;
    let mut counts = vec![0i64; 2 * states];
    for edge in &graph.edges {
        let (Some(&src), Some(&dst)) =
            (index.get(edge.start.as_str()), index.get(edge.end.as_str()))
        else {
            continue;
        };
        if edge.dashed {
            noted[if views[src].kind == K::Note { dst } else { src }] = true;
            continue;
        }
        counts[2 * src] += 1;
        degree[src] = degree[src].max(counts[2 * src]);
        counts[2 * dst + 1] += 1;
        degree[dst] = degree[dst].max(counts[2 * dst + 1]);
        if appear[src] == f64::INFINITY {
            appear[src] = appeared;
            appeared += 1.0;
        }
        if appear[dst] == f64::INFINITY {
            appear[dst] = appeared;
            appeared += 1.0;
        }
    }

    for i in 0..states {
        let node = node_of(&views, i);
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
        let font_size = style.font_size.unwrap_or(size);
        let is_across = across(&views, &logical, i);
        let mut c: Option<CNode> = None;
        let kind = views[i].kind;
        match kind {
            K::Composite => {
                let label = text(node.label.as_deref(), font_size, 4000.0, text_style);
                let title_h = if label.height > 0.0 {
                    label.height + 12.0
                } else {
                    0.0
                };
                let mut g = group(
                    inner_dir(&views, i),
                    GROUP_PAD,
                    title_h + GROUP_PAD,
                    GROUP_PAD,
                );
                g.min_w = label.width + 2.0 * GROUP_PAD;
                c = Some(g);
                let view = &mut views[i];
                view.label = label;
                view.title_h = title_h;
            }
            K::Region => {
                c = Some(group(
                    inner_dir(&views, i),
                    REGION_PAD,
                    REGION_PAD,
                    REGION_PAD,
                ));
            }
            K::Start => (views[i].w, views[i].h) = (14.0, 14.0),
            K::End => (views[i].w, views[i].h) = (18.0, 18.0),
            K::Choice => (views[i].w, views[i].h) = (30.0, 30.0),
            K::Fork | K::Join => {
                let length = clamp(40.0 * degree[i] as f64 + 16.0, 72.0, 320.0);
                views[i].w = if is_across { length } else { BAR };
                views[i].h = if is_across { BAR } else { length };
            }
            K::Hidden => {}
            K::Note => {
                let label = text(
                    node.label.as_deref(),
                    small,
                    wrap_width.max(NOTE_WRAP),
                    Style::NONE,
                );
                let view = &mut views[i];
                view.w = label.width + 24.0;
                view.h = label.height + 16.0;
                view.label = label;
            }
            K::State | K::Described => {
                let label = text(node.label.as_deref(), font_size, wrap_width, text_style);
                let mut width = label.width;
                let mut height = label.height + 2.0 * PAD_Y;
                let mut lines = Vec::new();
                if kind == K::Described {
                    height += 14.0;
                    for line in &node.description {
                        let raw = if line.is_empty() { " " } else { line.as_str() };
                        let label = text(Some(raw), small, wrap_width, text_style);
                        if label.width > width {
                            width = label.width;
                        }
                        height += label.height;
                        lines.push(label);
                    }
                }
                let view = &mut views[i];
                view.label = label;
                view.lines = lines;
                view.w = (width + 2.0 * PAD_X).max(56.0);
                view.h = height;
            }
        }
        views[i].style = Some(style);
        let mut c = c.unwrap_or_else(|| cnode(views[i].w, views[i].h));
        // The transitions on one side of a state's box spread along it, clear of its corners.
        if matches!(kind, K::State | K::Described) {
            c.span = (if is_across { views[i].w } else { views[i].h }) - 24.0;
        }
        cnodes.push(c);
    }

    let mut cedges: Vec<CEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut drawn = Vec::new();
    // The transitions each state makes to itself: how many, their widest label, and their
    // labels' height.
    let mut loop_count = vec![0usize; states];
    let mut loop_width = vec![0f64; states];
    let mut loop_height = vec![0f64; states];
    // A transition between a composite and a state inside it runs between that state and a
    // point just inside the composite, and is then drawn on to the composite's border.
    let mut anchors: HashMap<usize, usize> = HashMap::new();
    let encloses = |cnodes: &[CNode], logical: &[i64], composite: usize, item: usize| {
        if !cnodes[composite].is_group {
            return false;
        }
        let mut p = logical[item];
        while p != -1 {
            if p as usize == composite {
                return true;
            }
            p = logical[p as usize];
        }
        false
    };
    for edge in &graph.edges {
        let (Some(&src0), Some(&dst0)) =
            (index.get(edge.start.as_str()), index.get(edge.end.as_str()))
        else {
            continue;
        };
        let (mut src, mut dst) = (src0, dst0);
        let label = text(Some(&edge.label), small, wrap_width, Style::NONE);
        let label_w = if label.width > 0.0 {
            label.width + 12.0
        } else {
            0.0
        };
        let mut anchor =
            |composite: usize, first: bool, views: &mut Vec<View<'_>>, cnodes: &mut Vec<CNode>| {
                anchors.insert(cedges.len(), composite);
                appear.push(if first { -1.0 } else { f64::INFINITY });
                let mut c = cnode(0.0, 0.0);
                c.parent = composite as i64;
                cnodes.push(c);
                views.push(new_view(None, K::Hidden));
                views.len() - 1
            };
        if src == dst {
            loop_count[src] += 1;
            loop_width[src] = loop_width[src].max(label_w);
            loop_height[src] += label.height;
        } else if encloses(&cnodes, &logical, src, dst) {
            src = anchor(src, true, &mut views, &mut cnodes);
        } else if encloses(&cnodes, &logical, dst, src) {
            dst = anchor(dst, false, &mut views, &mut cnodes);
        }
        drawn.push(edge);
        cedges.push(cedge(
            src,
            dst,
            1,
            label_w,
            if label.height > 0.0 {
                label.height + 4.0
            } else {
                0.0
            },
        ));
        edge_labels.push(label);
    }

    // A state with notes, or a composite with self-transitions, sits in an unseen group of its
    // own: the notes go beside the state, across the flow, without disturbing its transitions.
    for i in 0..states {
        if owners[i] != -1 {
            continue;
        }
        let is_across = across(&views, &logical, i);
        // Loops hang off the right, or below when the flow is horizontal, with their labels
        // beyond them.
        let mut room = 0.0;
        if loop_count[i] > 0 {
            let reach = LOOP + (loop_count[i] - 1) as f64 * 8.0;
            room = if is_across {
                reach + loop_width[i]
            } else {
                reach * 0.75 + 4.0 + loop_height[i]
            };
            let item = &mut cnodes[i];
            if !item.is_group {
                if is_across {
                    item.w += 2.0 * room;
                    item.h = item.h.max(loop_height[i]);
                } else {
                    item.h += 2.0 * room;
                    item.w = item.w.max(loop_width[i]);
                }
                room = 0.0;
            }
        }
        if !noted[i] && room == 0.0 {
            cnodes[i].parent = logical[i];
            continue;
        }
        let mut c = if is_across {
            group(Dir::LR, room, 0.0, 0.0)
        } else {
            group(Dir::TB, 0.0, room, room)
        };
        c.parent = logical[i];
        appear.push(appear[i]);
        cnodes.push(c);
        views.push(new_view(None, K::Hidden));
        cnodes[i].parent = (cnodes.len() - 1) as i64;
    }
    for i in 0..states {
        if owners[i] != -1 {
            cnodes[i].parent = cnodes[owners[i] as usize].parent;
        }
    }

    {
        // Numbers the tree so that, at every level, initial states come first and the rest
        // follow in the order the transitions introduce them.
        let mut members: Vec<Vec<usize>> = vec![Vec::new(); cnodes.len()];
        let mut top: Vec<usize> = Vec::new();
        for (i, c) in cnodes.iter().enumerate() {
            if c.parent >= 0 {
                members[c.parent as usize].push(i);
            } else {
                top.push(i);
            }
        }
        let initial = |i: usize| if views[i].kind == K::Start { 0 } else { 1 };
        let by_flow = |a: &usize, b: &usize| {
            initial(*a)
                .cmp(&initial(*b))
                .then_with(|| {
                    appear[*a]
                        .partial_cmp(&appear[*b])
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then(a.cmp(b))
        };
        let mut seq = 0.0;
        top.sort_by(by_flow);
        top.reverse();
        let mut pending = top;
        while let Some(i) = pending.pop() {
            cnodes[i].seq = seq;
            seq += 1.0;
            let mut inside = std::mem::take(&mut members[i]);
            inside.sort_by(by_flow);
            pending.extend(inside.into_iter().rev());
        }
    }

    let layout = compound_layout(
        &mut cnodes,
        &mut cedges,
        root_dir,
        &LayeredOptions {
            node_sep: number_option(state_config, "nodeSpacing", NODE_SEP),
            edge_sep: EDGE_SEP,
            rank_sep: number_option(state_config, "rankSpacing", RANK_SEP),
            port_sep: 20.0,
            tracks: matches!(curve, Some("step" | "stepBefore" | "stepAfter")),
            straight: curve == Some("linear"),
            head_room: marker_trim("arrow_point"),
            ..LayeredOptions::default()
        },
    );

    let width = (layout.width + 2.0 * pad).ceil();
    let height = (layout.height + 2.0 * pad).ceil();
    let mut links = Vec::new();

    // Route points by rounded y, to find the edges that cross a composite's title bar.
    let mut crossings: HashMap<i64, Vec<f64>> = HashMap::new();
    for e in &cedges {
        let route = &e.route;
        let mut k = 3;
        while k + 3 < route.len() {
            crossings
                .entry(round(route[k + 1]) as i64)
                .or_default()
                .push(route[k]);
            k += 3;
        }
    }

    let mut children: Vec<Vec<usize>> = vec![Vec::new(); cnodes.len()];
    let mut order: Vec<usize> = Vec::new();
    for (i, c) in cnodes.iter().enumerate() {
        if c.parent >= 0 {
            children[c.parent as usize].push(i);
        } else {
            order.push(i);
        }
    }
    let roots = order.clone();
    let mut q = 0;
    while q < order.len() {
        let kids = children[order[q]].clone();
        order.extend(kids);
        q += 1;
    }

    let mut clusters = String::new();
    let mut dividers = String::new();
    let mut nodes_out = String::new();
    let mut notes_out = String::new();
    // Draws the dashed lines between the regions among a composite's members.
    let divide =
        |dividers: &mut String, items: &[usize], left: f64, top: f64, right: f64, bottom: f64| {
            let regions: Vec<usize> = items
                .iter()
                .copied()
                .filter(|&i| views[i].kind == K::Region)
                .collect();
            if regions.len() < 2 {
                return;
            }
            let lo = |i: usize, x: bool| {
                if x {
                    cnodes[i].x - cnodes[i].w / 2.0
                } else {
                    cnodes[i].y - cnodes[i].h / 2.0
                }
            };
            let hi = |i: usize, x: bool| {
                if x {
                    cnodes[i].x + cnodes[i].w / 2.0
                } else {
                    cnodes[i].y + cnodes[i].h / 2.0
                }
            };
            for x in [true, false] {
                let mut sorted = regions.clone();
                sorted.sort_by(|&a, &b| {
                    (lo(a, x) - lo(b, x))
                        .partial_cmp(&0.0)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(a.cmp(&b))
                });
                if sorted.windows(2).any(|w| hi(w[0], x) > lo(w[1], x) + 0.5) {
                    continue;
                }
                for w in sorted.windows(2) {
                    let at = num((hi(w[0], x) + lo(w[1], x)) / 2.0 + pad);
                    if x {
                        let _ = write!(
                            dividers,
                            "<path class=\"pele-divider\" d=\"M{at},{}V{}\"/>",
                            num(top),
                            num(bottom)
                        );
                    } else {
                        let _ = write!(
                            dividers,
                            "<path class=\"pele-divider\" d=\"M{},{at}H{}\"/>",
                            num(left),
                            num(right)
                        );
                    }
                }
                return;
            }
            // Regions that sit neither side by side nor in a stack each get an outline.
            for &i in &regions {
                let c = &cnodes[i];
                dividers.push_str(&rect(
                    c.x - c.w / 2.0 + pad,
                    c.y - c.h / 2.0 + pad,
                    c.w,
                    c.h,
                    &format!(" class=\"pele-divider\" rx=\"{}\"", num(CORNER)),
                ));
            }
        };
    divide(
        &mut dividers,
        &roots,
        pad / 2.0,
        pad / 2.0,
        width - pad / 2.0,
        height - pad / 2.0,
    );

    let mut fills = Vec::new();
    for &i in &order {
        let view = &views[i];
        let (Some(node), Some(style)) = (view.node, view.style.as_ref()) else {
            continue;
        };
        if view.kind >= K::Region {
            continue;
        }
        let c = &cnodes[i];
        let x = c.x + pad;
        let y = c.y + pad;
        let (w, h) = (view.w, view.h);
        let ink = crate::svg::ink::ink_fill(style, None);
        let label_attrs = format!(" class=\"pele-label\"{ink}{}", style.text);
        let mut body;
        match view.kind {
            K::Composite => {
                let left = x - c.w / 2.0;
                let top = y - c.h / 2.0;
                if crate::svg::ink::paints(&style.shape) {
                    fills.push(crate::svg::fills::Fill {
                        bounds: [left, top, left + c.w, top + c.h],
                        shape: style.shape.clone(),
                    });
                }
                body = rect(
                    left,
                    top,
                    c.w,
                    c.h,
                    &format!(
                        " rx=\"{}\" fill=\"var(--_a)\"{} stroke=\"var(--_b)\"{}",
                        num(CORNER),
                        if crate::svg::sets_fill(&style.shape) {
                            ""
                        } else {
                            " fill-opacity=\"0.5\""
                        },
                        style.shape
                    ),
                );
                if view.title_h > 0.0 {
                    let _ = write!(
                        body,
                        "<path d=\"M{},{}H{}\"{RULE}{}/>",
                        num(left),
                        num(top + view.title_h),
                        num(left + c.w),
                        style.line
                    );
                    body += &label_svg(
                        &view.label,
                        title_x(c, view.label.width, &crossings) + pad,
                        top + view.title_h / 2.0,
                        &format!(" class=\"pele-cluster-label\"{ink}{}", style.text),
                    );
                }
                divide(
                    &mut dividers,
                    &children[i],
                    left,
                    top + view.title_h,
                    left + c.w,
                    top + c.h,
                );
            }
            K::Start => body = format!("<circle r=\"7\" fill=\"var(--_l)\"{}/>", style.shape),
            K::End => {
                body = format!(
                    "<circle r=\"8.5\" fill=\"var(--_bg)\" stroke=\"var(--_l)\"{}/><circle r=\"5\" fill=\"var(--_l)\"{}/>",
                    style.line, style.shape
                )
            }
            K::Choice => {
                body = format!(
                    "<polygon points=\"0,{} {},0 0,{} {},0\"{SURFACE}{}/>",
                    num(-h / 2.0),
                    num(w / 2.0),
                    num(h / 2.0),
                    num(-w / 2.0),
                    style.shape
                )
            }
            K::Fork | K::Join => {
                body = rect(
                    -w / 2.0,
                    -h / 2.0,
                    w,
                    h,
                    &format!(" rx=\"2\" fill=\"var(--_l)\"{}", style.shape),
                )
            }
            K::Note => {
                let r = num(w / 2.0);
                let b = num(h / 2.0);
                let fold = num(w / 2.0 - FOLD);
                let corner = num(FOLD - h / 2.0);
                body = format!(
                    "<path d=\"M-{r},-{b}H{fold}L{r},{corner}V{b}H-{r}Z\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"/>\
                     <path d=\"M{fold},-{b}V{corner}H{r}\"{RULE}/>{}",
                    label_svg(
                        &view.label,
                        0.0,
                        0.0,
                        &format!(
                            " class=\"pele-label\" font-size=\"{}\" fill=\"var(--_m)\"",
                            num(small)
                        )
                    )
                );
            }
            _ => {
                body = rect(
                    -w / 2.0,
                    -h / 2.0,
                    w,
                    h,
                    &format!(
                        " rx=\"{}\"{SURFACE}{}",
                        num(CORNER.min(h / 2.0)),
                        style.shape
                    ),
                );
                if view.kind == K::State {
                    body += &label_svg(&view.label, 0.0, 0.0, &label_attrs);
                } else {
                    let mut at = PAD_Y - h / 2.0 + view.label.height;
                    let _ = write!(
                        body,
                        "<path d=\"M{},{}H{}\"{RULE}{}/>",
                        num(-w / 2.0),
                        num(at + 6.0),
                        num(w / 2.0),
                        style.line
                    );
                    body +=
                        &label_svg(&view.label, 0.0, at - view.label.height / 2.0, &label_attrs);
                    at += 14.0;
                    let attrs = format!(
                        " class=\"pele-state-description\" font-size=\"{}\"{}{}",
                        num(small),
                        crate::svg::ink::ink_fill(style, None),
                        style.text
                    );
                    for line in &view.lines {
                        body += &label_svg(line, 0.0, at + line.height / 2.0, &attrs);
                        at += line.height;
                    }
                }
            }
        }

        if let Some(link) = db.links.get(&node.id) {
            // Entity codes are resolved first, so the URL is checked in the form a browser
            // would follow.
            let tooltip = decode_entities(unquote(&link.tooltip));
            if !tooltip.is_empty() {
                body = format!("<title>{}</title>{body}", esc(&tooltip));
            }
            body = linked(
                &body,
                &safe_url(unquote(&link.url)),
                &node.id,
                &mut links,
                options,
                " target=\"_blank\"",
            );
        }
        let own: Vec<&str> = node
            .css_classes
            .split(' ')
            .filter(|c| !MERMAID_CLASSES.contains(c))
            .collect();
        let open = format!(
            "<g class=\"{}{}\" data-id=\"{}\"",
            view.kind.class(),
            class_names(&own.join(" ")),
            esc_text(&node.id)
        );
        match view.kind {
            K::Composite => {
                let _ = write!(clusters, "{open}>{body}</g>");
            }
            K::Note => {
                let _ = write!(
                    notes_out,
                    "{open} transform=\"translate({},{})\">{body}</g>",
                    num(x),
                    num(y)
                );
            }
            _ => {
                let _ = write!(
                    nodes_out,
                    "{open} transform=\"translate({},{})\">{body}</g>",
                    num(x),
                    num(y)
                );
            }
        }
    }

    // Moves a route end from the layout box onto the drawn shape, and spreads the ends along a
    // bar.
    let attach = |route: &mut [f64], at: usize, i: usize| {
        let c = &cnodes[i];
        if c.is_group {
            return;
        }
        let view = &views[i];
        let bar = matches!(view.kind, K::Fork | K::Join);
        let other = if at == 0 { 3 } else { at - 3 };
        // 0 is the x axis and 1 the y axis; the end sits on a side that faces along `axis`.
        for axis in [1usize, 0] {
            let (extent, drawn_extent, breadth) = if axis == 1 {
                (c.h, view.h, view.w)
            } else {
                (c.w, view.w, view.h)
            };
            let center = (if axis == 1 { c.y } else { c.x }) + pad;
            let cross = (if axis == 1 { c.x } else { c.y }) + pad;
            let delta = route[at + axis] - center;
            if (delta.abs() - extent / 2.0).abs() >= 0.5 {
                continue;
            }
            route[at + axis] -= sign(delta) * (extent - drawn_extent) / 2.0;
            if bar && breadth > drawn_extent {
                route[at + 1 - axis] = clamp(
                    route[other + 1 - axis],
                    cross - breadth / 2.0 + 6.0,
                    cross + breadth / 2.0 - 6.0,
                );
            }
            return;
        }
    };

    let mut reach = SideReach::default();
    for (i, ce) in cedges.iter().enumerate() {
        if ce.src == ce.dst || ce.route.len() < 6 || drawn[i].dashed {
            continue;
        }
        reach.add(
            ce.dst,
            end_side(&ce.route, ce.route.len() - 3, &cnodes[ce.dst]),
            "arrow_point",
        );
    }

    let mut casings = String::new();
    let mut edges_out = String::new();
    let mut labels_out = String::new();
    let mut loop_seen = vec![0usize; states];
    let mut loop_text = vec![0f64; states];
    for i in 0..cedges.len() {
        let edge = drawn[i];
        let label = &edge_labels[i];
        let end_type = if edge.dashed { "none" } else { "arrow_point" };
        let path: EdgePath;
        let (label_x, label_y);
        let ce = &cedges[i];
        if ce.src == ce.dst {
            let s = ce.src;
            let a = &cnodes[s];
            let view = &views[s];
            let k = loop_seen[s];
            loop_seen[s] += 1;
            // The loop hangs off a side the flow does not use: the right, or below when the
            // flow is horizontal.
            let side = across(&views, &logical, s);
            let w = if a.is_group { a.w } else { view.w };
            let h = if a.is_group { a.h } else { view.h };
            let out = (if side { w } else { h }) / 2.0;
            let spread = ((if side { h } else { w }) / 2.0 - 4.0)
                .min(8.0 + k as f64 * 6.0)
                .max(2.0);
            let reach = LOOP + k as f64 * 8.0;
            let trim = marker_trim(end_type) / or_one(reach.hypot(spread));
            // Offsets are given for a loop on the right and turned a quarter for one below.
            let px = |dx: f64, dy: f64| a.x + pad + if side { out + dx } else { -dy };
            let py = |dx: f64, dy: f64| a.y + pad + if side { dy } else { out + dx };
            let at = |dx: f64, dy: f64| format!("{},{}", num(px(dx, dy)), num(py(dx, dy)));
            let mut edx = px(-reach, -spread) - px(0.0, 0.0);
            let mut edy = py(-reach, -spread) - py(0.0, 0.0);
            let len = or_one(edx.hypot(edy));
            edx /= len;
            edy /= len;
            path = EdgePath {
                d: format!(
                    "M{}C{} {} {}",
                    at(0.0, -spread),
                    at(reach, -2.0 * spread),
                    at(reach, 2.0 * spread),
                    at(reach * trim, spread + spread * trim)
                ),
                sx: 0.0,
                sy: 0.0,
                sdx: 0.0,
                sdy: 0.0,
                ex: px(0.0, spread),
                ey: py(0.0, spread),
                edx,
                edy,
            };
            // Labels stack beyond the outermost loop, in the order of their transitions.
            let furthest = LOOP + (loop_count[s] - 1) as f64 * 8.0;
            let below = loop_text[s] + label.height / 2.0;
            loop_text[s] += label.height;
            label_x = if side {
                a.x + out + furthest + 4.0 + ce.label_w / 2.0
            } else {
                a.x
            };
            label_y = if side {
                a.y - loop_height[s] / 2.0 + below
            } else {
                a.y + out + furthest * 0.75 + 4.0 + below
            };
        } else {
            let mut route = ce.route.clone();
            let leaves = reach.of(ce.src, end_side(&route, 0, &cnodes[ce.src]));
            let arrives = reach.of(ce.dst, end_side(&route, route.len() - 3, &cnodes[ce.dst]));
            for p in route.chunks_mut(3) {
                p[0] += pad;
                p[1] += pad;
            }
            attach(&mut route, 0, ce.src);
            let last = route.len() - 3;
            attach(&mut route, last, ce.dst);
            if let Some(&composite) = anchors.get(&i).filter(|_| route.len() >= 6) {
                let g = &cnodes[composite];
                let inward = views[ce.src].kind == K::Hidden;
                let at = if inward { 0 } else { route.len() - 3 };
                let near = matches!(g.dir, Some(Dir::TB | Dir::LR)) == inward;
                let point = if vertical(g.dir) {
                    [
                        route[at],
                        g.y + pad
                            + if near {
                                views[composite].title_h - g.h / 2.0
                            } else {
                                g.h / 2.0
                            },
                        0.0,
                    ]
                } else {
                    [
                        g.x + pad + if near { -g.w / 2.0 } else { g.w / 2.0 },
                        route[at + 1],
                        1.0,
                    ]
                };
                if inward {
                    route.splice(0..0, point);
                } else {
                    route.extend(point);
                }
            }
            path = route_path(
                &route,
                curve,
                0.0,
                marker_trim(end_type),
                Some(leaves),
                Some(arrives),
            );
            label_x = ce.label_x;
            label_y = ce.label_y;
        }
        let over = crate::svg::fills::over_fills(&path.d, &[path.ex, path.ey], &fills, "");
        if over.as_deref() == Some("") {
            casings += &crate::svg::fills::edge_casing(&path, "none", end_type, "", false);
        }
        let color = over
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("var(--_l)");
        let class = if edge.dashed {
            format!(
                "pele-note-edge\" stroke=\"{}\" stroke-dasharray=\"3 4",
                over.as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("var(--_b)")
            )
        } else if color != "var(--_l)" {
            format!("pele-transition\" stroke=\"{color}")
        } else {
            "pele-transition".to_string()
        };
        let _ = write!(
            edges_out,
            "<g class=\"pele-edge {class}\" data-id=\"{}\"><path d=\"{}\"/>{}</g>",
            esc_text(&edge.id),
            path.d,
            marker(end_type, path.ex, path.ey, path.edx, path.edy, color)
        );
        if label.width > 0.0 {
            labels_out += &edge_label_svg(&edge.id, label, label_x + pad, label_y + pad, "");
        }
    }

    let mut content = String::new();
    if !clusters.is_empty() {
        let _ = write!(content, "<g class=\"pele-clusters\">{clusters}</g>");
    }
    if !dividers.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-dividers\" stroke-dasharray=\"4 4\"{RULE}>{dividers}</g>"
        );
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
            num(small)
        );
    }
    let _ = write!(
        content,
        "<g class=\"pele-nodes\">{nodes_out}{notes_out}</g>"
    );
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
            DiagramKind::State,
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

/// Picks where a composite's title sits in its title bar so that edges entering from above do
/// not cross it.
fn title_x(c: &CNode, width: f64, crossings: &HashMap<i64, Vec<f64>>) -> f64 {
    let Some(xs) = crossings.get(&(round(c.y - c.h / 2.0) as i64)) else {
        return c.x;
    };
    if width == 0.0 {
        return c.x;
    }
    let left = c.x - c.w / 2.0;
    let half = width / 2.0;
    let mut best = c.x;
    let mut best_gap = -1.0;
    for x in [c.x, left + GROUP_PAD + half, left + c.w - GROUP_PAD - half] {
        let mut gap = f64::INFINITY;
        for &cx in xs {
            if cx > left && cx < left + c.w {
                gap = gap.min((cx - x).abs() - half);
            }
        }
        if gap >= 6.0 {
            return x;
        }
        if gap > best_gap + 0.5 {
            best_gap = gap;
            best = x;
        }
    }
    best
}
