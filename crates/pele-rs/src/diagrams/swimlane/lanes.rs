//! The layout of a swimlane diagram: every top-level group is a lane, a band that runs the
//! length of the diagram along the flow. Ranks are shared by all lanes, so a step sits level
//! with the steps that happen at the same time in other lanes, and each node stays inside its
//! own lane. Ported from pele's `layout/lanes.ts`.

use crate::layout::compound::{CEdge, CNode, Dir};
use crate::layout::layered::{Kind, LEdge, LNode, LayeredOptions, ledge, lnode, orient, rank};
use crate::util::round;

/// The room above the lanes for their titles.
const LANE_HEAD: f64 = 34.0;
/// The room a lane keeps around its items.
const LANE_PAD: f64 = 24.0;
/// How many times the order within the ranks is settled.
const SWEEPS: usize = 8;
/// An edge gets a bend point in every rank it skips. Past this many, it is drawn as one curve.
const MAX_DUMMIES: usize = 20000;
/// How many parents a node may have before the walk up the tree gives up.
const MAX_HOPS: usize = 10000;
/// The work the routing may take before the lanes are laid out only once.
const ROUTE_BUDGET: f64 = 2_000_000.0;
/// Parallel lines closer than this are drawn as one.
const NEAR: f64 = 4.0;
/// How many times the lanes are laid out again.
const ROUNDS: usize = 3;
/// The share of the routing budget that the relay around a lane needs.
const RELAY: f64 = 8.0;
/// How close to a corner a turn stays.
const TURN: f64 = 20.0;
/// The room a lane keeps beside a handoff along its margin.
const MARGIN: f64 = 20.0;
/// How much of the margin is between the handoff and the lane's items.
const LEG: f64 = 14.0;
/// How much closer the ranks close up where only plain handoffs pass.
const CLOSE: f64 = 3.0;

/// `Math.sign`: the direction of a number, as a number.
fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// What the lanes hold, as the layout sees it.
struct Plan {
    /// The items to rank and place, as indexes into the nodes.
    real: Vec<usize>,
    /// The lane each item is in.
    item_lane: Vec<i32>,
    /// The relations between the items.
    ledges: Vec<LEdge>,
    /// The edge each relation came from.
    edge_of: Vec<usize>,
    /// The rank of each item.
    rank: Vec<i32>,
    /// How much room each item takes across the flow.
    size: Vec<f64>,
    /// How much room each item takes along the flow.
    depth: Vec<f64>,
    /// The lanes, as indexes into the nodes.
    lanes: Vec<usize>,
    /// The lane of each node, or -1 for none.
    lane_slot: Vec<i32>,
    /// The item of each node, or -1 for none.
    item_of: Vec<i32>,
    /// The number of lanes.
    lane_count: usize,
    /// The room each lane keeps around its items.
    lane_pads: Vec<f64>,
    /// The room kept after the last rank.
    end_pad: f64,
    /// Whether the flow runs across.
    sideways: bool,
    /// Whether the flow runs backwards.
    flipped: bool,
    /// The step between the ranks.
    step: i32,
}

/// What one pass of the layout is told: which relations are plain, and which lanes keep a margin
/// for a handoff.
struct Pass {
    plain: Vec<u8>,
    margins: Vec<u8>,
}

/// What one pass of the layout produced.
struct Placed {
    width: f64,
    height: f64,
    /// The relations that were given a route of their own.
    routed: Vec<u8>,
    /// The lanes that keep a margin for a handoff.
    margins: Vec<u8>,
    /// The work the routing took.
    work: f64,
}

