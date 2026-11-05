//! The drawing of a block diagram: the blocks in the grid of the composite that holds them, and
//! the links between them. Ported from pele's `diagrams/block/render.ts`.

use super::arrows::{OVERHANG, arrow_axis, arrow_mask, arrow_size, draw_arrow};
use super::db::{BlockModel, Kind};
use super::layout::{Cell, layout};
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{tighten, title_room};
use crate::diagrams::common::ordered::Ordered;
use crate::error::Error;
use crate::svg::edges::{EdgePath, edge_label_svg};
use crate::svg::markers::{marker, marker_trim};
use crate::svg::shapes::{draw_shape, shape_inset, shape_size};
use crate::svg::{
    Accessible, ResolvedStyle, class_names, esc_text, label_svg, num, resolve_style, rounded,
    svg_document,
};
use crate::text::Style;
use crate::text::label::{Label, layout_label};
use std::fmt::Write;

/// The room between the blocks.
const GAP: f64 = 12.0;
/// The width a label is wrapped at.
const WRAP: f64 = 200.0;
/// How far a bend of a link is rounded.
const CORNER: f64 = 8.0;
const SHAPE_ATTRS: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";
const LINE_ATTRS: &str = " stroke=\"var(--_b)\"";
/// Checking every link against every block is quadratic, so large diagrams route without it.
const OBSTACLE_BUDGET: f64 = 2_000_000.0;

/// What a block is in the drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewKind {
    Leaf,
    Arrow,
    Composite,
    Space,
}

/// A block as it is drawn.
struct View {
    /// The place of the block in the model's arena.
    block: usize,
    kind: ViewKind,
    /// The place of the view in the order the views were made.
    index: usize,
    shape: &'static str,
    label: Label,
    style: ResolvedStyle,
    mask: u32,
    dx: f64,
    dy: f64,
    /// The size as drawn, which for a fixed shape is smaller than its cell.
    bw: f64,
    bh: f64,
}

/// The way a link runs.
struct Route {
    /// The link.
    edge: usize,
    /// Two points for a straight line, three for a bend, none for a loop.
    points: Vec<f64>,
    /// The number of the loop, when the link joins a block to itself.
    loop_count: usize,
    label: Label,
    lx: f64,
    ly: f64,
}

/// The style a block is drawn in: the classes it is given, and the styles of its own.
fn style_of(model: &BlockModel, block: &super::db::Block) -> ResolvedStyle {
    let mut declarations: Vec<String> = Vec::new();
    // A style list is split on semicolons only, so one entry can still hold several
    // declarations.
    let add = |styles: &[String], declarations: &mut Vec<String>| {
        for style in styles {
            for part in style.split(';') {
                declarations.push(part.to_string());
            }
        }
    };
    let names: Vec<&str> = if block.classes.is_empty() {
        vec!["default"]
    } else {
        block.classes.iter().map(String::as_str).collect()
    };
    for name in names {
        if let Some(def) = model.classes.get(name) {
            add(&def.styles, &mut declarations);
        }
    }
    add(&block.styles, &mut declarations);
    resolve_style(&declarations)
}

/// Whether the outline of a block is round.
fn is_round(views: &[View], i: usize) -> bool {
    views[i].kind == ViewKind::Leaf && (views[i].shape == "circle" || views[i].shape == "dbl-circ")
}

/// Where a vertical line at `x` meets the outline, below the center (sign 1) or above it.
fn edge_y(cells: &[Cell], views: &[View], i: usize, x: f64, sign: f64) -> f64 {
    let cell = &cells[i];
    let view = &views[i];
    let hw = view.bw / 2.0;
    let hh = view.bh / 2.0;
    let dx = (x - cell.x).abs();
    let mut reach = hh;
    if is_round(views, i) {
        reach = (hw * hw - dx * dx).max(0.0).sqrt();
    } else if view.kind == ViewKind::Leaf && view.shape == "diam" {
        reach = hh * (1.0 - dx / hw).max(0.0);
    } else if view.kind == ViewKind::Leaf {
        let side = if sign > 0.0 { 2 } else { 0 };
        reach = hh - shape_inset(view.shape, view.bw, view.bh, side, 0.0);
    } else if view.kind == ViewKind::Arrow && arrow_axis(view.mask) == "x" {
        reach = hh - OVERHANG;
    }
    cell.y + sign * reach
}

/// Where a horizontal line at `y` meets the outline, right of the center (sign 1) or left of it.
fn edge_x(cells: &[Cell], views: &[View], i: usize, y: f64, sign: f64) -> f64 {
    let cell = &cells[i];
    let view = &views[i];
    let hw = view.bw / 2.0;
    let hh = view.bh / 2.0;
    let dy = (y - cell.y).abs();
    let mut reach = hw;
    if is_round(views, i) {
        reach = (hh * hh - dy * dy).max(0.0).sqrt();
    } else if view.kind == ViewKind::Leaf && view.shape == "diam" {
        reach = hw * (1.0 - dy / hh).max(0.0);
    } else if view.kind == ViewKind::Leaf {
        let side = if sign > 0.0 { 1 } else { 3 };
        reach = hw - shape_inset(view.shape, view.bw, view.bh, side, 0.0);
    } else if view.kind == ViewKind::Arrow && arrow_axis(view.mask) == "y" {
        reach = hw - OVERHANG;
    }
    cell.x + sign * reach
}

/// Where the line from the center toward (`tx`, `ty`) leaves the outline.
fn border(cells: &[Cell], views: &[View], i: usize, tx: f64, ty: f64) -> (f64, f64) {
    let cell = &cells[i];
    let view = &views[i];
    let dx = tx - cell.x;
    let dy = ty - cell.y;
    let hw = view.bw / 2.0;
    let hh = view.bh / 2.0;
    let k = if is_round(views, i) {
        hw / (dx.hypot(dy)).max(f64::MIN_POSITIVE)
    } else if view.kind == ViewKind::Leaf && view.shape == "diam" {
        1.0 / ((dx.abs() / hw) + (dy.abs() / hh)).max(f64::MIN_POSITIVE)
    } else {
        let by_x = if dx == 0.0 {
            f64::INFINITY
        } else {
            hw / dx.abs()
        };
        let by_y = if dy == 0.0 {
            f64::INFINITY
        } else {
            hh / dy.abs()
        };
        by_x.min(by_y)
    };
    let k = if k.is_infinite() { 0.0 } else { k };
    (cell.x + dx * k, cell.y + dy * k)
}

