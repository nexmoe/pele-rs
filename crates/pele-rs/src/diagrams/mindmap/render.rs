//! Mind map layout and SVG. Ported from pele's `diagrams/mindmap/render.ts`, without math and
//! icons.

use std::fmt::Write;

use super::db::{MindmapDb, Node};
use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{title_room, turn_to_fit};
use crate::preprocess::Value;
use crate::svg::shapes::{draw_shape, shape_inset, shape_size};
use crate::svg::{
    Accessible, class_names, esc_text, icon_svg, label_svg, num, series_color, svg_document,
    title_label,
};
use crate::text::icon_markup;
use crate::text::label::{Label, label_line, layout_label, line_middle};
use crate::util::round;

/// Indexed by Mermaid's node type. A node without delimiters is text on a line, except the root.
const SHAPES: [&str; 7] = ["", "rounded", "rect", "circle", "cloud", "bang", "hex"];
const ROOT_SHAPE: &str = "stadium";

const GAP_X: f64 = 36.0;
const GAP_Y: f64 = 8.0;
/// Between two subtrees, of which at least one has children of its own.
const GROUP_GAP: f64 = 18.0;
const BRANCH_GAP_X: f64 = 52.0;
const BRANCH_GAP_Y: f64 = 28.0;
const TEXT_PAD: f64 = 6.0;
const PILL_PAD: f64 = 12.0;
const TINT: f64 = 0.22;
/// Between the icon of a node and its text.
const ICON_GAP: f64 = 6.0;
/// Running down, each level is set in by this much, and links leave a node this far from its
/// left edge.
const INDENT: f64 = 28.0;
const STEM: f64 = 12.0;
const BEND: f64 = 8.0;
const ROW_GAP: f64 = 10.0;

/// A label with each line starting at `left`, for an outline, where centered lines would look
/// ragged.
fn left_label(label: &Label, left: f64, cy: f64) -> String {
    if label.lines.len() < 2 {
        return label_svg(
            label,
            left + label.width / 2.0,
            cy,
            " class=\"pele-rs-label\"",
        );
    }
    let top = cy - label.height / 2.0;
    let mut out = String::new();
    for k in 0..label.lines.len() {
        out += &label_svg(
            &label_line(label, k),
            left + label.widths[k] / 2.0,
            top + line_middle(label, k),
            " class=\"pele-rs-label\"",
        );
    }
    out
}

pub(crate) fn render_mindmap(db: &MindmapDb, input: &Input<'_>) -> Output {
    turn_to_fit(input.options, |down| draw(db, input, down))
}

/// Subtree bands, as `stack` lays them out.
struct Bands {
    /// The height each subtree takes.
    extent: Vec<f64>,
    /// A subtree's offset among its siblings.
    rel: Vec<f64>,
    /// The node's offset inside its band.
    own: Vec<f64>,
    /// The offset of the node's children's bands.
    kids: Vec<f64>,
    /// Where links meet each node, measured down from its top: its middle.
    anchor: Vec<f64>,
}

impl Bands {
    /// Stacks subtrees top to bottom. Returns the height at which their parent's anchor belongs,
    /// and the height of the stack.
    fn stack(&mut self, nodes: &[Node], list: &[usize], gap: f64) -> (f64, f64) {
        let mut y = 0.0;
        let mut last = 0.0;
        for (k, &c) in list.iter().enumerate() {
            self.rel[c] = y;
            let grouped = !nodes[c].children.is_empty()
                || list
                    .get(k + 1)
                    .is_some_and(|&n| !nodes[n].children.is_empty());
            last = if grouped { gap.max(GROUP_GAP) } else { gap };
            y += self.extent[c] + last;
        }
        let (a, b) = (list[0], list[list.len() - 1]);
        let middle = (self.rel[a]
            + self.own[a]
            + self.anchor[a]
            + self.rel[b]
            + self.own[b]
            + self.anchor[b])
            / 2.0;
        (middle, y - last)
    }
}

