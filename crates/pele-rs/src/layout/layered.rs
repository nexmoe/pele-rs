//! Layered (Sugiyama-style) layout of one flat graph. Ranks run down the y axis; callers
//! rotate the result for other directions. Ported from pele's `layout/layered.ts`, whose
//! header lists what the layout of edges aims for; the names of the steps here are the same.
//!
//! The port keeps pele's arithmetic in the same order, so that the coordinates come out the
//! same to the last bit.

#![allow(
    clippy::needless_range_loop,
    reason = "ported index arithmetic reads more plainly with explicit indices"
)]

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Node,
    Dummy,
    Label,
    StartPort,
    EndPort,
}

#[derive(Debug, Clone)]
pub(crate) struct LNode {
    pub w: f64,
    pub h: f64,
    pub kind: Kind,
    pub pin: i32,
    /// A port's cross-flow destination; NaN if unknown. Fully hinted port ranks keep this order.
    pub hint: f64,
    /// Initial cross-flow position; NaN if unknown. Used only when every node in a rank has one.
    pub key: f64,
    /// How much of the side that edges meet may be shared out between them; 0 keeps them all
    /// at its middle.
    pub span: f64,
    pub x: f64,
    pub y: f64,
    pub rank: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct LEdge {
    pub tail: usize,
    pub head: usize,
    pub minlen: i32,
    pub label_w: f64,
    pub label_h: f64,
    pub tail_dx: f64,
    pub head_dx: f64,
    /// An end that is held where its offset puts it, and takes no part in sharing out a
    /// node's side.
    pub tail_fixed: bool,
    pub head_fixed: bool,
    pub reversed: bool,
    pub points: Vec<f64>,
    pub label_x: f64,
    pub label_y: f64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LayeredOptions {
    pub node_sep: f64,
    pub edge_sep: f64,
    pub rank_sep: f64,
    pub port_sep: f64,
    /// Route edges at right angles, each on a track of its own across the gap between ranks.
    pub tracks: bool,
    /// Draw every edge straight from point to point: none is given a level to turn onto.
    pub straight: bool,
    /// The caller sets the ends of the edges on one side of a node apart itself.
    pub apart: bool,
    /// The room that the marker at the head of an edge takes.
    pub head_room: f64,
    /// Keep each group contiguous within ranks: `of` maps nodes to groups, `parent` groups to
    /// parents, both with -1 for none.
    pub groups: Option<(Vec<i32>, Vec<i32>)>,
    /// Compute cross-flow positions in `x` without full routing.
    pub order_only: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LayeredResult {
    pub width: f64,
    pub height: f64,
    /// With `order_only`: final crossings and crossings before grouping or reordering.
    pub crossings: i64,
    pub declared: i64,
}

pub(crate) fn lnode(w: f64, h: f64, kind: Kind) -> LNode {
    LNode {
        w,
        h,
        kind,
        pin: 0,
        hint: f64::NAN,
        key: f64::NAN,
        span: 0.0,
        x: 0.0,
        y: 0.0,
        rank: 0,
    }
}

pub(crate) fn ledge(tail: usize, head: usize, minlen: i32, label_w: f64, label_h: f64) -> LEdge {
    LEdge {
        tail,
        head,
        minlen,
        label_w,
        label_h,
        tail_dx: 0.0,
        head_dx: 0.0,
        tail_fixed: false,
        head_fixed: false,
        reversed: false,
        points: Vec::new(),
        label_x: 0.0,
        label_y: 0.0,
    }
}

const MAX_SWEEPS: usize = 24;
/// An edge gets a bend point in every rank it skips. Past this many, the longest edges are
/// drawn as one direct curve instead.
const MAX_DUMMIES: usize = 20000;
const PORT_PITCH: f64 = 16.0;
const MIN_PITCH: f64 = 8.0;
const MIN_SLOPE: f64 = 0.125;
const MAX_GAP: f64 = 3.0;
const TRACK_MARGIN: f64 = 18.0;
const TRACK_PITCH: f64 = 10.0;
const TRACK_SPREAD: f64 = 14.0;
const LEVEL_WIDE: f64 = 2.0;
const LEVEL_MARGIN: f64 = 28.0;
const LEVEL_PITCH: f64 = 12.0;
const LEVEL_LEAD: f64 = 12.0;
const LEVEL_CLEAR: f64 = 8.0;
const LEVEL_TOUCH: f64 = 4.0;
const CROSSED_SLOPE: f64 = 0.5;
const IN_LINE: f64 = 0.5;
const SNAP: f64 = 12.0;
const LEAN_CLEAR: f64 = 40.0;
const MAX_WEIGHED: usize = 400;
const SAMPLES: usize = 16;

const HELD: u8 = 0;
const LEAVES: u8 = 1;
const ARRIVES: u8 = 2;

const NONE: usize = usize::MAX;

fn is_dummy(kind: Kind) -> bool {
    kind == Kind::Dummy || kind == Kind::Label
}

/// Orders by a JavaScript comparator's number: negative, positive, or zero (and NaN) as equal.
pub(crate) fn by(d: f64) -> Ordering {
    if d < 0.0 {
        Ordering::Less
    } else if d > 0.0 {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

fn sort_range(list: &mut [usize], from: usize, to: usize, key: &[i64]) {
    for i in from + 1..to {
        let item = list[i];
        let k = key[item];
        let mut j = i;
        while j > from && (key[list[j - 1]] > k || (key[list[j - 1]] == k && list[j - 1] > item)) {
            list[j] = list[j - 1];
            j -= 1;
        }
        list[j] = item;
    }
}

/// The graph that gets ordered and positioned: every edge cut into rank-to-rank segments.
struct Graph {
    n: usize,
    real: usize,
    max_rank: usize,
    w: Vec<f64>,
    kind: Vec<Kind>,
    rank: Vec<usize>,
    /// Segment endpoints, and where each segment attaches on them.
    ef: Vec<usize>,
    et: Vec<usize>,
    efd: Vec<f64>,
    etd: Vec<f64>,
    succ_start: Vec<usize>,
    succ_e: Vec<usize>,
    pred_start: Vec<usize>,
    pred_e: Vec<usize>,
    layer_start: Vec<usize>,
    order: Vec<usize>,
    pos: Vec<usize>,
    offsets: bool,
    /// Ranks whose order must remain fixed.
    fixed: Vec<bool>,
    grp: Option<Vec<i32>>,
    group_parent: Option<Vec<i32>>,
}

/// The pieces of edges that each cross one gap.
#[derive(Default)]
struct Pieces {
    from: Vec<f64>,
    to: Vec<f64>,
    rank: Vec<usize>,
    lower: Vec<usize>,
    track: Vec<i64>,
    levels: Vec<i64>,
    wide: Vec<u8>,
}

pub(crate) fn layered(
    nodes: &mut [LNode],
    edges: &mut [LEdge],
    opt: &LayeredOptions,
) -> LayeredResult {
    let real = nodes.len();
    let none = LayeredResult::default();
    if real == 0 {
        return none;
    }
    let m = edges.len();
    if real == 1 && m == 0 && nodes[0].kind == Kind::Node {
        let node = &mut nodes[0];
        node.rank = 0;
        node.x = node.w / 2.0;
        node.y = node.h / 2.0;
        return LayeredResult {
            width: node.w,
            height: node.h,
            ..none
        };
    }

    // Every edge gets a middle rank of its own when labels or parallel edges need the room.
    let mut step = 1;
    if m > 0 {
        let mut seen = HashSet::new();
        for e in edges.iter() {
            let a = e.tail.min(e.head);
            let b = e.tail.max(e.head);
            let key = a * real + b;
            if e.label_w > 0.0 || seen.contains(&key) {
                step = 2;
                break;
            }
            seen.insert(key);
        }
    }

    orient(nodes, edges);
    rank(nodes, edges, step);

    let mut span = vec![0usize; m];
    let mut dummies = 0usize;
    for ei in 0..m {
        let e = &edges[ei];
        span[ei] = (nodes[e.head].rank - nodes[e.tail].rank).unsigned_abs() as usize;
        if span[ei] > 1 {
            dummies += span[ei] - 1;
        }
    }
    let mut direct = vec![false; m];
    if dummies > MAX_DUMMIES {
        let mut longest: Vec<usize> = (0..m).filter(|&ei| span[ei] > 1).collect();
        longest.sort_by(|&a, &b| span[b].cmp(&span[a]).then(a.cmp(&b)));
        for ei in longest {
            if dummies <= MAX_DUMMIES {
                break;
            }
            direct[ei] = true;
            dummies -= span[ei] - 1;
        }
    }

    let n = real + dummies;
    let mut w = vec![0.0; n];
    let mut h = vec![0.0; n];
    let mut kind = vec![Kind::Node; n];
    let mut rank_of = vec![0usize; n];
    let mut max_rank = 0;
    for i in 0..real {
        let node = &nodes[i];
        w[i] = node.w;
        h[i] = node.h;
        kind[i] = node.kind;
        rank_of[i] = node.rank as usize;
        max_rank = max_rank.max(node.rank as usize);
    }

    let mut segments = 0;
    for ei in 0..m {
        if !direct[ei] {
            segments += span[ei];
        }
    }
    let mut ef = vec![0usize; segments];
    let mut et = vec![0usize; segments];
    let mut efd = vec![0.0; segments];
    let mut etd = vec![0.0; segments];
    let mut first_dummy = vec![NONE; m];
    // Which way the edges with a label in each rank head: 1 down, 2 up, 3 both.
    let mut heads = vec![0u8; max_rank + 1];
    let mut label_node = vec![NONE; m];
    let spans = (0..real).any(|i| nodes[i].span > 0.0 && nodes[i].kind == Kind::Node);
    let mut efk = vec![HELD; if spans { segments } else { 0 }];
    let mut etk = vec![HELD; if spans { segments } else { 0 }];
    let mut first_seg = vec![NONE; m];
    let mut last_seg = vec![NONE; m];
    let mut key = vec![f64::NAN; n];
    for i in 0..real {
        key[i] = nodes[i].key;
    }
    let mut offsets = false;
    {
        let mut next = real;
        let mut ce = 0;
        for ei in 0..m {
            if direct[ei] {
                continue;
            }
            let e = &edges[ei];
            let (a, b) = if e.reversed {
                (e.head, e.tail)
            } else {
                (e.tail, e.head)
            };
            let (a_dx, b_dx) = if e.reversed {
                (e.head_dx, e.tail_dx)
            } else {
                (e.tail_dx, e.head_dx)
            };
            if a_dx != 0.0 || b_dx != 0.0 {
                offsets = true;
            }
            let ra = rank_of[a] as i64;
            let rb = rank_of[b] as i64;
            let mut label_rank = -1i64;
            if e.label_w > 0.0 {
                // The label goes halfway along; see pele for the rule.
                label_rank = ra + ((rb - ra) >> 1);
                if step == 2 && rb - ra >= 4 {
                    if label_rank & 1 == 1 {
                        label_rank += if e.reversed { 1 } else { -1 };
                    }
                } else if step == 2 && label_rank & 1 == 0 {
                    label_rank += if e.reversed { 1 } else { -1 };
                }
            }
            let mut prev = a;
            if rb - ra > 1 {
                first_dummy[ei] = next;
            }
            first_seg[ei] = ce;
            if spans
                && !(if e.reversed {
                    e.head_fixed
                } else {
                    e.tail_fixed
                })
            {
                efk[ce] = if e.reversed { ARRIVES } else { LEAVES };
            }
            for r in ra + 1..rb {
                let d = next;
                next += 1;
                rank_of[d] = r as usize;
                key[d] = key[a] + ((key[b] - key[a]) * (r - ra) as f64) / (rb - ra) as f64;
                if r == label_rank {
                    w[d] = e.label_w;
                    h[d] = e.label_h;
                    kind[d] = Kind::Label;
                    label_node[ei] = d;
                    heads[r as usize] |= if e.reversed { 2 } else { 1 };
                } else {
                    kind[d] = Kind::Dummy;
                }
                ef[ce] = prev;
                et[ce] = d;
                efd[ce] = if prev == a { a_dx } else { 0.0 };
                ce += 1;
                prev = d;
            }
            ef[ce] = prev;
            et[ce] = b;
            efd[ce] = if prev == a { a_dx } else { 0.0 };
            etd[ce] = b_dx;
            if spans
                && !(if e.reversed {
                    e.tail_fixed
                } else {
                    e.head_fixed
                })
            {
                etk[ce] = if e.reversed { LEAVES } else { ARRIVES };
            }
            last_seg[ei] = ce;
            ce += 1;
        }
    }

    // Bends belong to the innermost group containing both edge endpoints.
    let mut grp = None;
    if let Some((of, parent)) = &opt.groups {
        let mut g = vec![0i32; n];
        g[..real].copy_from_slice(&of[..real]);
        let depth = |mut gr: i32| {
            let mut d = 0;
            while gr >= 0 {
                d += 1;
                gr = parent[gr as usize];
            }
            d
        };
        let lca = |mut x: i32, mut y: i32| {
            let mut dx = depth(x);
            let mut dy = depth(y);
            while dx > dy {
                x = parent[x as usize];
                dx -= 1;
            }
            while dy > dx {
                y = parent[y as usize];
                dy -= 1;
            }
            while x != y {
                x = parent[x as usize];
                y = parent[y as usize];
            }
            x
        };
        for ei in 0..m {
            if first_dummy[ei] == NONE {
                continue;
            }
            let e = &edges[ei];
            let gr = lca(of[e.tail], of[e.head]);
            for d in first_dummy[ei]..first_dummy[ei] + span[ei] - 1 {
                g[d] = gr;
            }
        }
        grp = Some(g);
    }

    let mut succ_start = vec![0usize; n + 1];
    let mut pred_start = vec![0usize; n + 1];
    for e in 0..segments {
        succ_start[ef[e] + 1] += 1;
        pred_start[et[e] + 1] += 1;
    }
    for i in 0..n {
        succ_start[i + 1] += succ_start[i];
        pred_start[i + 1] += pred_start[i];
    }
    let mut succ_e = vec![0usize; segments];
    let mut pred_e = vec![0usize; segments];
    {
        let mut sf = succ_start[..n].to_vec();
        let mut pf = pred_start[..n].to_vec();
        for e in 0..segments {
            succ_e[sf[ef[e]]] = e;
            sf[ef[e]] += 1;
            pred_e[pf[et[e]]] = e;
            pf[et[e]] += 1;
        }
    }

    let mut layer_start = vec![0usize; max_rank + 2];
    for i in 0..n {
        layer_start[rank_of[i] + 1] += 1;
    }
    for r in 0..=max_rank {
        layer_start[r + 1] += layer_start[r];
    }
    let mut order = vec![0usize; n];
    let mut pos = vec![0usize; n];

    // Initial order: depth-first from the top so that subtrees start out contiguous.
    {
        let mut by_rank = vec![0usize; n];
        let mut fill = layer_start[..=max_rank].to_vec();
        for i in 0..n {
            by_rank[fill[rank_of[i]]] = i;
            fill[rank_of[i]] += 1;
        }
        fill.copy_from_slice(&layer_start[..=max_rank]);
        let mut visited = vec![false; n];
        let mut stack = Vec::new();
        for k in 0..n {
            if visited[by_rank[k]] {
                continue;
            }
            stack.push(by_rank[k]);
            while let Some(v) = stack.pop() {
                if visited[v] {
                    continue;
                }
                visited[v] = true;
                order[fill[rank_of[v]]] = v;
                fill[rank_of[v]] += 1;
                for j in (succ_start[v]..succ_start[v + 1]).rev() {
                    let w = et[succ_e[j]];
                    if !visited[w] {
                        stack.push(w);
                    }
                }
            }
        }
    }

    let sort_rank = |order: &mut [usize], r: usize, key: &dyn Fn(usize) -> f64| -> bool {
        let s = layer_start[r];
        let e = layer_start[r + 1];
        if e - s < 2 {
            return false;
        }
        if (s..e).any(|i| key(order[i]).is_nan()) {
            return false;
        }
        let mut layer: Vec<(usize, usize)> = order[s..e].iter().copied().enumerate().collect();
        layer.sort_by(|&(ia, a), &(ib, b)| by(key(a) - key(b)).then(ia.cmp(&ib)));
        for (i, (_, v)) in layer.into_iter().enumerate() {
            order[s + i] = v;
        }
        true
    };
    for r in 0..=max_rank {
        sort_rank(&mut order, r, &|v| key[v]);
    }

    let pinned = (0..real).any(|i| nodes[i].pin != 0);
    if pinned {
        let pin_of = |v: usize| if v < real { nodes[v].pin } else { 0 };
        for r in 0..=max_rank {
            let s = layer_start[r];
            let mut layer: Vec<(usize, usize)> = order[s..layer_start[r + 1]]
                .iter()
                .copied()
                .enumerate()
                .collect();
            layer.sort_by(|&(ia, a), &(ib, b)| pin_of(a).cmp(&pin_of(b)).then(ia.cmp(&ib)));
            for (i, (_, v)) in layer.into_iter().enumerate() {
                order[s + i] = v;
            }
        }
    }
    // Preserve port order to avoid crossings outside the group.
    let mut fixed = vec![false; max_rank + 1];
    let hint = |v: usize| {
        if v < real && (nodes[v].kind == Kind::StartPort || nodes[v].kind == Kind::EndPort) {
            nodes[v].hint
        } else {
            f64::NAN
        }
    };
    for r in [0, max_rank] {
        if sort_rank(&mut order, r, &hint) {
            fixed[r] = true;
        }
    }
    set_positions(&layer_start, &order, &mut pos);

    let mut g = Graph {
        n,
        real,
        max_rank,
        w,
        kind,
        rank: rank_of,
        ef,
        et,
        efd,
        etd,
        succ_start,
        succ_e,
        pred_start,
        pred_e,
        layer_start,
        order,
        pos,
        offsets,
        fixed,
        grp,
        group_parent: opt.groups.as_ref().map(|(_, parent)| parent.clone()),
    };

    let mut crossings = 0;
    let mut declared = 0;
    if segments > 0 && max_rank > 0 {
        (crossings, declared) = reduce_crossings(&mut g, nodes);
        set_positions(&g.layer_start, &g.order, &mut g.pos);
    }
    if opt.order_only {
        // Points are (rank, cross-flow position) pairs.
        let at = position(&mut g, opt);
        for i in 0..real {
            nodes[i].x = at[i];
        }
        for ei in 0..m {
            let e = &mut edges[ei];
            let (a, b) = if e.reversed {
                (e.head, e.tail)
            } else {
                (e.tail, e.head)
            };
            let mut pts = vec![g.rank[a] as f64, at[a]];
            if !direct[ei] && first_dummy[ei] != NONE {
                for d in first_dummy[ei]..first_dummy[ei] + span[ei] - 1 {
                    pts.push(g.rank[d] as f64);
                    pts.push(at[d]);
                }
            }
            pts.push(g.rank[b] as f64);
            pts.push(at[b]);
            e.points = pts;
        }
        return LayeredResult {
            crossings,
            declared,
            ..none
        };
    }

    if spans {
        spread(&mut g, nodes, &efk, &etk);
    }

    let mut xs = position(&mut g, opt);

    let empty = empty_ranks(&g);
    if spans {
        let free = free_ends(&g, &efk, &etk);
        align_labels(&g, nodes, &mut xs, &free, opt, empty.as_deref());
        settle(&mut g, nodes, &xs, &efk, &etk, &free, empty.as_deref());
        snap(&g, &mut xs, empty.as_deref(), opt);
        for ei in 0..m {
            if direct[ei] {
                continue;
            }
            let e = &mut edges[ei];
            let a_dx = g.efd[first_seg[ei]];
            let b_dx = g.etd[last_seg[ei]];
            e.tail_dx = if e.reversed { b_dx } else { a_dx };
            e.head_dx = if e.reversed { a_dx } else { b_dx };
        }
    }

    // A rank that holds nothing but the bends of edges is only there because some other edge
    // has a label or a twin. Its bends move onto the straight line between the ends of their
    // edges, and one that gets there is left out of the route.
    let mut on_line = vec![false; n];
    if let Some(empty) = empty.as_deref() {
        if !opt.tracks {
            let sep = if opt.apart { 0.0 } else { opt.edge_sep };
            straighten(&g, &mut xs, empty, &first_dummy, &span, sep, &mut on_line);
        }
    }
    if !opt.tracks {
        past_labels(&g, &mut xs, &first_dummy, &span, opt.edge_sep, &mut on_line);
    }
    let Graph {
        w: ref gw,
        kind: ref gkind,
        rank: ref grank,
        ref ef,
        ref et,
        ref efd,
        ref etd,
        ref succ_start,
        ref pred_start,
        ref layer_start,
        ref order,
        ..
    } = g;
    let _ = gw;
    let mut piece_start = vec![0usize; m + 1];
    let use_pieces = opt.tracks || (spans && !opt.straight);
    let mut pieces = Pieces::default();
    let mut track_count = vec![0i64; max_rank + 1];
    let is_empty = |r: usize| empty.as_ref().is_some_and(|e| e[r]);
    if use_pieces {
        for ei in 0..m {
            piece_start[ei] = pieces.from.len();
            if direct[ei] {
                continue;
            }
            let d0 = first_dummy[ei];
            let bends = if d0 == NONE {
                0..0
            } else {
                d0..d0 + span[ei] - 1
            };
            // An edge that keeps a bend to stay clear of its twin is drawn through the bend.
            let mut twin = false;
            for d in bends.clone() {
                let bare = is_empty(grank[d]);
                if !opt.tracks && bare != on_line[d] {
                    twin = true;
                }
            }
            if twin {
                continue;
            }
            let mut x = xs[ef[first_seg[ei]]] + efd[first_seg[ei]];
            let mut rank = grank[ef[first_seg[ei]]];
            for d in bends {
                if is_empty(grank[d]) {
                    continue;
                }
                pieces.from.push(x);
                pieces.to.push(xs[d]);
                pieces.rank.push(rank);
                pieces.lower.push(d);
                x = xs[d];
                rank = grank[d];
            }
            pieces.from.push(x);
            pieces.to.push(xs[et[last_seg[ei]]] + etd[last_seg[ei]]);
            pieces.rank.push(rank);
            pieces.lower.push(NONE);
        }
        piece_start[m] = pieces.from.len();
    }

    // Ranks that hold only border ports or their pass-through dummies need little room.
    let port_only = |r: usize, k: Kind| {
        let mut found = false;
        for i in layer_start[r]..layer_start[r + 1] {
            let kk = gkind[order[i]];
            if kk == k {
                found = true;
            } else if kk != Kind::Dummy {
                return false;
            }
        }
        found
    };
    let start_ranks = if port_only(0, Kind::StartPort) {
        step as usize
    } else {
        0
    };
    let end_ranks = if port_only(max_rank, Kind::EndPort) {
        step as usize
    } else {
        0
    };
    // The longest way across that any edge travels in the gap below each rank.
    let mut run = vec![0.0f64; max_rank + 1];
    for e in 0..segments {
        let across = (xs[ef[e]] + efd[e] - xs[et[e]] - etd[e]).abs();
        if across > run[grank[ef[e]]] {
            run[grank[ef[e]]] = across;
        }
    }
    let mut band_top = vec![0.0f64; max_rank + 1];
    let mut band_bottom = vec![0.0f64; max_rank + 1];
    let stepf = step as f64;
    let usual = |r: usize| {
        (if r < start_ranks || r as i64 >= max_rank as i64 - end_ranks as i64 {
            opt.port_sep
        } else {
            opt.rank_sep
        }) / stepf
    };
    let mut gap_under = vec![0.0f64; max_rank + 1];
    for r in 0..max_rank {
        gap_under[r] = if use_pieces {
            usual(r)
        } else {
            usual(r).max(((MAX_GAP * opt.rank_sep) / stepf).min(run[r] * MIN_SLOPE))
        };
    }
    // A rank between two ranks of nodes, which holds labels and bends only.
    let mut between = vec![true; max_rank + 1];
    for i in 0..real {
        between[grank[i]] = false;
    }
    for r in 1..max_rank {
        if opt.tracks || !between[r] || (heads[r] != 1 && heads[r] != 2) {
            continue;
        }
        let shift =
            (opt.head_room / 2.0).min(gap_under[if heads[r] == 1 { r - 1 } else { r }] / 2.0);
        gap_under[r - 1] += if heads[r] == 1 { -shift } else { shift };
        gap_under[r] += if heads[r] == 1 { shift } else { -shift };
    }
    let margin = if opt.tracks {
        TRACK_MARGIN
    } else {
        LEVEL_MARGIN
    };
    let least_pitch = if opt.tracks { TRACK_PITCH } else { LEVEL_PITCH };
    let most_pitch = if opt.tracks {
        TRACK_SPREAD
    } else {
        LEVEL_PITCH
    };
    let mut below = vec![0usize; max_rank + 1];
    if use_pieces {
        let mut room = vec![0.0f64; max_rank + 1];
        for r in 0..max_rank {
            if is_empty(r) {
                continue;
            }
            let mut next = r + 1;
            room[r] = gap_under[r];
            while next < max_rank && is_empty(next) {
                room[r] += gap_under[next];
                next += 1;
            }
            below[r] = next;
        }
        for k in 0..pieces.from.len() {
            let wide = (pieces.from[k] - pieces.to[k]).abs() > LEVEL_WIDE * room[pieces.rank[k]];
            pieces.wide.push(u8::from(wide));
        }
        let mut tall = vec![0.0f64; max_rank + 1];
        assign_tracks(&mut pieces, &mut track_count, !opt.tracks, &room, &mut tall);
        for r in 0..max_rank {
            if track_count[r] > 0 {
                gap_under[r] +=
                    0f64.max(2.0 * margin + (track_count[r] - 1) as f64 * least_pitch - room[r]);
            } else if tall[r] > 0.0 {
                gap_under[r] += 0f64.max((MAX_GAP * opt.rank_sep).min(tall[r]) - room[r]);
            }
        }
    }
    let mut y = 0.0;
    for r in 0..=max_rank {
        let mut hh = 0.0;
        for i in layer_start[r]..layer_start[r + 1] {
            if h[order[i]] > hh {
                hh = h[order[i]];
            }
        }
        band_top[r] = y;
        band_bottom[r] = y + hh;
        y += hh + gap_under[r];
    }
    let height = y;
    let inset = |d: usize| {
        if gkind[d] == Kind::Dummy && between[grank[d]] && !opt.tracks {
            (band_bottom[grank[d]] - band_top[grank[d]]) / 4.0
        } else {
            0.0
        }
    };
    let track_y = |piece: usize, on_track: i64| {
        let r = pieces.rank[piece];
        let top = band_bottom[r];
        let bottom = band_top[below[r]];
        let count = if opt.tracks {
            track_count[r]
        } else {
            pieces.levels[piece]
        };
        let pitch = if count > 1 {
            most_pitch.min((bottom - top - 2.0 * margin) / (count - 1) as f64)
        } else {
            0.0
        };
        (top + bottom) / 2.0 + (on_track as f64 - (count - 1) as f64 / 2.0) * pitch
    };

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    for i in 0..n {
        let half = g.w[i] / 2.0;
        if xs[i] - half < min_x {
            min_x = xs[i] - half;
        }
        if xs[i] + half > max_x {
            max_x = xs[i] + half;
        }
    }
    let mut arrives = vec![false; max_rank + 1];
    let mut leaves = vec![false; max_rank + 1];
    for i in 0..real {
        if gkind[i] != Kind::Node || !spans {
            continue;
        }
        if pred_start[i + 1] > pred_start[i] {
            arrives[grank[i]] = true;
        }
        if succ_start[i + 1] > succ_start[i] {
            leaves[grank[i]] = true;
        }
    }
    for i in 0..real {
        let node = &mut nodes[i];
        let r = node.rank as usize;
        node.x = xs[i] - min_x;
        node.y = if gkind[i] == Kind::Node && leaves[r] && !arrives[r] {
            band_bottom[r] - node.h / 2.0
        } else if gkind[i] == Kind::Node && arrives[r] && !leaves[r] {
            band_top[r] + node.h / 2.0
        } else {
            (band_top[r] + band_bottom[r]) / 2.0
        };
    }

    for ei in 0..m {
        let e = &edges[ei];
        let (fi, li) = if e.reversed {
            (e.head, e.tail)
        } else {
            (e.tail, e.head)
        };
        let first = &nodes[fi];
        let last = &nodes[li];
        let mut pts = vec![
            first.x + if e.reversed { e.head_dx } else { e.tail_dx },
            first.y + first.h / 2.0,
        ];
        let d0 = first_dummy[ei];
        if use_pieces && piece_start[ei + 1] > piece_start[ei] {
            for k in piece_start[ei]..piece_start[ei + 1] {
                if pieces.track[k] >= 0 && (opt.tracks || pieces.wide[k] == 1) {
                    let across = track_y(k, pieces.track[k]);
                    pts.extend([pieces.from[k] - min_x, across, pieces.to[k] - min_x, across]);
                } else if pieces.track[k] > 0 {
                    // A short piece among levels runs straight until it is past the level
                    // before its own, and turns in one S from there.
                    pts.extend([pieces.from[k] - min_x, track_y(k, pieces.track[k] - 1)]);
                }
                let d = pieces.lower[k];
                if d == NONE {
                    continue;
                }
                pts.extend([xs[d] - min_x, band_top[grank[d]] + inset(d)]);
                if band_bottom[grank[d]] > band_top[grank[d]] {
                    pts.extend([xs[d] - min_x, band_bottom[grank[d]] - inset(d)]);
                }
            }
        } else if d0 != NONE {
            for d in d0..d0 + span[ei] - 1 {
                if on_line[d] {
                    continue;
                }
                let x = xs[d] - min_x;
                let top = band_top[grank[d]];
                let bottom = band_bottom[grank[d]];
                pts.extend([x, top + inset(d)]);
                if bottom > top {
                    pts.extend([x, bottom - inset(d)]);
                }
            }
        }
        pts.push(last.x + if e.reversed { e.tail_dx } else { e.head_dx });
        pts.push(last.y - last.h / 2.0);
        if e.reversed {
            let len = pts.len();
            let (mut i, mut j) = (0, len - 2);
            while i < j {
                pts.swap(i, j);
                pts.swap(i + 1, j + 1);
                i += 2;
                j -= 2;
            }
        }
        let ln = label_node[ei];
        let (lx, ly) = if ln != NONE {
            (
                xs[ln] - min_x,
                (band_top[grank[ln]] + band_bottom[grank[ln]]) / 2.0,
            )
        } else {
            let len = pts.len();
            ((pts[0] + pts[len - 2]) / 2.0, (pts[1] + pts[len - 1]) / 2.0)
        };
        let e = &mut edges[ei];
        if ln != NONE || e.label_w > 0.0 {
            e.label_x = lx;
            e.label_y = ly;
        }
        e.points = pts;
    }

    LayeredResult {
        width: max_x - min_x,
        height,
        ..none
    }
}

fn set_positions(layer_start: &[usize], order: &[usize], pos: &mut [usize]) {
    for r in 0..layer_start.len() - 1 {
        let s = layer_start[r];
        for i in s..layer_start[r + 1] {
            pos[order[i]] = i - s;
        }
    }
}

/// Reverses the edges that close a cycle, found by depth-first search in declaration order.
pub(crate) fn orient(nodes: &[LNode], edges: &mut [LEdge]) {
    let n = nodes.len();
    let m = edges.len();
    let mut start = vec![0usize; n + 1];
    for e in edges.iter_mut() {
        e.reversed = nodes[e.head].kind == Kind::StartPort || nodes[e.tail].kind == Kind::EndPort;
        if !e.reversed {
            start[e.tail + 1] += 1;
        }
    }
    for i in 0..n {
        start[i + 1] += start[i];
    }
    let mut out = vec![0usize; start[n]];
    {
        let mut fill = start[..n].to_vec();
        for i in 0..m {
            if !edges[i].reversed {
                out[fill[edges[i].tail]] = i;
                fill[edges[i].tail] += 1;
            }
        }
    }
    let mut state = vec![0u8; n];
    let mut cursor = start[..n].to_vec();
    let mut stack = Vec::new();
    for root in 0..n {
        if state[root] != 0 {
            continue;
        }
        stack.push(root);
        state[root] = 1;
        while let Some(&v) = stack.last() {
            if cursor[v] < start[v + 1] {
                let e = &mut edges[out[cursor[v]]];
                cursor[v] += 1;
                let w = e.head;
                if state[w] == 1 {
                    e.reversed = true;
                } else if state[w] == 0 {
                    state[w] = 1;
                    stack.push(w);
                }
            } else {
                state[v] = 2;
                stack.pop();
            }
        }
    }
}

pub(crate) fn rank(nodes: &mut [LNode], edges: &[LEdge], step: i32) {
    let n = nodes.len();
    let m = edges.len();
    let mut from = vec![0usize; m];
    let mut to = vec![0usize; m];
    let mut len = vec![0i32; m];
    let mut out_start = vec![0usize; n + 1];
    let mut in_start = vec![0usize; n + 1];
    for i in 0..m {
        let e = &edges[i];
        let (a, b) = if e.reversed {
            (e.head, e.tail)
        } else {
            (e.tail, e.head)
        };
        from[i] = a;
        to[i] = b;
        len[i] = 1.max(e.minlen) * step;
        out_start[a + 1] += 1;
        in_start[b + 1] += 1;
    }
    for i in 0..n {
        out_start[i + 1] += out_start[i];
        in_start[i + 1] += in_start[i];
    }
    let mut out_e = vec![0usize; m];
    let mut in_e = vec![0usize; m];
    {
        let mut of = out_start[..n].to_vec();
        let mut inf = in_start[..n].to_vec();
        for i in 0..m {
            out_e[of[from[i]]] = i;
            of[from[i]] += 1;
            in_e[inf[to[i]]] = i;
            inf[to[i]] += 1;
        }
    }

    let mut r = vec![0i32; n];
    let mut indeg: Vec<usize> = (0..n).map(|i| in_start[i + 1] - in_start[i]).collect();
    let mut topo = vec![0usize; n];
    let mut count = 0;
    for i in 0..n {
        if indeg[i] == 0 {
            topo[count] = i;
            count += 1;
        }
    }
    let mut q = 0;
    while q < count {
        let v = topo[q];
        for k in out_start[v]..out_start[v + 1] {
            let ei = out_e[k];
            let w = to[ei];
            if r[v] + len[ei] > r[w] {
                r[w] = r[v] + len[ei];
            }
            indeg[w] -= 1;
            if indeg[w] == 0 {
                topo[count] = w;
                count += 1;
            }
        }
        q += 1;
    }

    // Longest-path ranking leaves loosely attached nodes far from their neighbours. Move each
    // node toward the side that has more edges.
    for _ in 0..8 {
        let mut changed = false;
        for q in (0..count).rev() {
            let v = topo[q];
            let ins = in_start[v + 1] - in_start[v];
            let outs = out_start[v + 1] - out_start[v];
            if ins == 0 && outs == 0 {
                continue;
            }
            let mut lo = i64::MIN;
            let mut hi = i64::MAX;
            for k in in_start[v]..in_start[v + 1] {
                let ei = in_e[k];
                lo = lo.max(i64::from(r[from[ei]] + len[ei]));
            }
            for k in out_start[v]..out_start[v + 1] {
                let ei = out_e[k];
                hi = hi.min(i64::from(r[to[ei]] - len[ei]));
            }
            let mut next = i64::from(r[v]);
            if outs > ins && hi != i64::MAX {
                next = hi;
            } else if ins > outs && lo != i64::MIN {
                next = lo;
            }
            if next != i64::from(r[v]) {
                r[v] = next as i32;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut min = i32::MAX;
    let mut max = i32::MIN;
    let mut start_ports = false;
    for i in 0..n {
        let kind = nodes[i].kind;
        if kind == Kind::StartPort {
            start_ports = true;
        }
        if kind == Kind::StartPort || kind == Kind::EndPort {
            continue;
        }
        min = min.min(r[i]);
        max = max.max(r[i]);
    }
    if min == i32::MAX {
        min = 0;
        max = 0;
    }
    for i in 0..n {
        if nodes[i].kind == Kind::StartPort {
            r[i] = min - step;
        } else if nodes[i].kind == Kind::EndPort {
            r[i] = max + step;
        }
    }
    if start_ports {
        min -= step;
    }
    for i in 0..n {
        nodes[i].rank = r[i] - min;
    }
}

/// The ends of edges that may move along one side of a node (0 the side its edges arrive at,
/// 1 the other), ordered by their opposite nodes to avoid crossings. Fills `ids` and `room`
/// (their minimum gaps) and returns the sum of the gaps.
fn side_ends(
    g: &Graph,
    v: usize,
    side: usize,
    span: f64,
    kinds: &[u8],
    ids: &mut Vec<usize>,
    room: &mut Vec<f64>,
) -> f64 {
    let (start, list, other) = if side == 0 {
        (&g.pred_start, &g.pred_e, &g.ef)
    } else {
        (&g.succ_start, &g.succ_e, &g.et)
    };
    for k in start[v]..start[v + 1] {
        if kinds[list[k]] != HELD {
            ids.push(list[k]);
        }
    }
    if ids.len() < 2 {
        return 0.0;
    }
    ids.sort_by(|&a, &b| g.pos[other[a]].cmp(&g.pos[other[b]]).then(a.cmp(&b)));
    room.clear();
    room.resize(ids.len(), 0.0);
    let fill = |room: &mut Vec<f64>, share: &dyn Fn(bool, bool) -> f64| {
        let mut total = 0.0;
        room[0] = 0.0;
        for k in 1..ids.len() {
            let same = kinds[ids[k]] == kinds[ids[k - 1]];
            room[k] = share(same && kinds[ids[k]] == LEAVES, same);
            total += room[k];
        }
        total
    };
    let mut total = fill(room, &|_, _| 1.0);
    if span / total < MIN_PITCH {
        total = fill(room, &|leaves, _| if leaves { 0.5 } else { 1.0 });
    }
    if span / total < MIN_PITCH {
        total = fill(room, &|_, same| if same { 0.0 } else { 1.0 });
    }
    if total == 0.0 {
        return 0.0;
    }
    let pitch = PORT_PITCH.min(span / total);
    for r in room.iter_mut().skip(1) {
        *r *= pitch;
    }
    total * pitch
}

/// Gives the edges that meet one side of a node places of their own along it, around its middle.
fn spread(g: &mut Graph, nodes: &[LNode], efk: &[u8], etk: &[u8]) {
    for v in 0..g.real {
        let span = nodes[v].span.min(g.w[v]);
        if span <= 0.0 || nodes[v].kind != Kind::Node {
            continue;
        }
        for side in 0..2 {
            let mut ids = Vec::new();
            let mut room = Vec::new();
            let total = side_ends(
                g,
                v,
                side,
                span,
                if side == 0 { etk } else { efk },
                &mut ids,
                &mut room,
            );
            if total == 0.0 {
                continue;
            }
            let dx = if side == 0 { &mut g.etd } else { &mut g.efd };
            let mut at = -total / 2.0;
            for k in 0..ids.len() {
                at += room[k];
                dx[ids[k]] = at;
            }
        }
    }
}

/// The segment of an edge that reaches the next point held in place, on the way up from
/// segment `e` (side 0) or down: past the bends in ranks that hold nothing else.
fn follow(g: &Graph, empty: Option<&[bool]>, mut e: usize, side: usize) -> usize {
    let Some(empty) = empty else { return e };
    for _ in 0..100000 {
        let u = if side == 0 { g.ef[e] } else { g.et[e] };
        if g.kind[u] != Kind::Dummy || !empty[g.rank[u]] {
            break;
        }
        e = if side == 0 {
            g.pred_e[g.pred_start[u]]
        } else {
            g.succ_e[g.succ_start[u]]
        };
    }
    e
}

/// How many ends that may move along it each node has on the side its edges arrive at, and on
/// the other.
fn free_ends(g: &Graph, efk: &[u8], etk: &[u8]) -> [Vec<i32>; 2] {
    let mut free = [vec![0i32; g.n], vec![0i32; g.n]];
    for e in 0..g.ef.len() {
        if etk[e] != HELD {
            free[0][g.et[e]] += 1;
        }
        if efk[e] != HELD {
            free[1][g.ef[e]] += 1;
        }
    }
    free
}

/// How far along the flow the handles of a plain S reach, as a share of the way it travels
/// along it.
pub(crate) fn ease(across: f64, along: f64) -> f64 {
    let a = across.abs();
    0.25f64.max(0.5f64.min((4.0 * along.abs()) / if a == 0.0 || a.is_nan() { 1.0 } else { a }))
}

/// Brings the labels and bends of each edge into line with an end of the edge.
fn align_labels(
    g: &Graph,
    nodes: &[LNode],
    xs: &mut [f64],
    free: &[Vec<i32>; 2],
    opt: &LayeredOptions,
    empty: Option<&[bool]>,
) {
    let sep = opt.edge_sep;
    let placed = |v: usize| {
        g.kind[v] == Kind::Label
            || (g.kind[v] == Kind::Dummy && !empty.is_some_and(|e| e[g.rank[v]]))
    };
    let end = |xs: &[f64], e: usize, side: usize| -> (f64, i32) {
        let u = if side == 0 { g.ef[e] } else { g.et[e] };
        let x = if side == 0 {
            xs[u] + g.efd[e]
        } else {
            xs[u] + g.etd[e]
        };
        (
            x,
            if u < g.real && nodes[u].kind == Kind::Node {
                free[1 - side][u]
            } else {
                0
            },
        )
    };
    let room = |xs: &[f64], v: usize| -> (f64, f64) {
        let s = g.layer_start[g.rank[v]];
        let i = s + g.pos[v];
        let clear = |other: usize| {
            g.w[other] / 2.0
                + g.w[v] / 2.0
                + if g.kind[other] == Kind::Node {
                    opt.node_sep / 2.0 + sep / 2.0
                } else if g.kind[other] == g.kind[v] {
                    sep
                } else {
                    sep / 2.0
                }
        };
        (
            if i > s {
                xs[g.order[i - 1]] + clear(g.order[i - 1])
            } else {
                f64::NEG_INFINITY
            },
            if i + 1 < g.layer_start[g.rank[v] + 1] {
                xs[g.order[i + 1]] - clear(g.order[i + 1])
            } else {
                f64::INFINITY
            },
        )
    };
    let choose = |x: f64, up: (f64, i32), down: (f64, i32), lo: f64, hi: f64| -> f64 {
        let up_first = if up.1 != down.1 {
            up.1 < down.1
        } else {
            (up.0 - x).abs() <= (down.0 - x).abs()
        };
        let choices = if up_first {
            [up.0, down.0]
        } else {
            [down.0, up.0]
        };
        choices
            .iter()
            .copied()
            .find(|&c| c >= lo && c <= hi)
            .unwrap_or_else(|| lo.max(hi.min(choices[0])))
    };
    let mut done = vec![false; g.n];
    let mut chain = Vec::new();
    for r in 1..g.max_rank {
        for i in g.layer_start[r]..g.layer_start[r + 1] {
            let v = g.order[i];
            if !placed(v) || done[v] {
                continue;
            }
            let mut top = v;
            while is_dummy(g.kind[g.ef[g.pred_e[g.pred_start[top]]]]) {
                top = g.ef[g.pred_e[g.pred_start[top]]];
            }
            let mut bottom = top;
            let mut lo = f64::NEG_INFINITY;
            let mut hi = f64::INFINITY;
            chain.clear();
            let mut d = top;
            while is_dummy(g.kind[d]) {
                bottom = d;
                if placed(d) {
                    done[d] = true;
                    chain.push(d);
                    let (least, most) = room(xs, d);
                    if least > lo {
                        lo = least;
                    }
                    if most < hi {
                        hi = most;
                    }
                }
                d = g.et[g.succ_e[g.succ_start[d]]];
            }
            if lo <= hi {
                let x = choose(
                    xs[chain[0]],
                    end(xs, g.pred_e[g.pred_start[top]], 0),
                    end(xs, g.succ_e[g.succ_start[bottom]], 1),
                    lo,
                    hi,
                );
                for &d in &chain {
                    xs[d] = x;
                }
                continue;
            }
            for &d in &chain {
                let (least, most) = room(xs, d);
                if least > most {
                    continue;
                }
                let up = end(xs, follow(g, empty, g.pred_e[g.pred_start[d]], 0), 0);
                let down = end(xs, follow(g, empty, g.succ_e[g.succ_start[d]], 1), 1);
                xs[d] = choose(xs[d], up, down, least, most);
            }
        }
    }
}

struct SideRun {
    v: usize,
    side: usize,
    span: f64,
    ids: Vec<usize>,
    room: Vec<f64>,
    all: Vec<usize>,
}

/// Once the nodes have their places, moves an end along its side when that lets its edge run
/// straight.
fn settle(
    g: &mut Graph,
    nodes: &[LNode],
    xs: &[f64],
    efk: &[u8],
    etk: &[u8],
    free: &[Vec<i32>; 2],
    empty: Option<&[bool]>,
) {
    let wanted = |g: &Graph, e: usize, side: usize, v: usize, span: f64, again: bool| -> f64 {
        if free[side][v] < 2 {
            return 0.0;
        }
        let e = follow(g, empty, e, side);
        let u = if side == 0 { g.ef[e] } else { g.et[e] };
        let point = if side == 0 {
            xs[u] + g.efd[e]
        } else {
            xs[u] + g.etd[e]
        };
        if again && (point - xs[v]).abs() <= span / 2.0 + IN_LINE {
            return (-span / 2.0).max((span / 2.0).min(point - xs[v]));
        }
        let mut middle = point;
        let mut width = 0.0;
        if u < g.real
            && nodes[u].kind == Kind::Node
            && (if side == 0 { efk } else { etk })[e] != HELD
            && free[1 - side][u] >= 2
        {
            width = 0f64.max(nodes[u].span.min(g.w[u]));
            if width > 0.0 {
                middle = xs[u];
            }
        }
        let lo = (xs[v] - span / 2.0).max(middle - width / 2.0);
        let hi = (xs[v] + span / 2.0).min(middle + width / 2.0);
        if lo > hi + IN_LINE {
            let lean = 0f64.max(span / 2.0 - LEAN_CLEAR);
            return (-lean).max(lean.min(point - xs[v]));
        }
        let place = (xs[v] * width + middle * span) / (width + span);
        lo.max(hi.min(place)) - xs[v]
    };
    let mut sides: Vec<SideRun> = Vec::new();
    for v in 0..g.real {
        let span = nodes[v].span.min(g.w[v]);
        if span <= 0.0 || nodes[v].kind != Kind::Node {
            continue;
        }
        for side in 0..2 {
            let mut ids = Vec::new();
            let mut room = Vec::new();
            side_ends(
                g,
                v,
                side,
                span,
                if side == 0 { etk } else { efk },
                &mut ids,
                &mut room,
            );
            if ids.is_empty() {
                continue;
            }
            if ids.len() == 1 {
                room = vec![0.0];
            }
            let (start, list, other) = if side == 0 {
                (&g.pred_start, &g.pred_e, &g.ef)
            } else {
                (&g.succ_start, &g.succ_e, &g.et)
            };
            let mut all: Vec<usize> = (start[v]..start[v + 1]).map(|k| list[k]).collect();
            all.sort_by(|&a, &b| g.pos[other[a]].cmp(&g.pos[other[b]]).then(a.cmp(&b)));
            sides.push(SideRun {
                v,
                side,
                span,
                ids,
                room,
                all,
            });
        }
    }
    let mut moved = true;
    let mut pass = 0;
    while moved && pass < 6 {
        moved = false;
        for run in &sides {
            let SideRun {
                v,
                side,
                span,
                ref ids,
                ref room,
                ref all,
            } = *run;
            let kinds = if side == 0 { etk } else { efk };
            let mut k = 0;
            let mut lo = -span / 2.0;
            for i in 0..=all.len() {
                let held = i < all.len() && kinds[all[i]] == HELD;
                if i < all.len() && !held {
                    continue;
                }
                let mut size = 0;
                let mut j = i;
                while j > 0 && kinds[all[j - 1]] != HELD {
                    size += 1;
                    j -= 1;
                }
                let dx_held = |g: &Graph, e: usize| if side == 0 { g.etd[e] } else { g.efd[e] };
                let hi = if held {
                    dx_held(g, all[i]) - PORT_PITCH
                } else {
                    span / 2.0
                };
                if size > 0 {
                    let mut sum: Vec<f64> = Vec::new();
                    let mut count: Vec<f64> = Vec::new();
                    let mut before = 0.0;
                    for j in 0..size {
                        if j > 0 {
                            before += room[k + j];
                        }
                        sum.push(wanted(g, ids[k + j], side, v, span, pass > 0) - before);
                        count.push(1.0);
                        let mut together = j > 0 && room[k + j] == 0.0;
                        while sum.len() > 1
                            && (together
                                || sum[sum.len() - 2] / count[count.len() - 2]
                                    > sum[sum.len() - 1] / count[count.len() - 1])
                        {
                            together = false;
                            let top = sum.pop().unwrap_or(0.0);
                            let pooled = count.pop().unwrap_or(0.0);
                            let last = sum.len() - 1;
                            sum[last] += top;
                            count[last] += pooled;
                        }
                    }
                    let total = before;
                    let mut j = 0;
                    before = 0.0;
                    for b in 0..sum.len() {
                        let level = if hi - total < lo {
                            (lo + hi - total) / 2.0
                        } else {
                            lo.max((hi - total).min(sum[b] / count[b]))
                        };
                        let mut c = 0.0;
                        while c < count[b] {
                            if j > 0 {
                                before += room[k + j];
                            }
                            let dx = if side == 0 { &mut g.etd } else { &mut g.efd };
                            if (dx[ids[k + j]] - level - before).abs() > 0.01 {
                                moved = true;
                            }
                            dx[ids[k + j]] = level + before;
                            c += 1.0;
                            j += 1;
                        }
                    }
                    k += size;
                }
                if held {
                    lo = dx_held(g, all[i]) + PORT_PITCH;
                }
            }
        }
        pass += 1;
    }
}

/// Half the room a node, bend or port takes across the flow, with its share of the gap beside it.
fn half_room(g: &Graph, opt: &LayeredOptions, v: usize) -> f64 {
    g.w[v] / 2.0
        + (if g.kind[v] == Kind::Node {
            opt.node_sep
        } else {
            opt.edge_sep
        }) / 2.0
}

/// Brings edges that are nearly straight into line, by moving a node a little across the flow
/// along with everything it is in line with already.
fn snap(g: &Graph, xs: &mut [f64], empty: Option<&[bool]>, opt: &LayeredOptions) {
    let passed = |u: usize| empty.is_some_and(|e| g.kind[u] == Kind::Dummy && e[g.rank[u]]);
    let mut a = Vec::new();
    let mut b = Vec::new();
    let mut da = Vec::new();
    let mut db = Vec::new();
    for e in 0..g.ef.len() {
        if passed(g.ef[e]) {
            continue;
        }
        let f = follow(g, empty, e, 1);
        a.push(g.ef[e]);
        b.push(g.et[f]);
        da.push(g.efd[e]);
        db.push(g.etd[f]);
    }
    let off = |xs: &[f64], k: usize| xs[b[k]] + db[k] - xs[a[k]] - da[k];
    let mut block: Vec<usize> = (0..g.n).collect();
    let mut members: Vec<Vec<usize>> = (0..g.n).map(|i| vec![i]).collect();
    let join = |block: &mut Vec<usize>, members: &mut Vec<Vec<usize>>, x: usize, y: usize| {
        let mut keep = block[x];
        let mut drop = block[y];
        if keep == drop {
            return;
        }
        if members[keep].len() < members[drop].len() {
            std::mem::swap(&mut keep, &mut drop);
        }
        let moving = std::mem::take(&mut members[drop]);
        for v in moving {
            block[v] = keep;
            members[keep].push(v);
        }
    };
    let mut near = Vec::new();
    for k in 0..a.len() {
        let d = off(xs, k).abs();
        if d < 0.01 {
            join(&mut block, &mut members, a[k], b[k]);
        } else if d <= SNAP {
            near.push(k);
        }
    }
    if near.is_empty() {
        return;
    }
    near.sort_by(|&x, &y| by(off(xs, x).abs() - off(xs, y).abs()).then(x.cmp(&y)));
    let gap = |v: usize| half_room(g, opt, v);
    let free = |xs: &[f64], block: &[usize], members: &[Vec<usize>], id: usize, shift: f64| {
        for &v in &members[id] {
            let s = g.layer_start[g.rank[v]];
            let end = g.layer_start[g.rank[v] + 1];
            let i = s + g.pos[v];
            if i > s
                && block[g.order[i - 1]] != id
                && xs[v] + shift - xs[g.order[i - 1]] < gap(v) + gap(g.order[i - 1]) - 0.01
            {
                return false;
            }
            if i + 1 < end
                && block[g.order[i + 1]] != id
                && xs[g.order[i + 1]] - xs[v] - shift < gap(v) + gap(g.order[i + 1]) - 0.01
            {
                return false;
            }
        }
        true
    };
    for k in near {
        let d = off(xs, k);
        if d.abs() < 0.01 {
            join(&mut block, &mut members, a[k], b[k]);
            continue;
        }
        if d.abs() > SNAP || block[a[k]] == block[b[k]] {
            continue;
        }
        let upper = block[a[k]];
        let lower = block[b[k]];
        let first = if members[upper].len() <= members[lower].len() {
            upper
        } else {
            lower
        };
        for id in [first, if first == upper { lower } else { upper }] {
            let shift = if id == upper { d } else { -d };
            if !free(xs, &block, &members, id, shift) {
                continue;
            }
            for &v in &members[id] {
                xs[v] += shift;
            }
            join(&mut block, &mut members, a[k], b[k]);
            break;
        }
    }
}

/// Marks the ranks that hold nothing but the bends of edges, if there are any.
fn empty_ranks(g: &Graph) -> Option<Vec<bool>> {
    let mut empty = vec![false; g.max_rank + 1];
    let mut any = false;
    for r in 1..g.max_rank {
        let s = g.layer_start[r];
        let e = g.layer_start[r + 1];
        let only = e > s && (s..e).all(|i| g.kind[g.order[i]] == Kind::Dummy);
        if only {
            empty[r] = true;
            any = true;
        }
    }
    any.then_some(empty)
}

fn sample(from: f64, to: f64, h: f64, out: &mut [f64; 2 * SAMPLES + 2]) {
    let c = ease(to - from, h);
    for i in 0..=SAMPLES {
        let t = i as f64 / SAMPLES as f64;
        let u = 1.0 - t;
        out[2 * i] = from + (to - from) * t * t * (3.0 - 2.0 * t);
        out[2 * i + 1] = h * (3.0 * c * t * u * u + 3.0 * (1.0 - c) * t * t * u + t * t * t);
    }
}

fn reaches(points: &[f64], line: &[f64], limit: f64) -> bool {
    let bound = limit * limit * (1.0 + 1e-9);
    for i in 0..=SAMPLES {
        let px = points[2 * i];
        let py = points[2 * i + 1];
        for j in 0..SAMPLES {
            let dx = line[2 * j + 2] - line[2 * j];
            let dy = line[2 * j + 3] - line[2 * j + 1];
            let len = dx * dx + dy * dy;
            let t = 0f64.max(1f64.min(
                ((px - line[2 * j]) * dx + (py - line[2 * j + 1]) * dy)
                    / if len == 0.0 || len.is_nan() { 1.0 } else { len },
            ));
            let ex = px - line[2 * j] - t * dx;
            let ey = py - line[2 * j + 1] - t * dy;
            if ex * ex + ey * ey < bound && ex.hypot(ey) < limit {
                return true;
            }
        }
    }
    false
}

/// Gives every piece that does not run straight down a track across its gap, and counts the
/// tracks of each gap.
fn assign_tracks(
    pieces: &mut Pieces,
    track_count: &mut [i64],
    curved: bool,
    room: &[f64],
    tall: &mut [f64],
) {
    let total = pieces.from.len();
    let mut by_rank: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut rank_index: HashMap<usize, usize> = HashMap::new();
    for k in 0..total {
        pieces.track.push(-1);
        pieces.levels.push(0);
        if (pieces.from[k] - pieces.to[k]).abs() < IN_LINE {
            continue;
        }
        let r = pieces.rank[k];
        match rank_index.get(&r) {
            Some(&at) => by_rank[at].1.push(k),
            None => {
                rank_index.insert(r, by_rank.len());
                by_rank.push((r, vec![k]));
            }
        }
    }
    if by_rank.is_empty() {
        return;
    }
    let from = pieces.from.clone();
    let to = pieces.to.clone();
    let lo_of: Vec<f64> = (0..total).map(|k| from[k].min(to[k])).collect();
    let hi_of: Vec<f64> = (0..total).map(|k| from[k].max(to[k])).collect();
    let within = |x: f64, k: usize| i32::from(x >= lo_of[k] - IN_LINE && x <= hi_of[k] + IN_LINE);
    let mates =
        |a: usize, b: usize| (from[a] - from[b]).abs() < IN_LINE || (to[a] - to[b]).abs() < IN_LINE;
    let overlap =
        |a: usize, b: usize| lo_of[a] < hi_of[b] - IN_LINE && lo_of[b] < hi_of[a] - IN_LINE;
    let clash = |a: usize, b: usize| overlap(a, b) && !mates(a, b);
    let meet = |a: usize, b: usize| {
        overlap(a, b)
            || within(to[a], b) + within(from[b], a) + within(to[b], a) + within(from[a], b) > 0
    };
    let crossings = |a: usize, b: usize| within(to[a], b) + within(from[b], a);
    let near_pairs = |list: &[usize]| -> Vec<usize> {
        let mut by_lo: Vec<usize> = (0..list.len()).collect();
        by_lo.sort_by(|&a, &b| by(lo_of[list[a]] - lo_of[list[b]]));
        let mut pairs = Vec::new();
        for x in 0..by_lo.len() {
            let i = by_lo[x];
            let reach = hi_of[list[i]] + 4.0 * IN_LINE;
            let mut y = x + 1;
            while y < by_lo.len() && lo_of[list[by_lo[y]]] <= reach {
                let j = by_lo[y];
                pairs.push(i.min(j));
                pairs.push(i.max(j));
                y += 1;
            }
        }
        pairs
    };

    let lay = |list: &mut Vec<usize>, track: &mut Vec<i64>, levels: &mut Vec<i64>| -> i64 {
        list.sort_by(|&a, &b| {
            by(hi_of[b] - lo_of[b] - (hi_of[a] - lo_of[a]))
                .then(by(lo_of[a] - lo_of[b]))
                .then(a.cmp(&b))
        });
        let count = list.len();
        let mut above: Vec<Vec<usize>> = Vec::new();
        let mut group: Vec<usize> = (0..count).collect();
        fn find(group: &mut [usize], mut i: usize) -> usize {
            while group[i] != i {
                group[i] = group[group[i]];
                i = group[i];
            }
            i
        }
        let weighed = count <= MAX_WEIGHED;
        let pairs = if weighed {
            near_pairs(list)
        } else {
            Vec::new()
        };
        let mut clashes: Vec<Vec<usize>> = if weighed {
            vec![Vec::new(); count]
        } else {
            Vec::new()
        };
        for p in (0..pairs.len()).step_by(2) {
            let (i, j) = (pairs[p], pairs[p + 1]);
            if !clash(list[i], list[j]) {
                continue;
            }
            clashes[i].push(j);
            clashes[j].push(i);
        }
        let mut sequence: Vec<usize> = (0..count).collect();
        if weighed {
            let mut waiting = vec![0i64; count];
            let mut after: Vec<Vec<usize>> = vec![Vec::new(); count];
            above = vec![Vec::new(); count];
            for p in (0..pairs.len()).step_by(2) {
                let (i, j) = (pairs[p], pairs[p + 1]);
                let (a, b) = (list[i], list[j]);
                if !meet(a, b) {
                    continue;
                }
                let fi = find(&mut group, i);
                let fj = find(&mut group, j);
                group[fi] = fj;
                let ab = crossings(a, b);
                let ba = crossings(b, a);
                if ab == ba {
                    continue;
                }
                let (first, second) = if ab < ba { (i, j) } else { (j, i) };
                above[second].push(first);
                after[first].push(second);
                waiting[second] += 1;
            }
            let mut placed = vec![false; count];
            sequence.clear();
            for _ in 0..count {
                let mut pick = NONE;
                for i in 0..count {
                    if placed[i] {
                        continue;
                    }
                    if pick == NONE || waiting[i] < waiting[pick] {
                        pick = i;
                    }
                    if waiting[i] == 0 {
                        break;
                    }
                }
                placed[pick] = true;
                sequence.push(pick);
                for &j in &after[pick] {
                    waiting[j] -= 1;
                }
            }
        }
        let mut taken = HashSet::new();
        let mut on_track: Vec<Vec<usize>> = Vec::new();
        let mut rows = 0i64;
        for &at in &sequence {
            let k = list[at];
            let mut t = 0i64;
            if let Some(list_above) = above.get(at) {
                for &i in list_above {
                    if track[list[i]] >= t {
                        t = track[list[i]] + 1;
                    }
                }
            }
            if weighed {
                taken.clear();
                for &i in &clashes[at] {
                    taken.insert(track[list[i]]);
                }
                while taken.contains(&t) {
                    t += 1;
                }
            } else {
                while on_track
                    .get(t as usize)
                    .is_some_and(|row| row.iter().any(|&other| clash(k, other)))
                {
                    t += 1;
                }
                if on_track.len() <= t as usize {
                    on_track.resize(t as usize + 1, Vec::new());
                }
                on_track[t as usize].push(k);
            }
            track[k] = t;
            if t >= rows {
                rows = t + 1;
            }
        }
        let mut first = vec![rows; count];
        let mut last = vec![-1i64; count];
        for i in 0..count {
            let root = find(&mut group, i);
            first[root] = first[root].min(track[list[i]]);
            last[root] = last[root].max(track[list[i]]);
        }
        for i in 0..count {
            let root = find(&mut group, i);
            levels[list[i]] = last[root] - first[root] + 1;
            if curved {
                track[list[i]] -= first[root];
            }
        }
        rows
    };

    let mut line_a = [0.0f64; 2 * SAMPLES + 2];
    let mut line_b = [0.0f64; 2 * SAMPLES + 2];
    for (r, all) in by_rank {
        if !curved {
            let mut all = all;
            track_count[r] = lay(&mut all, &mut pieces.track, &mut pieces.levels);
            continue;
        }
        if all.len() > MAX_WEIGHED {
            continue;
        }
        let mut far: Vec<usize> = all
            .iter()
            .copied()
            .filter(|&k| pieces.wide[k] == 1)
            .collect();
        lay(&mut far, &mut pieces.track, &mut pieces.levels);
        // Insertion order matters only for membership; a set is enough.
        let mut level: HashSet<usize> = far
            .iter()
            .copied()
            .filter(|&k| pieces.levels[k] > 1)
            .collect();
        let pairs = near_pairs(&all);
        for p in (0..pairs.len()).step_by(2) {
            let a = all[pairs[p]];
            let b = all[pairs[p + 1]];
            if (level.contains(&a) && level.contains(&b))
                || !clash(a, b)
                || (from[a] < to[a]) != (from[b] < to[b])
            {
                continue;
            }
            let limit = if pieces.wide[a] == 1 || pieces.wide[b] == 1 {
                LEVEL_CLEAR
            } else {
                LEVEL_TOUCH
            };
            let height = room[r];
            let hh = (height / 2.0).max(height - 2.0 * LEVEL_LEAD);
            sample(from[a], to[a], hh, &mut line_a);
            sample(from[b], to[b], hh, &mut line_b);
            if reaches(&line_a, &line_b, limit) || reaches(&line_b, &line_a, limit) {
                level.insert(a);
                level.insert(b);
            }
        }
        for &k in &far {
            pieces.track[k] = -1;
            pieces.levels[k] = 0;
        }
        if level.is_empty() {
            continue;
        }
        let mut crossing = 0;
        let mut alongside = 0;
        let mut beside = HashSet::new();
        for p in (0..pairs.len()).step_by(2) {
            let a = all[pairs[p]];
            let b = all[pairs[p + 1]];
            if level.contains(&a) && level.contains(&b) {
                if !overlap(a, b) {
                    continue;
                }
                if (from[a] < to[a]) != (from[b] < to[b]) {
                    crossing += 1;
                } else if !mates(a, b) {
                    alongside += 1;
                }
            } else if (level.contains(&a) || level.contains(&b)) && meet(a, b) {
                beside.insert(a);
                beside.insert(b);
            }
        }
        if crossing > alongside {
            for &k in &all {
                if (hi_of[k] - lo_of[k]) * CROSSED_SLOPE > tall[r] {
                    tall[r] = (hi_of[k] - lo_of[k]) * CROSSED_SLOPE;
                }
            }
            continue;
        }
        let mut members: Vec<usize> = all
            .iter()
            .copied()
            .filter(|k| level.contains(k) || (pieces.wide[*k] != 1 && beside.contains(k)))
            .collect();
        lay(&mut members, &mut pieces.track, &mut pieces.levels);
        let mut most = 0;
        for &k in &members {
            most = most.max(pieces.levels[k]);
        }
        track_count[r] = most;
    }
}

/// Moves the bends in ranks that hold only bends onto the straight line between the nearest
/// points of their edges that are held in place, and marks the ones that can be left out.
fn straighten(
    g: &Graph,
    xs: &mut [f64],
    empty: &[bool],
    first_dummy: &[usize],
    span: &[usize],
    sep: f64,
    on_line: &mut [bool],
) {
    let len = xs.len();
    let mut want = vec![0.0; len];
    let mut from = vec![0.0; len];
    let mut to = vec![0.0; len];
    for ei in 0..first_dummy.len() {
        let d0 = first_dummy[ei];
        if d0 == NONE {
            continue;
        }
        let end = d0 + span[ei] - 1;
        let base = g.rank[d0];
        let mut d = d0;
        while d < end {
            if !empty[base + d - d0] {
                d += 1;
                continue;
            }
            let mut last = d;
            while last + 1 < end && empty[base + last + 1 - d0] {
                last += 1;
            }
            let above = g.pred_e[g.pred_start[d]];
            let below = g.succ_e[g.succ_start[last]];
            let x0 = xs[g.ef[above]] + g.efd[above];
            let x1 = xs[g.et[below]] + g.etd[below];
            let steps = (last - d + 2) as f64;
            for k in d..=last {
                want[k] = x0 + ((x1 - x0) * (k - d + 1) as f64) / steps;
                from[k] = x0;
                to[k] = x1;
            }
            d = last + 1;
        }
    }
    let mut twins: Vec<usize> = Vec::new();
    for r in 1..g.max_rank {
        if !empty[r] {
            continue;
        }
        let s = g.layer_start[r];
        let end = g.layer_start[r + 1];
        let mut row: Vec<usize> = g.order[s..end].to_vec();
        row.sort_by(|&a, &b| {
            by(from[a] - from[b])
                .then(by(to[a] - to[b]))
                .then(a.cmp(&b))
        });
        let mut i = 0;
        while i < row.len() {
            twins.clear();
            let mut j = i;
            while j < row.len()
                && (from[row[j]] - from[row[i]]).abs() < 1.0
                && (to[row[j]] - to[row[i]]).abs() < 1.0
            {
                twins.push(row[j]);
                j += 1;
            }
            twins.sort_by(|&a, &b| g.pos[a].cmp(&g.pos[b]));
            let count = twins.len();
            for (k, &d) in twins.iter().enumerate() {
                xs[d] = want[d] + (k as f64 - (count - 1) as f64 / 2.0) * sep;
                if count == 1 || sep == 0.0 {
                    on_line[d] = true;
                }
            }
            i = j;
        }
    }
}

/// In a rank that holds labels and bends but no nodes, leaves out a bend where the edge would
/// pass clear of every label anyway.
fn past_labels(
    g: &Graph,
    xs: &mut [f64],
    first_dummy: &[usize],
    span: &[usize],
    clear: f64,
    on_line: &mut [bool],
) {
    let mut labelled = vec![false; g.max_rank + 1];
    let mut any = false;
    for r in 1..g.max_rank {
        let mut labels = false;
        let mut others = false;
        for i in g.layer_start[r]..g.layer_start[r + 1] {
            let kind = g.kind[g.order[i]];
            if kind == Kind::Label {
                labels = true;
            } else if kind != Kind::Dummy {
                others = true;
            }
        }
        if labels && !others {
            labelled[r] = true;
            any = true;
        }
    }
    if !any {
        return;
    }
    for ei in 0..first_dummy.len() {
        let d0 = first_dummy[ei];
        if d0 == NONE {
            continue;
        }
        let end = d0 + span[ei] - 1;
        for d in d0..end {
            if g.kind[d] != Kind::Dummy || !labelled[g.rank[d]] {
                continue;
            }
            let mut first = d;
            let mut last = d;
            while first > d0 && on_line[first - 1] {
                first -= 1;
            }
            while last + 1 < end && on_line[last + 1] {
                last += 1;
            }
            let above = g.pred_e[g.pred_start[first]];
            let below = g.succ_e[g.succ_start[last]];
            let from = xs[g.ef[above]] + g.efd[above];
            let to = xs[g.et[below]] + g.etd[below];
            let lo = from.min(to) - clear;
            let hi = from.max(to) + clear;
            let r = g.rank[d];
            let free = (g.layer_start[r]..g.layer_start[r + 1]).all(|i| {
                let label = g.order[i];
                !(g.kind[label] == Kind::Label
                    && xs[label] + g.w[label] / 2.0 > lo
                    && xs[label] - g.w[label] / 2.0 < hi)
            });
            if !free {
                continue;
            }
            on_line[d] = true;
            let steps = (last - first + 2) as f64;
            for k in first..=last {
                xs[k] = from + ((to - from) * (k - first + 1) as f64) / steps;
            }
        }
    }
}

/// Returns (final crossings, crossings before grouping or reordering).
fn reduce_crossings(g: &mut Graph, nodes: &[LNode]) -> (i64, i64) {
    let n = g.n;
    let real = g.real;
    let max_rank = g.max_rank;
    let segs = g.ef.len();
    let mut widest = 0;
    for r in 0..=max_rank {
        widest = widest.max(g.layer_start[r + 1] - g.layer_start[r]);
    }
    if widest < 2 {
        return (0, 0);
    }
    let offsets = g.offsets;
    // With offsets, edges that reach one node at different offsets take separate places.
    let mut place = vec![0usize; if offsets { segs } else { 0 }];
    let mut place_in = vec![0usize; if offsets { segs } else { 0 }];
    let mut places = vec![0usize; if offsets { n } else { 0 }];
    let mut leaving = g.succ_e.clone();
    let mut mixed = vec![false; if offsets { n } else { 0 }];
    if offsets {
        let mut arriving = g.pred_e.clone();
        let by_offset = |list: &mut [usize], from: usize, to: usize, dx: &[f64]| -> bool {
            for k in from + 1..to {
                if dx[list[k]] == dx[list[from]] {
                    continue;
                }
                list[from..to].sort_by(|&a, &b| by(dx[a] - dx[b]));
                return true;
            }
            false
        };
        for v in 0..n {
            places[v] = 1;
            if by_offset(&mut arriving, g.pred_start[v], g.pred_start[v + 1], &g.etd) {
                for k in g.pred_start[v] + 1..g.pred_start[v + 1] {
                    if g.etd[arriving[k]] != g.etd[arriving[k - 1]] {
                        places[v] += 1;
                    }
                    place_in[arriving[k]] = places[v] - 1;
                }
            }
            if by_offset(&mut leaving, g.succ_start[v], g.succ_start[v + 1], &g.efd) {
                mixed[v] = true;
            }
        }
    }
    let mut size = 1;
    while size < if offsets { widest + segs } else { widest } {
        size <<= 1;
    }
    let mut tree = vec![0i64; 2 * size];
    let mut targets = vec![0usize; segs];

    let count = |g: &Graph, place: &mut [usize], tree: &mut [i64], targets: &mut [usize]| -> i64 {
        let mut total = 0i64;
        for r in 0..max_rank {
            let mut south_len = g.layer_start[r + 2] - g.layer_start[r + 1];
            if offsets {
                let mut next = 0;
                for i in g.layer_start[r + 1]..g.layer_start[r + 2] {
                    let v = g.order[i];
                    for k in g.pred_start[v]..g.pred_start[v + 1] {
                        place[g.pred_e[k]] = next + place_in[g.pred_e[k]];
                    }
                    next += places[v];
                }
                south_len = next;
            }
            if south_len < 2 {
                continue;
            }
            let mut t = 0;
            for i in g.layer_start[r]..g.layer_start[r + 1] {
                let v = g.order[i];
                let from = g.succ_start[v];
                let to = g.succ_start[v + 1];
                let t0 = t;
                for k in from..to {
                    targets[t] = if offsets {
                        place[leaving[k]]
                    } else {
                        g.pos[g.et[g.succ_e[k]]]
                    };
                    t += 1;
                }
                let d = t - t0;
                if d < 2 {
                    continue;
                }
                if offsets && mixed[v] {
                    // Sort equal-offset edges by destination.
                    for a in t0 + 1..t {
                        let item = targets[a];
                        let dx = g.efd[leaving[from + a - t0]];
                        let mut b = a;
                        while b > t0
                            && g.efd[leaving[from + b - 1 - t0]] == dx
                            && targets[b - 1] > item
                        {
                            targets[b] = targets[b - 1];
                            b -= 1;
                        }
                        targets[b] = item;
                    }
                } else {
                    targets[t0..t].sort_unstable();
                }
            }
            if t < 2 {
                continue;
            }
            let mut leaves = 1;
            while leaves < south_len {
                leaves <<= 1;
            }
            tree[..2 * leaves].fill(0);
            for &target in targets.iter().take(t) {
                let mut i = target + leaves;
                tree[i] += 1;
                while i > 1 {
                    if i & 1 == 0 {
                        total += tree[i + 1];
                    }
                    i >>= 1;
                    tree[i] += 1;
                }
            }
        }
        total
    };

    let declared = count(g, &mut place, &mut tree, &mut targets);
    let mut bary = vec![0.0f64; n];

    // Keep groups contiguous, ordering each by its mean key.
    let group_parent = g.group_parent.clone();
    let groups = group_parent.as_ref().map_or(0, Vec::len);
    let mut depth = vec![0i32; groups];
    if let Some(parent) = &group_parent {
        for gr in 0..groups {
            let mut p = gr as i32;
            while p >= 0 {
                depth[gr] += 1;
                p = parent[p as usize];
            }
        }
    }
    let arrange = |g: &mut Graph, r: usize, key: &[f64]| {
        let (Some(grp), Some(parent)) = (g.grp.as_ref(), group_parent.as_ref()) else {
            return;
        };
        let s = g.layer_start[r];
        let e = g.layer_start[r + 1];
        let mut group_sum = vec![0.0f64; groups];
        let mut group_count = vec![0i64; groups];
        let mut group_first = vec![0usize; groups];
        for i in s..e {
            let v = g.order[i];
            let mut gr = grp[v];
            while gr >= 0 {
                let gu = gr as usize;
                if group_count[gu] == 0 {
                    group_first[gu] = i;
                }
                group_sum[gu] += key[v];
                group_count[gu] += 1;
                gr = parent[gu];
            }
        }
        let value = |item: usize| {
            if item >= n {
                group_sum[item - n] / group_count[item - n] as f64
            } else {
                key[item]
            }
        };
        let first = |item: usize| {
            if item >= n {
                group_first[item - n]
            } else {
                s + g.pos[item]
            }
        };
        let compare = |a: usize, b: usize| {
            let mut ia = a;
            let mut ib = b;
            let mut ca = grp[a];
            let mut cb = grp[b];
            let mut da = if ca < 0 { 0 } else { depth[ca as usize] };
            let mut db = if cb < 0 { 0 } else { depth[cb as usize] };
            while da > db {
                ia = n + ca as usize;
                ca = parent[ca as usize];
                da -= 1;
            }
            while db > da {
                ib = n + cb as usize;
                cb = parent[cb as usize];
                db -= 1;
            }
            while ca != cb {
                ia = n + ca as usize;
                ca = parent[ca as usize];
                ib = n + cb as usize;
                cb = parent[cb as usize];
            }
            by(value(ia) - value(ib)).then((first(ia) as i64 - first(ib) as i64).cmp(&0))
        };
        let mut row = g.order[s..e].to_vec();
        row.sort_by(|&a, &b| compare(a, b));
        g.order[s..e].copy_from_slice(&row);
        for i in s..e {
            let v = g.order[i];
            g.pos[v] = i - s;
        }
    };
    let has_groups = g.grp.is_some();
    if has_groups {
        let mut at = vec![0.0f64; n];
        for r in 0..=max_rank {
            for i in g.layer_start[r]..g.layer_start[r + 1] {
                at[g.order[i]] = (i - g.layer_start[r]) as f64;
            }
            arrange(g, r, &at);
        }
    }
    let mut best = if has_groups {
        count(g, &mut place, &mut tree, &mut targets)
    } else {
        declared
    };
    if best == 0 {
        return (0, declared);
    }
    let mut best_order = g.order.clone();
    let start = best.min(declared);

    let pin_of = |v: usize| if v < real { nodes[v].pin } else { 0 };
    let sweep = |g: &mut Graph, bary: &mut [f64], down: bool, bias_right: bool| {
        for stepi in 1..=max_rank {
            let r = if down { stepi } else { max_rank - stepi };
            if g.fixed[r] {
                continue;
            }
            let s = g.layer_start[r];
            let e = g.layer_start[r + 1];
            let mut moving = Vec::with_capacity(e - s);
            for i in s..e {
                let v = g.order[i];
                let pin = pin_of(v);
                if pin != 0 {
                    bary[v] = f64::from(pin) * 1e9;
                    moving.push(v);
                    continue;
                }
                let (start, list, other, other_dx) = if down {
                    (&g.pred_start, &g.pred_e, &g.ef, &g.efd)
                } else {
                    (&g.succ_start, &g.succ_e, &g.et, &g.etd)
                };
                let from = start[v];
                let to = start[v + 1];
                if from == to {
                    if has_groups {
                        let across = if down {
                            g.layer_start[r] - g.layer_start[r - 1]
                        } else {
                            g.layer_start[r + 2] - g.layer_start[r + 1]
                        };
                        bary[v] = ((i - s) as f64 * across as f64) / (e - s) as f64;
                        continue;
                    }
                    bary[v] = -1.0;
                    continue;
                }
                let mut sum = 0.0;
                for k in from..to {
                    let u = other[list[k]];
                    sum += g.pos[u] as f64;
                    if offsets && g.w[u] > 0.0 {
                        sum += other_dx[list[k]] / g.w[u];
                    }
                }
                bary[v] = sum / (to - from) as f64;
                moving.push(v);
            }
            if has_groups {
                arrange(g, r, bary);
                continue;
            }
            if moving.len() < 2 {
                continue;
            }
            let pos = &g.pos;
            moving.sort_by(|&a, &b| {
                by(bary[a] - bary[b]).then_with(|| {
                    if bias_right {
                        pos[b].cmp(&pos[a])
                    } else {
                        pos[a].cmp(&pos[b])
                    }
                })
            });
            let mut k = 0;
            for i in s..e {
                if bary[g.order[i]] != -1.0 {
                    g.order[i] = moving[k];
                    k += 1;
                }
            }
            for i in s..e {
                let v = g.order[i];
                g.pos[v] = i - s;
            }
        }
    };

    let out_of_order = |g: &Graph| -> i64 {
        let mut total = 0;
        for r in 0..=max_rank {
            for i in g.layer_start[r]..g.layer_start[r + 1] {
                if g.order[i] >= real {
                    continue;
                }
                for j in i + 1..g.layer_start[r + 1] {
                    if g.order[j] < g.order[i] {
                        total += 1;
                    }
                }
            }
        }
        total
    };
    let settle_positions = |g: &mut Graph| {
        for r in 0..=max_rank {
            for i in g.layer_start[r]..g.layer_start[r + 1] {
                let v = g.order[i];
                g.pos[v] = i - g.layer_start[r];
            }
        }
    };
    let at = |g: &Graph, e: usize, up: bool| -> f64 {
        let u = if up { g.ef[e] } else { g.et[e] };
        if offsets && g.w[u] > 0.0 {
            g.pos[u] as f64 + (if up { g.efd[e] } else { g.etd[e] }) / g.w[u]
        } else {
            g.pos[u] as f64
        }
    };
    let between = |g: &Graph, a: usize, b: usize| -> i64 {
        let mut c = 0;
        for k in g.pred_start[a]..g.pred_start[a + 1] {
            let x = at(g, g.pred_e[k], true);
            for j in g.pred_start[b]..g.pred_start[b + 1] {
                if x > at(g, g.pred_e[j], true) {
                    c += 1;
                }
            }
        }
        for k in g.succ_start[a]..g.succ_start[a + 1] {
            let x = at(g, g.succ_e[k], false);
            for j in g.succ_start[b]..g.succ_start[b + 1] {
                if x > at(g, g.succ_e[j], false) {
                    c += 1;
                }
            }
        }
        c
    };
    let transpose = |g: &mut Graph| {
        for _ in 0..8 {
            let mut improved = false;
            for r in 0..=max_rank {
                if g.fixed[r] {
                    continue;
                }
                let s = g.layer_start[r];
                let mut i = s;
                while i + 1 < g.layer_start[r + 1] {
                    let a = g.order[i];
                    let b = g.order[i + 1];
                    let same_group = g.grp.as_ref().is_none_or(|grp| grp[a] == grp[b]);
                    if pin_of(a) == pin_of(b) && same_group && between(g, b, a) < between(g, a, b) {
                        g.order[i] = b;
                        g.order[i + 1] = a;
                        g.pos[b] = i - s;
                        g.pos[a] = i + 1 - s;
                        improved = true;
                    }
                    i += 1;
                }
            }
            if !improved {
                break;
            }
        }
    };

    // The sweeps are run twice from the same start, going down first and going up first; of
    // equal results the one that keeps more nodes in their given order wins.
    let initial = g.order.clone();
    let small = n <= 600;
    let mut chosen = best_order.clone();
    let mut chosen_out = i64::MAX;
    for attempt in 0..if small { 2 } else { 1 } {
        if attempt > 0 {
            g.order.copy_from_slice(&initial);
            settle_positions(g);
        }
        let mut found = start;
        let mut found_order = initial.clone();
        let mut i = 0;
        let mut stale = 0;
        while stale < 4 && i < MAX_SWEEPS {
            sweep(g, &mut bary, (i + attempt) % 2 == 0, i % 4 >= 2);
            let c = count(g, &mut place, &mut tree, &mut targets);
            if c < found {
                found = c;
                found_order.copy_from_slice(&g.order);
                stale = 0;
                if c == 0 {
                    break;
                }
            }
            i += 1;
            stale += 1;
        }
        g.order.copy_from_slice(&found_order);
        if small && found > 0 {
            settle_positions(g);
            transpose(g);
            let c = count(g, &mut place, &mut tree, &mut targets);
            if c < found {
                found = c;
                found_order.copy_from_slice(&g.order);
            }
        }
        let out = if small { out_of_order(g) } else { 0 };
        if attempt == 0 || found < best || (found == best && out < chosen_out) {
            best = found;
            chosen = found_order;
            chosen_out = out;
        }
    }
    g.order.copy_from_slice(&chosen);
    settle_positions(g);
    best_order.copy_from_slice(&chosen);
    if has_groups {
        best = count(g, &mut place, &mut tree, &mut targets);
    }

    // A drawing and its mirror image cross as often too. Of those two, likewise.
    if !small || (0..real).any(|i| nodes[i].pin != 0) {
        return (best, declared);
    }
    if chosen_out == 0 || g.fixed[0] || g.fixed[max_rank] {
        return (best, declared);
    }
    for r in 0..=max_rank {
        let (s, e) = (g.layer_start[r], g.layer_start[r + 1]);
        g.order[s..e].reverse();
    }
    settle_positions(g);
    if count(g, &mut place, &mut tree, &mut targets) > best || out_of_order(g) >= chosen_out {
        g.order.copy_from_slice(&best_order);
    }
    (best, declared)
}

/// Brandes-Köpf horizontal coordinate assignment: four aligned layouts, balanced.
fn position(g: &mut Graph, opt: &LayeredOptions) -> Vec<f64> {
    let n = g.n;
    let max_rank = g.max_rank;
    let segments = g.ef.len();
    let gap: Vec<f64> = (0..n).map(|i| half_room(g, opt, i)).collect();

    // Neighbours in left-to-right order, so the medians can be read off directly.
    let mut key = vec![0i64; segments];
    for e in 0..segments {
        key[e] = g.pos[g.et[e]] as i64;
    }
    for v in 0..n {
        if g.succ_start[v + 1] - g.succ_start[v] > 1 {
            sort_range(&mut g.succ_e, g.succ_start[v], g.succ_start[v + 1], &key);
        }
    }
    for e in 0..segments {
        key[e] = g.pos[g.ef[e]] as i64;
    }
    for v in 0..n {
        if g.pred_start[v + 1] - g.pred_start[v] > 1 {
            sort_range(&mut g.pred_e, g.pred_start[v], g.pred_start[v + 1], &key);
        }
    }
    let g = &*g;

    // An edge between two real nodes may not cross a dummy-to-dummy segment.
    let mut conflict = vec![false; segments];
    for r in 1..=max_rank {
        let prev_len = g.layer_start[r] - g.layer_start[r - 1];
        let s = g.layer_start[r];
        let end = g.layer_start[r + 1];
        let mut k0 = 0;
        let mut scan = s;
        for i in s..end {
            let v = g.order[i];
            let mut inner = NONE;
            if is_dummy(g.kind[v]) {
                let u = g.ef[g.pred_e[g.pred_start[v]]];
                if is_dummy(g.kind[u]) {
                    inner = u;
                }
            }
            let k1 = if inner != NONE {
                g.pos[inner]
            } else {
                prev_len
            };
            if inner != NONE || i == end - 1 {
                for j in scan..=i {
                    let w = g.order[j];
                    let w_dummy = is_dummy(g.kind[w]);
                    for k in g.pred_start[w]..g.pred_start[w + 1] {
                        let e = g.pred_e[k];
                        let up = g.pos[g.ef[e]];
                        if (up < k0 || k1 < up) && !(w_dummy && is_dummy(g.kind[g.ef[e]])) {
                            conflict[e] = true;
                        }
                    }
                }
                scan = i + 1;
                k0 = k1;
            }
        }
    }

    let mut results: Vec<Vec<f64>> = Vec::with_capacity(4);
    let mut root = vec![0usize; n];
    let mut align = vec![0usize; n];
    let mut shift = vec![0.0f64; n];
    let mut e_from = vec![0usize; n];
    let mut e_to = vec![0usize; n];
    let mut e_w = vec![0.0f64; n];
    let mut indeg = vec![0i64; n];
    let mut out_start = vec![0usize; n + 1];
    let mut out_e = vec![0usize; n];
    let mut fill = vec![0usize; n];
    let mut topo = vec![0usize; n];
    let mut x = vec![0.0f64; n];

    for variant in 0..4 {
        let up = variant < 2;
        let right = variant & 1 == 1;
        let (start, list, other) = if up {
            (&g.pred_start, &g.pred_e, &g.ef)
        } else {
            (&g.succ_start, &g.succ_e, &g.et)
        };
        let (own_dx, other_dx) = if up {
            (&g.etd, &g.efd)
        } else {
            (&g.efd, &g.etd)
        };
        for i in 0..n {
            root[i] = i;
            align[i] = i;
            shift[i] = 0.0;
            indeg[i] = 0;
            out_start[i + 1] = 0;
            x[i] = 0.0;
        }
        out_start[0] = 0;

        for stepi in 0..=max_rank {
            let r = if up { stepi } else { max_rank - stepi };
            let s = g.layer_start[r];
            let end = g.layer_start[r + 1];
            let adjacent = if up { r as i64 - 1 } else { r as i64 + 1 };
            let adjacent_last = if adjacent >= 0 && adjacent <= max_rank as i64 {
                let a = adjacent as usize;
                g.layer_start[a + 1] as i64 - g.layer_start[a] as i64 - 1
            } else {
                0
            };
            let mut prev_idx = -1i64;
            let indices: Box<dyn Iterator<Item = usize>> = if right {
                Box::new((s..end).rev())
            } else {
                Box::new(s..end)
            };
            for i in indices {
                let v = g.order[i];
                let from = start[v];
                let d = start[v + 1] - from;
                if d == 0 {
                    continue;
                }
                let lo = from + ((d - 1) >> 1);
                let hi = from + (d >> 1);
                for pass in 0..2 {
                    if pass == 1 && lo == hi {
                        break;
                    }
                    let e = list[if (pass == 0) != right { lo } else { hi }];
                    let w = other[e];
                    let idx = if right {
                        adjacent_last - g.pos[w] as i64
                    } else {
                        g.pos[w] as i64
                    };
                    if align[v] == v && prev_idx < idx && !conflict[e] {
                        align[w] = v;
                        root[v] = root[w];
                        align[v] = root[v];
                        shift[v] = shift[w] + other_dx[e] - own_dx[e];
                        prev_idx = idx;
                    }
                }
            }
        }

        let mut block_edges = 0;
        for r in 0..=max_rank {
            let s = g.layer_start[r];
            let end = g.layer_start[r + 1];
            for i in s + 1..end {
                let (a, b) = if right {
                    (g.order[i], g.order[i - 1])
                } else {
                    (g.order[i - 1], g.order[i])
                };
                let sep = gap[a] + gap[b];
                e_from[block_edges] = root[a];
                e_to[block_edges] = root[b];
                e_w[block_edges] = if right {
                    sep - shift[a] + shift[b]
                } else {
                    sep + shift[a] - shift[b]
                };
                indeg[root[b]] += 1;
                out_start[root[a] + 1] += 1;
                block_edges += 1;
            }
        }
        for i in 0..n {
            out_start[i + 1] += out_start[i];
        }
        fill.copy_from_slice(&out_start[..n]);
        for e in 0..block_edges {
            out_e[fill[e_from[e]]] = e;
            fill[e_from[e]] += 1;
        }

        let mut count = 0;
        for i in 0..n {
            if root[i] == i && indeg[i] == 0 {
                topo[count] = i;
                count += 1;
            }
        }
        let mut q = 0;
        while q < count {
            let v = topo[q];
            for k in out_start[v]..out_start[v + 1] {
                let e = out_e[k];
                let w = e_to[e];
                if x[v] + e_w[e] > x[w] {
                    x[w] = x[v] + e_w[e];
                }
                indeg[w] -= 1;
                if indeg[w] == 0 {
                    topo[count] = w;
                    count += 1;
                }
            }
            q += 1;
        }
        for q in (0..count).rev() {
            let v = topo[q];
            let mut min = f64::INFINITY;
            for k in out_start[v]..out_start[v + 1] {
                let e = out_e[k];
                let cand = x[e_to[e]] - e_w[e];
                if cand < min {
                    min = cand;
                }
            }
            if min != f64::INFINITY && min > x[v] {
                x[v] = min;
            }
        }
        let xs: Vec<f64> = (0..n)
            .map(|i| (if right { -x[root[i]] } else { x[root[i]] }) + shift[i])
            .collect();
        results.push(xs);
    }

    let mut narrowest = 0;
    let mut best_width = f64::INFINITY;
    let mut mins = [0.0f64; 4];
    let mut maxs = [0.0f64; 4];
    for k in 0..4 {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        let xs = &results[k];
        for i in 0..n {
            let half = g.w[i] / 2.0;
            if xs[i] - half < lo {
                lo = xs[i] - half;
            }
            if xs[i] + half > hi {
                hi = xs[i] + half;
            }
        }
        mins[k] = lo;
        maxs[k] = hi;
        if hi - lo < best_width {
            best_width = hi - lo;
            narrowest = k;
        }
    }
    for k in 0..4 {
        let delta = if k & 1 == 0 {
            mins[narrowest] - mins[k]
        } else {
            maxs[narrowest] - maxs[k]
        };
        if delta != 0.0 {
            for v in results[k].iter_mut() {
                *v += delta;
            }
        }
    }

    (0..n)
        .map(|i| {
            let (mut a, mut b, mut c, mut d) =
                (results[0][i], results[1][i], results[2][i], results[3][i]);
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            if c > d {
                std::mem::swap(&mut c, &mut d);
            }
            if a > c {
                std::mem::swap(&mut a, &mut c);
            }
            if b > d {
                std::mem::swap(&mut b, &mut d);
            }
            if b > c {
                std::mem::swap(&mut b, &mut c);
            }
            let _ = (a, d);
            (b + c) / 2.0
        })
        .collect()
}