/// Whether a straight line crosses the box of a block.
fn crosses_box(
    cells: &[Cell],
    views: &[View],
    i: usize,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
) -> bool {
    let cell = &cells[i];
    let view = &views[i];
    let l = cell.x - view.bw / 2.0 + 1.0;
    let r = cell.x + view.bw / 2.0 - 1.0;
    let t = cell.y - view.bh / 2.0 + 1.0;
    let b = cell.y + view.bh / 2.0 - 1.0;
    if (x1 < l && x2 < l) || (x1 > r && x2 > r) || (y1 < t && y2 < t) || (y1 > b && y2 > b) {
        return false;
    }
    if x1 == x2 || y1 == y2 {
        return true;
    }
    // The line is diagonal and its box overlaps: the block is hit unless all four of its
    // corners are on one side.
    let dx = x2 - x1;
    let dy = y2 - y1;
    let side = |x: f64, y: f64| (dx * (y - y1) - dy * (x - x1)).signum();
    let first = side(l, t);
    first == 0.0 || side(r, t) != first || side(r, b) != first || side(l, b) != first
}

/// How many blocks a route crosses.
fn hits(
    cells: &[Cell],
    views: &[View],
    points: &[f64],
    s: usize,
    t: usize,
    obstacles: &[usize],
) -> usize {
    let mut count = 0;
    for &v in obstacles {
        if v == s || v == t {
            continue;
        }
        let mut i = 0;
        while i + 3 < points.len() {
            if crosses_box(
                cells,
                views,
                v,
                points[i],
                points[i + 1],
                points[i + 2],
                points[i + 3],
            ) {
                count += 1;
                break;
            }
            i += 2;
        }
    }
    count
}

/// A straight line where the two blocks share a row or a column, otherwise one bend between
/// facing sides.
fn route_edge(
    cells: &[Cell],
    views: &[View],
    s: usize,
    t: usize,
    shift: f64,
    obstacles: Option<&[usize]>,
) -> Vec<f64> {
    let sl = cells[s].x - views[s].bw / 2.0;
    let sr = cells[s].x + views[s].bw / 2.0;
    let st = cells[s].y - views[s].bh / 2.0;
    let sb = cells[s].y + views[s].bh / 2.0;
    let tl = cells[t].x - views[t].bw / 2.0;
    let tr = cells[t].x + views[t].bw / 2.0;
    let tt = cells[t].y - views[t].bh / 2.0;
    let tb = cells[t].y + views[t].bh / 2.0;
    let x1 = sl.max(tl);
    let x2 = sr.min(tr);
    let y1 = st.max(tt);
    let y2 = sb.min(tb);
    let over_x = x2 - x1 > 6.0;
    let over_y = y2 - y1 > 6.0;

    if over_x && over_y {
        // One block is inside the other: join the inner one to the nearest side of the outer.
        let inner = if views[s].bw * views[s].bh <= views[t].bw * views[t].bh {
            s
        } else {
            t
        };
        let outer = if inner == s { t } else { s };
        let up = cells[inner].y - views[inner].bh / 2.0 - (cells[outer].y - views[outer].bh / 2.0);
        let down =
            cells[outer].y + views[outer].bh / 2.0 - (cells[inner].y + views[inner].bh / 2.0);
        let left =
            cells[inner].x - views[inner].bw / 2.0 - (cells[outer].x - views[outer].bw / 2.0);
        let right =
            cells[outer].x + views[outer].bw / 2.0 - (cells[inner].x + views[inner].bw / 2.0);
        let least = up.min(down).min(left).min(right);
        let points = if least == down {
            vec![
                cells[inner].x,
                edge_y(cells, views, inner, cells[inner].x, 1.0),
                cells[inner].x,
                cells[outer].y + views[outer].bh / 2.0,
            ]
        } else if least == up {
            vec![
                cells[inner].x,
                edge_y(cells, views, inner, cells[inner].x, -1.0),
                cells[inner].x,
                cells[outer].y - views[outer].bh / 2.0,
            ]
        } else if least == right {
            vec![
                edge_x(cells, views, inner, cells[inner].y, 1.0),
                cells[inner].y,
                cells[outer].x + views[outer].bw / 2.0,
                cells[inner].y,
            ]
        } else {
            vec![
                edge_x(cells, views, inner, cells[inner].y, -1.0),
                cells[inner].y,
                cells[outer].x - views[outer].bw / 2.0,
                cells[inner].y,
            ]
        };
        return if inner == s {
            points
        } else {
            vec![points[2], points[3], points[0], points[1]]
        };
    }
    if over_x {
        let margin = 6.0f64.min((x2 - x1) / 2.0);
        let x = ((x1 + x2) / 2.0 + shift).max(x1 + margin).min(x2 - margin);
        let down = if cells[t].y > cells[s].y { 1.0 } else { -1.0 };
        return vec![
            x,
            edge_y(cells, views, s, x, down),
            x,
            edge_y(cells, views, t, x, -down),
        ];
    }
    if over_y {
        let margin = 6.0f64.min((y2 - y1) / 2.0);
        let y = ((y1 + y2) / 2.0 + shift).max(y1 + margin).min(y2 - margin);
        let right = if cells[t].x > cells[s].x { 1.0 } else { -1.0 };
        return vec![
            edge_x(cells, views, s, y, right),
            y,
            edge_x(cells, views, t, y, -right),
            y,
        ];
    }

    let right = if cells[t].x > cells[s].x { 1.0 } else { -1.0 };
    let down = if cells[t].y > cells[s].y { 1.0 } else { -1.0 };
    let across = vec![
        edge_x(cells, views, s, cells[s].y, right),
        cells[s].y,
        cells[t].x,
        cells[s].y,
        cells[t].x,
        edge_y(cells, views, t, cells[t].x, -down),
    ];
    let along = vec![
        cells[s].x,
        edge_y(cells, views, s, cells[s].x, down),
        cells[s].x,
        cells[t].y,
        edge_x(cells, views, t, cells[t].y, -right),
        cells[t].y,
    ];
    let wide = (cells[t].x - cells[s].x).abs() >= (cells[t].y - cells[s].y).abs();
    let (first, second) = if wide {
        (&across, &along)
    } else {
        (&along, &across)
    };
    let mut best = first.clone();
    let Some(obstacles) = obstacles else {
        return best;
    };
    let mut least = hits(cells, views, &best, s, t, obstacles);
    if least == 0 {
        return best;
    }
    let count = hits(cells, views, second, s, t, obstacles);
    if count < least {
        best = second.clone();
        least = count;
    }
    if least == 0 {
        return best;
    }
    let (sx, sy) = border(cells, views, s, cells[t].x, cells[t].y);
    let (ex, ey) = border(cells, views, t, cells[s].x, cells[s].y);
    let direct = vec![sx, sy, ex, ey];
    if hits(cells, views, &direct, s, t, obstacles) < least {
        direct
    } else {
        best
    }
}

