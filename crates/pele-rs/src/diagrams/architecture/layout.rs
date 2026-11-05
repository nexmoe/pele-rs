//! Where the services and the junctions of an architecture diagram are drawn.
//!
//! Mermaid hands the sides of the edges to a force-directed solver as relative placement
//! constraints. pele applies them directly, and so does this: every service and junction takes
//! a cell, an edge `a:R -- L:b` puts b in the cell to the right of a, and a group is a solid
//! rectangle of cells, so a group never encloses a node that is not in it. Each container (the
//! diagram, then every group) is laid out on its own, innermost first, and a finished group is
//! placed in its parent as one block.
//!
//! Ported from pele's `diagrams/architecture/layout.ts`.

use std::collections::HashMap;

use crate::diagrams::architecture::db::ArchitectureModel;
use crate::diagrams::architecture::parser::{AlignDirection, Side};

/// Where everything of the diagram is.
pub(crate) struct Grid {
    /// The column and the row of each node.
    pub col: Vec<i32>,
    pub row: Vec<i32>,
    /// The column, the row and the size in cells of each group.
    pub group_col: Vec<i32>,
    pub group_row: Vec<i32>,
    pub group_cols: Vec<i32>,
    pub group_rows: Vec<i32>,
    /// The group of each group and of each node, or -1 for the diagram itself.
    pub group_parent: Vec<i32>,
    /// Whether a group holds nothing.
    pub group_empty: Vec<bool>,
    pub node_parent: Vec<i32>,
}

const DX: [i32; 4] = [-1, 1, 0, 0];
const DY: [i32; 4] = [0, 0, -1, 1];

/// Blocks up to this many cells are found through the cell map; larger ones are compared one by
/// one.
const SMALL: i64 = 64;
const OFFSET: i64 = 1 << 25;
const SPAN: i64 = 1 << 26;

/// The state of the placement.
struct Placer<'a> {
    /// The number of nodes.
    n: usize,
    /// The number of groups.
    g: usize,
    /// The container of each item: the nodes first, then the groups.
    parent: &'a [i32],
    /// The links between the items, and the side each of them leaves from.
    link_a: &'a [i32],
    link_b: &'a [i32],
    side_a: &'a [i32],
    side_b: &'a [i32],
    /// The item of the container each link relates.
    item_a: &'a [i32],
    item_b: &'a [i32],
    /// The links each item takes part in, by kind.
    hint_links: &'a [Vec<usize>],
    edge_links: &'a [Vec<usize>],
    /// The place of each item in its container, and its size in cells.
    x: Vec<i32>,
    y: Vec<i32>,
    w: Vec<i32>,
    h: Vec<i32>,
    placed: Vec<bool>,
    /// The items in the order they were placed.
    order: Vec<i32>,
    /// The cell each item of the current container takes.
    cells: HashMap<i64, i32>,
    /// The items of the current container that are too large for the cell map.
    large: Vec<i32>,
    /// How far a block has to slide, for each kind of block.
    reach: HashMap<(i32, i32, i32, i32, i32, i32), i32>,
    /// The place of the first item of the current part.
    first: usize,
    /// The offset of the item the placement is at, from the container of that item.
    ax: i32,
    ay: i32,
}

/// The cell a cell of the map is under.
fn cell_key(cx: i32, cy: i32) -> i64 {
    (i64::from(cx) + OFFSET) * SPAN + (i64::from(cy) + OFFSET)
}

