//! The routes of the relations between the shapes of a C4 diagram. Ported from pele's
//! `route.ts`, which draws them the way Mermaid draws the relations of a flowchart.

use std::collections::HashMap;

use crate::svg::num;

/// The shared shape a box is drawn as. Mermaid 12.1.0's `c4ShapeAdapter.ts` gives each keyword
/// of a C4 diagram one of the shapes of the flowchart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Form {
    /// A person.
    Person,
    /// A cylinder, for a database.
    Cyl,
    /// A cylinder on its side, for a queue.
    HCyl,
    /// A box with rounded corners, for a system or a container.
    Rounded,
    /// A box with a folded corner, for a component.
    FrRect,
}

impl Form {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Form::Person => "person",
            Form::Cyl => "cyl",
            Form::HCyl => "h-cyl",
            Form::Rounded => "rounded",
            Form::FrRect => "fr-rect",
        }
    }
}

/// A laid out shape or boundary: top-left corner, size, and the shared shape it is drawn as.
#[derive(Debug, Clone)]
pub(crate) struct Frame {
    /// What tells two frames of the same shape apart.
    pub id: usize,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub form: Form,
}

const LOOP: f64 = 28.0;
pub(crate) const LANE: f64 = 20.0;
const LANE_STEP: f64 = 7.0;
const CORNER: f64 = 8.0;
// A wide label on an upright lane leans this far out from it, to the middle of the gap between
// columns.
const LEAN: f64 = 30.0;
// Label widths that fit the gap between two columns, and where there is room to spare.
pub(crate) const NARROW: f64 = 88.0;
const WIDE: f64 = 160.0;
// The space a label's background adds to the width of its text.
pub(crate) const LABEL_PAD: f64 = 8.0;
// The shared person shape: a head of radius 13 centered 14 below the top, over a body that
// starts 26 below it.
pub(crate) const HEAD: f64 = 26.0;

// Parallel stretches closer than this read as one line, and a line this close to a boundary's
// frame reads as part of it.
const NEAR: f64 = 6.0;
const FRAME_NEAR: f64 = 3.0;
const PORT_STEP: f64 = 12.0;

/// The cap radii `drawShape` uses for the two cylinders.
pub(crate) fn cyl_ry(w: f64) -> f64 {
    12.0f64.min(6.0f64.max(w / 14.0))
}

pub(crate) fn queue_rx(h: f64) -> f64 {
    12.0f64.min(h / 4.0)
}