/// Builds the path through two or three points, rounding the bend and leaving room for markers.
fn trace(points: &[f64], start_trim: f64, end_trim: f64) -> EdgePath {
    let n = points.len();
    let sx = points[0];
    let sy = points[1];
    let ex = points[n - 2];
    let ey = points[n - 1];
    let mut ax = points[2] - sx;
    let mut ay = points[3] - sy;
    let first = ax.hypot(ay);
    ax /= if first == 0.0 { 1.0 } else { first };
    ay /= if first == 0.0 { 1.0 } else { first };
    let mut bx = ex - points[n - 4];
    let mut by = ey - points[n - 3];
    let last = bx.hypot(by);
    bx /= if last == 0.0 { 1.0 } else { last };
    by /= if last == 0.0 { 1.0 } else { last };
    let mut start_trim = start_trim;
    let mut end_trim = end_trim;
    if n == 4 && start_trim + end_trim > first {
        let scale = first / (start_trim + end_trim);
        start_trim *= scale;
        end_trim *= scale;
    }
    start_trim = start_trim.min(first);
    end_trim = end_trim.min(last);
    let mut d = format!(
        "M{},{}",
        num(sx + ax * start_trim),
        num(sy + ay * start_trim)
    );
    if n == 6 {
        let r = 0.0f64.max(CORNER.min(first - start_trim).min(last - end_trim));
        d += &format!(
            "L{},{}Q{},{} {},{}",
            num(points[2] - ax * r),
            num(points[3] - ay * r),
            num(points[2]),
            num(points[3]),
            num(points[2] + bx * r),
            num(points[3] + by * r)
        );
    }
    d += &format!("L{},{}", num(ex - bx * end_trim), num(ey - by * end_trim));
    EdgePath {
        d,
        sx,
        sy,
        sdx: -ax,
        sdy: -ay,
        ex,
        ey,
        edx: bx,
        edy: by,
    }
}

/// A self-loop is an arc around the top right corner of the block, or around the matching point
/// on a round or diamond outline. Gives the arc's center and radius and the angles where it
/// leaves and returns.
fn loop_arc(cells: &[Cell], views: &[View], i: usize, k: usize) -> (f64, f64, f64, f64, f64) {
    let hw = views[i].bw / 2.0;
    let hh = views[i].bh / 2.0;
    let r = (10.0 + 5.0 * (k as f64 - 1.0)).min(hw).min(hh).max(1.0);
    if is_round(views, i) {
        let px = cells[i].x + hw * std::f64::consts::FRAC_1_SQRT_2;
        let py = cells[i].y - hw * std::f64::consts::FRAC_1_SQRT_2;
        let half = 2.0 * ((r / (2.0 * hw)).min(1.0)).asin();
        let angle =
            |at: f64| (cells[i].y - hw * at.sin() - py).atan2(cells[i].x + hw * at.cos() - px);
        return (
            px,
            py,
            r,
            angle(std::f64::consts::FRAC_PI_4 + half),
            angle(std::f64::consts::FRAC_PI_4 - half),
        );
    }
    if views[i].kind == ViewKind::Leaf && views[i].shape == "diam" {
        let slope = hh.atan2(hw);
        return (
            cells[i].x + hw / 2.0,
            cells[i].y - hh / 2.0,
            r,
            slope + std::f64::consts::PI,
            slope,
        );
    }
    (
        cells[i].x + hw,
        cells[i].y - hh,
        r,
        std::f64::consts::PI,
        std::f64::consts::FRAC_PI_2,
    )
}

/// The path of a self-loop.
fn loop_path(
    cells: &[Cell],
    views: &[View],
    i: usize,
    k: usize,
    start_trim: f64,
    end_trim: f64,
) -> EdgePath {
    let (px, py, r, a1, a2) = loop_arc(cells, views, i, k);
    let from = a1 + start_trim / r;
    let to = a2 - end_trim / r;
    let mut sweep = (to - from) % (2.0 * std::f64::consts::PI);
    if sweep <= 0.0 {
        sweep += 2.0 * std::f64::consts::PI;
    }
    let x1 = px + r * from.cos();
    let y1 = py + r * from.sin();
    let x2 = px + r * to.cos();
    let y2 = py + r * to.sin();
    let sx = px + r * a1.cos();
    let sy = py + r * a1.sin();
    let ex = px + r * a2.cos();
    let ey = py + r * a2.sin();
    // Each marker lies along the chord it covers, so it follows the curve instead of the tangent
    // at its tip.
    let sl = (sx - x1).hypot(sy - y1);
    let el = (ex - x2).hypot(ey - y2);
    EdgePath {
        d: format!(
            "M{},{}A{},{} 0 {} 1 {},{}",
            num(x1),
            num(y1),
            num(r),
            num(r),
            if sweep > std::f64::consts::PI { 1 } else { 0 },
            num(x2),
            num(y2)
        ),
        sx,
        sy,
        sdx: if sl > 0.0 { (sx - x1) / sl } else { a1.sin() },
        sdy: if sl > 0.0 { (sy - y1) / sl } else { -a1.cos() },
        ex,
        ey,
        edx: if el > 0.0 { (ex - x2) / el } else { -a2.sin() },
        edy: if el > 0.0 { (ey - y2) / el } else { a2.cos() },
    }
}