impl Placer<'_> {
    /// The offset of an item from the container it is placed in.
    fn anchor(&mut self, node: i32, item: i32) {
        self.ax = 0;
        self.ay = 0;
        let mut cur = node;
        while cur != item {
            self.ax += self.x[cur as usize];
            self.ay += self.y[cur as usize];
            cur = self.n as i32 + self.parent[cur as usize];
        }
    }

    /// Places an item and takes the cells it holds.
    fn put(&mut self, item: i32, px: i32, py: i32) {
        let i = item as usize;
        self.x[i] = px;
        self.y[i] = py;
        self.placed[i] = true;
        self.order.push(item);
        if i64::from(self.w[i]) * i64::from(self.h[i]) > SMALL {
            self.large.push(item);
            return;
        }
        for cy in py..py + self.h[i] {
            for cx in px..px + self.w[i] {
                self.cells.insert(cell_key(cx, cy), item);
            }
        }
    }

    /// Whether an item and a block of cells share a cell.
    fn overlaps(&self, item: i32, rx: i32, ry: i32, rw: i32, rh: i32) -> bool {
        let i = item as usize;
        self.x[i] < rx + rw
            && rx < self.x[i] + self.w[i]
            && self.y[i] < ry + rh
            && ry < self.y[i] + self.h[i]
    }

    /// The item a block of cells runs into, if any.
    fn collide(&self, rx: i32, ry: i32, rw: i32, rh: i32) -> i32 {
        if i64::from(rw) * i64::from(rh) > SMALL {
            for &item in &self.order[self.first..] {
                if self.overlaps(item, rx, ry, rw, rh) {
                    return item;
                }
            }
            return -1;
        }
        for cy in ry..ry + rh {
            for cx in rx..rx + rw {
                if let Some(&hit) = self.cells.get(&cell_key(cx, cy)) {
                    return hit;
                }
            }
        }
        for &item in &self.large {
            if self.overlaps(item, rx, ry, rw, rh) {
                return item;
            }
        }
        -1
    }

    /// How far a block must slide along (sx, sy) to be clear. Each step jumps past what it hit,
    /// and the result is kept, because a later block starting from the same place cannot stop
    /// sooner.
    fn slide(&mut self, rx: i32, ry: i32, rw: i32, rh: i32, sx: i32, sy: i32) -> i32 {
        let key = (rx, ry, rw, rh, sx, sy);
        let mut k = *self.reach.get(&key).unwrap_or(&0);
        loop {
            let hit = self.collide(rx + k * sx, ry + k * sy, rw, rh);
            if hit < 0 {
                break;
            }
            let i = hit as usize;
            k = if sx > 0 {
                self.x[i] + self.w[i] - rx
            } else if sx < 0 {
                rx + rw - self.x[i]
            } else if sy > 0 {
                self.y[i] + self.h[i] - ry
            } else {
                ry + rh - self.y[i]
            };
        }
        self.reach.insert(key, k);
        k
    }

    /// Places the other end of a link, next to the one that is already placed.
    fn follow(&mut self, from: i32, k: usize) {
        let forward = self.item_a[k] == from;
        let to = if forward {
            self.item_b[k]
        } else {
            self.item_a[k]
        };
        if self.placed[to as usize] {
            return;
        }
        let sp = if forward {
            self.side_a[k]
        } else {
            self.side_b[k]
        } as usize;
        let sq = if forward {
            self.side_b[k]
        } else {
            self.side_a[k]
        } as usize;
        self.anchor(
            if forward {
                self.link_a[k]
            } else {
                self.link_b[k]
            },
            from,
        );
        let pax = self.ax;
        let pay = self.ay;
        self.anchor(
            if forward {
                self.link_b[k]
            } else {
                self.link_a[k]
            },
            to,
        );
        let mut dx = DX[sp];
        let mut dy = DY[sp];
        let mut shift_x = 0;
        let mut shift_y = 0;
        let facing = DX[sp] + DX[sq] == 0 && DY[sp] + DY[sq] == 0;
        if sp == sq {
            // Like sides: the edge loops around, so the nodes stack across the direction it leaves
            // in.
            dx = if DX[sp] == 0 { 1 } else { 0 };
            dy = 1 - dx;
        } else if !facing {
            // A bend: one step along the side the edge leaves, one step back from the side it
            // enters.
            shift_x = -DX[sq];
            shift_y = -DY[sq];
        }
        let (mut bx, mut by);
        if dx != 0 {
            bx = if dx > 0 {
                self.x[from as usize] + self.w[from as usize]
            } else {
                self.x[from as usize] - self.w[to as usize]
            };
            by = self.y[from as usize] + pay + shift_y - self.ay;
        } else {
            by = if dy > 0 {
                self.y[from as usize] + self.h[from as usize]
            } else {
                self.y[from as usize] - self.h[to as usize]
            };
            bx = self.x[from as usize] + pax + shift_x - self.ax;
        }
        if facing {
            // When several placed nodes reach this one through the same pair of sides, line it up
            // with the middle one, not with whichever happened to be visited first.
            let list = self.edge_links[to as usize].clone();
            if list.len() > 1 {
                let mut wanted: Vec<i32> = Vec::new();
                for &j in &list {
                    let to_is_b = self.item_b[j] == to;
                    let other = if to_is_b {
                        self.item_a[j]
                    } else {
                        self.item_b[j]
                    };
                    if !self.placed[other as usize]
                        || (if to_is_b {
                            self.side_a[j]
                        } else {
                            self.side_b[j]
                        }) as usize
                            != sp
                        || (if to_is_b {
                            self.side_b[j]
                        } else {
                            self.side_a[j]
                        }) as usize
                            != sq
                    {
                        continue;
                    }
                    self.anchor(
                        if to_is_b {
                            self.link_a[j]
                        } else {
                            self.link_b[j]
                        },
                        other,
                    );
                    let at = if dx != 0 {
                        self.y[other as usize] + self.ay
                    } else {
                        self.x[other as usize] + self.ax
                    };
                    self.anchor(
                        if to_is_b {
                            self.link_b[j]
                        } else {
                            self.link_a[j]
                        },
                        to,
                    );
                    wanted.push(at - if dx != 0 { self.ay } else { self.ax });
                }
                if wanted.len() > 1 {
                    wanted.sort_unstable();
                    if dx != 0 {
                        by = wanted[(wanted.len() - 1) >> 1];
                    } else {
                        bx = wanted[(wanted.len() - 1) >> 1];
                    }
                }
            }
            // Taken: move sideways, to whichever side is nearer, so the edge can still leave
            // straight.
            let px = if dx == 0 { 1 } else { 0 };
            let py = 1 - px;
            let forth = self.slide(bx, by, self.w[to as usize], self.h[to as usize], px, py);
            let back = if forth == 0 {
                0
            } else {
                self.slide(bx, by, self.w[to as usize], self.h[to as usize], -px, -py)
            };
            let near = if forth <= back { forth } else { -back };
            bx += px * near;
            by += py * near;
        } else {
            let far = self.slide(bx, by, self.w[to as usize], self.h[to as usize], dx, dy);
            bx += dx * far;
            by += dy * far;
        }
        self.put(to, bx, by);
    }

    /// Lays out the items of a container.
    fn layout(&mut self, c: usize, kids: &[Vec<i32>]) {
        let items = &kids[c];
        if items.is_empty() {
            return;
        }
        self.order.clear();
        let mut starts: Vec<usize> = Vec::new();
        for &start in items {
            if self.placed[start as usize] {
                continue;
            }
            self.first = self.order.len();
            starts.push(self.first);
            self.cells.clear();
            self.large.clear();
            self.reach.clear();
            self.put(start, 0, 0);
            // Declared alignments are followed as soon as a node is placed, ahead of any edge.
            let mut hint_at = self.first;
            let mut edge_at = self.first;
            let mut link_at = 0;
            loop {
                if hint_at < self.order.len() {
                    let from = self.order[hint_at];
                    hint_at += 1;
                    let list = self.hint_links[from as usize].clone();
                    for k in list {
                        self.follow(from, k);
                    }
                    continue;
                }
                if edge_at >= self.order.len() {
                    break;
                }
                let list = self.edge_links[self.order[edge_at] as usize].clone();
                if link_at >= list.len() {
                    edge_at += 1;
                    link_at = 0;
                    continue;
                }
                let from = self.order[edge_at];
                let k = list[link_at];
                link_at += 1;
                self.follow(from, k);
            }
        }
        starts.push(self.order.len());

        // Unconnected parts go side by side, wrapping into rows that keep the whole roughly
        // square.
        let count = starts.len() - 1;
        let mut min_x = vec![0; count];
        let mut min_y = vec![0; count];
        let mut part_w = vec![0; count];
        let mut part_h = vec![0; count];
        let mut area = 0.0_f64;
        let mut widest = 0;
        for p in 0..count {
            let (mut x0, mut y0) = (i32::MAX, i32::MAX);
            let (mut x1, mut y1) = (i32::MIN, i32::MIN);
            for i in starts[p]..starts[p + 1] {
                let item = self.order[i] as usize;
                x0 = x0.min(self.x[item]);
                y0 = y0.min(self.y[item]);
                x1 = x1.max(self.x[item] + self.w[item]);
                y1 = y1.max(self.y[item] + self.h[item]);
            }
            min_x[p] = x0;
            min_y[p] = y0;
            part_w[p] = x1 - x0;
            part_h[p] = y1 - y0;
            area += f64::from(part_w[p]) * f64::from(part_h[p]);
            widest = widest.max(part_w[p]);
        }
        let limit = widest.max((area * 2.0).sqrt().ceil() as i32);
        let mut cx = 0;
        let mut cy = 0;
        let mut shelf = 0;
        let mut width = 0;
        for p in 0..count {
            if cx > 0 && cx + part_w[p] > limit {
                cy += shelf;
                cx = 0;
                shelf = 0;
            }
            for i in starts[p]..starts[p + 1] {
                let item = self.order[i] as usize;
                self.x[item] += cx - min_x[p];
                self.y[item] += cy - min_y[p];
            }
            cx += part_w[p];
            shelf = shelf.max(part_h[p]);
            width = width.max(cx);
        }
        if c < self.g {
            self.w[self.n + c] = width;
            self.h[self.n + c] = cy + shelf;
        }
    }
}

