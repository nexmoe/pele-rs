//! The drawing of an Ishikawa diagram: the causes of an effect, laid out as a fishbone.
//!
//! The layout is pele's own: the causes hang off a spine in pairs, one above and one below, and
//! each cause that has causes of its own gets a bone of its own.

use super::db::IshikawaModel;
use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{title_room, turn_to_fit};
use crate::diagrams::common::literal::break_label;
use crate::svg::markers::{ARROW, marker};
use crate::svg::{Accessible, ROUNDED, esc_text, label_svg, num, svg_document};
use crate::text::Style;
use crate::text::TextMeasurer;
use crate::text::label::{Label, layout_label};
use crate::util::round;

/// How far a bone moves toward the tail for each unit it rises from the spine.
const SLANT: f64 = 0.5;
/// The length of a bone, as `Math.hypot(SLANT, 1)` reads it.
const LENGTH: f64 = 1.118033988749895;
const FIRST_ROW: f64 = 14.0;
const ROW_GAP: f64 = 6.0;
const BONE_END: f64 = 8.0;
const TWIG_STEP: f64 = 22.0;
const STUB: f64 = 18.0;
const TEXT_GAP: f64 = 5.0;
const PAIR_GAP: f64 = 26.0;
const HEAD_GAP: f64 = 34.0;
const TAIL: f64 = 28.0;
const BOX_PAD_X: f64 = 10.0;
const BOX_PAD_Y: f64 = 5.0;

/// A cause below a category. Bones at even depths run level, from the slanted bone above them
/// toward the tail; bones at odd depths slant away from the spine, from the level bone above.
#[derive(Debug)]
struct Bone {
    label: Label,
    depth: usize,
    /// The bone this one hangs off.
    parent: Option<usize>,
    /// Its place among its siblings, counted from the spine outward.
    slot: usize,
    kids: usize,
    /// How far its row is from the spine.
    rise: f64,
    /// Where it meets its parent, measured level from the category's bone: 0 on it, negative
    /// toward the tail.
    at: f64,
    /// Where its text ends.
    text_end: f64,
}

/// A category of causes, with the bones that carry them.
#[derive(Debug)]
struct Category {
    node: usize,
    label: Label,
    box_w: f64,
    box_h: f64,
    bones: Vec<Bone>,
    /// The length of its bone, as distance from the spine.
    rise: f64,
    /// How far its causes reach toward the tail, measured level from its bone.
    reach: f64,
}

/// A node whose row is still to come.
#[derive(Debug)]
struct Frame {
    node: usize,
    depth: usize,
    parent: Option<usize>,
    slot: usize,
    /// Set when the bone was made and only its row is still to come.
    bone: Option<usize>,
}

/// Draws an Ishikawa diagram.
pub(crate) fn render_ishikawa(model: &IshikawaModel, input: &Input<'_>) -> Output {
    turn_to_fit(input.options, |down| draw(model, input, down))
}