/// JavaScript's `Math.sign`.
fn sign(value: f64) -> f64 {
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// How far along the segment from (px, py) by (dx, dy) it first meets the rectangle; infinity
/// when it misses.
#[allow(clippy::too_many_arguments, reason = "a segment and a rectangle")]
fn hit_rect(x0: f64, y0: f64, x1: f64, y1: f64, px: f64, py: f64, dx: f64, dy: f64) -> f64 {
    let mut enter = 0.0f64;
    let mut leave = 1.0f64;
    if dx == 0.0 {
        if px < x0 || px > x1 {
            return f64::INFINITY;
        }
    } else {
        let a = (x0 - px) / dx;
        let b = (x1 - px) / dx;
        enter = enter.max(a.min(b));
        leave = leave.min(a.max(b));
    }
    if dy == 0.0 {
        if py < y0 || py > y1 {
            return f64::INFINITY;
        }
    } else {
        let a = (y0 - py) / dy;
        let b = (y1 - py) / dy;
        enter = enter.max(a.min(b));
        leave = leave.min(a.max(b));
    }
    if enter <= leave { enter } else { f64::INFINITY }
}

/// How far along the segment it first meets the ellipse; infinity when it misses.
#[allow(clippy::too_many_arguments, reason = "a segment and an ellipse")]
fn hit_ellipse(cx: f64, cy: f64, rx: f64, ry: f64, px: f64, py: f64, dx: f64, dy: f64) -> f64 {
    let ux = dx / rx;
    let uy = dy / ry;
    let vx = (px - cx) / rx;
    let vy = (py - cy) / ry;
    let a = ux * ux + uy * uy;
    let b = 2.0 * (ux * vx + uy * vy);
    let c = vx * vx + vy * vy - 1.0;
    let disc = b * b - 4.0 * a * c;
    if a == 0.0 || disc < 0.0 {
        return f64::INFINITY;
    }
    let t = (-b - disc.sqrt()) / (2.0 * a);
    if (0.0..=1.0).contains(&t) {
        t
    } else if c <= 0.0 {
        0.0
    } else {
        f64::INFINITY
    }
}

/// Where a segment from outside first meets the outline of a shape, as a fraction of the
/// segment.
fn hit(box_: &Frame, px: f64, py: f64, qx: f64, qy: f64) -> f64 {
    let dx = qx - px;
    let dy = qy - py;
    match box_.form {
        Form::Person => hit_rect(
            box_.x,
            box_.y + HEAD,
            box_.x + box_.w,
            box_.y + box_.h,
            px,
            py,
            dx,
            dy,
        )
        .min(hit_ellipse(
            box_.x + box_.w / 2.0,
            box_.y + 14.0,
            13.0,
            13.0,
            px,
            py,
            dx,
            dy,
        )),
        Form::Cyl => {
            let ry = cyl_ry(box_.w);
            hit_rect(
                box_.x,
                box_.y + ry,
                box_.x + box_.w,
                box_.y + box_.h - ry,
                px,
                py,
                dx,
                dy,
            )
            .min(hit_ellipse(
                box_.x + box_.w / 2.0,
                box_.y + ry,
                box_.w / 2.0,
                ry,
                px,
                py,
                dx,
                dy,
            ))
            .min(hit_ellipse(
                box_.x + box_.w / 2.0,
                box_.y + box_.h - ry,
                box_.w / 2.0,
                ry,
                px,
                py,
                dx,
                dy,
            ))
        }
        Form::HCyl => {
            let rx = queue_rx(box_.h);
            hit_rect(
                box_.x + rx,
                box_.y,
                box_.x + box_.w - rx,
                box_.y + box_.h,
                px,
                py,
                dx,
                dy,
            )
            .min(hit_ellipse(
                box_.x + rx,
                box_.y + box_.h / 2.0,
                rx,
                box_.h / 2.0,
                px,
                py,
                dx,
                dy,
            ))
            .min(hit_ellipse(
                box_.x + box_.w - rx,
                box_.y + box_.h / 2.0,
                rx,
                box_.h / 2.0,
                px,
                py,
                dx,
                dy,
            ))
        }
        // A box with rounded corners and a box with a folded corner are met at their frame.
        Form::Rounded | Form::FrRect => hit_rect(
            box_.x,
            box_.y,
            box_.x + box_.w,
            box_.y + box_.h,
            px,
            py,
            dx,
            dy,
        ),
    }
}

/// Whether a route crosses a rectangle.
pub(crate) fn crosses(points: &[f64], x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
    let mut i = 2;
    while i < points.len() {
        let px = points[i - 2];
        let py = points[i - 1];
        if hit_rect(x0, y0, x1, y1, px, py, points[i] - px, points[i + 1] - py) <= 1.0 {
            return true;
        }
        i += 2;
    }
    false
}

fn contains(outer: &Frame, inner: &Frame) -> bool {
    outer.x <= inner.x
        && outer.y <= inner.y
        && outer.x + outer.w >= inner.x + inner.w
        && outer.y + outer.h >= inner.y + inner.h
}

/// The straight line from a to b: x, y of its start, then of its end.
fn connect(a: &Frame, b: &Frame, shift: f64) -> (f64, f64, f64, f64) {
    let ax = a.x + a.w / 2.0;
    let ay = a.y + a.h / 2.0;
    let bx = b.x + b.w / 2.0;
    let by = b.y + b.h / 2.0;
    let down = contains(a, b);
    if down || contains(b, a) {
        // One holds the other: a short line from the nearest side of the outer box.
        let outer = if down { a } else { b };
        let inner = if down { b } else { a };
        let ix = inner.x + inner.w / 2.0;
        let iy = inner.y + inner.h / 2.0;
        let left = inner.x - outer.x;
        let right = outer.x + outer.w - inner.x - inner.w;
        let top = inner.y - outer.y;
        let bottom = outer.y + outer.h - inner.y - inner.h;
        let least = left.min(right).min(top).min(bottom);
        let ox = if least == left {
            outer.x
        } else if least == right {
            outer.x + outer.w
        } else {
            ix
        };
        let oy = if least == left || least == right {
            iy
        } else if least == bottom {
            outer.y + outer.h
        } else {
            outer.y
        };
        let t = hit(inner, ox, oy, ix, iy).min(1.0);
        let mx = ox + (ix - ox) * t;
        let my = oy + (iy - oy) * t;
        return if down {
            (ox, oy, mx, my)
        } else {
            (mx, my, ox, oy)
        };
    }

    let mut px = ax;
    let mut py = ay;
    let mut qx = bx;
    let mut qy = by;
    // Shapes that share a column or a row are joined by an upright or level line through the
    // shared span.
    let x0 = a.x.max(b.x);
    let x1 = (a.x + a.w).min(b.x + b.w);
    let y0 = a.y.max(b.y);
    let y1 = (a.y + a.h).min(b.y + b.h);
    if x0 < x1 {
        px = (x0 + x1) / 2.0;
        qx = px;
    } else if y0 < y1 {
        py = (y0 + y1) / 2.0;
        qy = py;
    }
    let len = ((qx - px).powi(2) + (qy - py).powi(2)).sqrt();
    let len = if len == 0.0 { 1.0 } else { len };
    let nx = (-(qy - py) / len) * shift;
    let ny = ((qx - px) / len) * shift;
    let mut ta = hit(a, qx + nx, qy + ny, px + nx, py + ny);
    let mut tb = hit(b, px + nx, py + ny, qx + nx, qy + ny);
    if ta <= 1.0 && tb <= 1.0 {
        px += nx;
        py += ny;
        qx += nx;
        qy += ny;
    } else {
        px = ax;
        py = ay;
        qx = bx;
        qy = by;
        ta = hit(a, qx, qy, px, py).min(1.0);
        tb = hit(b, px, py, qx, qy).min(1.0);
    }
    (
        qx + (px - qx) * ta,
        qy + (py - qy) * ta,
        px + (qx - px) * tb,
        py + (qy - py) * tb,
    )
}

/// The route of a relation.
#[derive(Debug, Clone)]
pub(crate) struct Route {
    pub d: String,
    // Where the markers go: the tip, and the way it points.
    pub sx: f64,
    pub sy: f64,
    pub sdx: f64,
    pub sdy: f64,
    pub ex: f64,
    pub ey: f64,
    pub edx: f64,
    pub edy: f64,
    // The stretch the label may sit on.
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    // The side of that stretch the label is pushed to, as a unit vector; zero centers it on the
    // stretch.
    pub nx: f64,
    pub ny: f64,
    // When set, the label stays over the line and moves that way by at most this much.
    pub lean: f64,
    // The widest the label should be.
    pub room: f64,
    // The corners of the line, as x, y pairs.
    pub points: Vec<f64>,
}

/// What the label of a route does.
#[derive(Debug, Clone, Copy)]
struct Label {
    nx: f64,
    ny: f64,
    lean: f64,
    room: f64,
}

impl Default for Label {
    fn default() -> Self {
        Self {
            nx: 0.0,
            ny: 0.0,
            lean: 0.0,
            room: WIDE,
        }
    }
}

/// What the routes drawn so far take up, so that the next keeps apart from them: upright
/// stretches as x, top, bottom; level ones as y, left, right; and where lines leave or reach
/// each side of a shape.
#[derive(Debug, Default)]
pub(crate) struct Lanes {
    upright: Vec<f64>,
    level: Vec<f64>,
    frames: [Vec<f64>; 2],
    taken: HashMap<usize, Vec<f64>>,
    // Routes that found no lane of their own: a line left straight behind a shape, or a lane that
    // runs along another.
    pub squeezed: usize,
}

impl Lanes {
    pub(crate) fn free(&self, upright: bool, at: f64, a0: f64, a1: f64) -> bool {
        let lo = a0.min(a1);
        let hi = a0.max(a1);
        let near = |list: &[f64], gap: f64| -> bool {
            let mut i = 0;
            while i + 2 < list.len() {
                if (list[i] - at).abs() < gap && list[i + 1] < hi && list[i + 2] > lo {
                    return true;
                }
                i += 3;
            }
            false
        };
        let lines = if upright { &self.upright } else { &self.level };
        let frames = if upright {
            &self.frames[0]
        } else {
            &self.frames[1]
        };
        !near(lines, NEAR) && !near(frames, FRAME_NEAR)
    }

    fn crossings(&self, points: &[f64]) -> usize {
        let mut count = 0;
        let mut i = 2;
        while i < points.len() {
            let upright = points[i] == points[i - 2];
            let at = if upright { points[i] } else { points[i + 1] };
            let lo = if upright {
                points[i - 1].min(points[i + 1])
            } else {
                points[i - 2].min(points[i])
            };
            let hi = if upright {
                points[i - 1].max(points[i + 1])
            } else {
                points[i - 2].max(points[i])
            };
            let list = if upright { &self.level } else { &self.upright };
            let mut j = 0;
            while j + 2 < list.len() {
                if list[j] > lo && list[j] < hi && list[j + 1] < at && list[j + 2] > at {
                    count += 1;
                }
                j += 3;
            }
            i += 2;
        }
        count
    }

    /// The frame of a boundary, which the lines keep away from.
    pub(crate) fn frame(&mut self, box_: &Frame) {
        self.frames[0].extend_from_slice(&[
            box_.x,
            box_.y,
            box_.y + box_.h,
            box_.x + box_.w,
            box_.y,
            box_.y + box_.h,
        ]);
        self.frames[1].extend_from_slice(&[
            box_.y,
            box_.x,
            box_.x + box_.w,
            box_.y + box_.h,
            box_.x,
            box_.x + box_.w,
        ]);
    }

    fn take(&mut self, points: &[f64]) {
        let mut i = 2;
        while i < points.len() {
            if points[i] == points[i - 2] {
                self.upright.extend_from_slice(&[
                    points[i],
                    points[i - 1].min(points[i + 1]),
                    points[i - 1].max(points[i + 1]),
                ]);
            } else if points[i + 1] == points[i - 1] {
                self.level.extend_from_slice(&[
                    points[i + 1],
                    points[i - 2].min(points[i]),
                    points[i - 2].max(points[i]),
                ]);
            }
            i += 2;
        }
    }

    /// Where on one side of a shape (0 left, 1 right, 2 top, 3 bottom) a line can leave or
    /// arrive, between lo and hi: the places clear of the lines already there, nearest `want`
    /// first.
    fn ports(&self, box_: &Frame, side: usize, want: f64, lo: f64, hi: f64) -> Vec<f64> {
        let empty: Vec<f64> = Vec::new();
        let used = self.taken.get(&box_.id).unwrap_or(&empty);
        let mut out: Vec<f64> = Vec::new();
        let mut k = 0;
        while k < 9 && out.len() < 4 {
            let at =
                want + if k & 1 == 1 { 1.0 } else { -1.0 } * (k as f64 / 2.0).ceil() * PORT_STEP;
            if at < lo || at > hi {
                k += 1;
                continue;
            }
            let mut clear = true;
            let mut i = 0;
            while i + 1 < used.len() {
                if used[i] == side as f64 && (used[i + 1] - at).abs() < PORT_STEP {
                    clear = false;
                }
                i += 2;
            }
            if clear {
                out.push(at);
            }
            k += 1;
        }
        if out.is_empty() { vec![want] } else { out }
    }

    fn claim(&mut self, box_: &Frame, side: usize, at: f64) {
        self.taken
            .entry(box_.id)
            .or_default()
            .extend_from_slice(&[side as f64, at]);
    }
}

/// The path of a line through its corners.
fn path(points: &[f64], start_trim: f64, end_trim: f64) -> String {
    let n = points.len();
    let ux = sign(points[2] - points[0]);
    let uy = sign(points[3] - points[1]);
    let mut d = format!(
        "M{},{}",
        num(points[0] + ux * start_trim),
        num(points[1] + uy * start_trim)
    );
    let mut i = 2;
    while i < n - 2 {
        let x = points[i];
        let y = points[i + 1];
        let ix = sign(x - points[i - 2]);
        let iy = sign(y - points[i - 1]);
        let ox = sign(points[i + 2] - x);
        let oy = sign(points[i + 3] - y);
        let r = CORNER
            .min((x - points[i - 2] + y - points[i - 1]).abs() / 2.0)
            .min((points[i + 2] - x + points[i + 3] - y).abs() / 2.0);
        if ix != 0.0 {
            d += &format!("H{}", num(x - ix * r));
        } else {
            d += &format!("V{}", num(y - iy * r));
        }
        d += &format!(
            "Q{},{} {},{}",
            num(x),
            num(y),
            num(x + ox * r),
            num(y + oy * r)
        );
        i += 2;
    }
    let lx = sign(points[n - 2] - points[n - 4]);
    let ly = sign(points[n - 1] - points[n - 3]);
    if lx != 0.0 {
        d += &format!("H{}", num(points[n - 2] - lx * end_trim));
    } else {
        d += &format!("V{}", num(points[n - 1] - ly * end_trim));
    }
    d
}

fn clear_of(points: &[f64], others: &[Frame], a: &Frame, b: &Frame) -> bool {
    for other in others {
        if other.id != a.id
            && other.id != b.id
            && crosses(
                points,
                other.x,
                other.y,
                other.x + other.w,
                other.y + other.h,
            )
        {
            return false;
        }
    }
    true
}

/// The route for a line with corners through `points`, from a's outline to b's. `along` is the
/// stretch its label sits on.
fn bent(points: &[f64], along: usize, start_trim: f64, end_trim: f64, label: Label) -> Route {
    let n = points.len();
    Route {
        d: path(points, start_trim, end_trim),
        sx: points[0],
        sy: points[1],
        sdx: -sign(points[2] - points[0]),
        sdy: -sign(points[3] - points[1]),
        ex: points[n - 2],
        ey: points[n - 1],
        edx: sign(points[n - 2] - points[n - 4]),
        edy: sign(points[n - 1] - points[n - 3]),
        x0: points[along],
        y0: points[along + 1],
        x1: points[along + 2],
        y1: points[along + 3],
        nx: label.nx,
        ny: label.ny,
        lean: label.lean,
        room: label.room,
        points: points.to_vec(),
    }
}

/// The lanes a line may cross between two places.
fn across(lo: f64, hi: f64) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    let mid = (lo + hi) / 2.0;
    let step = LANE_STEP.max((hi - lo) / 4.0);
    for k in 0..9 {
        let at = mid + if k & 1 == 1 { 1.0 } else { -1.0 } * (k as f64 / 2.0).ceil() * step;
        if at >= lo + NEAR && at <= hi - NEAR {
            out.push(at);
        }
    }
    out
}