/// `down` draws the map as an outline: the root at the top, and each level set in under its
/// parent. It is far narrower than branches spreading to both sides, and as tall as it needs
/// to be.
fn draw(db: &MindmapDb, input: &Input<'_>, down: bool) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let pad = options.padding.unwrap_or(8.0);
    let wrap = match input.config.get("mindmap") {
        Some(Value::Map(m)) => match m.get("maxNodeWidth") {
            Some(Value::Number(w)) if *w > 0.0 => w.min(2000.0),
            _ => 200.0,
        },
        _ => 200.0,
    };
    // `colorBy: depth` gives the nodes of one level one color across all branches; anything else
    // is the default, a color per branch.
    let by_depth = match input.config.get("mindmap") {
        Some(Value::Map(m)) => matches!(m.get("colorBy"), Some(Value::String(s)) if s == "depth"),
        _ => false,
    };

    let nodes = &db.nodes;
    let n = nodes.len();
    let mut parent = vec![0usize; n];
    let mut depth = vec![0usize; n];
    for i in 0..n {
        for &c in &nodes[i].children {
            parent[c] = i;
            depth[c] = depth[i] + 1;
        }
    }
    // An outline has the width the host gives it, less what its level is set in.
    let room = match options.max_width {
        Some(m) if down && m > 0.0 => m - 2.0 * pad,
        _ => f64::INFINITY,
    };
    let mut labels = Vec::with_capacity(n);
    let mut shapes = Vec::with_capacity(n);
    let mut w = vec![0.0; n];
    let mut h = vec![0.0; n];
    let mut dy = vec![0.0; n];
    let mut bands = Bands {
        extent: vec![0.0; n],
        rel: vec![0.0; n],
        own: vec![0.0; n],
        kids: vec![0.0; n],
        anchor: vec![0.0; n],
    };

    // The icon of each node the host has one for, with its markup. One it has not takes no room.
    let mut icons: Vec<Option<std::rc::Rc<str>>> = Vec::with_capacity(n);
    let glyph = size * 1.25;

    for (i, node) in nodes.iter().enumerate() {
        let icon = node
            .icon
            .as_deref()
            .and_then(|name| icon_markup(measurer, name));
        let reserved = if icon.is_some() {
            glyph + ICON_GAP
        } else {
            0.0
        };
        icons.push(icon);
        let fit = wrap.min(72f64.max(room - depth[i] as f64 * INDENT - 2.0 * TEXT_PAD - reserved));
        let label = layout_label(
            Some(&node.descr),
            true,
            measurer,
            size,
            fit,
            crate::text::Style::NONE,
        );
        let gap = if label.width > 0.0 { ICON_GAP } else { 0.0 };
        let tw = label.width + if icons[i].is_some() { glyph + gap } else { 0.0 };
        let th = label.height.max(round(size * 1.5));
        let mut shape = match SHAPES.get(usize::from(node.kind)) {
            Some(&s) if !s.is_empty() => s,
            _ if i == 0 => ROOT_SHAPE,
            _ => "",
        };
        // In an outline the root heads a column, and the trunk drops from under its left end. A
        // circle or a cloud would stand wide of that, so the root is a pill there unless it is a
        // box already.
        if down && i == 0 && shape != "rect" && shape != "rounded" {
            shape = ROOT_SHAPE;
        }
        shapes.push(shape);
        if shape.is_empty() {
            // A node with no shape of its own is plain text when it ends a branch, and text in a
            // tinted pill when the branch goes on from it, which sets the levels apart.
            let pill = !node.children.is_empty();
            w[i] = tw + 2.0 * if pill { PILL_PAD } else { TEXT_PAD };
            h[i] = th + if pill { 6.0 } else { 0.0 };
            bands.anchor[i] = h[i] / 2.0;
        } else {
            let s = shape_size(shape, tw, th);
            w[i] = s.w;
            h[i] = s.h;
            dy[i] = s.dy;
            bands.anchor[i] = s.h / 2.0;
        }
        labels.push(label);
    }

    // Nodes are in depth-first order, so going backwards visits children before their parent.
    for i in (1..n).rev() {
        let children = &nodes[i].children;
        if children.is_empty() {
            bands.extent[i] = h[i];
            continue;
        }
        let (middle, stacked) = bands.stack(nodes, children, GAP_Y);
        let top = middle - bands.anchor[i];
        let low = top.min(0.0);
        bands.extent[i] = stacked.max(top + h[i]) - low;
        bands.own[i] = top - low;
        bands.kids[i] = -low;
    }

    let mut x = vec![0.0; n];
    let mut y = vec![0.0; n];
    let mut band = vec![0.0; n];
    let mut side = vec![0.0f64; n];
    // Running down: how far the trunk under each node has been drawn.
    let mut trunk = vec![0.0; n];

    if down {
        // Nodes are in depth-first order, which is the order of an outline's rows.
        let mut row = 0.0;
        for i in 0..n {
            if i > 0 && parent[i] == 0 {
                row += ROW_GAP;
            }
            x[i] = depth[i] as f64 * INDENT + w[i] / 2.0;
            y[i] = row;
            row += h[i] + ROW_GAP;
        }
    } else if n > 0 {
        // The first branches go right and the rest left, split where the two sides are closest
        // in height.
        let branches = &nodes[0].children;
        let count = branches.len();
        let total: f64 = branches.iter().map(|&b| bands.extent[b]).sum();
        let mut split = count;
        let mut best = f64::INFINITY;
        let mut right = 0.0;
        for k in 1..count {
            right += bands.extent[branches[k - 1]];
            let diff = (2.0 * right - total + (2.0 * k as f64 - count as f64) * BRANCH_GAP_Y).abs();
            if diff < best {
                best = diff;
                split = k;
            }
        }
        for (list, dir) in [(&branches[..split], 1.0), (&branches[split..], -1.0)] {
            if list.is_empty() {
                continue;
            }
            let (middle, _) = bands.stack(nodes, list, BRANCH_GAP_Y);
            for &c in list {
                band[c] = bands.rel[c] - middle;
                side[c] = dir;
            }
        }
        y[0] = -h[0] / 2.0;
        for i in 0..n {
            if i > 0 {
                y[i] = band[i] + bands.own[i];
            }
            for &c in &nodes[i].children {
                if i > 0 {
                    side[c] = side[i];
                    band[c] = band[i] + bands.kids[i] + bands.rel[c];
                }
                let gap = if i > 0 { GAP_X } else { BRANCH_GAP_X };
                x[c] = x[i] + side[c] * (w[i] / 2.0 + gap + w[c] / 2.0);
            }
        }
    }

    let anchor = &bands.anchor;
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut out = String::new();
    let mut edges = String::new();
    let mut body = String::new();
    let mut color = String::new();
    let flush = |out: &mut String, edges: &mut String, body: &mut String, color: &str, section| {
        if body.is_empty() {
            return;
        }
        let _ = write!(
            out,
            "<g class=\"pele-rs-branch pele-rs-section-{section}\"><g class=\"pele-rs-edges\" fill=\"none\" stroke=\"{color}\" stroke-width=\"1.5\">{edges}</g>{body}</g>"
        );
        edges.clear();
        body.clear();
    };

    // The root is drawn last, on top of the links that leave it.
    for k in 1..=n {
        let i = k % n;
        let node = &nodes[i];
        let shape = shapes[i];
        let label = &labels[i];
        let cx = x[i];
        let top = y[i];
        min_x = min_x.min(cx - w[i] / 2.0);
        max_x = max_x.max(cx + w[i] / 2.0);
        min_y = min_y.min(top);
        max_y = max_y.max(top + h[i]);

        if i == 0 || parent[i] == 0 {
            let section = nodes[k - 1].section.unwrap_or(0);
            flush(&mut out, &mut edges, &mut body, &color, section);
        }
        if i > 0 {
            if by_depth {
                color = series_color(node.depth_section.unwrap_or(0));
            } else if parent[i] == 0 {
                color = series_color(node.section.unwrap_or(0));
            }
            // A link takes the color of its node. By branch that is the color of the group the
            // link is in already, so the attribute is only written by depth.
            let stroke = if by_depth {
                format!(" stroke=\"{color}\"")
            } else {
                String::new()
            };
            let p = parent[i];
            if down {
                // The link drops from under its parent and turns in to the node's left. It
                // starts where the link to the sibling above turned in, so each stretch of a
                // shared trunk has one color. Under the root's rounded end, the trunk starts
                // where that end has come down to the bottom.
                let stem = if p == 0 {
                    STEM.max(h[0] / 2.0).min(INDENT - BEND)
                } else {
                    STEM
                };
                let x1 = x[p] - w[p] / 2.0 + stem.min(w[p] / 2.0);
                let y1 = if trunk[p] != 0.0 {
                    trunk[p]
                } else {
                    y[p] + h[p]
                };
                let left = cx - w[i] / 2.0;
                let x2 = left
                    + if shape.is_empty() {
                        0.0
                    } else {
                        shape_inset(shape, w[i], h[i], 3, 0.0)
                    };
                let y2 = top + anchor[i];
                let bend = BEND.min(x2 - x1).min(y2 - y1).max(0.0);
                // The next link carries on from where this one leaves the trunk, so the trunk
                // has no gap.
                trunk[p] = y2 - bend;
                let _ = write!(
                    edges,
                    "<path class=\"pele-rs-edge\"{stroke} d=\"M{},{}V{}Q{},{} {},{}H{}\"/>",
                    num(x1),
                    num(y1),
                    num(y2 - bend),
                    num(x1),
                    num(y2),
                    num(x1 + bend),
                    num(y2),
                    num(x2)
                );
            } else {
                let dir = side[i];
                let from = if dir > 0.0 { 1 } else { 3 };
                let inset = |shape: &str, w, h, side| {
                    if shape.is_empty() {
                        0.0
                    } else {
                        shape_inset(shape, w, h, side, 0.0)
                    }
                };
                let x1 = x[p] + dir * (w[p] / 2.0 - inset(shapes[p], w[p], h[p], from));
                let y1 = y[p] + anchor[p];
                let near = cx - (dir * w[i]) / 2.0;
                let x2 = near + dir * inset(shape, w[i], h[i], 4 - from);
                let y2 = top + anchor[i];
                let mid = num((x1 + x2) / 2.0);
                let _ = write!(
                    edges,
                    "<path class=\"pele-rs-edge\"{stroke} d=\"M{},{}C{mid},{} {mid},{} {},{}\"/>",
                    num(x1),
                    num(y1),
                    num(y1),
                    num(y2),
                    num(x2),
                    num(y2)
                );
            }
        }

        let pill = shape.is_empty() && !node.children.is_empty();
        // Every node off the root is filled with a tint of its branch's color and outlined in
        // it, whether its shape was given in the source or is the pill of a node that branches.
        // The root is neutral.
        let tint = format!(" fill=\"{color}\" fill-opacity=\"{TINT}\" stroke=\"{color}\"");
        let inner = if !shape.is_empty() {
            if i > 0 {
                draw_shape(shape, w[i], h[i], &tint, &format!(" stroke=\"{color}\""))
            } else {
                draw_shape(
                    shape,
                    w[i],
                    h[i],
                    " fill=\"var(--_s)\" stroke=\"var(--_b)\"",
                    " stroke=\"var(--_b)\"",
                )
            }
        } else if pill {
            format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{tint}/>",
                num(-w[i] / 2.0),
                num(-h[i] / 2.0),
                num(w[i]),
                num(h[i]),
                num(h[i] / 2.0)
            )
        } else {
            String::new()
        };
        let kind = if !shape.is_empty() {
            shape
        } else if pill {
            "branch"
        } else {
            "text"
        };
        // The icon stands before the text, and the two are centered together.
        let mut inner = inner;
        let mut shift = 0.0;
        if let (Some(name), Some(markup)) = (node.icon.as_deref(), &icons[i]) {
            let gap = if label.width > 0.0 { ICON_GAP } else { 0.0 };
            shift = (glyph + gap) / 2.0;
            inner += &icon_svg(
                name,
                -shift - label.width / 2.0,
                dy[i] - glyph / 2.0,
                glyph,
                markup,
            );
        }
        let text = if down && shape.is_empty() {
            left_label(label, shift - label.width / 2.0, dy[i])
        } else {
            label_svg(label, shift, dy[i], " class=\"pele-rs-label\"")
        };
        let group = format!(
            "<g class=\"pele-rs-node pele-rs-shape-{kind}{}{}\" data-id=\"{}\" transform=\"translate({},{})\">{inner}{text}</g>",
            if i > 0 { "" } else { " pele-rs-root" },
            class_names(node.class.as_deref().unwrap_or("")),
            esc_text(&node.node_id),
            num(cx),
            num(top + h[i] / 2.0)
        );
        if i > 0 {
            body += &group;
        } else {
            out += &group;
        }
    }

    let title = title_label(db.title.as_deref(), measurer, size, title_room(options));
    let title_height = if title.height > 0.0 {
        title.height + 16.0
    } else {
        0.0
    };
    let width = (max_x - min_x).max(title.width);
    let total_width = (width + 2.0 * pad).ceil();
    let total_height = (max_y - min_y + title_height + 2.0 * pad).ceil();
    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-rs-title\" font-weight=\"var(--_tw)\"",
    ) + &format!(
        "<g transform=\"translate({},{})\">{out}</g>",
        num(pad - min_x + (width - max_x + min_x) / 2.0),
        num(pad + title_height - min_y)
    );
    Output {
        svg: svg_document(
            DiagramKind::Mindmap,
            total_width,
            total_height,
            size,
            options,
            &Accessible::default(),
            &content,
        ),
        width: total_width,
        height: total_height,
        links: Vec::new(),
    }
}
