//! The layout of a C4 diagram: Mermaid's grid of shapes and boundaries, as pele's `layout.ts`
//! draws it.

use crate::svg::markers::ARROW;
use crate::text::label::{Label, layout_label};
use crate::text::{Style, TextMeasurer};

use super::db::{Model, Ref, Shape};
use super::route::{Form, Frame, HEAD, LANE, NARROW};

const SHAPE_W: f64 = 216.0;
const SHAPE_MIN_H: f64 = 64.0;
const PAD_X: f64 = 14.0;
const PAD_Y: f64 = 12.0;
// Room between two columns for the label of a line across them, clear of an arrowhead on either
// side, with an arrowhead's length to spare for a word a little wider than the label room.
const GAP_X: f64 = NARROW + super::route::LABEL_PAD + 3.0 * ARROW;
const GAP_Y: f64 = 88.0;
// Room for a relation to run along the inside of a boundary.
pub(crate) const GROUP_PAD: f64 = LANE + 8.0;
const GROUP_GAP: f64 = 48.0;
// Where a boundary's heading starts, from its top-left corner.
pub(crate) const HEAD_LEFT: f64 = 16.0;
pub(crate) const HEAD_TOP: f64 = 10.0;

/// A shape of the diagram, laid out.
#[derive(Debug, Clone)]
pub(crate) struct Node {
    /// The shape in the model this node draws.
    pub shape: usize,
    pub form: Form,
    /// Type, name, technology, description.
    pub labels: Vec<Label>,
    pub text_h: f64,
    pub frame: Frame,
}

/// A boundary of the diagram, laid out.
#[derive(Debug, Clone)]
pub(crate) struct Group {
    /// The boundary in the model this group draws.
    pub boundary: usize,
    pub kids: Vec<usize>,
    pub nodes: Vec<usize>,
    /// Name, type, description.
    pub labels: Vec<Label>,
    pub head: f64,
    pub head_w: f64,
    pub head_x: f64,
    pub frame: Frame,
}

/// The layout of a C4 diagram.
#[derive(Debug, Clone, Default)]
pub(crate) struct Layout {
    /// The boundaries that are drawn, parents before children. The first is the unnamed top
    /// level.
    pub order: Vec<usize>,
    pub nodes: Vec<Node>,
    pub groups: Vec<Group>,
    /// The place of every shape and boundary.
    pub boxes: std::collections::HashMap<Ref, Frame>,
}

/// The shape keywords Mermaid accepts in `$shape`, `$sprite` or `$tags`, as names of the shared
/// shapes. Mermaid 12.1.0's `c4ShapeAdapter.ts` gives them these.
fn form_of(shape: &Shape) -> Form {
    let named = |text: &str| -> Option<Form> {
        match text.to_lowercase().as_str() {
            "person" => Some(Form::Person),
            "box" | "rounded" => Some(Form::Rounded),
            "cylinder" | "database" | "db" => Some(Form::Cyl),
            "queue" | "pipe" => Some(Form::HCyl),
            "component" => Some(Form::FrRect),
            _ => None,
        }
    };
    if let Some(form) = shape.element.shape.as_deref().and_then(named) {
        return form;
    }
    if let Some(form) = shape.element.sprite.as_deref().and_then(named) {
        return form;
    }
    if let Some(tags) = &shape.element.tags {
        for tag in tags.split(',') {
            if let Some(form) = named(tag.trim()) {
                return form;
            }
        }
    }
    // Without a keyword the kind of the shape decides: a person, a database, a queue, a
    // component, and everything else a box.
    let kind = shape.kind.as_str();
    if kind.ends_with("person") {
        Form::Person
    } else if kind.ends_with("_db") {
        Form::Cyl
    } else if kind.ends_with("_queue") {
        Form::HCyl
    } else if kind.ends_with("component") {
        Form::FrRect
    } else {
        Form::Rounded
    }
}