/// The places a label may sit on a leg, in the order they are tried.
const SPOTS: [f64; 5] = [0.5, 0.3, 0.7, 0.15, 0.85];

/// Picks the point on a leg where a label of the given size covers no block other than the two
/// it joins.
#[allow(
    clippy::too_many_arguments,
    reason = "the places a label may sit are many"
)]
fn label_spot(
    cells: &[Cell],
    views: &[View],
    points: &[f64],
    at: usize,
    w: f64,
    h: f64,
    s: usize,
    t: usize,
    obstacles: Option<&[usize]>,
) -> f64 {
    let Some(obstacles) = obstacles else {
        return 0.5;
    };
    for spot in SPOTS {
        let x = points[at] + (points[at + 2] - points[at]) * spot;
        let y = points[at + 1] + (points[at + 3] - points[at + 1]) * spot;
        let mut clear = true;
        for &v in obstacles {
            if v != s
                && v != t
                && (x - cells[v].x).abs() < (w + views[v].bw) / 2.0
                && (y - cells[v].y).abs() < (h + views[v].bh) / 2.0
            {
                clear = false;
                break;
            }
        }
        if clear {
            return spot;
        }
    }
    0.5
}

// Label boxes use the same obstacle checks during routing and stacking.
#[allow(
    clippy::too_many_arguments,
    reason = "reference geometry and label bounds"
)]
fn covers(
    cells: &[Cell],
    views: &[View],
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    s: usize,
    t: usize,
    obstacles: Option<&[usize]>,
    titles: &[[f64; 4]],
) -> bool {
    let Some(obstacles) = obstacles else {
        return false;
    };
    obstacles.iter().copied().any(|v| {
        v != s
            && v != t
            && (x - cells[v].x).abs() < (w + views[v].bw) / 2.0
            && (y - cells[v].y).abs() < (h + views[v].bh) / 2.0
    }) || titles
        .iter()
        .any(|v| (x - v[0]).abs() < (w + v[2]) / 2.0 && (y - v[1]).abs() < (h + v[3]) / 2.0)
}
#[allow(
    clippy::too_many_arguments,
    reason = "reference route and label placement inputs"
)]
fn stack_labels(
    stack: &[usize],
    routes: &mut [Route],
    legs: &[Vec<f64>],
    cells: &[Cell],
    views: &[View],
    s: usize,
    t: usize,
    obstacles: Option<&[usize]>,
    titles: &[[f64; 4]],
) -> f64 {
    const LABEL_GAP: f64 = 4.0;
    let vertical = legs[stack[0]][0] == legs[stack[0]][2];
    let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
    let mut need = -LABEL_GAP;
    let mut crowded = false;
    for (a, &i) in stack.iter().enumerate() {
        let leg = &legs[i];
        if if vertical {
            leg[0] != leg[2]
        } else {
            leg[1] != leg[3]
        } {
            return 0.0;
        }
        lo = lo.max(if vertical {
            leg[1].min(leg[3])
        } else {
            leg[0].min(leg[2])
        });
        hi = hi.min(if vertical {
            leg[1].max(leg[3])
        } else {
            leg[0].max(leg[2])
        });
        let p = &routes[i];
        need += if vertical {
            p.label.height
        } else {
            p.label.width + 8.0
        };
        need += LABEL_GAP;
        for &j in &stack[..a] {
            let q = &routes[j];
            if (p.lx - q.lx).abs() < (p.label.width + q.label.width) / 2.0 + 8.0
                && (p.ly - q.ly).abs() < (p.label.height + q.label.height) / 2.0
            {
                crowded = true;
            }
        }
    }
    if !crowded {
        return 0.0;
    }
    let room = hi - lo - 2.0 * crate::svg::markers::ARROW;
    if need > room {
        return need - room;
    }
    let mut along = Vec::new();
    let mut next = (lo + hi - need) / 2.0;
    for &i in stack {
        let p = &routes[i];
        let size = if vertical {
            p.label.height
        } else {
            p.label.width + 8.0
        };
        let at = next + size / 2.0;
        if covers(
            cells,
            views,
            if vertical { p.lx } else { at },
            if vertical { at } else { p.ly },
            p.label.width + 8.0,
            p.label.height,
            s,
            t,
            obstacles,
            titles,
        ) {
            return 0.0;
        }
        along.push(at);
        next += size + LABEL_GAP;
    }
    for (&i, at) in stack.iter().zip(along) {
        if vertical {
            routes[i].ly = at;
        } else {
            routes[i].lx = at;
        }
    }
    0.0
}
fn widen(
    cells: &mut [Cell],
    parents: &[Option<usize>],
    s: usize,
    t: usize,
    vertical: bool,
    short: f64,
    base: &mut std::collections::HashMap<(usize, bool), Vec<f64>>,
) -> bool {
    let mut chain = std::collections::HashSet::new();
    let mut at = Some(s);
    while let Some(i) = at {
        chain.insert(i);
        at = parents[i];
    }
    let (mut tb, mut box_at) = (None, Some(t));
    while box_at.is_some_and(|i| !chain.contains(&i)) {
        tb = box_at;
        box_at = parents[box_at.unwrap()];
    }
    let (Some(b), Some(tb)) = (box_at, tb) else {
        return false;
    };
    if b == s {
        return false;
    }
    let mut sb = s;
    while parents[sb] != Some(b) {
        sb = parents[sb].unwrap();
    }
    let at = if vertical {
        if cells[sb].row == cells[tb].row {
            return false;
        }
        cells[sb].row.max(cells[tb].row) - 1
    } else {
        let (l, r) = if cells[sb].col < cells[tb].col {
            (sb, tb)
        } else {
            (tb, sb)
        };
        if cells[r].col < cells[l].col + cells[l].span {
            return false;
        }
        cells[r].col - 1
    } as usize;
    let gaps = if vertical {
        &mut cells[b].row_gaps
    } else {
        &mut cells[b].col_gaps
    };
    let original = base.entry((b, vertical)).or_insert_with(|| gaps.clone());
    if gaps.len() <= at {
        gaps.resize(at + 1, 0.0);
    }
    gaps[at] = gaps[at].max(original.get(at).copied().unwrap_or(0.0) + short);
    true
}
fn overlap(routes: &[Route]) -> f64 {
    let mut area = 0.0;
    for (i, p) in routes.iter().enumerate() {
        for q in &routes[i + 1..] {
            let pw = (p.label.width + 8.0) / 2.0;
            let qw = (q.label.width + 8.0) / 2.0;
            let w = (p.lx + pw).min(q.lx + qw) - (p.lx - pw).max(q.lx - qw);
            let h = (p.ly + p.label.height / 2.0).min(q.ly + q.label.height / 2.0)
                - (p.ly - p.label.height / 2.0).max(q.ly - q.label.height / 2.0);
            if p.label.width > 0.0 && q.label.width > 0.0 && w > 0.0 && h > 0.0 {
                area += w * h;
            }
        }
    }
    area
}
fn bounds(
    cells: &[Cell],
    views: &[View],
    routes: &[Route],
    ends: &[(usize, usize)],
    content: (f64, f64),
) -> (f64, f64, f64, f64) {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (0.0f64, 0.0f64, content.0, content.1);
    let mut grow = |x: f64, y: f64, w: f64, h: f64| {
        min_x = min_x.min(x - w / 2.0);
        min_y = min_y.min(y - h / 2.0);
        max_x = max_x.max(x + w / 2.0);
        max_y = max_y.max(y + h / 2.0);
    };
    for (i, route) in routes.iter().enumerate() {
        if route.loop_count > 0 {
            let (px, py, r, _, _) = loop_arc(cells, views, ends[i].0, route.loop_count);
            grow(px, py, 2.0 * r + 2.0, 2.0 * r + 2.0);
        }
        if route.label.width > 0.0 {
            grow(
                route.lx,
                route.ly,
                route.label.width + 8.0,
                route.label.height,
            );
        }
    }
    (min_x, min_y, max_x, max_y)
}