/// The best route found so far.
struct Best {
    points: Vec<f64>,
    cost: f64,
    upright: bool,
    sides: (usize, usize),
}

/// Whether the route through `points` is better than the one found so far.
#[allow(
    clippy::too_many_arguments,
    reason = "the route and the shapes it keeps clear of"
)]
fn consider(
    points: &[f64],
    upright: bool,
    sides: (usize, usize),
    others: &[Frame],
    a: &Frame,
    b: &Frame,
    lanes: &Lanes,
    best: &mut Option<Best>,
) {
    if !clear_of(points, others, a, b) {
        return;
    }
    let mut i = 2;
    while i < points.len() {
        let up = points[i] == points[i - 2];
        let at = if up { points[i] } else { points[i + 1] };
        let from = if up { points[i - 1] } else { points[i - 2] };
        let to = if up { points[i + 1] } else { points[i] };
        if !lanes.free(up, at, from, to) {
            return;
        }
        i += 2;
    }
    let mut cost = lanes.crossings(points) as f64 * 1e6;
    i = 2;
    while i < points.len() {
        cost += (points[i] - points[i - 2]).abs() + (points[i + 1] - points[i - 1]).abs();
        i += 2;
    }
    let better = match best {
        None => true,
        Some(current) => cost < current.cost,
    };
    if better {
        *best = Some(Best {
            points: points.to_vec(),
            cost,
            upright,
            sides,
        });
    }
}

