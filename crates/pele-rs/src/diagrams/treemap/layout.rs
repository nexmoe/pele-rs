//! The layout of a treemap: a squarified treemap, as pele draws it.
//! Ported from pele's `diagrams/treemap/layout.ts`.

use crate::util::round;

use super::db::Tree;

/// A box of the layout: what is drawn, and where.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Box {
    /// The item the box draws.
    pub node: usize,
    /// The box above this one, if any.
    pub parent: Option<usize>,
    /// The place of the box among the items of the branch it belongs to, which picks its color.
    pub series: usize,
    /// The value of the box: the value of a leaf, or the sum of the leaves under a section.
    pub value: f64,
    /// The boxes under this one.
    pub children: Vec<usize>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// The space a section keeps for its header, and around its children on the other three sides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Insets {
    /// The space between two boxes.
    pub gap: f64,
    /// The space a section keeps for its header.
    pub header: f64,
    /// The space a section keeps around its children on the other three sides.
    pub side: f64,
}

/// Lays a treemap out, and returns every box with a size, parents before their children.
pub(crate) fn layout_treemap(tree: &Tree, width: f64, height: f64, insets: Insets) -> Vec<Box> {
    let mut boxes: Vec<Box> = Vec::new();

    // The items that have no section above them.
    let mut all: Vec<usize> = Vec::new();
    for (index, &root) in tree.roots.iter().enumerate() {
        all.push(new_box(tree, root, None, index, &mut boxes));
    }
    // The items under them, breadth first.
    let mut i = 0;
    while i < all.len() {
        let parent = all[i];
        let series = boxes[parent].series;
        let children = tree.nodes[boxes[parent].node].children.clone();
        for child in children {
            let index = new_box(tree, child, Some(parent), series, &mut boxes);
            boxes[parent].children.push(index);
            all.push(index);
        }
        i += 1;
    }
    // A section is worth what is under it.
    for &index in all.iter().rev() {
        let value = boxes[index].value;
        if let Some(parent) = boxes[index].parent {
            boxes[parent].value += value;
        }
    }

    let mut placed: Vec<usize> = Vec::new();
    let top: Vec<usize> = all[..tree.roots.len()].to_vec();
    place(
        &mut boxes,
        &top,
        Rect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: height,
        },
        insets,
        &mut placed,
    );
    let mut i = 0;
    while i < placed.len() {
        let index = placed[i];
        let children = boxes[index].children.clone();
        if !children.is_empty() {
            let (bx, by, bw, bh) = {
                let box_at = &boxes[index];
                (box_at.x, box_at.y, box_at.w, box_at.h)
            };
            place(
                &mut boxes,
                &children,
                Rect {
                    x: bx + insets.side,
                    y: by + insets.header,
                    w: bw - 2.0 * insets.side,
                    h: bh - insets.header - insets.side,
                },
                insets,
                &mut placed,
            );
        }
        i += 1;
    }

    placed
        .into_iter()
        .map(|index| boxes[index].clone())
        .collect()
}

/// The box of an item: it is worth its own value, or nothing at all.
fn new_box(
    tree: &Tree,
    node: usize,
    parent: Option<usize>,
    series: usize,
    boxes: &mut Vec<Box>,
) -> usize {
    let item = &tree.nodes[node];
    let value = match (item.kind, item.value) {
        (super::db::NodeKind::Leaf, Some(value)) if value.is_finite() && value > 0.0 => value,
        _ => 0.0,
    };
    boxes.push(Box {
        node,
        parent,
        series,
        value,
        children: Vec::new(),
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    });
    boxes.len() - 1
}