/// Places everything of the diagram.
pub(crate) fn place_on_grid(model: &ArchitectureModel) -> Grid {
    let nodes: Vec<_> = model.nodes.values().collect();
    let groups: Vec<_> = model.groups.values().collect();
    let n = nodes.len();
    let g = groups.len();
    let root = g;

    let mut node_index: HashMap<&str, i32> = HashMap::new();
    let mut group_index: HashMap<&str, i32> = HashMap::new();
    // The items are the nodes (0..n-1) then the groups (n..n+g-1). The containers are the groups
    // (0..g-1) then the diagram.
    let mut parent = vec![root as i32; n + g];
    let mut depth = vec![0_i32; g + 1];
    let mut kids: Vec<Vec<i32>> = vec![Vec::new(); g + 1];
    for (gi, group) in groups.iter().enumerate() {
        group_index.insert(group.id.as_str(), gi as i32);
    }
    for (gi, group) in groups.iter().enumerate() {
        let p = group.parent.as_deref().and_then(|id| group_index.get(id));
        match p {
            Some(&p) if p as usize != gi => {
                parent[n + gi] = p;
                depth[gi] = depth[p as usize] + 1;
            }
            _ => depth[gi] = 1,
        }
    }
    for (ni, node) in nodes.iter().enumerate() {
        node_index.insert(node.id.as_str(), ni as i32);
    }
    for (ni, node) in nodes.iter().enumerate() {
        if let Some(&p) = node.parent.as_deref().and_then(|id| group_index.get(id)) {
            parent[ni] = p;
        }
        kids[parent[ni] as usize].push(ni as i32);
    }
    for gi in 0..g {
        kids[parent[n + gi] as usize].push((n + gi) as i32);
    }

    // A link asks for one node to sit beside another. It belongs to the innermost container that
    // holds both ends, and there it relates the two children of that container the ends are in.
    let mut link_a: Vec<i32> = Vec::new();
    let mut link_b: Vec<i32> = Vec::new();
    let mut side_a: Vec<i32> = Vec::new();
    let mut side_b: Vec<i32> = Vec::new();
    let mut item_a: Vec<i32> = Vec::new();
    let mut item_b: Vec<i32> = Vec::new();
    let mut hint_links: Vec<Vec<usize>> = vec![Vec::new(); n + g];
    let mut edge_links: Vec<Vec<usize>> = vec![Vec::new(); n + g];

    {
        let mut add_link = |a: i32, b: i32, sa: i32, sb: i32, lists: &mut Vec<Vec<usize>>| {
            if a == b {
                return;
            }
            let mut ia = a;
            let mut ib = b;
            let mut ca = parent[a as usize];
            let mut cb = parent[b as usize];
            while ca != cb {
                if depth[ca as usize] >= depth[cb as usize] {
                    ia = n as i32 + ca;
                    ca = parent[ia as usize];
                } else {
                    ib = n as i32 + cb;
                    cb = parent[ib as usize];
                }
            }
            let k = link_a.len();
            link_a.push(a);
            link_b.push(b);
            side_a.push(sa);
            side_b.push(sb);
            item_a.push(ia);
            item_b.push(ib);
            lists[ia as usize].push(k);
            lists[ib as usize].push(k);
        };

        for hint in &model.layout_hints {
            let row = hint.direction == AlignDirection::Row;
            for i in 1..hint.members.len() {
                let a = node_index.get(hint.members[i - 1].as_str()).copied();
                let b = node_index.get(hint.members[i].as_str()).copied();
                if let (Some(a), Some(b)) = (a, b) {
                    let (sa, sb) = if row {
                        (Side::Right.index() as i32, Side::Left.index() as i32)
                    } else {
                        (Side::Bottom.index() as i32, Side::Top.index() as i32)
                    };
                    add_link(a, b, sa, sb, &mut hint_links);
                }
            }
        }
        // An edge between two like sides says least about where its ends go, so those come last.
        for like_sides in [false, true] {
            for edge in &model.edges {
                if (edge.lhs_dir == edge.rhs_dir) != like_sides {
                    continue;
                }
                let a = node_index.get(edge.lhs_id.as_str()).copied();
                let b = node_index.get(edge.rhs_id.as_str()).copied();
                if let (Some(a), Some(b)) = (a, b) {
                    add_link(
                        a,
                        b,
                        edge.lhs_dir.index() as i32,
                        edge.rhs_dir.index() as i32,
                        &mut edge_links,
                    );
                }
            }
        }
    }

    let mut placer = Placer {
        n,
        g,
        parent: &parent,
        link_a: &link_a,
        link_b: &link_b,
        side_a: &side_a,
        side_b: &side_b,
        item_a: &item_a,
        item_b: &item_b,
        hint_links: &hint_links,
        edge_links: &edge_links,
        x: vec![0; n + g],
        y: vec![0; n + g],
        w: vec![1; n + g],
        h: vec![1; n + g],
        placed: vec![false; n + g],
        order: Vec::new(),
        cells: HashMap::new(),
        large: Vec::new(),
        reach: HashMap::new(),
        first: 0,
        ax: 0,
        ay: 0,
    };
    for c in (0..g).rev() {
        placer.layout(c, &kids);
    }
    placer.layout(root, &kids);

    let mut group_col = vec![0; g];
    let mut group_row = vec![0; g];
    let mut group_parent = vec![0; g];
    let mut group_empty = vec![false; g];
    for gi in 0..g {
        let p = parent[n + gi];
        group_parent[gi] = if p == root as i32 { -1 } else { p };
        group_col[gi] = placer.x[n + gi]
            + if p == root as i32 {
                0
            } else {
                group_col[p as usize]
            };
        group_row[gi] = placer.y[n + gi]
            + if p == root as i32 {
                0
            } else {
                group_row[p as usize]
            };
        group_empty[gi] = kids[gi].is_empty();
    }
    let mut col = vec![0; n];
    let mut row = vec![0; n];
    let mut node_parent = vec![0; n];
    for ni in 0..n {
        let p = parent[ni];
        node_parent[ni] = if p == root as i32 { -1 } else { p };
        col[ni] = placer.x[ni]
            + if p == root as i32 {
                0
            } else {
                group_col[p as usize]
            };
        row[ni] = placer.y[ni]
            + if p == root as i32 {
                0
            } else {
                group_row[p as usize]
            };
    }
    Grid {
        col,
        row,
        group_col,
        group_row,
        group_cols: placer.w[n..].to_vec(),
        group_rows: placer.h[n..].to_vec(),
        group_parent,
        group_empty,
        node_parent,
    }
}