/// Draws the fishbone, or the causes one under another when the host is narrow.
fn draw(model: &IshikawaModel, input: &Input<'_>, down: bool) -> Output {
    let size = input.options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let pad = input.options.padding.unwrap_or(8.0);
    if down {
        return vertical_causes(model, input);
    }
    let title = layout_label(
        model.title.as_deref(),
        false,
        input.measurer,
        size,
        4000.0,
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };

    let mut body = String::new();
    let mut min_x = 0.0;
    let mut max_x = 0.0;
    let mut min_y = 0.0;
    let mut max_y = 0.0;
    if let Some(root) = model.root {
        let head = layout_label(
            Some(model.text(root)),
            false,
            input.measurer,
            size,
            200.0,
            Style::BOLD,
        );
        let head_h = (head.height + 24.0).max(48.0);
        let head_w = head.width + 24.0 + head_h / 2.0;
        let mut categories: Vec<Category> = model.nodes[root]
            .children
            .iter()
            .enumerate()
            .map(|(i, &node)| arrange(model, node, i % 2 == 0, input.measurer, size, small))
            .collect();
        // Even a category without causes has a bone long enough to carry its name clear of the
        // head.
        for category in &mut categories {
            category.rise = category.rise.max(head_h / 2.0 + 14.0);
        }

        // Categories meet the spine in pairs, one above and one below, each pair as far toward
        // the tail from the one before as the wider of the two needs.
        let mut xs: Vec<f64> = Vec::new();
        let mut x = -HEAD_GAP;
        let mut i = 0;
        while i < categories.len() {
            if i > 0 {
                let mut room = 0.0f64;
                for side in 0..2 {
                    let before = categories.get(i - 2 + side);
                    let next = categories.get(i + side);
                    if let Some(before) = before {
                        room = room.max(before.reach + next.map_or(0.0, |n| n.box_w / 2.0));
                    }
                }
                x -= room + PAIR_GAP;
            }
            xs.push(x);
            xs.push(x);
            i += 2;
        }
        let tail = if categories.is_empty() {
            -HEAD_GAP
        } else {
            x - TAIL
        };

        max_x = head_w;
        min_x = tail;
        min_y = -head_h / 2.0;
        max_y = head_h / 2.0;
        let mut groups = String::new();
        for (i, category) in categories.iter().enumerate() {
            let sign = if i % 2 == 0 { -1.0 } else { 1.0 };
            let ax = xs[i];
            let end_x = ax - category.rise * SLANT;
            let end_y = sign * category.rise;
            // The bone stops short of the spine to leave its arrowhead a clean point.
            let trim = ARROW - 1.0;
            let mut d = format!(
                "M{},{}L{},{}",
                num(ax - (SLANT / LENGTH) * trim),
                num((sign * trim) / LENGTH),
                num(end_x),
                num(end_y)
            );
            let mut texts = String::new();
            for bone in &category.bones {
                let y = sign * bone.rise;
                let px = |at: f64, rise: f64| ax + at - rise * SLANT;
                if bone.depth % 2 == 0 {
                    d += &format!(
                        "M{},{}H{}",
                        num(px(bone.at, bone.rise)),
                        num(y),
                        num(px(bone.text_end + TEXT_GAP, bone.rise))
                    );
                } else {
                    let from = category.bones[bone.parent.expect("a twig has a parent")].rise;
                    d += &format!(
                        "M{},{}L{},{}",
                        num(px(bone.at, from)),
                        num(sign * from),
                        num(px(bone.at, bone.rise)),
                        num(y)
                    );
                }
                let right = px(bone.text_end, bone.rise);
                texts += &label_svg(
                    &bone.label,
                    right - bone.label.width / 2.0,
                    y,
                    if bone.depth > 2 {
                        " fill=\"var(--_m)\""
                    } else {
                        ""
                    },
                );
                min_x = min_x.min(right - bone.label.width);
            }
            let box_y = end_y + sign * (category.box_h / 2.0);
            min_x = min_x.min(end_x - category.box_w / 2.0);
            max_x = max_x.max(end_x + category.box_w / 2.0);
            min_y = min_y.min(box_y - category.box_h / 2.0);
            max_y = max_y.max(box_y + category.box_h / 2.0);
            groups += &format!(
                "<g class=\"pele-ishikawa-category\" data-id=\"{}\">",
                esc_text(model.text(category.node))
            );
            groups += &format!(
                "<path class=\"pele-ishikawa-bones\" d=\"{d}\" fill=\"none\" stroke=\"var(--_l)\"/>"
            );
            groups += &marker(
                "arrow_point",
                ax,
                0.0,
                SLANT / LENGTH,
                -sign / LENGTH,
                "var(--_l)",
            );
            groups += &format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{ROUNDED}/>",
                num(end_x - category.box_w / 2.0),
                num(box_y - category.box_h / 2.0),
                num(category.box_w),
                num(category.box_h)
            );
            groups += &label_svg(&category.label, end_x, box_y, " class=\"pele-label\"");
            if !texts.is_empty() {
                groups += &format!(
                    "<g class=\"pele-ishikawa-causes\" font-size=\"{}\">{texts}</g>",
                    num(small)
                );
            }
            groups += "</g>";
        }

        let r = head_h / 2.0;
        body += &format!(
            "<path class=\"pele-ishikawa-spine\" d=\"M{},0H{}\" fill=\"none\" stroke=\"var(--_l)\"/>",
            num(tail),
            num(-(ARROW - 1.0))
        );
        body += &marker("arrow_point", 0.0, 0.0, 1.0, 0.0, "var(--_l)");
        body += &groups;
        body += &format!(
            "<g class=\"pele-ishikawa-head\" data-id=\"{}\">",
            esc_text(model.text(root))
        );
        body += &format!(
            "<path d=\"M0,{}H{}A{},{} 0 0 1 {},{}H0Z\" fill=\"var(--_s)\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"/>",
            num(-r),
            num(head_w - r),
            num(r),
            num(r),
            num(head_w - r),
            num(r)
        );
        body += &label_svg(
            &head,
            12.0 + head.width / 2.0,
            0.0,
            " class=\"pele-label\" font-weight=\"var(--_hw)\"",
        );
        body += "</g>";
    }

    let inner = (max_x - min_x).max(title.width);
    let width = (inner + 2.0 * pad).ceil();
    let height = (max_y - min_y + title_height + 2.0 * pad).ceil();
    let ox = pad + (inner - (max_x - min_x)) / 2.0 - min_x;
    let oy = pad + title_height - min_y;
    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    if !body.is_empty() {
        content += &format!(
            "<g transform=\"translate({},{})\">{body}</g>",
            num(ox),
            num(oy)
        );
    }
    let svg = svg_document(
        DiagramKind::Ishikawa,
        width,
        height,
        size,
        input.options,
        &Accessible::default(),
        &content,
    );
    Output {
        svg,
        width,
        height,
        links: Vec::new(),
    }
}