/// Lays the lanes out: the items of every lane are ranked together, the lanes are placed side by
/// side across the flow, and the relations are routed. Returns the width and the height of the
/// drawing.
pub(crate) fn lane_layout(
    nodes: &mut [CNode],
    edges: &mut [CEdge],
    dir: Dir,
    opt: &LayeredOptions,
    lane_pad: Option<f64>,
) -> (f64, f64) {
    let n = nodes.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let sideways = matches!(dir, Dir::LR | Dir::RL);
    let flipped = matches!(dir, Dir::BT | Dir::RL);
    let lane_pad = lane_pad.unwrap_or(LANE_PAD);

    // Lanes in the order they were written. The graph lists groups last-declared first.
    let mut lanes: Vec<usize> = Vec::new();
    for i in (0..n).rev() {
        if nodes[i].parent < 0 && nodes[i].is_group {
            lanes.push(i);
        }
    }
    let lane_count = lanes.len().max(1);
    let mut lane_slot = vec![-1i32; n];
    for (k, &id) in lanes.iter().enumerate() {
        lane_slot[id] = k as i32;
    }
    let mut has_child = vec![false; n];
    for i in 0..n {
        if nodes[i].parent >= 0 {
            has_child[nodes[i].parent as usize] = true;
        }
    }

    // What gets ranked and placed: nodes, and groups that are empty and so act as nodes.
    let mut item_of = vec![-1i32; n];
    let mut real: Vec<usize> = Vec::new();
    for i in 0..n {
        if lane_slot[i] >= 0 {
            continue;
        }
        if !nodes[i].is_group || !has_child[i] {
            item_of[i] = real.len() as i32;
            real.push(i);
        }
    }

    // The lane each item is in, and the room that lane keeps around it.
    let mut item_lane = Vec::with_capacity(real.len());
    let mut rooms = Vec::with_capacity(real.len());
    for &i in &real {
        let mut t = i;
        let mut room = lane_pad;
        let mut hops = 0;
        while nodes[t].parent >= 0 && hops < MAX_HOPS {
            t = nodes[t].parent as usize;
            if lane_slot[t] < 0 {
                room += nodes[t]
                    .pad_x
                    .max(nodes[t].pad_top)
                    .max(nodes[t].pad_bottom);
            }
            hops += 1;
        }
        item_lane.push(lane_slot[t].max(0));
        rooms.push(room);
    }
    let mut lane_pads = vec![lane_pad; lane_count];
    let mut end_pad = lane_pad;
    for (k, &_i) in real.iter().enumerate() {
        let lane = item_lane[k].max(0) as usize;
        if rooms[k] > lane_pads[lane] {
            lane_pads[lane] = rooms[k];
        }
        if rooms[k] > end_pad {
            end_pad = rooms[k];
        }
    }

    // The items and the relations between them, as the layered layout sees them.
    let mut lnodes: Vec<LNode> = real
        .iter()
        .map(|&i| {
            let node = &nodes[i];
            let (cross, along) = if sideways {
                (node.h, node.w)
            } else {
                (node.w, node.h)
            };
            lnode(cross, along, Kind::Node)
        })
        .collect();
    let mut ledges: Vec<LEdge> = Vec::new();
    let mut edge_of: Vec<usize> = Vec::new();
    let mut step = 1;
    let mut seen: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for (ei, e) in edges.iter().enumerate() {
        let a = item_of[e.src];
        let b = item_of[e.dst];
        if a < 0 || b < 0 || a == b {
            continue;
        }
        let key = (a.min(b) as usize) * real.len() + (a.max(b) as usize);
        if e.label_w > 0.0 || seen.contains(&key) {
            step = 2;
        }
        seen.insert(key);
        let (lw, lh) = if sideways {
            (e.label_h, e.label_w)
        } else {
            (e.label_w, e.label_h)
        };
        ledges.push(ledge(a as usize, b as usize, e.minlen, lw, lh));
        edge_of.push(ei);
    }
    orient(&lnodes, &mut ledges);
    rank(&mut lnodes, &ledges, step);

    let plan = Plan {
        real,
        item_lane,
        ledges,
        edge_of,
        rank: lnodes.iter().map(|n| n.rank).collect(),
        size: lnodes.iter().map(|n| n.w).collect(),
        depth: lnodes.iter().map(|n| n.h).collect(),
        lanes,
        lane_slot,
        item_of,
        lane_count,
        lane_pads,
        end_pad,
        sideways,
        flipped,
        step,
    };

    let m = plan.ledges.len();
    let mut pass = Pass {
        plain: vec![0u8; m],
        margins: vec![0u8; lane_count],
    };
    let mut placed = plan.place(nodes, edges, opt, &pass);
    // A handoff that was given a simple route needs no room kept for it through the ranks it
    // passes, so the lanes are laid out again without it. A handoff that then loses its route
    // goes back to the ranked one. Routing is most of the work, so a large graph is laid out once.
    for round in 0..ROUNDS {
        if placed.work > ROUTE_BUDGET / RELAY {
            break;
        }
        let next: Vec<u8> = if round == 0 {
            placed.routed.clone()
        } else {
            pass.plain
                .iter()
                .zip(placed.routed.iter())
                .map(|(&on, &routed)| on & routed)
                .collect()
        };
        if next == pass.plain && placed.margins == pass.margins {
            break;
        }
        pass.plain = next;
        pass.margins = placed.margins.clone();
        placed = plan.place(nodes, edges, opt, &pass);
    }
    if pass
        .plain
        .iter()
        .zip(placed.routed.iter())
        .any(|(&on, &routed)| on != 0 && routed == 0)
    {
        pass.plain = vec![0u8; m];
        pass.margins = vec![0u8; lane_count];
        placed = plan.place(nodes, edges, opt, &pass);
    }
    (placed.width, placed.height)
}

