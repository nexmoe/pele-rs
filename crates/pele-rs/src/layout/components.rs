//! Moves the disconnected parts of a laid-out graph into rows, without changing the layout
//! inside a part. Ported from pele's `layout/components.ts`.

use std::collections::HashMap;

use super::compound::{CEdge, CNode, CompoundResult, shift_layout};

struct Part {
    nodes: Vec<usize>,
    edges: Vec<usize>,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl Part {
    fn include(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.left = self.left.min(x - w / 2.0);
        self.top = self.top.min(y - h / 2.0);
        self.right = self.right.max(x + w / 2.0);
        self.bottom = self.bottom.max(y + h / 2.0);
    }
}

fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Wraps the parts into rows no wider than `width`, `gap` apart. Returns `None`, and leaves
/// the layout as it is, when there is one part or wrapping would not make it narrower.
pub(crate) fn wrap_components(
    nodes: &mut [CNode],
    edges: &mut [CEdge],
    width: f64,
    gap: f64,
) -> Option<CompoundResult> {
    let mut parent: Vec<usize> = (0..nodes.len()).collect();
    let join = |parent: &mut Vec<usize>, a: usize, b: usize| {
        let ra = root(parent, a);
        let rb = root(parent, b);
        parent[ra] = rb;
    };
    for (i, c) in nodes.iter().enumerate() {
        if c.parent >= 0 {
            join(&mut parent, i, c.parent as usize);
        }
    }
    for e in edges.iter() {
        join(&mut parent, e.src, e.dst);
    }

    let mut parts: Vec<Part> = Vec::new();
    let mut part_of: HashMap<usize, usize> = HashMap::new();
    for (i, c) in nodes.iter().enumerate() {
        let id = root(&mut parent, i);
        let k = *part_of.entry(id).or_insert_with(|| {
            parts.push(Part {
                nodes: Vec::new(),
                edges: Vec::new(),
                left: f64::INFINITY,
                top: f64::INFINITY,
                right: f64::NEG_INFINITY,
                bottom: f64::NEG_INFINITY,
            });
            parts.len() - 1
        });
        parts[k].nodes.push(i);
        parts[k].include(c.x, c.y, c.w, c.h);
    }
    if parts.len() < 2 {
        return None;
    }
    for (j, e) in edges.iter().enumerate() {
        let part = &mut parts[part_of[&root(&mut parent, e.src)]];
        part.edges.push(j);
        for k in (0..e.route.len()).step_by(3) {
            part.include(e.route[k], e.route[k + 1], 0.0, 0.0);
        }
        if e.label_w > 0.0 {
            part.include(e.label_x, e.label_y, e.label_w, e.label_h);
        }
    }

    let (mut x, mut y, mut row_height, mut right) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut from, mut to) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut shifts: Vec<(f64, f64)> = Vec::new();
    for part in &parts {
        let w = part.right - part.left;
        let h = part.bottom - part.top;
        if x > 0.0 && x + w > width {
            y += row_height + gap;
            x = 0.0;
            row_height = 0.0;
        }
        shifts.push((x - part.left, y - part.top));
        from = from.min(part.left);
        to = to.max(part.right);
        right = right.max(x + w);
        row_height = row_height.max(h);
        x += w + gap;
    }
    if right >= to - from {
        return None;
    }
    for (part, &(ox, oy)) in parts.iter().zip(&shifts) {
        for &i in &part.nodes {
            shift_layout(std::slice::from_mut(&mut nodes[i]), &mut [], ox, oy);
        }
        for &j in &part.edges {
            shift_layout(&mut [], std::slice::from_mut(&mut edges[j]), ox, oy);
        }
    }
    Some(CompoundResult {
        width: right,
        height: y + row_height,
    })
}