/// Draws a block diagram.
pub(crate) fn render_block(model: &BlockModel, input: &Input<'_>) -> Result<Output, Error> {
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let edge_size = (size * 0.875).round();
    let pad = options.padding.unwrap_or(8.0);
    let configured = match input.config.get("block") {
        Some(crate::preprocess::Value::Map(m)) => match m.get("padding") {
            Some(crate::preprocess::Value::Number(n)) if *n >= 0.0 => Some(*n),
            _ => None,
        },
        _ => None,
    };
    let wrap_width = |tight: bool| if tight { 80.0 } else { WRAP };

    let draw = |tight: bool| -> Output {
        let gap = configured
            .map(|n| n.min(200.0))
            .unwrap_or(GAP)
            .min(if tight { 6.0 } else { f64::INFINITY });
        let wrap = wrap_width(tight);

        // The views of the blocks, and the cells they are laid out in.
        let mut cells: Vec<Cell> = Vec::new();
        let mut views: Vec<View> = Vec::new();

        let make_view = |model: &BlockModel,
                         block: usize,
                         index: usize,
                         cells: &mut Vec<Cell>,
                         views: &mut Vec<View>| {
            let b = &model.arena[block];
            let style = if b.kind == Kind::Space {
                resolve_style::<&str>(&[])
            } else {
                style_of(model, b)
            };
            let text_style = if style.bold { Style::BOLD } else { Style::NONE };
            let text_style = if style.italic {
                text_style | Style::ITALIC
            } else {
                text_style
            };
            let font_size = style.font_size.unwrap_or(size);
            let mut kind = ViewKind::Leaf;
            let mut shape = "";
            let mut head = 0.0;
            let mut mask = 0;
            let mut dx = 0.0;
            let mut dy = 0.0;
            let mut nw = 0.0;
            let mut nh = 0.0;
            let label;
            match b.kind {
                Kind::Space => {
                    kind = ViewKind::Space;
                    label = layout_label(None, false, measurer, font_size, wrap, Style::NONE);
                }
                Kind::Composite => {
                    kind = ViewKind::Composite;
                    label = layout_label(
                        b.label.as_deref(),
                        false,
                        measurer,
                        font_size,
                        if tight { title_room(options) } else { 4000.0 },
                        text_style,
                    );
                    head = if label.height > 0.0 {
                        label.height + 4.0
                    } else {
                        0.0
                    };
                    nw = label.width + 2.0 * gap;
                    nh = head + 2.0 * gap;
                }
                Kind::BlockArrow => {
                    kind = ViewKind::Arrow;
                    label = layout_label(
                        b.label.as_deref(),
                        false,
                        measurer,
                        font_size,
                        wrap,
                        text_style,
                    );
                    let blank = label.lines.iter().all(|line| {
                        line.iter()
                            .all(|part| part.icon.is_none() && part.text.trim().is_empty())
                    });
                    mask = arrow_mask(&b.directions);
                    let s = if blank {
                        arrow_size(mask, 0.0, 0.0)
                    } else {
                        arrow_size(mask, label.width, label.height)
                    };
                    nw = s.w;
                    nh = s.h;
                    dx = s.dx;
                    dy = s.dy;
                }
                _ => {
                    shape = b.kind.shape();
                    label = layout_label(
                        b.label.as_deref(),
                        false,
                        measurer,
                        font_size,
                        wrap,
                        text_style,
                    );
                    let s = shape_size(shape, label.width, label.height);
                    nw = s.w;
                    // A cylinder's caps deepen as it widens, so it is sized for the deepest ones.
                    nh = if shape == "cyl" {
                        s.h.max(label.height + 40.0)
                    } else {
                        s.h
                    };
                    dy = s.dy;
                }
            }
            let span = if kind == ViewKind::Space {
                1
            } else {
                b.width_in_columns.unwrap_or(1)
            };
            let mut cell = Cell::new(nw, nh, span);
            cell.columns = if kind == ViewKind::Composite {
                b.columns.unwrap_or(-1)
            } else {
                -1
            };
            cell.space = kind == ViewKind::Space;
            cell.head = head;
            cells.push(cell);
            views.push(View {
                block,
                kind,
                index,
                shape,
                label,
                style,
                mask,
                dx,
                dy,
                bw: 0.0,
                bh: 0.0,
            });
        };

        make_view(model, model.root, 0, &mut cells, &mut views);
        cells[0].nw = 0.0;
        cells[0].nh = 0.0;
        let mut parents = vec![None];
        let mut at = 0;
        while at < cells.len() {
            let parent = at;
            at += 1;
            if views[parent].kind != ViewKind::Composite {
                continue;
            }
            let children = model.arena[views[parent].block].children.clone();
            for child in children {
                let index = views.len();
                make_view(model, child, index, &mut cells, &mut views);
                cells[parent].children.push(index);
                parents.push(Some(parent));
            }
        }

        let widths: Vec<_> = cells.iter().map(|c| c.nw).collect();
        let place = |cells: &mut Vec<Cell>, views: &mut Vec<View>| {
            for (c, &w) in cells.iter_mut().zip(&widths) {
                c.nw = w;
            }
            let content = layout(cells, 0, gap).unwrap_or((0.0, 0.0));
            let mut titles = Vec::new();
            let mut obstacles: Vec<usize> = Vec::new();
            for i in 0..views.len() {
                if views[i].kind == ViewKind::Leaf && is_fixed(views[i].shape) {
                    views[i].bw = cells[i].nw;
                    views[i].bh = cells[i].nh;
                } else if views[i].kind == ViewKind::Arrow {
                    views[i].bw = if cells[i].span > 1 {
                        cells[i].w
                    } else {
                        cells[i].nw
                    };
                    views[i].bh = cells[i].nh;
                } else {
                    views[i].bw = cells[i].w;
                    views[i].bh = if views[i].kind == ViewKind::Leaf && is_wide(views[i].shape) {
                        cells[i].nh
                    } else {
                        cells[i].h
                    };
                }
                if views[i].kind == ViewKind::Leaf && views[i].shape == "cyl" {
                    views[i].dy = 12.0f64.min(6.0f64.max(views[i].bw / 14.0)) / 2.0;
                }
                if views[i].kind == ViewKind::Leaf || views[i].kind == ViewKind::Arrow {
                    obstacles.push(i);
                } else if cells[i].head > 0.0 {
                    titles.push([
                        cells[i].x,
                        cells[i].y - cells[i].h / 2.0 + (cells[i].head + gap) / 2.0 + 2.0,
                        views[i].label.width,
                        views[i].label.height,
                    ]);
                }
            }

            (content, obstacles, titles)
        };
        let (mut content, mut obstacles, mut titles) = place(&mut cells, &mut views);

        // The view of every block of the arena: a block that only holds room, or that waits
        // for a class or a style, has none.
        let mut view_of: Vec<Option<usize>> = vec![None; model.arena.len()];
        for (index, view) in views.iter().enumerate() {
            view_of[view.block] = Some(index);
        }

        // Links between the same two blocks are spread apart instead of drawn on top of each
        // other.
        let mut pairs: Ordered<usize> = Ordered::default();
        let mut labelled: Ordered<usize> = Ordered::default();
        let mut ends: Vec<(usize, usize)> = Vec::new();
        let mut drawn: Vec<usize> = Vec::new();
        for (at, edge) in model.edges.iter().enumerate() {
            let s = edge
                .start
                .as_deref()
                .and_then(|id| model.by_id.get(id).copied())
                .and_then(|at| view_of.get(at).copied().flatten());
            let t = edge
                .end
                .as_deref()
                .and_then(|id| model.by_id.get(id).copied())
                .and_then(|at| view_of.get(at).copied().flatten());
            let (Some(s), Some(t)) = (s, t) else {
                continue;
            };
            // A block that is only there to hold room has no view of its own.
            if views[s].kind == ViewKind::Space || views[t].kind == ViewKind::Space {
                continue;
            }
            let key = format!(
                "{}|{}",
                views[s].index.min(views[t].index),
                views[s].index.max(views[t].index)
            );
            let total = pairs.get(&key).copied().unwrap_or(0) + 1;
            pairs.set(&key, total);
            if !edge.label.is_empty() && !labelled.contains(&key) {
                labelled.set(&key, 1);
            }
            ends.push((s, t));
            drawn.push(at);
        }
        let checked = drawn.len() as f64 * obstacles.len() as f64 <= OBSTACLE_BUDGET;
        let route = |cells: &[Cell], views: &[View], obstacles: &[usize]| {
            let check = checked.then_some(obstacles);
            let mut stacks: Vec<Vec<usize>> = Vec::new();
            let mut stack_at = std::collections::HashMap::new();
            let mut legs = vec![Vec::new(); drawn.len()];
            let mut seen: Ordered<usize> = Ordered::default();
            let mut routes: Vec<Route> = Vec::new();
            for (at, edge_at) in drawn.iter().enumerate() {
                let edge = &model.edges[*edge_at];
                let (s, t) = ends[at];
                let key = format!(
                    "{}|{}",
                    views[s].index.min(views[t].index),
                    views[s].index.max(views[t].index)
                );
                let total = pairs.get(&key).copied().unwrap_or(1);
                let k = seen.get(&key).copied().unwrap_or(0);
                seen.set(&key, k + 1);
                let label = layout_label(
                    if edge.label.is_empty() {
                        None
                    } else {
                        Some(edge.label.as_str())
                    },
                    false,
                    measurer,
                    edge_size,
                    WRAP,
                    Style::NONE,
                );
                let points: Vec<f64>;
                let lx: f64;
                let ly: f64;
                if s == t {
                    let (px, py, r, _, _) = loop_arc(cells, views, s, k + 1);
                    points = Vec::new();
                    lx = px + r + 6.0 + label.width / 2.0;
                    ly = py - r / 2.0;
                } else {
                    let step = if labelled.contains(&key) {
                        (edge_size * 1.5).round() + 2.0
                    } else {
                        14.0
                    };
                    points = route_edge(
                        cells,
                        views,
                        s,
                        t,
                        (k as f64 - (total as f64 - 1.0) / 2.0) * step,
                        check,
                    );
                    let n = points.len();
                    // The label goes on the longer leg.
                    let first_leg = (points[2] - points[0]).abs() + (points[3] - points[1]).abs();
                    let last_leg = (points[n - 2] - points[n - 4]).abs()
                        + (points[n - 1] - points[n - 3]).abs();
                    let leg_at = if n == 6 && last_leg > first_leg { 2 } else { 0 };
                    let spot = if label.width > 0.0 {
                        label_spot(
                            cells,
                            views,
                            &points,
                            leg_at,
                            label.width + 8.0,
                            label.height,
                            s,
                            t,
                            check,
                        )
                    } else {
                        0.5
                    };
                    lx = points[leg_at] + (points[leg_at + 2] - points[leg_at]) * spot;
                    ly = points[leg_at + 1] + (points[leg_at + 3] - points[leg_at + 1]) * spot;
                    if label.width > 0.0 && total > 1 {
                        legs[at] = points[leg_at..leg_at + 4].to_vec();
                        let slot = *stack_at.entry(key.clone()).or_insert_with(|| {
                            stacks.push(Vec::new());
                            stacks.len() - 1
                        });
                        stacks[slot].push(at);
                    }
                }
                routes.push(Route {
                    edge: *edge_at,
                    points,
                    loop_count: if s == t { k + 1 } else { 0 },
                    label,
                    lx,
                    ly,
                });
            }

            (routes, stacks, legs)
        };
        let settle = |grow: bool,
                      cells: &mut Vec<Cell>,
                      views: &[View],
                      obstacles: &[usize],
                      titles: &[[f64; 4]]| {
            let (mut routes, stacks, legs) = route(cells, views, obstacles);
            let mut base = std::collections::HashMap::new();
            let mut grown = false;
            for stack in stacks {
                if stack.len() < 2 {
                    continue;
                }
                let (s, t) = ends[stack[0]];
                let short = stack_labels(
                    &stack,
                    &mut routes,
                    &legs,
                    cells,
                    views,
                    s,
                    t,
                    checked.then_some(obstacles),
                    titles,
                );
                let vertical = legs[stack[0]][0] == legs[stack[0]][2];
                if grow
                    && short > 0.0
                    && (vertical || !tight)
                    && widen(cells, &parents, s, t, vertical, short.ceil(), &mut base)
                {
                    grown = true;
                }
            }
            (routes, grown)
        };
        let (mut routes, grown) = settle(
            (drawn.len() * drawn.len()) as f64 <= OBSTACLE_BUDGET,
            &mut cells,
            &views,
            &obstacles,
            &titles,
        );
        if grown {
            let before = overlap(&routes);
            let bound = bounds(&cells, &views, &routes, &ends, content);
            let narrow = bound.2 - bound.0;
            let mut grown = true;
            for round in 0..3 {
                if !grown {
                    break;
                }
                (content, obstacles, titles) = place(&mut cells, &mut views);
                (routes, grown) = settle(round < 2, &mut cells, &views, &obstacles, &titles);
            }
            let bound = bounds(&cells, &views, &routes, &ends, content);
            let wide = bound.2 - bound.0;
            let room = options
                .max_width
                .filter(|&w| w > 0.0)
                .unwrap_or(f64::INFINITY);
            if overlap(&routes) > before - 1.0
                || (wide > narrow && (wide + 2.0 * pad).ceil() > room)
            {
                for c in &mut cells {
                    c.row_gaps.clear();
                    c.col_gaps.clear();
                }
                (content, obstacles, titles) = place(&mut cells, &mut views);
                (routes, _) = settle(false, &mut cells, &views, &obstacles, &titles);
            }
        }
        let (min_x, min_y, max_x, max_y) = bounds(&cells, &views, &routes, &ends, content);

        let title = layout_label(
            model.title.as_deref(),
            false,
            measurer,
            size,
            if tight { title_room(options) } else { 4000.0 },
            Style::BOLD,
        );
        let title_height = if title.height > 0.0 {
            title.height + 12.0
        } else {
            0.0
        };
        let inner = (max_x - min_x).max(title.width);
        let ox = pad - min_x + (inner - (max_x - min_x)) / 2.0;
        let oy = pad + title_height - min_y;
        for cell in cells.iter_mut() {
            cell.x += ox;
            cell.y += oy;
        }
        let width = (inner + 2.0 * pad).ceil();
        let height = (max_y - min_y + title_height + 2.0 * pad).ceil();

        let mut fills = Vec::new();
        let mut clusters = String::new();
        let mut nodes = String::new();
        for i in 1..views.len() {
            let view = &views[i];
            if view.kind == ViewKind::Space {
                continue;
            }
            let block = &model.arena[view.block];
            let classes = class_names(&block.classes.join(" "));
            let id = esc_text(&block.id);
            if view.kind == ViewKind::Composite {
                let top = cells[i].y - cells[i].h / 2.0;
                if crate::svg::ink::paints(&view.style.shape) {
                    fills.push(crate::svg::fills::Fill {
                        bounds: [
                            cells[i].x - cells[i].w / 2.0,
                            top,
                            cells[i].x + cells[i].w / 2.0,
                            top + cells[i].h,
                        ],
                        shape: view.style.shape.clone(),
                    });
                }
                let opacity = if crate::svg::sets_fill(&view.style.shape) {
                    ""
                } else {
                    " fill-opacity=\"0.5\""
                };
                write!(
                    clusters,
                    "<g class=\"pele-cluster{classes}\" data-id=\"{id}\">\
                     <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\" \
                     {opacity} stroke=\"var(--_b)\"{}/>{}</g>",
                    num(cells[i].x - cells[i].w / 2.0),
                    num(top),
                    num(cells[i].w),
                    num(cells[i].h),
                    rounded(&view.style.shape),
                    label_svg(
                        &view.label,
                        cells[i].x,
                        top + (cells[i].head + gap) / 2.0 + 2.0,
                        &format!(
                            " class=\"pele-cluster-label\"{}{}",
                            crate::svg::ink::ink_fill(&view.style, Some("var(--_m)")),
                            view.style.text
                        ),
                    ),
                )
                .expect("writing to a string does not fail");
                continue;
            }
            let arrow = view.kind == ViewKind::Arrow;
            let mut label = view.label.clone();
            if cells[i].span > 1
                && label.width > 0.0
                && label.lines.len() > 1
                && view.bw > cells[i].nw
                && (arrow || !is_fixed(view.shape))
            {
                let style = (if view.style.bold {
                    Style::BOLD
                } else {
                    Style::NONE
                }) | (if view.style.italic {
                    Style::ITALIC
                } else {
                    Style::NONE
                });
                label = layout_label(
                    block.label.as_deref(),
                    false,
                    measurer,
                    label.size,
                    label.width + view.bw - cells[i].nw,
                    style,
                );
            }
            let ink = if arrow || crate::svg::shapes::shape_filled(view.shape) {
                crate::svg::ink::ink_fill(&view.style, None)
            } else {
                String::new()
            };
            write!(
                nodes,
                "<g class=\"pele-node pele-shape-{}{classes}\" data-id=\"{id}\" \
                 transform=\"translate({},{})\">{}{}</g>",
                if arrow { "block-arrow" } else { view.shape },
                num(cells[i].x),
                num(cells[i].y),
                if arrow {
                    draw_arrow(
                        view.mask,
                        view.bw,
                        view.bh,
                        &format!("{SHAPE_ATTRS}{}", view.style.shape),
                    )
                } else {
                    draw_shape(
                        view.shape,
                        view.bw,
                        view.bh,
                        &format!("{SHAPE_ATTRS}{}", view.style.shape),
                        &format!("{LINE_ATTRS}{}", view.style.line),
                    )
                },
                label_svg(
                    &label,
                    view.dx,
                    view.dy,
                    &format!(" class=\"pele-label\"{ink}{}", view.style.text),
                ),
            )
            .expect("writing to a string does not fail");
        }

        let mut casings = String::new();
        let mut edges = String::new();
        let mut labels = String::new();
        for (at, route) in routes.iter().enumerate() {
            let edge = &model.edges[route.edge];
            let start_type = edge.arrow_type_start.as_str();
            let end_type = edge.arrow_type_end.as_str();
            let path = if route.loop_count > 0 {
                loop_path(
                    &cells,
                    &views,
                    ends[at].0,
                    route.loop_count,
                    marker_trim(start_type),
                    marker_trim(end_type),
                )
            } else {
                let mut points = route.points.clone();
                let mut i = 0;
                while i < points.len() {
                    points[i] += ox;
                    points[i + 1] += oy;
                    i += 2;
                }
                trace(&points, marker_trim(start_type), marker_trim(end_type))
            };
            let over = crate::svg::fills::over_fills(
                &path.d,
                &[path.sx, path.sy, path.ex, path.ey],
                &fills,
                "",
            );
            if over.as_deref() == Some("") {
                casings += &crate::svg::fills::edge_casing(
                    &path,
                    start_type,
                    end_type,
                    "",
                    edge.thickness == "thick",
                );
            }
            let color = over
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("var(--_l)");
            let over_attr = over
                .as_ref()
                .filter(|s| !s.is_empty())
                .map_or_else(String::new, |s| format!(" stroke=\"{s}\""));
            let id = esc_text(&edge.id);
            let width = if edge.thickness == "thick" {
                " stroke-width=\"2.5\""
            } else {
                ""
            };
            let dash = if edge.pattern == "dotted" {
                " stroke-dasharray=\"3 4\""
            } else {
                ""
            };
            let arrows = format!(
                "{}{}",
                marker(start_type, path.sx, path.sy, path.sdx, path.sdy, color),
                marker(end_type, path.ex, path.ey, path.edx, path.edy, color)
            );
            write!(
                edges,
                "<g class=\"pele-edge\" data-id=\"{id}\"{over_attr}><path d=\"{}\"{width}{dash}/>{arrows}</g>",
                path.d,
            )
            .expect("writing to a string does not fail");
            if route.label.width > 0.0 {
                labels.push_str(&edge_label_svg(
                    &edge.id,
                    &route.label,
                    route.lx + ox,
                    route.ly + oy,
                    "",
                ));
            }
        }

        let content = label_svg(
            &title,
            width / 2.0,
            pad + title.height / 2.0,
            " class=\"pele-title\" font-weight=\"var(--_tw)\"",
        ) + &if clusters.is_empty() {
            String::new()
        } else {
            format!("<g class=\"pele-clusters\">{clusters}</g>")
        } + &crate::svg::fills::casings_svg(&casings)
            + &if edges.is_empty() {
                String::new()
            } else {
                format!(
                    "<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" \
                 stroke-linecap=\"round\">{edges}</g>"
                )
            }
            + &format!("<g class=\"pele-nodes\">{nodes}</g>")
            + &if labels.is_empty() {
                String::new()
            } else {
                format!("<g class=\"pele-edge-labels\" font-size=\"{edge_size}\">{labels}</g>")
            };

        let acc = Accessible {
            title: None,
            descr: None,
        };
        Output {
            svg: svg_document(
                crate::DiagramKind::Block,
                width,
                height,
                size,
                options,
                &acc,
                &content,
            ),
            width,
            height,
            links: Vec::new(),
        }
    };

    Ok(tighten(options, draw))
}

/// Whether a shape keeps its own size in a larger cell, where the others stretch to fill it.
fn is_fixed(shape: &str) -> bool {
    matches!(shape, "circle" | "dbl-circ" | "diam")
}

/// Whether a shape only widens, because its slants and curves are set by its height.
fn is_wide(shape: &str) -> bool {
    matches!(
        shape,
        "stadium" | "hex" | "odd" | "lean-r" | "lean-l" | "trap-b" | "trap-t"
    )
}