impl Plan {
    /// Lays the lanes out once, with the handoffs that are given a simple route told apart from
    /// the ones that are not.
    fn place(
        &self,
        nodes: &mut [CNode],
        edges: &mut [CEdge],
        opt: &LayeredOptions,
        pass: &Pass,
    ) -> Placed {
        let (lane_pads, end_pad, plain, margins) =
            (&self.lane_pads, self.end_pad, &pass.plain, &pass.margins);
        let m = self.ledges.len();
        let lane_count = self.lane_count;
        // The room a lane keeps around its items, which grows where a handoff needs a margin.
        let pad: Vec<f64> = (0..lane_count)
            .map(|lane| {
                let room = lane_pads[lane];
                if margins[lane] != 0 {
                    room.max(MARGIN)
                } else {
                    room
                }
            })
            .collect();
        let mut used = vec![0u8; lane_count];

        // The ranks each relation crosses.
        let mut span = vec![0i32; m];
        let mut dummies = 0usize;
        for k in 0..m {
            let e = &self.ledges[k];
            span[k] = (self.rank[e.head] - self.rank[e.tail]).abs();
            if plain[k] == 0 {
                dummies += (span[k] - 1).max(0) as usize;
            }
        }
        let direct = dummies > MAX_DUMMIES;
        let total = self.real.len() + if direct { 0 } else { dummies };

        let mut rank = vec![0i32; total];
        let mut lane = vec![0i32; total];
        let mut size = vec![0f64; total];
        let mut depth = vec![0f64; total];
        let mut dummy = vec![false; total];
        let mut max_rank = 0i32;
        for k in 0..self.real.len() {
            rank[k] = self.rank[k];
            lane[k] = self.item_lane[k];
            size[k] = self.size[k];
            depth[k] = self.depth[k];
            if rank[k] > max_rank {
                max_rank = rank[k];
            }
        }

        // Every relation that crosses more than one rank gets a bend point in each rank it
        // passes, so that it can turn there.
        let mut first_dummy = vec![-1i32; m];
        let mut label_item = vec![-1i32; m];
        let mut links: Vec<usize> = Vec::new();
        {
            let mut next = self.real.len();
            for k in 0..m {
                let e = &self.ledges[k];
                let a = if e.reversed { e.head } else { e.tail };
                let b = if e.reversed { e.tail } else { e.head };
                if direct || span[k] < 2 || plain[k] != 0 {
                    links.extend_from_slice(&[a, b]);
                    continue;
                }
                let ra = rank[a];
                let rb = rank[b];
                let mut label_rank = -1;
                if e.label_w > 0.0 {
                    label_rank = ra + ((rb - ra) >> 1);
                    if self.step == 2 && (label_rank & 1) == 0 {
                        label_rank -= 1;
                    }
                }
                first_dummy[k] = next as i32;
                let mut prev = a;
                let mut r = ra + 1;
                while r < rb {
                    let d = next;
                    next += 1;
                    rank[d] = r;
                    // The edge crosses from its first lane to its last one evenly along the way.
                    lane[d] = lane[a]
                        + round(
                            (f64::from(lane[b] - lane[a]) * f64::from(r - ra)) / f64::from(rb - ra),
                        ) as i32;
                    dummy[d] = true;
                    if r == label_rank {
                        size[d] = e.label_w;
                        depth[d] = e.label_h;
                        label_item[k] = d as i32;
                    }
                    links.extend_from_slice(&[prev, d]);
                    prev = d;
                    r += 1;
                }
                links.extend_from_slice(&[prev, b]);
            }
        }

        // The neighbours of every item, up and down the flow.
        let mut up_start = vec![0usize; total + 1];
        let mut down_start = vec![0usize; total + 1];
        for k in (0..links.len()).step_by(2) {
            down_start[links[k] + 1] += 1;
            up_start[links[k + 1] + 1] += 1;
        }
        for i in 0..total {
            up_start[i + 1] += up_start[i];
            down_start[i + 1] += down_start[i];
        }
        let mut up = vec![0usize; links.len() / 2];
        let mut down = vec![0usize; links.len() / 2];
        {
            let mut up_at = up_start[..total].to_vec();
            let mut down_at = down_start[..total].to_vec();
            for k in (0..links.len()).step_by(2) {
                down[down_at[links[k]]] = links[k + 1];
                down_at[links[k]] += 1;
                up[up_at[links[k + 1]]] = links[k];
                up_at[links[k + 1]] += 1;
            }
        }

        // The order within each rank: by lane first, then by where the item's neighbours sit.
        let mut rows: Vec<Vec<usize>> = Vec::with_capacity(max_rank as usize + 1);
        for _ in 0..=max_rank {
            rows.push(Vec::new());
        }
        for i in 0..total {
            rows[rank[i] as usize].push(i);
        }
        let mut pos = vec![0f64; total];
        let mut key = vec![0f64; total];
        for row in rows.iter_mut() {
            for (k, &i) in row.iter().enumerate() {
                key[i] = k as f64;
            }
            settle(row, &lane, &key, &mut pos);
        }
        for sweep in 0..SWEEPS {
            if max_rank == 0 {
                break;
            }
            let downward = sweep % 2 == 0;
            for s in 1..=max_rank {
                let row = &mut rows[if downward {
                    s as usize
                } else {
                    (max_rank - s) as usize
                }];
                let (start, list) = if downward {
                    (&up_start, &up)
                } else {
                    (&down_start, &down)
                };
                for &i in row.iter() {
                    let mut sum = 0f64;
                    let count = start[i + 1] - start[i];
                    for k in start[i]..start[i + 1] {
                        sum += pos[list[k]];
                    }
                    key[i] = if count > 0 {
                        sum / count as f64
                    } else {
                        pos[i]
                    };
                }
                settle(row, &lane, &key, &mut pos);
            }
        }

        // The items of one rank that share a lane sit side by side as a run.
        let gap = |a: usize, b: usize| {
            if dummy[a] || dummy[b] {
                opt.edge_sep
            } else {
                opt.node_sep
            }
        };

        // Lane breadth: enough for its fullest rank, and for its title.
        let mut breadth = vec![0f64; lane_count];
        let mut head = if !self.lanes.is_empty() {
            LANE_HEAD
        } else {
            0.0
        };
        for (k, &id) in self.lanes.iter().enumerate() {
            let lane = &nodes[id];
            breadth[k] = (lane.min_w - 2.0 * pad[k]).max(0.0);
            if lane.pad_top > head {
                head = lane.pad_top;
            }
        }
        for row in rows.iter() {
            each_run(row, &lane, &mut |from, to| {
                let size = run_size(row, from, to, &size, &dummy, opt);
                let lane = lane[row[from]] as usize;
                if size > breadth[lane] {
                    breadth[lane] = size;
                }
            });
        }
        let mut lane_start = vec![0f64; lane_count + 1];
        for k in 0..lane_count {
            lane_start[k + 1] = lane_start[k] + breadth[k] + 2.0 * pad[k];
        }

        // Across the flow: each lane's items start centered, then line up with their neighbours in
        // the same lane as far as the lane and the items beside them allow.
        let mut c = vec![0f64; total];
        let mut want = vec![0f64; total];
        for row in rows.iter() {
            each_run(row, &lane, &mut |from, to| {
                let lane = lane[row[from]] as usize;
                let mut at = (lane_start[lane] + pad[lane] + (lane_start[lane + 1] - pad[lane])
                    - run_size(row, from, to, &size, &dummy, opt))
                    / 2.0;
                for k in from..=to {
                    c[row[k]] = at + size[row[k]] / 2.0;
                    if k < to {
                        at += size[row[k]] + gap(row[k], row[k + 1]);
                    }
                }
            });
        }
        for pass in 0..4 {
            if max_rank == 0 {
                break;
            }
            let downward = pass % 2 == 0;
            for s in 1..=max_rank {
                let row = &rows[if downward {
                    s as usize
                } else {
                    (max_rank - s) as usize
                }];
                let (start, list) = if downward {
                    (&up_start, &up)
                } else {
                    (&down_start, &down)
                };
                for &i in row {
                    let mut sum = 0f64;
                    let mut count = 0;
                    for k in start[i]..start[i + 1] {
                        if lane[list[k]] != lane[i] {
                            continue;
                        }
                        sum += c[list[k]];
                        count += 1;
                    }
                    want[i] = if count > 0 { sum / count as f64 } else { c[i] };
                }
                each_run(row, &lane, &mut |from, to| {
                    let lane = lane[row[from]] as usize;
                    let mut floor = lane_start[lane] + pad[lane];
                    for k in from..=to {
                        let i = row[k];
                        c[i] = want[i].max(floor + size[i] / 2.0);
                        if k < to {
                            floor = c[i] + size[i] / 2.0 + gap(i, row[k + 1]);
                        }
                    }
                    let mut ceil = lane_start[lane + 1] - pad[lane];
                    for k in (from..=to).rev() {
                        let i = row[k];
                        c[i] = c[i].min(ceil - size[i] / 2.0);
                        if k > from {
                            ceil = c[i] - size[i] / 2.0 - gap(row[k - 1], i);
                        }
                    }
                });
            }
        }

        // Along the flow: one band per rank, after the lane titles.
        let mut band_start = vec![0f64; max_rank as usize + 1];
        let mut band_end = vec![0f64; max_rank as usize + 1];
        // The room after a rank is for the relations that turn there or are only as long as they
        // were asked to be. Where just plain handoffs and straight lines pass, the ranks close up,
        // as far as the label of a handoff between them leaves it a line to sit on.
        let mut sep = vec![opt.rank_sep / f64::from(self.step); max_rank as usize + 1];
        if plain.contains(&1) {
            let full = opt.rank_sep / f64::from(self.step);
            sep.fill(full / CLOSE);
            for k in 0..m {
                let e = &self.ledges[k];
                let a = if e.reversed { e.head } else { e.tail };
                let b = if e.reversed { e.tail } else { e.head };
                if plain[k] != 0 {
                    if e.label_h <= 0.0 || span[k] > self.step {
                        continue;
                    }
                    let room = full.min(
                        (e.label_h + 2.0 * TURN - depth[a].min(depth[b]) / 2.0)
                            / f64::from(span[k]),
                    );
                    for r in rank[a]..rank[b] {
                        if room > sep[r as usize] {
                            sep[r as usize] = room;
                        }
                    }
                    continue;
                }
                let d0 = first_dummy[k];
                if d0 < 0 || span[k] <= self.step * e.minlen {
                    for r in rank[a]..rank[b] {
                        sep[r as usize] = full;
                    }
                    continue;
                }
                let d0 = i64::from(d0);
                let span = i64::from(span[k]);
                for d in (d0 - 1)..(d0 + span - 1) {
                    let u = if d < d0 { a } else { d as usize };
                    let v = if d + 1 < d0 + span - 1 {
                        (d + 1) as usize
                    } else {
                        b
                    };
                    if lane[u] != lane[v] || (c[u] - c[v]).abs() >= 0.5 {
                        sep[rank[u] as usize] = full;
                    }
                }
            }
        }
        let mut at = head + end_pad;
        for r in 0..=max_rank {
            let mut d = 0f64;
            for &i in rows[r as usize].iter() {
                if depth[i] > d {
                    d = depth[i];
                }
            }
            band_start[r as usize] = at;
            band_end[r as usize] = at + d;
            at += d + if r == max_rank { 0.0 } else { sep[r as usize] };
        }
        let length = at + end_pad;
        let breadth_total = lane_start[lane_count];

        let flip = |a: f64| if self.flipped { head + length - a } else { a };
        let px = |c: f64, a: f64| if self.sideways { flip(a) } else { c };
        let py = |c: f64, a: f64| if self.sideways { c } else { flip(a) };
        let axis = if self.sideways { 1.0 } else { 0.0 };

        for k in 0..self.real.len() {
            let a = (band_start[rank[k] as usize] + band_end[rank[k] as usize]) / 2.0;
            let node = &mut nodes[self.real[k]];
            node.x = px(c[k], a);
            node.y = py(c[k], a);
        }
        for (k, &id) in self.lanes.iter().enumerate() {
            let mid = (lane_start[k] + lane_start[k + 1]) / 2.0;
            let size = lane_start[k + 1] - lane_start[k];
            let lane = &mut nodes[id];
            lane.x = if self.sideways { length / 2.0 } else { mid };
            lane.y = if self.sideways { mid } else { length / 2.0 };
            lane.w = if self.sideways { length } else { size };
            lane.h = if self.sideways { size } else { length };
            lane.pad_top = head;
        }

        // Groups inside a lane are drawn around their members, deepest first.
        let mut depth_of = vec![0i32; nodes.len()];
        let mut inner: Vec<usize> = Vec::new();
        for i in 0..nodes.len() {
            if !nodes[i].is_group || self.lane_slot[i] >= 0 || self.item_of[i] >= 0 {
                continue;
            }
            let mut d = 0i32;
            let mut p = nodes[i].parent;
            while p >= 0 && (d as usize) < MAX_HOPS {
                d += 1;
                p = nodes[p as usize].parent;
            }
            depth_of[i] = d;
            inner.push(i);
        }
        inner.sort_by(|&a, &b| depth_of[b].cmp(&depth_of[a]).then(a.cmp(&b)));
        let mut bounds = vec![f64::INFINITY; nodes.len() * 4];
        for i in 0..nodes.len() {
            bounds[i * 4 + 2] = f64::NEG_INFINITY;
            bounds[i * 4 + 3] = f64::NEG_INFINITY;
        }
        for &i in self.real.iter() {
            let p = nodes[i].parent;
            if p >= 0 && self.lane_slot[p as usize] < 0 {
                let node = &nodes[i];
                grow(&mut bounds, p as usize, node.x, node.y, node.w, node.h);
            }
        }
        for &g in inner.iter() {
            if bounds[g * 4] == f64::INFINITY {
                continue;
            }
            let (min_x, min_y, max_x, max_y) = (
                bounds[g * 4],
                bounds[g * 4 + 1],
                bounds[g * 4 + 2],
                bounds[g * 4 + 3],
            );
            let node = &mut nodes[g];
            let w = (max_x - min_x + 2.0 * node.pad_x).max(node.min_w);
            node.x = (min_x + max_x) / 2.0;
            node.w = w;
            node.y = (min_y - node.pad_top + max_y + node.pad_bottom) / 2.0;
            node.h = max_y - min_y + node.pad_top + node.pad_bottom;
            let p = node.parent;
            if p >= 0 && self.lane_slot[p as usize] < 0 {
                let (x, y, w, h) = (node.x, node.y, node.w, node.h);
                grow(&mut bounds, p as usize, x, y, w, h);
            }
        }

        // The route of every relation: from its first item to its last, turning in the ranks it
        // passes.
        for e in edges.iter_mut() {
            e.route.clear();
        }
        for k in 0..m {
            let e = &self.ledges[k];
            let out = &mut edges[self.edge_of[k]];
            let a = if e.reversed { e.head } else { e.tail };
            let b = if e.reversed { e.tail } else { e.head };
            let mut pts: Vec<f64> = Vec::with_capacity(8);
            pts.push(c[a]);
            pts.push(
                band_end[rank[a] as usize]
                    - (band_end[rank[a] as usize] - band_start[rank[a] as usize] - depth[a]) / 2.0,
            );
            let d0 = first_dummy[k];
            if d0 >= 0 {
                let d0 = d0 as usize;
                for d in d0..(d0 + span[k] as usize - 1) {
                    pts.push(c[d]);
                    pts.push(band_start[rank[d] as usize]);
                    if band_end[rank[d] as usize] > band_start[rank[d] as usize] {
                        pts.push(c[d]);
                        pts.push(band_end[rank[d] as usize]);
                    }
                }
            }
            pts.push(c[b]);
            pts.push(
                band_start[rank[b] as usize]
                    + (band_end[rank[b] as usize] - band_start[rank[b] as usize] - depth[b]) / 2.0,
            );
            let mut route: Vec<f64> = Vec::with_capacity(pts.len() / 2 * 3);
            for p in (0..pts.len()).step_by(2) {
                route.push(px(pts[p], pts[p + 1]));
                route.push(py(pts[p], pts[p + 1]));
                route.push(axis);
            }
            if e.reversed {
                let mut i = 0;
                let mut j = route.len() - 3;
                while i < j {
                    for t in 0..2 {
                        route.swap(i + t, j + t);
                    }
                    i += 3;
                    j -= 3;
                }
            }
            out.route = route;
            let label = label_item[k];
            if label >= 0 {
                let label = label as usize;
                let mid = (band_start[rank[label] as usize] + band_end[rank[label] as usize]) / 2.0;
                out.label_x = px(c[label], mid);
                out.label_y = py(c[label], mid);
            } else {
                out.label_x = (out.route[0] + out.route[out.route.len() - 3]) / 2.0;
                out.label_y = (out.route[1] + out.route[out.route.len() - 2]) / 2.0;
            }
        }

        // An edge that starts or ends at a lane or a group runs straight to its border.
        for e in edges.iter_mut() {
            if !e.route.is_empty() || e.src == e.dst {
                continue;
            }
            let a = &nodes[e.src];
            let b = &nodes[e.dst];
            let (ax, ay) = border(a, b);
            let (bx, by) = border(b, a);
            let flat = if (bx - ax).abs() > (by - ay).abs() {
                1.0
            } else {
                0.0
            };
            e.route = vec![ax, ay, flat, bx, by, flat];
            e.label_x = (ax + bx) / 2.0;
            e.label_y = (ay + by) / 2.0;
        }

        // A handoff can leave towards the other lane, then arrive along the flow (or vice versa).
        // In particular, a decision's return branch must not leave through its incoming side and
        // retrace the path past the preceding step. Try the two single-corner routes, keeping the
        // ranked route when a node or another label blocks them.
        let mut routed = vec![0u8; m];
        let route_points: usize = edges.iter().map(|e| e.route.len() / 3).sum();
        let work = m as f64 * (self.real.len() + edges.len() + route_points) as f64;
        if work <= ROUTE_BUDGET {
            let labels: Vec<(f64, f64)> = edges
                .iter()
                .map(|e| {
                    (
                        if self.sideways { e.label_y } else { e.label_x },
                        flip(if self.sideways { e.label_x } else { e.label_y }),
                    )
                })
                .collect();
            let mut router = Router {
                edges,
                labels,
                c: &c,
                size: &size,
                depth: &depth,
                band_start: &band_start,
                band_end: &band_end,
                rank: &rank,
                real_len: self.real.len(),
                sideways: self.sideways,
                breadth_total,
                length,
                head,
                flipped: self.flipped,
                axis,
            };
            for (k, routed_k) in routed.iter_mut().enumerate() {
                let e = &self.ledges[k];
                let ei = self.edge_of[k];
                let a = e.tail;
                let b = e.head;
                if lane[a] == lane[b] || (c[b] - c[a]).abs() < 1.0 {
                    continue;
                }
                let ac = c[a];
                let bc = c[b];
                let aa = router.amid(a);
                let ba = router.amid(b);
                let dc = sign(bc - ac);
                let da = sign(ba - aa);
                let label_w = e.label_w;
                let label_h = e.label_h;

                let handoff = Handoff {
                    edge: ei,
                    from: a,
                    to: b,
                };
                if router
                    .attempt(
                        &[
                            ac + dc * size[a] / 2.0,
                            aa,
                            bc,
                            aa,
                            bc,
                            ba - da * depth[b] / 2.0,
                        ],
                        2.0,
                        handoff,
                        label_w,
                        label_h,
                    )
                    .is_some()
                {
                    *routed_k = 1;
                    continue;
                }
                if router
                    .attempt(
                        &[
                            ac,
                            aa + da * depth[a] / 2.0,
                            ac,
                            ba,
                            bc - dc * size[b] / 2.0,
                            ba,
                        ],
                        2.0,
                        handoff,
                        label_w,
                        label_h,
                    )
                    .is_some()
                {
                    *routed_k = 1;
                    continue;
                }
                // With both corners blocked, as for a return past the steps between, the handoff
                // goes around: out to the margin of a lane, along it, and back in level with its
                // end. It takes the margin that crosses the fewest other relations and shares the
                // fewest ends with them, then the shortest way.
                if work > ROUTE_BUDGET / RELAY {
                    continue;
                }
                let mut around: Vec<(Vec<f64>, f64, usize)> = Vec::new();
                let order = if lane[a] < lane[b] {
                    vec![lane[b], lane[a]]
                } else {
                    vec![lane[a], lane[b]]
                };
                for l in order {
                    let l = l as usize;
                    // A lane that keeps no margin yet has less room there, and the handoff stays
                    // inside it.
                    let inset = (pad[l] - LEG).max((pad[l] / 2.0).min(MARGIN - LEG));
                    for ch in [lane_start[l] + inset, lane_start[l + 1] - inset] {
                        if (ch - ac).abs() < size[a] / 2.0 + 4.0
                            || (ch - bc).abs() < size[b] / 2.0 + 4.0
                        {
                            continue;
                        }
                        let pts = vec![
                            ac + sign(ch - ac) * size[a] / 2.0,
                            aa,
                            ch,
                            aa,
                            ch,
                            ba,
                            bc + sign(ch - bc) * size[b] / 2.0,
                            ba,
                        ];
                        let mut cost =
                            (router.meets(router.px(pts[0], pts[1]), router.py(pts[0], pts[1]), ei)
                                + router.meets(
                                    router.px(pts[6], pts[7]),
                                    router.py(pts[6], pts[7]),
                                    ei,
                                )) as f64
                                * 1e6;
                        for p in (2..pts.len()).step_by(2) {
                            let x0 = router.px(pts[p - 2], pts[p - 1]);
                            let y0 = router.py(pts[p - 2], pts[p - 1]);
                            let x1 = router.px(pts[p], pts[p + 1]);
                            let y1 = router.py(pts[p], pts[p + 1]);
                            cost += router.crossings(
                                x0.min(x1),
                                y0.min(y1),
                                x1.max(x0),
                                y1.max(y0),
                                ei,
                            ) as f64
                                * 1e6
                                + (x1 - x0).abs()
                                + (y1 - y0).abs();
                        }
                        around.push((pts, cost, l));
                    }
                }
                around.sort_by(|p, q| p.1.partial_cmp(&q.1).unwrap_or(std::cmp::Ordering::Equal));
                for (pts, _, l) in around {
                    if router
                        .attempt(&pts, 1.0, handoff, label_w, label_h)
                        .is_some()
                    {
                        *routed_k = 1;
                        used[l] = 1;
                        break;
                    }
                }
            }
        }

        Placed {
            width: if self.sideways { length } else { breadth_total },
            height: if self.sideways { breadth_total } else { length },
            routed,
            margins: used,
            work,
        }
    }
}