/// A route that goes around the shapes between the two ends.
fn elbow(
    a: &Frame,
    b: &Frame,
    others: &[Frame],
    lanes: &mut Lanes,
    start_trim: f64,
    end_trim: f64,
) -> Option<Route> {
    let ax = a.x + a.w / 2.0;
    let ay = a.y + a.h / 2.0;
    let bx = b.x + b.w / 2.0;
    let by = b.y + b.h / 2.0;
    let mut best: Option<Best> = None;

    if a.x + a.w <= b.x || b.x + b.w <= a.x {
        let right = bx > ax;
        let down = sign(by - ay);
        let sides = if right { (1, 0) } else { (0, 1) };
        let lanes_across = across(
            if right { a.x + a.w } else { b.x + b.w },
            if right { b.x } else { a.x },
        );
        'across_x: for y0 in lanes.ports(
            a,
            sides.0,
            ay + (down * a.h) / 4.0,
            a.y + 8.0,
            a.y + a.h - 8.0,
        ) {
            for y1 in lanes.ports(
                b,
                sides.1,
                by - (down * b.h) / 4.0,
                b.y + 8.0,
                b.y + b.h - 8.0,
            ) {
                for lane in &lanes_across {
                    let x0 = lane + (ax - lane) * hit(a, *lane, y0, ax, y0).min(1.0);
                    let x1 = lane + (bx - lane) * hit(b, *lane, y1, bx, y1).min(1.0);
                    let before = best.as_ref().map(|current| current.cost);
                    consider(
                        &[x0, y0, *lane, y0, *lane, y1, x1, y1],
                        true,
                        sides,
                        others,
                        a,
                        b,
                        lanes,
                        &mut best,
                    );
                    if let Some(before) = before {
                        if best.as_ref().is_some_and(|current| current.cost < before) {
                            break 'across_x;
                        }
                    } else if best.is_some() {
                        break 'across_x;
                    }
                }
            }
        }
    }

    if a.y + a.h <= b.y || b.y + b.h <= a.y {
        let below = by > ay;
        let sideways = sign(bx - ax);
        let sides = if below { (3, 2) } else { (2, 3) };
        let lanes_across = across(
            if below { a.y + a.h } else { b.y + b.h },
            if below { b.y } else { a.y },
        );
        'across_y: for x0 in lanes.ports(
            a,
            sides.0,
            ax + (sideways * a.w) / 4.0,
            a.x + 8.0,
            a.x + a.w - 8.0,
        ) {
            for x1 in lanes.ports(
                b,
                sides.1,
                bx - (sideways * b.w) / 4.0,
                b.x + 8.0,
                b.x + b.w - 8.0,
            ) {
                for lane in &lanes_across {
                    let y0 = lane + (ay - lane) * hit(a, x0, *lane, x0, ay).min(1.0);
                    let y1 = lane + (by - lane) * hit(b, x1, *lane, x1, by).min(1.0);
                    let before = best.as_ref().map(|current| current.cost);
                    consider(
                        &[x0, y0, x0, *lane, x1, *lane, x1, y1],
                        false,
                        sides,
                        others,
                        a,
                        b,
                        lanes,
                        &mut best,
                    );
                    if let Some(before) = before {
                        if best.as_ref().is_some_and(|current| current.cost < before) {
                            break 'across_y;
                        }
                    } else if best.is_some() {
                        break 'across_y;
                    }
                }
            }
        }
    }

    let best = best?;
    let points = best.points;
    lanes.take(&points);
    lanes.claim(
        a,
        best.sides.0,
        if best.upright { points[1] } else { points[0] },
    );
    lanes.claim(
        b,
        best.sides.1,
        if best.upright { points[7] } else { points[6] },
    );
    // The label sits on the middle stretch, across the gap.
    let label = Label {
        room: if best.upright { NARROW } else { WIDE },
        ..Label::default()
    };
    Some(bent(&points, 2, start_trim, end_trim, label))
}