/// Gives every cause of a category a row of its own, so that no two texts can meet, and
/// measures how far the whole of it reaches. A level bone takes the row nearest the spine and
/// its twigs the rows beyond; a slanted twig takes the row beyond everything that hangs off it.
/// The outline may be nested as deep as its indentation goes, so the walk keeps a stack of its
/// own.
fn arrange(
    model: &IshikawaModel,
    node: usize,
    up: bool,
    measurer: &dyn TextMeasurer,
    size: f64,
    small: f64,
) -> Category {
    let label = layout_label(
        Some(model.text(node)),
        false,
        measurer,
        size,
        180.0,
        Style::NONE,
    );
    let line_height = round(small * 1.5);
    let mut bones: Vec<Bone> = Vec::new();
    let mut rows: Vec<usize> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    // Above the spine the first cause goes farthest out, so that the causes read from the top
    // down.
    push_children(model, node, up, 2, None, &mut stack);
    while let Some(frame) = stack.pop() {
        if let Some(bone) = frame.bone {
            rows.push(bone);
            continue;
        }
        let index = bones.len();
        bones.push(Bone {
            label: layout_label(
                Some(model.text(frame.node)),
                false,
                measurer,
                small,
                200.0,
                Style::NONE,
            ),
            depth: frame.depth,
            parent: frame.parent,
            slot: frame.slot,
            kids: model.nodes[frame.node].children.len(),
            rise: 0.0,
            at: 0.0,
            text_end: 0.0,
        });
        if frame.depth % 2 == 0 {
            rows.push(index);
        } else {
            stack.push(Frame {
                bone: Some(index),
                ..frame
            });
        }
        push_children(
            model,
            frame.node,
            up,
            frame.depth + 1,
            Some(index),
            &mut stack,
        );
    }

    let mut rise = FIRST_ROW;
    for &index in &rows {
        let height = bones[index].label.height.max(line_height) + ROW_GAP;
        bones[index].rise = rise + height / 2.0;
        rise += height;
    }
    let box_w = label.width + 2.0 * BOX_PAD_X;
    let box_h = label.height + 2.0 * BOX_PAD_Y;
    let mut reach = box_w / 2.0;
    for index in 0..bones.len() {
        let (at, text_end) = {
            let bone = &bones[index];
            if bone.depth % 2 == 0 {
                // A level bone takes the place of its parent, and runs from there toward the
                // tail, leaving room for the twigs that hang off it.
                let at = match bone.parent {
                    Some(parent) => bones[parent].at,
                    None => 0.0,
                };
                let text_end =
                    at - if bone.kids > 0 {
                        (bone.kids as f64 + 1.0) * TWIG_STEP
                    } else {
                        STUB
                    } - TEXT_GAP;
                (at, text_end)
            } else {
                // Twigs stand along their level bone, the one nearest the spine nearest the
                // tail.
                let parent = &bones[bone.parent.expect("a twig has a parent")];
                let at = parent.at - (parent.kids as f64 - bone.slot as f64) * TWIG_STEP;
                (at, at - TEXT_GAP)
            }
        };
        bones[index].at = at;
        bones[index].text_end = text_end;
        reach = reach.max(bones[index].label.width - text_end);
    }
    Category {
        node,
        label,
        box_w,
        box_h,
        bones,
        rise: rise + BONE_END,
        reach,
    }
}

/// Puts the children of a node on the stack, so that they are read in the order they are drawn.
fn push_children(
    model: &IshikawaModel,
    node: usize,
    up: bool,
    depth: usize,
    parent: Option<usize>,
    stack: &mut Vec<Frame>,
) {
    let children = &model.nodes[node].children;
    let n = children.len();
    for slot in (0..n).rev() {
        stack.push(Frame {
            node: children[if up { n - 1 - slot } else { slot }],
            depth,
            parent,
            slot,
            bone: None,
        });
    }
}