/// Orders the items of a rank and keeps the place each one has in it.
fn settle(row: &mut [usize], lane: &[i32], key: &[f64], pos: &mut [f64]) {
    row.sort_by(|&a, &b| {
        lane[a]
            .cmp(&lane[b])
            .then_with(|| {
                key[a]
                    .partial_cmp(&key[b])
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.cmp(&b))
    });
    for (k, &i) in row.iter().enumerate() {
        pos[i] = k as f64;
    }
}

/// The runs of a rank that share a lane: the items beside each other.
fn each_run(row: &[usize], lane: &[i32], visit: &mut dyn FnMut(usize, usize)) {
    let mut from = 0;
    while from < row.len() {
        let mut to = from;
        while to + 1 < row.len() && lane[row[to + 1]] == lane[row[from]] {
            to += 1;
        }
        visit(from, to);
        from = to + 1;
    }
}

/// The room a run of items takes across the flow.
fn run_size(
    row: &[usize],
    from: usize,
    to: usize,
    size: &[f64],
    dummy: &[bool],
    opt: &LayeredOptions,
) -> f64 {
    let gap = |a: usize, b: usize| {
        if dummy[a] || dummy[b] {
            opt.edge_sep
        } else {
            opt.node_sep
        }
    };
    let mut total = size[row[from]];
    for k in from..to {
        total += gap(row[k], row[k + 1]) + size[row[k + 1]];
    }
    total
}

/// Grows the box of a group around a member.
fn grow(bounds: &mut [f64], group: usize, x: f64, y: f64, w: f64, h: f64) {
    if x - w / 2.0 < bounds[group * 4] {
        bounds[group * 4] = x - w / 2.0;
    }
    if y - h / 2.0 < bounds[group * 4 + 1] {
        bounds[group * 4 + 1] = y - h / 2.0;
    }
    if x + w / 2.0 > bounds[group * 4 + 2] {
        bounds[group * 4 + 2] = x + w / 2.0;
    }
    if y + h / 2.0 > bounds[group * 4 + 3] {
        bounds[group * 4 + 3] = y + h / 2.0;
    }
}

/// The point where the line from one box's center to another's leaves the first box. When the
/// other center is inside the box, the point is on the nearest side instead.
fn border(from: &CNode, to: &CNode) -> (f64, f64) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let hw = (from.w / 2.0).max(0.5);
    let hh = (from.h / 2.0).max(0.5);
    if dx.abs() < hw && dy.abs() < hh {
        let to_side = hw - dx.abs();
        let to_end = hh - dy.abs();
        return if to_side < to_end {
            (from.x + if dx < 0.0 { -hw } else { hw }, to.y)
        } else {
            (to.x, from.y + if dy < 0.0 { -hh } else { hh })
        };
    }
    let scale = (if dx != 0.0 {
        hw / dx.abs()
    } else {
        f64::INFINITY
    })
    .min(if dy != 0.0 {
        hh / dy.abs()
    } else {
        f64::INFINITY
    });
    (from.x + dx * scale, from.y + dy * scale)
}