/// Measures a shape and the four labels it carries.
fn measure(
    shape: &Shape,
    measurer: &dyn TextMeasurer,
    size: f64,
    small: f64,
    width: f64,
    id: usize,
) -> Node {
    let form = form_of(shape);
    let cap_w = if form == Form::HCyl { 36.0 } else { 0.0 };
    let cap_h = if form == Form::Person {
        HEAD
    } else if form == Form::Cyl {
        24.0
    } else {
        0.0
    };
    let inner = width - 2.0 * PAD_X - cap_w;
    let kind = format!("«{}»", shape.kind.replace('_', " "));
    let techn = shape
        .element
        .techn
        .as_ref()
        .filter(|t| !t.is_empty())
        .map(|t| format!("[{t}]"));
    let labels = vec![
        layout_label(
            Some(&kind),
            false,
            measurer,
            small - 1.0,
            inner,
            Style::NONE,
        ),
        layout_label(
            Some(&shape.element.label),
            false,
            measurer,
            size,
            inner,
            Style::BOLD,
        ),
        layout_label(techn.as_deref(), false, measurer, small, inner, Style::NONE),
        layout_label(
            shape.element.descr.as_deref(),
            false,
            measurer,
            small,
            inner,
            Style::NONE,
        ),
    ];
    let mut tw = 0.0f64;
    let mut text_h = if labels[3].height > 0.0 { 4.0 } else { 0.0 };
    for label in &labels {
        tw = tw.max(label.width);
        text_h += label.height;
    }
    Node {
        shape: 0,
        form,
        labels,
        text_h,
        frame: Frame {
            id,
            x: 0.0,
            y: 0.0,
            w: width.max(tw + 2.0 * PAD_X + cap_w),
            h: SHAPE_MIN_H.max(text_h + 2.0 * PAD_Y + cap_h),
            form,
        },
    }
}

/// Places boxes left to right in rows of `per_row`, from (0, 0), and returns the width and the
/// height the rows take. Shapes are stretched to the height of their row; boundaries keep their
/// own.
fn rows(boxes: &mut [Frame], per_row: usize, gap_x: f64, gap_y: f64, stretch: bool) -> (f64, f64) {
    let mut width = 0.0f64;
    let mut y = 0.0f64;
    let mut from = 0;
    while from < boxes.len() {
        let to = boxes.len().min(from + per_row.max(1));
        let mut x = 0.0f64;
        let mut row_h = 0.0f64;
        for box_ in &boxes[from..to] {
            row_h = row_h.max(box_.h);
        }
        for box_ in &mut boxes[from..to] {
            box_.x = x;
            box_.y = y;
            if stretch {
                box_.h = row_h;
            }
            x += box_.w + gap_x;
        }
        width = width.max(x - gap_x);
        y += row_h + gap_y;
        from += per_row.max(1);
    }
    (width, if boxes.is_empty() { 0.0 } else { y - gap_y })
}