/// Draws the causes one under another, for a host that is too narrow for the fishbone.
fn vertical_causes(model: &IshikawaModel, input: &Input<'_>) -> Output {
    let size = input.options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let pad = input.options.padding.unwrap_or(8.0);
    let room = (input.options.max_width.unwrap_or(320.0) - 2.0 * pad).max(140.0);
    let title = layout_label(
        model.title.as_deref(),
        false,
        input.measurer,
        size,
        title_room(input.options),
        Style::BOLD,
    );

    /// A cause, with the place it is drawn at.
    struct Entry {
        node: usize,
        depth: usize,
        parent: Option<usize>,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        label: Label,
    }

    let mut order: Vec<Entry> = Vec::new();
    let mut pending: Vec<(usize, usize, Option<usize>)> = Vec::new();
    if let Some(root) = model.root {
        pending.push((root, 0, None));
    }
    let mut depth = 0usize;
    while let Some((node, entry_depth, parent)) = pending.pop() {
        let index = order.len();
        order.push(Entry {
            node,
            depth: entry_depth,
            parent,
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
            label: Label::default(),
        });
        depth = depth.max(entry_depth);
        let children = &model.nodes[node].children;
        for i in (0..children.len()).rev() {
            pending.push((children[i], entry_depth + 1, Some(index)));
        }
    }
    let indent = 20.0f64.min(((room - 120.0) / depth.max(1) as f64).max(2.0));

    let mut y = pad
        + if title.height > 0.0 {
            title.height + 12.0
        } else {
            0.0
        };
    let mut width = title.width;
    let mut nodes = String::new();
    let mut edges = String::new();
    for index in 0..order.len() {
        let (node, entry_depth, parent) = {
            let entry = &order[index];
            (entry.node, entry.depth, entry.parent)
        };
        let x = pad + entry_depth as f64 * indent;
        let label_room = (room - entry_depth as f64 * indent - 20.0).max(60.0);
        let label = break_label(
            layout_label(
                Some(model.text(node)),
                false,
                input.measurer,
                if entry_depth < 2 { size } else { small },
                label_room,
                if entry_depth < 2 {
                    Style::BOLD
                } else {
                    Style::NONE
                },
            ),
            input.measurer,
            label_room,
        );
        let w = label.width + 20.0;
        let h = label.height + 12.0;
        let entry_y = y;
        y += h + 12.0;
        width = width.max(x - pad + w);
        {
            let entry = &mut order[index];
            entry.x = x;
            entry.y = entry_y;
            entry.w = w;
            entry.h = h;
            entry.label = label;
        }
        let framed = entry_depth < 2;
        let class = match entry_depth {
            0 => "pele-ishikawa-head",
            1 => "pele-ishikawa-category",
            _ => "pele-ishikawa-cause",
        };
        nodes += &format!(
            "<g class=\"pele-node {class}\" data-id=\"{}\">",
            esc_text(model.text(node))
        );
        if framed {
            nodes += &format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"/>",
                num(x),
                num(entry_y),
                num(w),
                num(h),
                if entry_depth == 0 { 12 } else { 4 }
            );
        }
        let mut attrs = String::from(" class=\"pele-label\"");
        if entry_depth < 2 {
            attrs.push_str(" font-weight=\"var(--_hw)\"");
        }
        if entry_depth > 1 {
            attrs.push_str(&format!(" font-size=\"{}\"", num(small)));
        }
        nodes += &label_svg(&order[index].label, x + w / 2.0, entry_y + h / 2.0, &attrs);
        nodes += "</g>";
        if let Some(parent) = parent {
            let parent = &order[parent];
            let gx = parent.x + indent / 2.0;
            edges += &format!(
                "<path class=\"pele-edge\" d=\"M{},{}V{}H{}\" fill=\"none\" stroke=\"var(--_b)\"/>",
                num(gx),
                num(parent.y + parent.h),
                num(entry_y + h / 2.0),
                num(x)
            );
        }
    }
    let total_width = (width + 2.0 * pad).ceil();
    let height = ((pad + title.height).max(y - 12.0) + pad).ceil();
    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &edges
        + &nodes;
    let svg = svg_document(
        DiagramKind::Ishikawa,
        total_width,
        height,
        size,
        input.options,
        &Accessible::default(),
        &content,
    );
    Output {
        svg,
        width: total_width,
        height,
        links: Vec::new(),
    }
}