/// A handoff between two items: the relation, the item it leaves and the item it arrives at.
#[derive(Clone, Copy)]
struct Handoff {
    /// The relation.
    edge: usize,
    /// The item it leaves.
    from: usize,
    /// The item it arrives at.
    to: usize,
}

/// A stretch of the drawing: across the flow from `c0` to `c1`, along it from `a0` to `a1`.
#[derive(Clone, Copy)]
struct Box4 {
    c0: f64,
    a0: f64,
    c1: f64,
    a1: f64,
}

/// The routing of the handoffs between the lanes.
struct Router<'a> {
    edges: &'a mut [CEdge],
    /// Where the label of each edge sits, across the flow and along it.
    labels: Vec<(f64, f64)>,
    c: &'a [f64],
    size: &'a [f64],
    depth: &'a [f64],
    band_start: &'a [f64],
    band_end: &'a [f64],
    rank: &'a [i32],
    real_len: usize,
    sideways: bool,
    breadth_total: f64,
    length: f64,
    head: f64,
    flipped: bool,
    axis: f64,
}

impl Router<'_> {
    fn flip(&self, a: f64) -> f64 {
        if self.flipped {
            self.head + self.length - a
        } else {
            a
        }
    }

    fn px(&self, c: f64, a: f64) -> f64 {
        if self.sideways { self.flip(a) } else { c }
    }

    fn py(&self, c: f64, a: f64) -> f64 {
        if self.sideways { c } else { self.flip(a) }
    }

    /// Where an item sits along the flow.
    fn amid(&self, i: usize) -> f64 {
        let r = self.rank[i] as usize;
        (self.band_start[r] + self.band_end[r]) / 2.0
    }

    /// Whether a stretch of the drawing overlaps a box.
    fn overlaps(&self, b: Box4, lc: f64, la: f64, cw: f64, ah: f64) -> bool {
        b.c1 > lc - cw / 2.0 && b.c0 < lc + cw / 2.0 && b.a1 > la - ah / 2.0 && b.a0 < la + ah / 2.0
    }

    /// Whether a stretch of the drawing is free of the items and of the labels of the other
    /// relations.
    fn clear(&self, b: Box4, src: i64, dst: i64, ei: usize) -> bool {
        for i in 0..self.real_len {
            if i as i64 != src
                && i as i64 != dst
                && self.overlaps(
                    b,
                    self.c[i],
                    self.amid(i),
                    self.size[i] + 8.0,
                    self.depth[i] + 8.0,
                )
            {
                return false;
            }
        }
        for (i, e) in self.edges.iter().enumerate() {
            if i != ei
                && e.label_w > 0.0
                && e.src != e.dst
                && self.overlaps(
                    b,
                    self.labels[i].0,
                    self.labels[i].1,
                    if self.sideways { e.label_h } else { e.label_w },
                    if self.sideways { e.label_w } else { e.label_h },
                )
            {
                return false;
            }
        }
        true
    }

    /// How many other relations have a stretch inside a box.
    fn crossings(&self, x0: f64, y0: f64, x1: f64, y1: f64, ei: usize) -> usize {
        let mut count = 0;
        for (i, e) in self.edges.iter().enumerate() {
            if i == ei {
                continue;
            }
            let route = &e.route;
            let mut j = 3;
            while j < route.len() {
                if route[j - 3].max(route[j]) > x0
                    && route[j - 3].min(route[j]) < x1
                    && route[j - 2].max(route[j + 1]) > y0
                    && route[j - 2].min(route[j + 1]) < y1
                {
                    count += 1;
                    break;
                }
                j += 3;
            }
        }
        count
    }

    /// How many other relations start or end at the same point.
    fn meets(&self, x: f64, y: f64, ei: usize) -> usize {
        let mut count = 0;
        for (i, e) in self.edges.iter().enumerate() {
            if i == ei {
                continue;
            }
            let route = &e.route;
            if route.is_empty() {
                continue;
            }
            let last = route.len() - 3;
            if (route[0] - x).abs() < 1.0 && (route[1] - y).abs() < 1.0
                || (route[last] - x).abs() < 1.0 && (route[last + 1] - y).abs() < 1.0
            {
                count += 1;
            }
        }
        count
    }

    /// Whether a relation would run along a line another one already uses. Those that leave the
    /// same step or arrive at the same step may share a trunk; any other pair must not.
    fn retraces(&self, x0: f64, y0: f64, x1: f64, y1: f64, ei: usize) -> bool {
        let upright = (x1 - x0).abs() < 0.5;
        let (lo, hi) = if upright {
            (y0.min(y1), y0.max(y1))
        } else {
            (x0.min(x1), x0.max(x1))
        };
        let line = if upright { x0 } else { y0 };
        for (i, other) in self.edges.iter().enumerate() {
            if i == ei || other.src == self.edges[ei].src || other.dst == self.edges[ei].dst {
                continue;
            }
            let route = &other.route;
            let mut j = 3;
            while j < route.len() {
                let (u0, u1) = if upright {
                    (route[j - 3], route[j])
                } else {
                    (route[j - 2], route[j + 1])
                };
                if (u0 - line).abs() < NEAR && (u1 - line).abs() < NEAR {
                    let (v0, v1) = if upright {
                        (route[j - 2], route[j + 1])
                    } else {
                        (route[j - 3], route[j])
                    };
                    if hi.min(v0.max(v1)) - lo.max(v0.min(v1)) > 1.0 {
                        return true;
                    }
                }
                j += 3;
            }
        }
        false
    }

    /// Tries to give a handoff a route of its own, and keeps it when nothing blocks it.
    fn attempt(
        &mut self,
        pts: &[f64],
        margin: f64,
        handoff: Handoff,
        label_w: f64,
        label_h: f64,
    ) -> Option<()> {
        let Handoff {
            edge: ei,
            from: a,
            to: b,
        } = handoff;
        for p in (2..pts.len()).step_by(2) {
            let stretch = Box4 {
                c0: pts[p - 2].min(pts[p]) - margin,
                a0: pts[p - 1].min(pts[p + 1]) - margin,
                c1: pts[p - 2].max(pts[p]) + margin,
                a1: pts[p - 1].max(pts[p + 1]) + margin,
            };
            if !self.clear(stretch, a as i64, b as i64, ei) {
                return None;
            }
            if self.retraces(
                self.px(pts[p - 2], pts[p - 1]),
                self.py(pts[p - 2], pts[p - 1]),
                self.px(pts[p], pts[p + 1]),
                self.py(pts[p], pts[p + 1]),
                ei,
            ) {
                return None;
            }
        }
        let mut label = None;
        if self.edges[ei].label_w > 0.0 {
            let cw = label_w;
            let ah = label_h;
            let out_label_w = self.edges[ei].label_w;
            let out_label_h = self.edges[ei].label_h;
            let mut lc = 0f64;
            let mut la = 0f64;
            let mut longest = -1f64;
            for p in (2..pts.len()).step_by(2) {
                let cc = (pts[p - 2] + pts[p]) / 2.0;
                let ca = (pts[p - 1] + pts[p + 1]) / 2.0;
                let across = (pts[p] - pts[p - 2]).abs();
                let along = (pts[p + 1] - pts[p - 1]).abs();
                let run = across + along;
                if run <= longest
                    || (if across > 0.0 {
                        across < cw + 8.0
                    } else {
                        along < ah + 8.0
                    })
                {
                    continue;
                }
                if cc - cw / 2.0 < 0.0
                    || cc + cw / 2.0 > self.breadth_total
                    || ca - ah / 2.0 < 0.0
                    || ca + ah / 2.0 > self.length
                {
                    continue;
                }
                // Include the ends here: the label must clear both shapes it connects.
                let label_box = Box4 {
                    c0: cc - cw / 2.0,
                    a0: ca - ah / 2.0,
                    c1: cc + cw / 2.0,
                    a1: ca + ah / 2.0,
                };
                if !self.clear(label_box, -1, -1, ei) {
                    continue;
                }
                // A label moved onto a handoff must not cover a return path that keeps its
                // ranked route. A segment's whole box is kept clear, including its curve.
                let lx = self.px(cc, ca);
                let ly = self.py(cc, ca);
                if self.crossings(
                    lx - out_label_w / 2.0,
                    ly - out_label_h / 2.0,
                    lx + out_label_w / 2.0,
                    ly + out_label_h / 2.0,
                    ei,
                ) > 0
                {
                    continue;
                }
                lc = cc;
                la = ca;
                longest = run;
            }
            if longest < 0.0 {
                return None;
            }
            label = Some((lc, la));
        }
        // A point short of each corner and one past it keep the turn close to the corner, so
        // that the line stays under its label and the handoff takes little room.
        let mut way: Vec<f64> = vec![pts[0], pts[1]];
        for p in (2..pts.len()).step_by(2) {
            let dx = pts[p] - pts[p - 2];
            let dy = pts[p + 1] - pts[p - 1];
            let run = dx.abs() + dy.abs();
            let first = if p > 2 { TURN } else { 0.0 };
            let last = if p + 2 < pts.len() { TURN } else { 0.0 };
            if run > first + last + 1.0 {
                if first != 0.0 {
                    way.extend_from_slice(&[
                        pts[p - 2] + dx / run * first,
                        pts[p - 1] + dy / run * first,
                    ]);
                }
                if last != 0.0 {
                    way.extend_from_slice(&[
                        pts[p] - dx / run * last,
                        pts[p + 1] - dy / run * last,
                    ]);
                }
            }
            way.extend_from_slice(&[pts[p], pts[p + 1]]);
        }
        let mut route: Vec<f64> = Vec::with_capacity(way.len() / 2 * 3);
        for p in (0..way.len()).step_by(2) {
            let crosswise = if p == 0 {
                way[0] != way[2]
            } else {
                way[p] != way[p - 2]
            };
            route.push(self.px(way[p], way[p + 1]));
            route.push(self.py(way[p], way[p + 1]));
            route.push(if crosswise {
                1.0 - self.axis
            } else {
                self.axis
            });
        }
        let placed = label.map(|(lc, la)| (self.px(lc, la), self.py(lc, la)));
        if let Some((lc, la)) = label {
            self.labels[ei] = (lc, la);
        }
        let out = &mut self.edges[ei];
        out.route = route;
        if let Some((lx, ly)) = placed {
            out.label_x = lx;
            out.label_y = ly;
        }
        Some(())
    }
}
