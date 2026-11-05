//! Layout of a graph whose nodes may be nested in groups. Each group is laid out on its own,
//! in its own direction, and then placed in its parent as a single box. An edge that crosses a
//! group's border passes through a port on the side facing its other end. Ported from pele's
//! `layout/compound.ts`.

#![allow(
    clippy::needless_range_loop,
    reason = "ported index arithmetic reads more plainly with explicit indices"
)]

use std::collections::HashMap;

use super::layered::{
    Kind, LEdge, LNode, LayeredOptions, LayeredResult, by, ease, layered, ledge, lnode, orient,
    rank,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dir {
    TB,
    BT,
    LR,
    RL,
}

impl Dir {
    pub(crate) fn vertical(self) -> bool {
        matches!(self, Dir::TB | Dir::BT)
    }
}

/// The direction a diagram names, or top to bottom for anything else.
pub(crate) fn direction(dir: Option<&str>) -> Dir {
    match dir {
        Some("BT") => Dir::BT,
        Some("LR") => Dir::LR,
        Some("RL") => Dir::RL,
        _ => Dir::TB,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CNode {
    pub w: f64,
    pub h: f64,
    pub parent: i64,
    pub is_group: bool,
    pub dir: Option<Dir>,
    pub pad_x: f64,
    pub pad_top: f64,
    pub pad_bottom: f64,
    pub min_w: f64,
    /// Declaration order; a group takes the smallest value among its members.
    pub seq: f64,
    /// How much of a side its edges may spread along; 0 keeps them at its middle.
    pub span: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct CEdge {
    pub src: usize,
    pub dst: usize,
    pub minlen: i32,
    pub label_w: f64,
    pub label_h: f64,
    /// x, y, axis triples; axis is 0 where the edge runs vertically into the point, 1
    /// horizontally.
    pub route: Vec<f64>,
    pub label_x: f64,
    pub label_y: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CompoundResult {
    pub width: f64,
    pub height: f64,
}

pub(crate) fn cnode(w: f64, h: f64) -> CNode {
    CNode {
        w,
        h,
        parent: -1,
        is_group: false,
        dir: None,
        pad_x: 0.0,
        pad_top: 0.0,
        pad_bottom: 0.0,
        min_w: 0.0,
        seq: 0.0,
        span: 0.0,
        x: 0.0,
        y: 0.0,
    }
}

pub(crate) fn cedge(src: usize, dst: usize, minlen: i32, label_w: f64, label_h: f64) -> CEdge {
    CEdge {
        src,
        dst,
        minlen,
        label_w,
        label_h,
        route: Vec::new(),
        label_x: 0.0,
        label_y: 0.0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

fn start_side(dir: Dir) -> Side {
    match dir {
        Dir::TB => Side::Top,
        Dir::BT => Side::Bottom,
        Dir::LR => Side::Left,
        Dir::RL => Side::Right,
    }
}

fn opposite(side: Side) -> Side {
    match side {
        Side::Top => Side::Bottom,
        Side::Bottom => Side::Top,
        Side::Left => Side::Right,
        Side::Right => Side::Left,
    }
}

#[derive(Debug, Clone)]
struct PortEdge {
    e: usize,
    inner: usize,
    out: bool,
    side: Side,
    /// Cross-flow position outside the group from a previous layout; NaN if unknown.
    hint: f64,
    px: f64,
    py: f64,
    route: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
struct LevelEdge {
    e: usize,
    a: usize,
    b: usize,
}

#[derive(Debug, Clone)]
struct Level {
    id: i64,
    dir: Dir,
    items: Vec<usize>,
    edges: Vec<LevelEdge>,
    ports: Vec<PortEdge>,
    /// Index into `ports` by `edge * 2 + (0 out, 1 in)`.
    port_of: HashMap<usize, usize>,
    routes: HashMap<usize, Vec<f64>>,
    x: f64,
    y: f64,
}

const MAX_PORTS: usize = 40000;
const MAX_ORDERED_PORTS: usize = 400;
const MAX_ORDERED_EDGES: usize = 500;
const FLAT_SLACK: f64 = 1.5;
const FLAT_SPARE: f64 = 4.0;
const FAN: f64 = 24.0;
const IN_LINE: f64 = 0.5;
const CURVE_PIECES: usize = 3;
const CORNER_PIECES: usize = 2;
const STEP_PIECES: usize = 3;
const HANDLE: f64 = 0.62;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Style {
    Curved,
    Stepped,
    Linear,
}

pub(crate) fn style_of(opt: &LayeredOptions) -> Style {
    if opt.tracks {
        Style::Stepped
    } else if opt.straight {
        Style::Linear
    } else {
        Style::Curved
    }
}

struct Snapshot {
    nodes: Vec<f64>,
    routes: Vec<Vec<f64>>,
    labels: Vec<f64>,
    levels: Vec<f64>,
    total: CompoundResult,
}

fn snapshot(nodes: &[CNode], edges: &[CEdge], levels: &[Level], total: CompoundResult) -> Snapshot {
    let mut at = Vec::new();
    for c in nodes {
        at.extend([c.x, c.y, c.w, c.h]);
    }
    let mut labels = Vec::new();
    for e in edges {
        labels.extend([e.label_x, e.label_y]);
    }
    let mut places = Vec::new();
    for level in levels {
        places.extend([level.x, level.y]);
        for port in &level.ports {
            places.extend([port.px, port.py]);
        }
    }
    Snapshot {
        nodes: at,
        routes: edges.iter().map(|e| e.route.clone()).collect(),
        labels,
        levels: places,
        total,
    }
}

fn restore(
    s: Snapshot,
    nodes: &mut [CNode],
    edges: &mut [CEdge],
    levels: &mut [Level],
) -> CompoundResult {
    for (i, c) in nodes.iter_mut().enumerate() {
        c.x = s.nodes[4 * i];
        c.y = s.nodes[4 * i + 1];
        c.w = s.nodes[4 * i + 2];
        c.h = s.nodes[4 * i + 3];
    }
    for (i, (e, route)) in edges.iter_mut().zip(s.routes).enumerate() {
        e.route = route;
        e.label_x = s.labels[2 * i];
        e.label_y = s.labels[2 * i + 1];
    }
    let mut k = 0;
    for level in levels {
        level.x = s.levels[k];
        level.y = s.levels[k + 1];
        k += 2;
        for port in &mut level.ports {
            port.px = s.levels[k];
            port.py = s.levels[k + 1];
            k += 2;
        }
    }
    s.total
}

/// Counts how often the routes cross, approximating curves with straight pieces. Fans into or
/// out of a shared node do not count.
pub(crate) fn route_crossings(nodes: &[CNode], edges: &[CEdge], style: Style) -> i64 {
    struct Seg {
        ax: f64,
        ay: f64,
        bx: f64,
        by: f64,
        low: f64,
        high: f64,
        owner: usize,
        ends: u8,
    }
    let mut segs: Vec<Seg> = Vec::new();
    let mut top = f64::INFINITY;
    let mut bottom = f64::NEG_INFINITY;
    let (mut from_x, mut from_y, mut to_x, mut to_y) = (f64::NAN, f64::NAN, f64::NAN, f64::NAN);
    let mut add =
        |segs: &mut Vec<Seg>, ei: usize, part: u8, mut x0: f64, mut y0: f64, x1: f64, y1: f64| {
            let on = x0 == to_x
                && y0 == to_y
                && ((y1 == y0 && from_y == y0) || (x1 == x0 && from_x == x0));
            if on {
                if let Some(p) = segs.last() {
                    if p.owner == ei
                        && p.ends == part
                        && (x0 - from_x) * (x1 - x0) + (y0 - from_y) * (y1 - y0) > 0.0
                    {
                        segs.pop();
                        x0 = from_x;
                        y0 = from_y;
                    }
                }
            }
            from_x = x0;
            from_y = y0;
            to_x = x1;
            to_y = y1;
            let s = Seg {
                ax: x0.min(x1),
                bx: x0.max(x1),
                ay: if x0 <= x1 { y0 } else { y1 },
                by: if x0 <= x1 { y1 } else { y0 },
                low: y0.min(y1),
                high: y0.max(y1),
                owner: ei,
                ends: part,
            };
            top = top.min(s.low);
            bottom = bottom.max(s.high);
            segs.push(s);
        };
    for (ei, e) in edges.iter().enumerate() {
        if e.src == e.dst || e.route.len() <= 3 {
            continue;
        }
        let r = &e.route;
        let aligned =
            |k: usize| (r[k] - r[k - 3]).abs() < 0.01 || (r[k + 1] - r[k - 2]).abs() < 0.01;
        let length = |k: usize| (r[k] - r[k - 3]).abs() + (r[k + 1] - r[k - 2]).abs();
        let turns = |k: usize| {
            style == Style::Curved
                && k > 0
                && k + 3 < r.len()
                && aligned(k)
                && aligned(k + 3)
                && length(k) >= 0.01
                && length(k + 3) >= 0.01
                && ((r[k] - r[k - 3]).abs() < 0.01) != ((r[k + 3] - r[k]).abs() < 0.01)
        };
        let (mut at_x, mut at_y) = (r[0], r[1]);
        let mut k = 3;
        while k < r.len() {
            let part = u8::from(k == 3) | if k == r.len() - 3 { 2 } else { 0 };
            let (x0, y0) = (at_x, at_y);
            let (x1, y1) = (r[k], r[k + 1]);
            at_x = x1;
            at_y = y1;
            if aligned(k) || style == Style::Linear {
                if !turns(k) {
                    add(&mut segs, ei, part, x0, y0, x1, y1);
                    k += 3;
                    continue;
                }
                let before = if k >= 6 && turns(k - 3) {
                    length(k) / 2.0
                } else {
                    length(k)
                };
                let after = if turns(k + 3) {
                    length(k + 3) / 2.0
                } else {
                    length(k + 3)
                };
                let ux = (x1 - r[k - 3]) / length(k);
                let uy = (y1 - r[k - 2]) / length(k);
                let vx = (r[k + 3] - x1) / length(k + 3);
                let vy = (r[k + 4] - y1) / length(k + 3);
                let sx = x1 - ux * before;
                let sy = y1 - uy * before;
                add(&mut segs, ei, part, x0, y0, sx, sy);
                let c1x = x1 - ux * before * (1.0 - HANDLE);
                let c1y = y1 - uy * before * (1.0 - HANDLE);
                let c2x = x1 + vx * after * (1.0 - HANDLE);
                let c2y = y1 + vy * after * (1.0 - HANDLE);
                at_x = x1 + vx * after;
                at_y = y1 + vy * after;
                let turn = part | if k + 3 == r.len() - 3 { 2 } else { 0 };
                let (mut px, mut py) = (sx, sy);
                for i in 1..=CORNER_PIECES {
                    let t = i as f64 / CORNER_PIECES as f64;
                    let u = 1.0 - t;
                    let x = u * u * u * sx
                        + 3.0 * u * u * t * c1x
                        + 3.0 * u * t * t * c2x
                        + t * t * t * at_x;
                    let y = u * u * u * sy
                        + 3.0 * u * u * t * c1y
                        + 3.0 * u * t * t * c2y
                        + t * t * t * at_y;
                    add(&mut segs, ei, turn, px, py, x, y);
                    px = x;
                    py = y;
                }
                k += 3;
                continue;
            }
            let down = r[k + 2] == 0.0;
            if style == Style::Stepped {
                let mx = if down { x0 } else { (x0 + x1) / 2.0 };
                let my = if down { (y0 + y1) / 2.0 } else { y0 };
                let (nx, ny) = if down { (x1, my) } else { (mx, y1) };
                add(&mut segs, ei, part, x0, y0, mx, my);
                add(&mut segs, ei, part, mx, my, nx, ny);
                add(&mut segs, ei, part, nx, ny, x1, y1);
                k += 3;
                continue;
            }
            let c = if down {
                ease(x1 - x0, y1 - y0)
            } else {
                ease(y1 - y0, x1 - x0)
            };
            let (mut px, mut py) = (x0, y0);
            for i in 1..=CURVE_PIECES {
                let t = i as f64 / CURVE_PIECES as f64;
                let u = 1.0 - t;
                let across = t * t * (3.0 - 2.0 * t);
                let along = 3.0 * c * t * u * u + 3.0 * (1.0 - c) * t * t * u + t * t * t;
                let x = x0 + (x1 - x0) * if down { across } else { along };
                let y = y0 + (y1 - y0) * if down { along } else { across };
                add(&mut segs, ei, part, px, py, x, y);
                px = x;
                py = y;
            }
            k += 3;
        }
    }
    let _ = STEP_PIECES;
    let count = segs.len();
    // Count each pair only in the first band shared by both segments.
    let bands = if bottom > top {
        ((count as f64).sqrt().round() as usize).max(1)
    } else {
        1
    };
    let scale = if bottom > top {
        bands as f64 / (bottom - top)
    } else {
        0.0
    };
    let band_of = |y: f64| (bands - 1).min(((y - top) * scale).floor().max(0.0) as usize);
    let mut in_band: Vec<Vec<usize>> = vec![Vec::new(); bands];
    let first: Vec<usize> = segs.iter().map(|s| band_of(s.low)).collect();
    for (p, s) in segs.iter().enumerate() {
        for b in first[p]..=band_of(s.high) {
            in_band[b].push(p);
        }
    }
    // Edges merging into a shared endpoint run do not count as crossings.
    let mut runs = vec![0.0f64; edges.len() * 8];
    let in_line = |r: &[f64], a: usize, b: usize, c: usize| {
        ((r[a] - r[b]).abs() < IN_LINE && (r[a] - r[c]).abs() < IN_LINE)
            || ((r[a + 1] - r[b + 1]).abs() < IN_LINE && (r[a + 1] - r[c + 1]).abs() < IN_LINE)
    };
    for (ei, e) in edges.iter().enumerate() {
        let r = &e.route;
        if r.len() < 6 {
            continue;
        }
        let mut k = 3;
        while k + 3 < r.len() && in_line(r, 0, k, k + 3) {
            k += 3;
        }
        let last = r.len() - 3;
        let mut j = last as i64 - 3;
        while j >= 3 && in_line(r, last, j as usize, j as usize - 3) {
            j -= 3;
        }
        let j = j.max(0) as usize;
        runs[ei * 8..ei * 8 + 8].copy_from_slice(&[
            r[0],
            r[1],
            r[k],
            r[k + 1],
            r[j],
            r[j + 1],
            r[last],
            r[last + 1],
        ]);
    }
    let on_run = |ei: usize, end: u8, x: f64, y: f64| {
        let o = ei * 8 + if end == 1 { 0 } else { 4 };
        let (x0, y0) = (runs[o], runs[o + 1]);
        let dx = runs[o + 2] - x0;
        let dy = runs[o + 3] - y0;
        let length = dx * dx + dy * dy;
        let t = if length > 0.0 {
            0f64.max(1f64.min(((x - x0) * dx + (y - y0) * dy) / length))
        } else {
            0.0
        };
        (x - x0 - t * dx).hypot(y - y0 - t * dy) < IN_LINE
    };
    let mut total = 0;
    for (band, list) in in_band.iter_mut().enumerate() {
        list.sort_by(|&p, &q| by(segs[p].ax - segs[q].ax).then(p.cmp(&q)));
        for s in 0..list.len() {
            let p = list[s];
            let sp = &segs[p];
            let dx = sp.bx - sp.ax;
            let dy = sp.by - sp.ay;
            let here = first[p] == band;
            for &q in &list[s + 1..] {
                let sq = &segs[q];
                if sq.ax > sp.bx {
                    break;
                }
                if sq.high < sp.low
                    || sq.low > sp.high
                    || sp.owner == sq.owner
                    || (!here && first[q] != band)
                {
                    continue;
                }
                let d1 = dx * (sq.ay - sp.ay) - dy * (sq.ax - sp.ax);
                let d2 = dx * (sq.by - sp.ay) - dy * (sq.bx - sp.ax);
                if (d1 > 0.0) == (d2 > 0.0) {
                    continue;
                }
                let ex = sq.bx - sq.ax;
                let ey = sq.by - sq.ay;
                let d3 = ex * (sp.ay - sq.ay) - ey * (sp.ax - sq.ax);
                let d4 = ex * (sp.by - sq.ay) - ey * (sp.bx - sq.ax);
                if (d3 > 0.0) == (d4 > 0.0) {
                    continue;
                }
                let ep = &edges[sp.owner];
                let eq = &edges[sq.owner];
                let at = d3 / (d3 - d4);
                let x = sp.ax + at * dx;
                let y = sp.ay + at * dy;
                let fan = |v: usize, pe: u8, qe: u8| {
                    (sp.ends & pe != 0 && sq.ends & qe != 0)
                        || ((x - nodes[v].x).abs() < nodes[v].w / 2.0 + FAN
                            && (y - nodes[v].y).abs() < nodes[v].h / 2.0 + FAN)
                        || (on_run(sp.owner, pe, x, y) && on_run(sq.owner, qe, x, y))
                };
                if (ep.src == eq.src && fan(ep.src, 1, 1))
                    || (ep.src == eq.dst && fan(ep.src, 1, 2))
                    || (ep.dst == eq.src && fan(ep.dst, 2, 1))
                    || (ep.dst == eq.dst && fan(ep.dst, 2, 2))
                {
                    continue;
                }
                total += 1;
            }
        }
    }
    total
}

pub(crate) fn shift_layout(nodes: &mut [CNode], edges: &mut [CEdge], ox: f64, oy: f64) {
    for c in nodes {
        c.x += ox;
        c.y += oy;
    }
    for e in edges {
        for k in (0..e.route.len()).step_by(3) {
            e.route[k] += ox;
            e.route[k + 1] += oy;
        }
        e.label_x += ox;
        e.label_y += oy;
    }
}

/// What `build` makes of one level for the layered layout.
struct Built {
    lnodes: Vec<LNode>,
    ledges: Vec<LEdge>,
    dummies: Vec<i64>,
}

/// A level's layout input and what it gave, kept to reuse while the input is unchanged.
type Laid = (Vec<f64>, Vec<LNode>, Vec<LEdge>, LayeredResult);

struct Context<'a> {
    nodes: &'a mut [CNode],
    edges: &'a mut [CEdge],
    levels: Vec<Level>,
    /// The level of each group, by node index, and of the root at index `n`.
    level_index: HashMap<i64, usize>,
    local: Vec<usize>,
    key: Vec<f64>,
    keyed: bool,
    far_hint: Option<FarHint>,
    opt: &'a LayeredOptions,
    ordered: bool,
    laid: HashMap<i64, Vec<Laid>>,
    total: CompoundResult,
    root_dir: Dir,
}

/// What the flat layout tells about the ports leaving a group at its last rank.
struct FarHint {
    ledge_of: Vec<i64>,
    ledges: Vec<LEdge>,
    last: Vec<i64>,
}

fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()))
}

impl Context<'_> {
    fn level(&self, id: i64) -> &Level {
        &self.levels[self.level_index[&id]]
    }

    fn port(&self, group: usize, key: usize) -> &PortEdge {
        let level = self.level(group as i64);
        &level.ports[level.port_of[&key]]
    }

    fn port_mut(&mut self, group: usize, key: usize) -> &mut PortEdge {
        let li = self.level_index[&(group as i64)];
        let level = &mut self.levels[li];
        let pi = level.port_of[&key];
        &mut level.ports[pi]
    }

    fn endpoint_below(&self, item: usize, e: usize, out: bool) -> bool {
        let edge = &self.edges[e];
        (if out { edge.src } else { edge.dst }) != item
    }

    fn far_hint(&self, level: &Level, port: &PortEdge) -> f64 {
        let Some(far) = &self.far_hint else {
            return f64::NAN;
        };
        let le = far.ledge_of[port.e];
        if le < 0 || level.id < 0 || !port.out || port.side != opposite(start_side(level.dir)) {
            return f64::NAN;
        }
        let points = &far.ledges[le as usize].points;
        let last = far.last[level.id as usize] as f64;
        let mut inside = self.key[self.edges[port.e].src];
        let mut outside = 0.0;
        for k in (0..points.len()).step_by(2) {
            if points[k] == last {
                inside = points[k + 1];
            }
            if points[k] == last + 1.0 {
                outside = points[k + 1];
            }
        }
        inside + outside * 1e-6
    }

    fn build(&mut self, li: usize, sized: bool) -> Built {
        let level = &self.levels[li];
        let vertical = level.dir.vertical();
        let start = start_side(level.dir);
        let mut lnodes = Vec::new();
        let mut ledges = Vec::new();
        for (i, &item) in level.items.iter().enumerate() {
            self.local[item] = i;
            let node = &self.nodes[item];
            let mut ln = if sized {
                if vertical {
                    lnode(node.w, node.h, Kind::Node)
                } else {
                    lnode(node.h, node.w, Kind::Node)
                }
            } else {
                lnode(0.0, 0.0, Kind::Node)
            };
            ln.span = if node.is_group {
                node.span.min(ln.w - 2.0 * node.pad_x)
            } else {
                node.span
            };
            ln.key = if self.keyed { self.key[item] } else { f64::NAN };
            lnodes.push(ln);
        }
        let held = |ctx: &Self, item: usize, ei: usize, out: bool| {
            ctx.nodes[item].is_group && ctx.endpoint_below(item, ei, out)
        };
        let offset = |ctx: &Self, item: usize, ei: usize, out: bool| -> f64 {
            if !sized || !held(ctx, item, ei, out) {
                return 0.0;
            }
            let port = ctx.port(item, ei * 2 + usize::from(!out));
            if vertical {
                port.px - ctx.nodes[item].w / 2.0
            } else {
                port.py - ctx.nodes[item].h / 2.0
            }
        };
        for le in &level.edges {
            let e = &self.edges[le.e];
            let mut edge = ledge(self.local[le.a], self.local[le.b], e.minlen, 0.0, 0.0);
            if sized {
                edge.label_w = if vertical { e.label_w } else { e.label_h };
                edge.label_h = if vertical { e.label_h } else { e.label_w };
                edge.tail_dx = offset(self, le.a, le.e, true);
                edge.head_dx = offset(self, le.b, le.e, false);
                edge.tail_fixed = held(self, le.a, le.e, true);
                edge.head_fixed = held(self, le.b, le.e, false);
            }
            ledges.push(edge);
        }
        let mut dummies = vec![-1i64; level.ports.len()];
        for (i, port) in level.ports.iter().enumerate() {
            if port.side == start || port.side == opposite(start) {
                let d = lnodes.len();
                let mut ln = lnode(
                    0.0,
                    0.0,
                    if port.side == start {
                        Kind::StartPort
                    } else {
                        Kind::EndPort
                    },
                );
                ln.key = if self.keyed {
                    self.key[if port.out {
                        self.edges[port.e].dst
                    } else {
                        self.edges[port.e].src
                    }]
                } else {
                    f64::NAN
                };
                ln.hint = if port.hint.is_nan() && self.keyed && self.far_hint.is_some() {
                    self.far_hint(level, port)
                } else {
                    port.hint
                };
                lnodes.push(ln);
                dummies[i] = d as i64;
                let inner = self.local[port.inner];
                let mut edge = if port.out {
                    ledge(inner, d, 1, 0.0, 0.0)
                } else {
                    ledge(d, inner, 1, 0.0, 0.0)
                };
                if port.out {
                    edge.tail_dx = offset(self, port.inner, port.e, true);
                    edge.tail_fixed = held(self, port.inner, port.e, true);
                } else {
                    edge.head_dx = offset(self, port.inner, port.e, false);
                    edge.head_fixed = held(self, port.inner, port.e, false);
                }
                ledges.push(edge);
            } else {
                lnodes[self.local[port.inner]].pin =
                    if port.side == Side::Top || port.side == Side::Left {
                        -1
                    } else {
                        1
                    };
            }
        }
        Built {
            lnodes,
            ledges,
            dummies,
        }
    }

    fn lay_out(
        &mut self,
        id: i64,
        mut lnodes: Vec<LNode>,
        mut ledges: Vec<LEdge>,
    ) -> (Vec<LNode>, Vec<LEdge>, LayeredResult) {
        if !self.ordered {
            let size = layered(&mut lnodes, &mut ledges, self.opt);
            return (lnodes, ledges, size);
        }
        let mut given = Vec::new();
        for ln in &lnodes {
            given.extend([
                ln.w,
                ln.h,
                ln.kind as u8 as f64,
                f64::from(ln.pin),
                ln.hint,
                ln.key,
                ln.span,
            ]);
        }
        for le in &ledges {
            given.extend([
                le.tail as f64,
                le.head as f64,
                f64::from(le.minlen),
                le.label_w,
                le.label_h,
                le.tail_dx,
                le.head_dx,
                f64::from(u8::from(le.tail_fixed)),
                f64::from(u8::from(le.head_fixed)),
            ]);
        }
        let past = self.laid.entry(id).or_default();
        if let Some((_, n, e, s)) = past.iter().find(|(g, ..)| same(g, &given)) {
            return (n.clone(), e.clone(), *s);
        }
        let size = layered(&mut lnodes, &mut ledges, self.opt);
        past.push((given, lnodes.clone(), ledges.clone(), size));
        (lnodes, ledges, size)
    }

    fn place(&mut self) {
        // Rank parents first to determine which group sides edges cross.
        for li in 0..self.levels.len() {
            let ports = self.levels[li].ports.clone();
            for port in &ports {
                if self.nodes[port.inner].is_group
                    && self.endpoint_below(port.inner, port.e, port.out)
                {
                    self.port_mut(port.inner, port.e * 2 + usize::from(!port.out))
                        .side = port.side;
                }
            }
            let level_edges = self.levels[li].edges.clone();
            if level_edges.is_empty() {
                continue;
            }
            let needed = level_edges.iter().any(|le| {
                let e = &self.edges[le.e];
                (self.nodes[le.a].is_group && e.src != le.a)
                    || (self.nodes[le.b].is_group && e.dst != le.b)
            });
            if !needed {
                continue;
            }
            let Built {
                mut lnodes,
                mut ledges,
                ..
            } = self.build(li, false);
            orient(&lnodes, &mut ledges);
            rank(&mut lnodes, &ledges, 1);
            let start = start_side(self.levels[li].dir);
            for (i, le) in level_edges.iter().enumerate() {
                let (src, dst) = (self.edges[le.e].src, self.edges[le.e].dst);
                let a_above = !ledges[i].reversed;
                if self.nodes[le.a].is_group && src != le.a {
                    self.port_mut(le.a, le.e * 2).side =
                        if a_above { opposite(start) } else { start };
                }
                if self.nodes[le.b].is_group && dst != le.b {
                    self.port_mut(le.b, le.e * 2 + 1).side =
                        if a_above { start } else { opposite(start) };
                }
            }
        }

        // Lay out children first so their sizes are known.
        for li in (0..self.levels.len()).rev() {
            let id = self.levels[li].id;
            let dir = self.levels[li].dir;
            let vertical = dir.vertical();
            let flip = dir == Dir::BT || dir == Dir::RL;
            let built = self.build(li, true);
            let dummies = built.dummies;
            let (lnodes, ledges, size) = self.lay_out(id, built.lnodes, built.ledges);
            let group = (id >= 0).then(|| self.nodes[id as usize].clone());
            let content_w = if vertical { size.width } else { size.height };
            let content_h = if vertical { size.height } else { size.width };
            let pad_x = group.as_ref().map_or(0.0, |g| g.pad_x);
            let pad_top = group.as_ref().map_or(0.0, |g| g.pad_top);
            let box_w = (content_w + 2.0 * pad_x).max(group.as_ref().map_or(0.0, |g| g.min_w));
            let box_h = content_h + pad_top + group.as_ref().map_or(0.0, |g| g.pad_bottom);
            let off_x = (box_w - content_w) / 2.0;
            let off_y = pad_top;
            let tx = |lx: f64, ly: f64| {
                off_x
                    + if vertical {
                        lx
                    } else if flip {
                        size.height - ly
                    } else {
                        ly
                    }
            };
            let ty = |lx: f64, ly: f64| {
                off_y
                    + if vertical {
                        if flip { size.height - ly } else { ly }
                    } else {
                        lx
                    }
            };
            let axis = if vertical { 0.0 } else { 1.0 };
            let convert = |points: &[f64]| {
                let mut out = Vec::with_capacity(points.len() / 2 * 3);
                for i in (0..points.len()).step_by(2) {
                    out.extend([
                        tx(points[i], points[i + 1]),
                        ty(points[i], points[i + 1]),
                        axis,
                    ]);
                }
                out
            };

            let items = self.levels[li].items.clone();
            for (i, &item) in items.iter().enumerate() {
                self.nodes[item].x = tx(lnodes[i].x, lnodes[i].y);
                self.nodes[item].y = ty(lnodes[i].x, lnodes[i].y);
            }
            let level_edges = self.levels[li].edges.clone();
            for (i, le) in level_edges.iter().enumerate() {
                let edge = &ledges[i];
                self.levels[li].routes.insert(le.e, convert(&edge.points));
                let e = &mut self.edges[le.e];
                if e.label_w > 0.0 {
                    e.label_x = tx(edge.label_x, edge.label_y);
                    e.label_y = ty(edge.label_x, edge.label_y);
                }
            }
            let mut k = level_edges.len();
            for i in 0..self.levels[li].ports.len() {
                let port = self.levels[li].ports[i].clone();
                let item = self.nodes[port.inner].clone();
                let edge_out = |bx: &mut f64, by: &mut f64| match port.side {
                    Side::Top => *by = 0.0,
                    Side::Bottom => *by = box_h,
                    Side::Left => *bx = 0.0,
                    Side::Right => *bx = box_w,
                };
                let (px, py, route);
                if dummies[i] >= 0 {
                    let mut r = convert(&ledges[k].points);
                    k += 1;
                    let at = if port.out { r.len() - 3 } else { 0 };
                    let (mut bx, mut by) = (r[at], r[at + 1]);
                    edge_out(&mut bx, &mut by);
                    if port.out {
                        r.extend([bx, by, axis]);
                    } else {
                        r.splice(0..0, [bx, by, axis]);
                    }
                    px = bx;
                    py = by;
                    route = r;
                } else {
                    let (mut ix, mut iy) = (item.x, item.y);
                    if item.is_group && self.endpoint_below(port.inner, port.e, port.out) {
                        let inner = self.port(port.inner, port.e * 2 + usize::from(!port.out));
                        ix += inner.px - item.w / 2.0;
                        iy += inner.py - item.h / 2.0;
                    } else {
                        match port.side {
                            Side::Top => iy -= item.h / 2.0,
                            Side::Bottom => iy += item.h / 2.0,
                            Side::Left => ix -= item.w / 2.0,
                            Side::Right => ix += item.w / 2.0,
                        }
                    }
                    let (mut bx, mut by) = (ix, iy);
                    edge_out(&mut bx, &mut by);
                    let a = if port.side == Side::Top || port.side == Side::Bottom {
                        0.0
                    } else {
                        1.0
                    };
                    px = bx;
                    py = by;
                    route = if port.out {
                        vec![ix, iy, a, bx, by, a]
                    } else {
                        vec![bx, by, a, ix, iy, a]
                    };
                }
                let p = &mut self.levels[li].ports[i];
                p.px = px;
                p.py = py;
                p.route = route;
            }

            if id >= 0 {
                self.nodes[id as usize].w = box_w;
                self.nodes[id as usize].h = box_h;
            } else {
                self.total = CompoundResult {
                    width: box_w,
                    height: box_h,
                };
            }
        }

        for li in 0..self.levels.len() {
            let (ox, oy) = (self.levels[li].x, self.levels[li].y);
            for item in self.levels[li].items.clone() {
                let node = &mut self.nodes[item];
                node.x += ox;
                node.y += oy;
                if node.is_group {
                    let (x, y) = (node.x - node.w / 2.0, node.y - node.h / 2.0);
                    let child = self.level_index[&(item as i64)];
                    self.levels[child].x = x;
                    self.levels[child].y = y;
                }
            }
        }

        fn append(route: &mut Vec<f64>, part: &[f64], ox: f64, oy: f64) {
            for i in (0..part.len()).step_by(3) {
                let x = part[i] + ox;
                let y = part[i + 1] + oy;
                let len = route.len();
                if len >= 3
                    && (route[len - 3] - x).abs() < 0.01
                    && (route[len - 2] - y).abs() < 0.01
                {
                    continue;
                }
                route.extend([x, y, part[i + 2]]);
            }
        }

        for li in 0..self.levels.len() {
            let level_id = self.levels[li].id;
            for le in self.levels[li].edges.clone() {
                let (src, dst) = (self.edges[le.e].src, self.edges[le.e].dst);
                let mut route = Vec::new();
                let mut item = src;
                while self.nodes[item].parent != level_id {
                    let parent = self.nodes[item].parent;
                    let lv = self.level(parent);
                    let port = &lv.ports[lv.port_of[&(le.e * 2)]];
                    append(&mut route, &port.route, lv.x, lv.y);
                    item = parent as usize;
                }
                let lv = &self.levels[li];
                append(&mut route, &lv.routes[&le.e], lv.x, lv.y);
                let mut inward = Vec::new();
                let mut item = dst;
                while self.nodes[item].parent != level_id {
                    let parent = self.nodes[item].parent;
                    inward.push(self.level_index[&parent]);
                    item = parent as usize;
                }
                for &ii in inward.iter().rev() {
                    let lv = &self.levels[ii];
                    let port = &lv.ports[lv.port_of[&(le.e * 2 + 1)]];
                    append(&mut route, &port.route, lv.x, lv.y);
                }
                let (lx, ly) = (self.levels[li].x, self.levels[li].y);
                let e = &mut self.edges[le.e];
                e.route = route;
                if e.label_w > 0.0 {
                    e.label_x += lx;
                    e.label_y += ly;
                }
            }
        }
    }
}

/// Lays out a graph whose nodes may be nested in groups.
pub(crate) fn compound_layout(
    nodes: &mut [CNode],
    edges: &mut [CEdge],
    root_dir: Dir,
    opt: &LayeredOptions,
) -> CompoundResult {
    let n = nodes.len();
    let mut levels: Vec<Level> = Vec::new();
    let mut level_index: HashMap<i64, usize> = HashMap::new();
    let make_level = |levels: &mut Vec<Level>,
                      index: &mut HashMap<i64, usize>,
                      id: i64,
                      dir: Dir,
                      items: Vec<usize>| {
        index.insert(id, levels.len());
        levels.push(Level {
            id,
            dir,
            items,
            edges: Vec::new(),
            ports: Vec::new(),
            port_of: HashMap::new(),
            routes: HashMap::new(),
            x: 0.0,
            y: 0.0,
        });
    };

    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut root_items = Vec::new();
    for i in 0..n {
        let p = nodes[i].parent;
        if p >= 0 && (p as usize) < n && p as usize != i && nodes[p as usize].is_group {
            children[p as usize].push(i);
        } else {
            nodes[i].parent = -1;
            root_items.push(i);
        }
    }
    make_level(&mut levels, &mut level_index, -1, root_dir, root_items);
    let mut depth = vec![0usize; n];
    let mut placed = vec![false; n];
    let descend = |from: usize,
                   levels: &mut Vec<Level>,
                   index: &mut HashMap<i64, usize>,
                   depth: &mut Vec<usize>,
                   placed: &mut Vec<bool>,
                   nodes: &[CNode],
                   children: &[Vec<usize>]| {
        let mut q = from;
        while q < levels.len() {
            let (id, dir, items) = (levels[q].id, levels[q].dir, levels[q].items.clone());
            for item in items {
                placed[item] = true;
                depth[item] = if id < 0 { 0 } else { depth[id as usize] + 1 };
                if nodes[item].is_group {
                    make_level(
                        levels,
                        index,
                        item as i64,
                        nodes[item].dir.unwrap_or(dir),
                        children[item].clone(),
                    );
                }
            }
            q += 1;
        }
    };
    descend(
        0,
        &mut levels,
        &mut level_index,
        &mut depth,
        &mut placed,
        nodes,
        &children,
    );
    // Groups that contain each other are unreachable from the root; lift them out.
    for i in 0..n {
        if placed[i] {
            continue;
        }
        let former = nodes[i].parent as usize;
        children[former].retain(|&c| c != i);
        nodes[i].parent = -1;
        levels[0].items.push(i);
        placed[i] = true;
        depth[i] = 0;
        if nodes[i].is_group {
            let from = levels.len();
            make_level(
                &mut levels,
                &mut level_index,
                i as i64,
                nodes[i].dir.unwrap_or(root_dir),
                children[i].clone(),
            );
            descend(
                from,
                &mut levels,
                &mut level_index,
                &mut depth,
                &mut placed,
                nodes,
                &children,
            );
        }
    }

    for q in (1..levels.len()).rev() {
        let seq = levels[q]
            .items
            .iter()
            .map(|&i| nodes[i].seq)
            .fold(f64::INFINITY, f64::min);
        if seq != f64::INFINITY {
            nodes[levels[q].id as usize].seq = seq;
        }
    }
    for level in &mut levels {
        level
            .items
            .sort_by(|&a, &b| by(nodes[a].seq - nodes[b].seq).then(a.cmp(&b)));
    }

    // Every group border an edge crosses costs a port. Past the budget, an edge is drawn
    // between the groups that hold its ends, at the level they share.
    let parent_of = |nodes: &[CNode], i: usize| nodes[i].parent as usize;
    let mut ports = 0;
    for e in edges.iter_mut() {
        let (mut a, mut b) = (e.src, e.dst);
        let mut crossed = 0;
        while depth[a] > depth[b] {
            a = parent_of(nodes, a);
            crossed += 1;
        }
        while depth[b] > depth[a] {
            b = parent_of(nodes, b);
            crossed += 1;
        }
        while a != b && nodes[a].parent != nodes[b].parent {
            a = parent_of(nodes, a);
            b = parent_of(nodes, b);
            crossed += 2;
        }
        if a == b {
            continue;
        }
        if ports + crossed > MAX_PORTS {
            e.src = a;
            e.dst = b;
        } else {
            ports += crossed;
        }
    }

    let mut loops = Vec::new();
    let mut stray = Vec::new();
    for ei in 0..edges.len() {
        let e = &mut edges[ei];
        e.route.clear();
        if e.src == e.dst {
            loops.push(ei);
            continue;
        }
        let (mut a, mut b) = (e.src, e.dst);
        let mut a_path = Vec::new();
        let mut b_path = Vec::new();
        while depth[a] > depth[b] {
            a_path.push(a);
            a = parent_of(nodes, a);
        }
        while depth[b] > depth[a] {
            b_path.push(b);
            b = parent_of(nodes, b);
        }
        if a == b {
            stray.push(ei);
            continue;
        }
        while nodes[a].parent != nodes[b].parent {
            a_path.push(a);
            b_path.push(b);
            a = parent_of(nodes, a);
            b = parent_of(nodes, b);
        }
        levels[level_index[&nodes[a].parent]]
            .edges
            .push(LevelEdge { e: ei, a, b });
        for (path, out) in [(a_path, true), (b_path, false)] {
            for inner in path {
                let li = level_index[&nodes[inner].parent];
                let level = &mut levels[li];
                level
                    .port_of
                    .insert(ei * 2 + usize::from(!out), level.ports.len());
                level.ports.push(PortEdge {
                    e: ei,
                    inner,
                    out,
                    side: Side::Top,
                    hint: f64::NAN,
                    px: 0.0,
                    py: 0.0,
                    route: Vec::new(),
                });
            }
        }
    }

    let ordered = ports > 1 && ports <= MAX_ORDERED_PORTS && edges.len() <= MAX_ORDERED_EDGES;

    // Use a flat layout to coordinate node and port order across groups.
    let mut key = vec![f64::NAN; n];
    let mut flat_crossings = 0;
    let mut far_hint = None;
    let flat = levels.len() > 1 && ordered && levels.iter().all(|l| l.dir == root_dir);
    if flat {
        let vertical = root_dir.vertical();
        let mut leaf = vec![-1i64; n];
        let mut group = vec![-1i64; n];
        let mut lnodes = Vec::new();
        let mut of = Vec::new();
        let mut parent: Vec<i32> = Vec::new();
        for i in 0..n {
            if nodes[i].is_group {
                group[i] = parent.len() as i64;
                parent.push(-1);
            }
        }
        for i in 0..n {
            let p = nodes[i].parent;
            if nodes[i].is_group {
                parent[group[i] as usize] = if p >= 0 { group[p as usize] as i32 } else { -1 };
            } else {
                leaf[i] = lnodes.len() as i64;
                let (w, h) = if vertical {
                    (nodes[i].w, nodes[i].h)
                } else {
                    (nodes[i].h, nodes[i].w)
                };
                lnodes.push(lnode(w, h, Kind::Node));
                of.push(if p >= 0 { group[p as usize] as i32 } else { -1 });
            }
        }
        let mut ledges = Vec::new();
        let mut ledge_of = vec![-1i64; edges.len()];
        for (ei, e) in edges.iter().enumerate() {
            if e.src == e.dst || leaf[e.src] < 0 || leaf[e.dst] < 0 {
                continue;
            }
            ledge_of[ei] = ledges.len() as i64;
            let (lw, lh) = if vertical {
                (e.label_w, e.label_h)
            } else {
                (e.label_h, e.label_w)
            };
            ledges.push(ledge(
                leaf[e.src] as usize,
                leaf[e.dst] as usize,
                e.minlen,
                lw,
                lh,
            ));
        }
        let flat_opt = LayeredOptions {
            groups: Some((of, parent)),
            order_only: true,
            ..opt.clone()
        };
        let flat_layout = layered(&mut lnodes, &mut ledges, &flat_opt);
        flat_crossings = flat_layout.crossings.min(flat_layout.declared);
        let mut sum = vec![0.0f64; n];
        let mut count = vec![0i64; n];
        let mut last = vec![-1i64; n];
        for i in 0..n {
            if leaf[i] < 0 {
                continue;
            }
            let l = &lnodes[leaf[i] as usize];
            key[i] = l.x;
            let mut p = nodes[i].parent;
            while p >= 0 {
                let pu = p as usize;
                sum[pu] += key[i];
                count[pu] += 1;
                last[pu] = last[pu].max(i64::from(l.rank));
                p = nodes[pu].parent;
            }
        }
        for i in 0..n {
            if count[i] > 0 {
                key[i] = sum[i] / count[i] as f64;
            }
        }
        far_hint = Some(FarHint {
            ledge_of,
            ledges,
            last,
        });
    }

    let style = style_of(opt);
    let edge_count = edges.len();
    let mut ctx = Context {
        nodes,
        edges,
        levels,
        level_index,
        local: vec![0; n],
        key,
        keyed: flat,
        far_hint,
        opt,
        ordered,
        laid: HashMap::new(),
        total: CompoundResult::default(),
        root_dir,
    };
    let _ = ctx.root_dir;
    ctx.place();
    let mut crossings = -1;
    if flat {
        crossings = route_crossings(ctx.nodes, ctx.edges, style);
        if crossings as f64 > flat_crossings as f64 * FLAT_SLACK + FLAT_SPARE {
            let first = snapshot(ctx.nodes, ctx.edges, &ctx.levels, ctx.total);
            ctx.keyed = false;
            ctx.place();
            let own = route_crossings(ctx.nodes, ctx.edges, style);
            if own >= crossings {
                ctx.keyed = true;
                ctx.total = restore(first, ctx.nodes, ctx.edges, &mut ctx.levels);
            } else {
                crossings = own;
            }
        }
    }

    // Refine port order using routes outside each group.
    if ctx.keyed == flat && ordered {
        let mut again = false;
        for li in 0..ctx.levels.len() {
            let level = &ctx.levels[li];
            if level.id < 0 {
                continue;
            }
            let bx0 = &ctx.nodes[level.id as usize];
            let vertical = level.dir.vertical();
            let end = opposite(start_side(level.dir));
            let mut seen: Vec<(f64, f64)> = Vec::new();
            let mut hints = Vec::new();
            for (pi, port) in level.ports.iter().enumerate() {
                if port.side != end {
                    continue;
                }
                let route = &ctx.edges[port.e].route;
                let bx = level.x + port.px;
                let by = level.y + port.py;
                let mut at = -1i64;
                let mut k = 0;
                while k < route.len() && (at < 0 || port.out) {
                    if (route[k] - bx).abs() < 0.01 && (route[k + 1] - by).abs() < 0.01 {
                        at = k as i64;
                    }
                    k += 3;
                }
                if at < 0 {
                    continue;
                }
                let a = if vertical { 0 } else { 1 };
                let at = at as usize;
                let mut next = if port.out {
                    at as i64 + 3
                } else {
                    at as i64 - 3
                };
                while next >= 0
                    && (next as usize) < route.len()
                    && (route[next as usize + a] - route[at + a]).abs() < IN_LINE
                {
                    next += if port.out { 3 } else { -3 };
                }
                if next < 0 || next as usize >= route.len() {
                    continue;
                }
                let hint = route[next as usize + a]
                    - if vertical {
                        bx0.x - bx0.w / 2.0
                    } else {
                        bx0.y - bx0.h / 2.0
                    };
                hints.push((pi, hint));
                seen.push((if vertical { port.px } else { port.py }, hint));
            }
            for (pi, hint) in hints {
                ctx.levels[li].ports[pi].hint = hint;
            }
            seen.sort_by(|p, q| by(p.0 - q.0));
            if seen.windows(2).any(|w| w[0].1 > w[1].1) {
                again = true;
            }
        }
        if again {
            let before = if crossings >= 0 {
                crossings
            } else {
                route_crossings(ctx.nodes, ctx.edges, style)
            };
            if before > 0 {
                let first = snapshot(ctx.nodes, ctx.edges, &ctx.levels, ctx.total);
                ctx.place();
                if route_crossings(ctx.nodes, ctx.edges, style) >= before {
                    ctx.total = restore(first, ctx.nodes, ctx.edges, &mut ctx.levels);
                }
            }
        }
    }
    let total = ctx.total;
    let _ = edge_count;

    for ei in stray {
        let (a, b) = (
            edges_node(ctx.nodes, ctx.edges[ei].src),
            edges_node(ctx.nodes, ctx.edges[ei].dst),
        );
        let e = &mut ctx.edges[ei];
        e.route = vec![a.0, a.1, 0.0, b.0, b.1, 0.0];
        e.label_x = (a.0 + b.0) / 2.0;
        e.label_y = (a.1 + b.1) / 2.0;
    }
    for ei in loops {
        let node = ctx.nodes[ctx.edges[ei].src].clone();
        let e = &mut ctx.edges[ei];
        e.label_x = node.x + node.w / 2.0 + 28.0 + e.label_w / 2.0;
        e.label_y = node.y;
    }
    total
}

fn edges_node(nodes: &[CNode], i: usize) -> (f64, f64) {
    (nodes[i].x, nodes[i].y)
}