/// The route of a relation from a to b. `shift` moves the line sideways when another runs the
/// opposite way. `others` are the shapes to steer clear of; without them that check is skipped.
/// `lanes` holds what the routes before it take up, so that this one keeps apart from them.
pub(crate) fn route(
    a: &Frame,
    b: &Frame,
    shift: f64,
    others: Option<&[Frame]>,
    lanes: &mut Lanes,
    start_trim: f64,
    end_trim: f64,
) -> Route {
    if a.id == b.id {
        let spread = (a.h / 2.0 - 4.0).min(10.0);
        let len = (LOOP * LOOP + spread * spread).sqrt();
        let x = a.x + a.w;
        let cy = a.y + a.h / 2.0;
        let ux = LOOP / len;
        let uy = spread / len;
        return Route {
            d: format!(
                "M{},{}C{},{} {},{} {},{}",
                num(x + ux * start_trim),
                num(cy - spread - uy * start_trim),
                num(x + LOOP),
                num(cy - spread * 2.0),
                num(x + LOOP),
                num(cy + spread * 2.0),
                num(x + ux * end_trim),
                num(cy + spread + uy * end_trim)
            ),
            sx: x,
            sy: cy - spread,
            sdx: -ux,
            sdy: uy,
            ex: x,
            ey: cy + spread,
            edx: -ux,
            edy: -uy,
            x0: x + LOOP + 4.0,
            y0: cy,
            x1: x + LOOP + 4.0,
            y1: cy,
            nx: 1.0,
            ny: 0.0,
            lean: 0.0,
            room: 64.0,
            points: Vec::new(),
        };
    }

    let (sx, sy, ex, ey) = connect(a, b, shift);
    let upright = sx == ex;
    let mut blocked = false;
    if let Some(others) = others {
        if !contains(a, b) && !contains(b, a) {
            for other in others {
                if other.id != a.id
                    && other.id != b.id
                    && hit_rect(
                        other.x,
                        other.y,
                        other.x + other.w,
                        other.y + other.h,
                        sx,
                        sy,
                        ex - sx,
                        ey - sy,
                    ) <= 1.0
                {
                    blocked = true;
                }
            }
        }
    }
    if let Some(others) = others {
        if blocked && !upright && sy != ey {
            if let Some(around) = elbow(a, b, others, lanes, start_trim, end_trim) {
                return around;
            }
            lanes.squeezed += 1;
        }
        if blocked && (upright || sy == ey) {
            // The lane on one side of the shapes the line runs through.
            let lane_on = |forward: bool, others: &[Frame]| -> f64 {
                let mut lane = if forward {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                };
                let widen = |box_: &Frame, lane: &mut f64| {
                    let edge = if upright { box_.x } else { box_.y };
                    *lane = if forward {
                        lane.max(edge + if upright { box_.w } else { box_.h })
                    } else {
                        lane.min(edge)
                    };
                };
                widen(a, &mut lane);
                widen(b, &mut lane);
                for other in others {
                    if other.id != a.id
                        && other.id != b.id
                        && hit_rect(
                            other.x,
                            other.y,
                            other.x + other.w,
                            other.y + other.h,
                            sx,
                            sy,
                            ex - sx,
                            ey - sy,
                        ) <= 1.0
                    {
                        widen(other, &mut lane);
                    }
                }
                lane
            };
            let lo = if upright { a.y.min(b.y) } else { a.x.min(b.x) };
            let hi = if upright {
                (a.y + a.h).max(b.y + b.h)
            } else {
                (a.x + a.w).max(b.x + b.w)
            };
            let outside = |forward: bool, lane: f64, others: &[Frame]| -> bool {
                for other in others {
                    if other.id == a.id || other.id == b.id {
                        continue;
                    }
                    let near = if upright {
                        other.y < hi && other.y + other.h > lo
                    } else {
                        other.x < hi && other.x + other.w > lo
                    };
                    let start = if upright { other.x } else { other.y };
                    let end = start + if upright { other.w } else { other.h };
                    if near && (if forward { end > lane } else { start < lane }) {
                        return false;
                    }
                }
                true
            };
            let ax = a.x + a.w / 2.0;
            let ay = a.y + a.h / 2.0;
            let bx = b.x + b.w / 2.0;
            let by = b.y + b.h / 2.0;
            let towards = if upright {
                sign(by - ay)
            } else {
                sign(bx - ax)
            };
            // The route on one side: each end leaves its shape a quarter of the way from the
            // middle towards the other, clear of the straight lines that meet the middle of that
            // side and of other routes there. The lane runs a little further out for each route
            // already along that gap, and never behind a shape. `step` is how many lanes out it
            // went; 5 when none was free.
            let along = |forward: bool, lanes: &Lanes| -> (Vec<f64>, usize, f64, f64, usize) {
                let lane = lane_on(forward, others);
                let sign = if forward { 1.0 } else { -1.0 };
                let side = if upright {
                    if forward { 1 } else { 0 }
                } else if forward {
                    3
                } else {
                    2
                };
                let p0 = if upright {
                    lanes.ports(
                        a,
                        side,
                        ay + (towards * a.h) / 4.0,
                        a.y + 8.0,
                        a.y + a.h - 8.0,
                    )[0]
                } else {
                    lanes.ports(
                        a,
                        side,
                        ax + (towards * a.w) / 4.0,
                        a.x + 8.0,
                        a.x + a.w - 8.0,
                    )[0]
                };
                let p1 = if upright {
                    lanes.ports(
                        b,
                        side,
                        by - (towards * b.h) / 4.0,
                        b.y + 8.0,
                        b.y + b.h - 8.0,
                    )[0]
                } else {
                    lanes.ports(
                        b,
                        side,
                        bx - (towards * b.w) / 4.0,
                        b.x + 8.0,
                        b.x + b.w - 8.0,
                    )[0]
                };
                let points_at = |at: f64| -> Vec<f64> {
                    if upright {
                        let x0 = at + (ax - at) * hit(a, at, p0, ax, p0).min(1.0);
                        let x1 = at + (bx - at) * hit(b, at, p1, bx, p1).min(1.0);
                        vec![x0, p0, at, p0, at, p1, x1, p1]
                    } else {
                        let y0 = at + (ay - at) * hit(a, p0, at, p0, ay).min(1.0);
                        let y1 = at + (by - at) * hit(b, p1, at, p1, by).min(1.0);
                        vec![p0, y0, p0, at, p1, at, p1, y1]
                    }
                };
                let mut points = points_at(lane + sign * LANE);
                for k in 0..=4 {
                    let at = lane + sign * (LANE + k as f64 * LANE_STEP);
                    let tried = points_at(at);
                    if !clear_of(&tried, others, a, b) {
                        continue;
                    }
                    points = tried;
                    if lanes.free(upright, at, p0, p1) {
                        return (points, side, p0, p1, k);
                    }
                }
                (points, side, p0, p1, 5)
            };
            let natural = if upright { ey > sy } else { ex < sx };
            let out = outside(natural, lane_on(natural, others), others);
            let back = outside(!natural, lane_on(!natural, others), others);
            let first = along(natural, lanes);
            let other = if out == back {
                Some(along(!natural, lanes))
            } else {
                None
            };
            let cost = |way: &(Vec<f64>, usize, f64, f64, usize)| -> f64 {
                lanes.crossings(&way.0) as f64 * 8.0 + way.4 as f64
            };
            let forward = if back && !out {
                !natural
            } else if let Some(other) = &other {
                if cost(other) < cost(&first) {
                    !natural
                } else {
                    natural
                }
            } else {
                natural
            };
            let (points, side, p0, p1, step) = if forward == natural {
                first
            } else {
                other.unwrap_or_else(|| along(!natural, lanes))
            };
            let sign = if forward { 1.0 } else { -1.0 };
            if step > 4 {
                lanes.squeezed += 1;
            }
            lanes.take(&points);
            lanes.claim(a, side, p0);
            lanes.claim(b, side, p1);
            let label = if upright {
                Label {
                    nx: sign,
                    ny: 0.0,
                    lean: LEAN,
                    room: NARROW,
                }
            } else {
                Label {
                    nx: 0.0,
                    ny: 0.0,
                    lean: 0.0,
                    room: WIDE,
                }
            };
            return bent(&points, 2, start_trim, end_trim, label);
        }
    }

    let len = ((ex - sx).powi(2) + (ey - sy).powi(2)).sqrt();
    let len = if len == 0.0 { 1.0 } else { len };
    let dx = (ex - sx) / len;
    let dy = (ey - sy) / len;
    if upright || sy == ey {
        lanes.take(&[sx, sy, ex, ey]);
    }
    Route {
        d: format!(
            "M{},{}L{},{}",
            num(sx + dx * start_trim),
            num(sy + dy * start_trim),
            num(ex - dx * end_trim),
            num(ey - dy * end_trim)
        ),
        sx,
        sy,
        sdx: -dx,
        sdy: -dy,
        ex,
        ey,
        edx: dx,
        edy: dy,
        x0: sx,
        y0: sy,
        x1: ex,
        y1: ey,
        nx: if shift != 0.0 { -dy } else { 0.0 },
        ny: if shift != 0.0 { dx } else { 0.0 },
        lean: 0.0,
        room: if dx.abs() > dy.abs() { NARROW } else { WIDE },
        points: vec![sx, sy, ex, ey],
    }
}