/// A rectangle of the drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Lays the boxes out in the rectangle as one squarified treemap: rows are added along the
/// shorter side, and a row stops growing when another box would make its boxes less square.
fn squarify(boxes: &mut [Box], order: &[usize], total: f64, rect: Rect, gap: f64) {
    let Rect {
        mut x,
        mut y,
        mut w,
        mut h,
    } = rect;
    let mut rest = total;
    let mut from = 0;
    while from < order.len() {
        let scale = (w * h) / rest;
        let side = w.min(h);
        let mut sum = boxes[order[from]].value * scale;
        let largest = sum;
        let mut worst =
            ((side * side * largest) / (sum * sum)).max((sum * sum) / (side * side * largest));
        let mut to = from + 1;
        while to < order.len() {
            let smallest = boxes[order[to]].value * scale;
            let next = sum + smallest;
            let ratio = ((side * side * largest) / (next * next))
                .max((next * next) / (side * side * smallest));
            if ratio > worst {
                break;
            }
            worst = ratio;
            sum = next;
            to += 1;
        }

        let thickness = sum / side;
        let mut offset = 0.0;
        for (i, &index) in order[from..to].iter().enumerate() {
            let i = from + i;
            let length = if i == to - 1 {
                side - offset
            } else {
                (boxes[index].value * scale) / thickness
            };
            let bx = if w >= h { x } else { x + offset };
            let by = if w >= h { y + offset } else { y };
            let x0 = round(bx + gap / 2.0);
            let y0 = round(by + gap / 2.0);
            boxes[index].x = x0;
            boxes[index].y = y0;
            boxes[index].w = round(bx + if w >= h { thickness } else { length } - gap / 2.0) - x0;
            boxes[index].h = round(by + if w >= h { length } else { thickness } - gap / 2.0) - y0;
            offset += length;
            rest -= boxes[index].value;
        }
        if w >= h {
            x += thickness;
            w -= thickness;
        } else {
            y += thickness;
            h -= thickness;
        }
        from = to;
    }
}

/// Lays the boxes of one branch out in the rectangle, and keeps the ones with a size.
fn place(
    boxes: &mut [Box],
    children: &[usize],
    rect: Rect,
    insets: Insets,
    placed: &mut Vec<usize>,
) {
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return;
    }
    let mut shown: Vec<usize> = children
        .iter()
        .copied()
        .filter(|&index| boxes[index].value > 0.0)
        .collect();
    // The biggest first; the order of the rest is kept.
    shown.sort_by(|&a, &b| {
        boxes[b]
            .value
            .partial_cmp(&boxes[a].value)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut total = 0.0;
    for &index in &shown {
        total += boxes[index].value;
    }
    if shown.is_empty() || !total.is_finite() {
        return;
    }
    squarify(
        boxes,
        &shown,
        total,
        Rect {
            x: rect.x - insets.gap / 2.0,
            y: rect.y - insets.gap / 2.0,
            w: rect.w + insets.gap,
            h: rect.h + insets.gap,
        },
        insets.gap,
    );
    for &index in &shown {
        if boxes[index].w > 0.0 && boxes[index].h > 0.0 {
            placed.push(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagrams::treemap::db::build_db;
    use crate::diagrams::treemap::parser::parse_treemap;

    fn layout(src: &str) -> Vec<Box> {
        let db = build_db(&parse_treemap(src).expect("parses"));
        layout_treemap(
            &db.tree,
            640.0,
            400.0,
            Insets {
                gap: 4.0,
                header: 28.0,
                side: 6.0,
            },
        )
    }

    #[test]
    fn lays_a_single_leaf_out() {
        let boxes = layout("treemap\n\"A\": 10\n");
        assert_eq!(boxes.len(), 1);
        assert_eq!(boxes[0].value, 10.0);
        assert_eq!(boxes[0].w, 640.0);
        assert_eq!(boxes[0].h, 400.0);
    }

    #[test]
    fn gives_a_section_the_value_under_it() {
        let boxes = layout("treemap\n\"A\"\n  \"B\": 10\n  \"C\": 30\n");
        // The section, and the two leaves under it.
        assert_eq!(boxes.len(), 3);
        assert_eq!(boxes[0].value, 40.0);
        assert_eq!(boxes[0].series, 0);
    }

    #[test]
    fn leaves_out_a_leaf_without_a_value() {
        let boxes = layout("treemap\n\"A\": 0\n");
        assert!(boxes.is_empty());
    }

    #[test]
    fn gives_each_root_its_own_color() {
        let boxes = layout("treemap\n\"A\": 10\n\"B\": 20\n");
        assert_eq!(boxes.len(), 2);
        // The boxes are laid out biggest first, so the bigger root comes first.
        assert_eq!(boxes[0].series, 1);
        assert_eq!(boxes[1].series, 0);
    }
}