/// Mermaid's grid: in each boundary, its shapes in rows of `c4ShapeInRow`, then the boundaries
/// inside it in rows of `c4BoundaryInRow`, each sized around its own content.
pub(crate) fn layout_c4(
    model: &Model,
    measurer: &dyn TextMeasurer,
    size: f64,
    small: f64,
    room: Option<f64>,
    gap: f64,
) -> Layout {
    let group_pad = if room.is_none() { GROUP_PAD } else { 12.0 };
    let mut layout = Layout::default();

    for (at, boundary) in model.boundaries.iter().enumerate() {
        let label_room = match room {
            None => 4000.0,
            Some(room) => 80.0f64.max(room - 2.0 * HEAD_LEFT - 2.0 * GROUP_PAD * 2.0),
        };
        let labels = if boundary.element.parent.is_some() {
            let kind = boundary
                .element
                .type_
                .as_ref()
                .filter(|t| !t.is_empty())
                .map(|t| format!("[{t}]"))
                .unwrap_or_default();
            vec![
                layout_label(
                    Some(&boundary.element.label),
                    false,
                    measurer,
                    size,
                    label_room,
                    Style::BOLD,
                ),
                layout_label(Some(&kind), false, measurer, small, label_room, Style::NONE),
                layout_label(
                    boundary.element.descr.as_deref(),
                    false,
                    measurer,
                    small,
                    label_room,
                    Style::NONE,
                ),
            ]
        } else {
            Vec::new()
        };
        let mut head = 0.0;
        let mut head_w = 0.0f64;
        for label in &labels {
            head += label.height;
            head_w = head_w.max(label.width);
        }
        layout.groups.push(Group {
            boundary: at,
            kids: Vec::new(),
            nodes: Vec::new(),
            labels,
            head: if head > 0.0 {
                head + HEAD_TOP + 6.0 + LANE
            } else {
                group_pad
            },
            head_w,
            head_x: HEAD_LEFT,
            frame: Frame {
                id: at,
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
                form: Form::Rounded,
            },
        });
    }

    // A group is a boundary: the two share their place in the model.
    let root = 0;
    for at in 0..model.boundaries.len() {
        let Some(parent) = model.boundaries[at].element.parent else {
            continue;
        };
        if parent >= model.boundaries.len() {
            continue;
        }
        layout.groups[parent].kids.push(at);
    }

    // A boundary that ends up inside itself is not reachable from the top, as in Mermaid.
    let mut order: Vec<usize> = vec![root];
    let mut depths: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    depths.insert(root, 0);
    let mut i = 0;
    while i < order.len() {
        let parent = order[i];
        let parent_depth = depths.get(&parent).copied().unwrap_or(0);
        for kid in layout.groups[parent].kids.clone() {
            depths.insert(kid, parent_depth + 1);
            order.push(kid);
        }
        i += 1;
    }
    layout.order = order.clone();

    for (at, shape) in model.shapes.iter().enumerate() {
        let Some(parent) = shape.element.parent else {
            continue;
        };
        let group = parent;
        let depth = depths.get(&parent).copied().unwrap_or(0);
        let node_width = match room {
            None => SHAPE_W,
            Some(room) => 100.0f64.max(SHAPE_W.min(room - 2.0 * depth as f64 * group_pad - 32.0)),
        };
        let mut node = measure(shape, measurer, size, small, node_width, at);
        node.shape = at;
        layout.nodes.push(node);
        let node_at = layout.nodes.len() - 1;
        layout.groups[group].nodes.push(node_at);
    }

    let per_row = model.shape_in_row.max(1);
    let groups_per_row = model.boundary_in_row.max(1);
    let mut kids_top: std::collections::HashMap<usize, f64> = std::collections::HashMap::new();
    for at in (0..order.len()).rev() {
        let group = order[at];
        let gap_x = if room.is_none() { GAP_X } else { gap };
        let count = |boxes: &[Frame], limit: usize, gap: f64| -> usize {
            if room.is_none() {
                return limit;
            }
            let mut widest = 1.0f64;
            for box_ in boxes {
                widest = widest.max(box_.w);
            }
            let room = room.unwrap_or_default();
            let per = ((room + gap - 2.0 * group_pad) / (widest + gap)).floor();
            if per < 1.0 {
                1
            } else {
                limit.min(per as usize)
            }
        };
        let mut node_frames: Vec<Frame> = layout.groups[group]
            .nodes
            .iter()
            .map(|&at| layout.nodes[at].frame.clone())
            .collect();
        let mut kid_frames: Vec<Frame> = layout.groups[group]
            .kids
            .iter()
            .map(|&at| layout.groups[at].frame.clone())
            .collect();
        let node_count = count(&node_frames, per_row, gap_x);
        let kid_count = count(&kid_frames, groups_per_row, GROUP_GAP);
        let (grid_w, grid_h) = rows(&mut node_frames, node_count, gap_x, GAP_Y, true);
        let (kids_w, kids_h) = rows(&mut kid_frames, kid_count, GROUP_GAP, GROUP_GAP, false);
        let node_ids: Vec<usize> = layout.groups[group].nodes.clone();
        let kid_ids: Vec<usize> = layout.groups[group].kids.clone();
        for (at, frame) in node_ids.iter().zip(&node_frames) {
            layout.nodes[*at].frame = frame.clone();
        }
        for (at, frame) in kid_ids.iter().zip(&kid_frames) {
            layout.groups[*at].frame = frame.clone();
        }
        let between = if grid_h > 0.0 && kids_h > 0.0 {
            GROUP_GAP
        } else {
            0.0
        };
        let frame = if group == root { 0.0 } else { group_pad };
        kids_top.insert(group, grid_h + between);
        let head_room = layout.groups[group].head_w + 2.0 * HEAD_LEFT - 2.0 * frame;
        let head = if frame == 0.0 {
            0.0
        } else {
            layout.groups[group].head
        };
        layout.groups[group].frame.w = grid_w.max(kids_w).max(head_room) + 2.0 * frame;
        layout.groups[group].frame.h = head + grid_h + between + kids_h + frame;
    }

    for &group in &order {
        let left = layout.groups[group].frame.x + if group == root { 0.0 } else { group_pad };
        let top = layout.groups[group].frame.y
            + if group == root {
                0.0
            } else {
                layout.groups[group].head
            };
        for &node_at in &layout.groups[group].nodes.clone() {
            layout.nodes[node_at].frame.x += left;
            layout.nodes[node_at].frame.y += top;
        }
        let kids_top_at = kids_top.get(&group).copied().unwrap_or(0.0);
        for &kid in &layout.groups[group].kids.clone() {
            layout.groups[kid].frame.x += left;
            layout.groups[kid].frame.y += top + kids_top_at;
        }
    }

    for node in &layout.nodes {
        layout
            .boxes
            .insert(Ref::Shape(node.shape), node.frame.clone());
    }
    for group in &layout.groups {
        layout
            .boxes
            .insert(Ref::Boundary(group.boundary), group.frame.clone());
    }
    layout
}
