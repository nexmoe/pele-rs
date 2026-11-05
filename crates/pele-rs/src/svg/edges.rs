//! Edge paths, markers and labels. Ported from pele's `svg/edges.ts`.

use std::collections::HashMap;

pub(crate) use super::markers::{ARROW, marker, marker_trim};
use super::{esc_text, label_svg, num};
use crate::layout::layered::ease;
use crate::text::label::Label;

/// The most a corner is rounded by in a stepped route.
const CORNER: f64 = 12.0;
/// How far the handles of a wide turn reach along its two legs.
const HANDLE: f64 = 0.62;
/// How far an edge runs straight before a marker at its end, where it has the room.
pub(crate) const LEAD: f64 = 6.0;

#[derive(Debug, Clone, Default)]
pub(crate) struct EdgePath {
    pub d: String,
    pub sx: f64,
    pub sy: f64,
    pub sdx: f64,
    pub sdy: f64,
    pub ex: f64,
    pub ey: f64,
    pub edx: f64,
    pub edy: f64,
}

/// How far from each side of each node the edges that meet it start to turn: as far as the
/// largest marker there needs, for every edge on the side.
#[derive(Default)]
pub(crate) struct SideReach(HashMap<(usize, u8), f64>);

impl SideReach {
    pub(crate) fn add(&mut self, node: usize, side: Option<u8>, kind: &str) {
        let trim = marker_trim(kind);
        let Some(side) = side else { return };
        let entry = self.0.entry((node, side)).or_insert(0.0);
        if trim > 0.0 && trim + LEAD > *entry {
            *entry = trim + LEAD;
        }
    }

    pub(crate) fn of(&self, node: usize, side: Option<u8>) -> f64 {
        side.and_then(|s| self.0.get(&(node, s)).copied())
            .unwrap_or(0.0)
    }
}

fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn or_one(x: f64) -> f64 {
    if x == 0.0 || x.is_nan() { 1.0 } else { x }
}

/// Builds the path for a route of x, y, axis triples, trimming both ends to leave room for
/// markers. Between two points not in line a route is one S along the flow, or a step in a
/// stepped route; where the route turns a corner, it turns as widely as it can.
pub(crate) fn route_path(
    route: &[f64],
    curve: Option<&str>,
    start_trim: f64,
    end_trim: f64,
    start_reach: Option<f64>,
    end_reach: Option<f64>,
) -> EdgePath {
    let start_reach = start_reach.unwrap_or(if start_trim > 0.0 {
        start_trim + LEAD
    } else {
        0.0
    });
    let end_reach = end_reach.unwrap_or(if end_trim > 0.0 { end_trim + LEAD } else { 0.0 });
    let linear = curve == Some("linear");
    let stepped = matches!(curve, Some("step" | "stepBefore" | "stepAfter"));
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    let mut axes: Vec<f64> = Vec::new();
    let near =
        |ax: f64, ay: f64, bx: f64, by: f64| (ax - bx).abs() < 0.01 && (ay - by).abs() < 0.01;
    for p in route.chunks(3) {
        let (x, y, axis) = (p[0], p[1], p[2]);
        if stepped {
            if let (Some(&px), Some(&py)) = (xs.last(), ys.last()) {
                if (x - px).abs() >= 0.01 && (y - py).abs() >= 0.01 {
                    let my = match curve {
                        Some("stepBefore") => py,
                        Some("stepAfter") => y,
                        _ => (py + y) / 2.0,
                    };
                    let mx = match curve {
                        Some("stepBefore") => px,
                        Some("stepAfter") => x,
                        _ => (px + x) / 2.0,
                    };
                    let turns = if axis == 0.0 {
                        [px, my, x, my]
                    } else {
                        [mx, py, mx, y]
                    };
                    for k in [0, 2] {
                        if near(turns[k], turns[k + 1], px, py)
                            || near(turns[k], turns[k + 1], x, y)
                        {
                            continue;
                        }
                        xs.push(turns[k]);
                        ys.push(turns[k + 1]);
                        axes.push(axis);
                    }
                }
            }
        }
        xs.push(x);
        ys.push(y);
        axes.push(axis);
    }
    let mut last = xs.len() - 1;
    let aligned = |xs: &[f64], ys: &[f64], i: usize| {
        (xs[i] - xs[i - 1]).abs() < 0.01 || (ys[i] - ys[i - 1]).abs() < 0.01
    };
    let straight = |xs: &[f64], ys: &[f64], i: usize| linear || aligned(xs, ys, i);
    let unit = |xs: &[f64], ys: &[f64], i: usize, j: usize, axis: f64| -> (f64, f64) {
        if !straight(xs, ys, i.max(j)) {
            return if axis == 0.0 {
                (0.0, or_one(sign(ys[j] - ys[i])))
            } else {
                (or_one(sign(xs[j] - xs[i])), 0.0)
            };
        }
        let dx = xs[j] - xs[i];
        let dy = ys[j] - ys[i];
        let len = or_one(dx.hypot(dy));
        (dx / len, dy / len)
    };
    let (sdx, sdy) = unit(&xs, &ys, 0, 1, axes[1]);
    let (edx, edy) = unit(&xs, &ys, last - 1, last, axes[last]);
    let (sx, sy, ex, ey) = (xs[0], ys[0], xs[last], ys[last]);
    xs[0] += sdx * start_trim;
    ys[0] += sdy * start_trim;
    xs[last] -= edx * end_trim;
    ys[last] -= edy * end_trim;
    let length =
        |xs: &[f64], ys: &[f64], i: usize| (xs[i] - xs[i - 1]).abs() + (ys[i] - ys[i - 1]).abs();
    let turns = |xs: &[f64], ys: &[f64], last: usize, i: usize| {
        !linear
            && i > 0
            && i < last
            && aligned(xs, ys, i)
            && aligned(xs, ys, i + 1)
            && length(xs, ys, i) >= 0.01
            && length(xs, ys, i + 1) >= 0.01
            && ((xs[i] - xs[i - 1]).abs() < 0.01) != ((xs[i + 1] - xs[i]).abs() < 0.01)
    };
    let along = |xs: &[f64], ys: &[f64], i: usize, ux: f64, uy: f64| {
        (xs[i] - xs[i - 1]).abs() * ux.abs() + (ys[i] - ys[i - 1]).abs() * uy.abs()
    };
    let lead = |reach: f64, trim: f64, run: f64| {
        let room = 0f64.max(reach - LEAD - trim).min(run / 4.0);
        room + (reach - trim - room).min((run - room) / 4.0)
    };
    let arrives = if end_reach > end_trim
        && (!straight(&xs, &ys, last) || (last >= 1 && turns(&xs, &ys, last, last - 1)))
    {
        lead(end_reach, end_trim, along(&xs, &ys, last, edx, edy))
    } else {
        0.0
    };
    let leaves = if start_reach > start_trim && (!straight(&xs, &ys, 1) || turns(&xs, &ys, last, 1))
    {
        lead(start_reach, start_trim, along(&xs, &ys, 1, sdx, sdy))
    } else {
        0.0
    };
    if arrives >= 1.0 {
        xs.insert(last, xs[last] - edx * arrives);
        ys.insert(last, ys[last] - edy * arrives);
        axes.insert(last, axes[last]);
        last += 1;
    }
    if leaves >= 1.0 {
        xs.insert(1, xs[0] + sdx * leaves);
        ys.insert(1, ys[0] + sdy * leaves);
        axes.insert(1, axes[1]);
        last += 1;
    }

    let mut d = format!("M{},{}", num(xs[0]), num(ys[0]));
    let (mut at_x, mut at_y) = (xs[0], ys[0]);
    let line_to = |d: &mut String, at_x: &mut f64, at_y: &mut f64, x: f64, y: f64| {
        if near(x, y, *at_x, *at_y) {
            return;
        }
        d.push_str(&format!("L{},{}", num(x), num(y)));
        *at_x = x;
        *at_y = y;
    };
    for i in 1..=last {
        let (x, y) = (xs[i], ys[i]);
        if !straight(&xs, &ys, i) {
            let down = axes[i] == 0.0;
            let along = if down { y - at_y } else { x - at_x };
            let k = along.abs() * ease(if down { x - at_x } else { y - at_y }, along);
            let ux = if down { 0.0 } else { sign(along) * k };
            let uy = if down { sign(along) * k } else { 0.0 };
            d.push_str(&format!(
                "C{},{} {},{} {},{}",
                num(at_x + ux),
                num(at_y + uy),
                num(x - ux),
                num(y - uy),
                num(x),
                num(y)
            ));
            at_x = x;
            at_y = y;
            continue;
        }
        if !turns(&xs, &ys, last, i) {
            // A point part of the way along one straight run adds nothing to the path.
            if i < last
                && aligned(&xs, &ys, i)
                && aligned(&xs, &ys, i + 1)
                && (x - xs[i - 1]) * (xs[i + 1] - x) + (y - ys[i - 1]) * (ys[i + 1] - y) > 0.0
            {
                continue;
            }
            line_to(&mut d, &mut at_x, &mut at_y, x, y);
            continue;
        }
        let mut before = if turns(&xs, &ys, last, i - 1) {
            length(&xs, &ys, i) / 2.0
        } else {
            length(&xs, &ys, i)
        };
        let mut after = if turns(&xs, &ys, last, i + 1) {
            length(&xs, &ys, i + 1) / 2.0
        } else {
            length(&xs, &ys, i + 1)
        };
        if stepped {
            before = CORNER.min(before).min(after);
            after = before;
        }
        let (ux, uy) = unit(&xs, &ys, i - 1, i, axes[i]);
        let (vx, vy) = unit(&xs, &ys, i, i + 1, axes[i + 1]);
        line_to(
            &mut d,
            &mut at_x,
            &mut at_y,
            x - ux * before,
            y - uy * before,
        );
        at_x = x + vx * after;
        at_y = y + vy * after;
        if stepped {
            d.push_str(&format!(
                "Q{},{} {},{}",
                num(x),
                num(y),
                num(at_x),
                num(at_y)
            ));
        } else {
            d.push_str(&format!(
                "C{},{} {},{} {},{}",
                num(x - ux * before * (1.0 - HANDLE)),
                num(y - uy * before * (1.0 - HANDLE)),
                num(x + vx * after * (1.0 - HANDLE)),
                num(y + vy * after * (1.0 - HANDLE)),
                num(at_x),
                num(at_y)
            ));
        }
    }
    EdgePath {
        d,
        sx,
        sy,
        sdx: -sdx,
        sdy: -sdy,
        ex,
        ey,
        edx,
        edy,
    }
}

/// An edge label on a patch of the background, so the line does not run through the text.
pub(crate) fn edge_label_svg(id: &str, label: &Label, x: f64, y: f64, attrs: &str) -> String {
    let w = label.width + 8.0;
    let h = label.height;
    format!(
        "<g class=\"pele-rs-edge-label\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" fill=\"var(--_bg)\"/>{}</g>",
        esc_text(id),
        num(x - w / 2.0),
        num(y - h / 2.0),
        num(w),
        num(h),
        label_svg(label, x, y, attrs)
    )
}

/// A self-loop beside a node of drawn size `w` by `h`. `k` counts the loops already on the
/// node, so each one reaches a little further. Returns the path and where its label goes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn loop_path(
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
    k: usize,
    sideways: bool,
    base: f64,
    trim: f64,
    label: (f64, f64),
) -> (EdgePath, f64, f64) {
    let k = k as f64;
    let half = (if sideways { w } else { h }) / 2.0;
    let spread = (half - 4.0).min(8.0 + k * 6.0);
    let reach = base + k * 8.0;
    let len = or_one(reach.hypot(spread));
    let at = |out: f64, along: f64| {
        if sideways {
            format!("{},{}", num(cx + along), num(cy + h / 2.0 + out))
        } else {
            format!("{},{}", num(cx + w / 2.0 + out), num(cy + along))
        }
    };
    let tx = (reach / len) * trim;
    let ty = (spread / len) * trim;
    let (lx, ly) = if sideways {
        (cx, cy + h / 2.0 + reach + 4.0 + label.1 / 2.0)
    } else {
        (cx + w / 2.0 + reach + 4.0 + label.0 / 2.0, cy)
    };
    let path = EdgePath {
        d: format!(
            "M{}C{} {} {}",
            at(0.0, -spread),
            at(reach, -spread * 2.0),
            at(reach, spread * 2.0),
            at(tx, spread + ty)
        ),
        sx: if sideways { cx - spread } else { cx + w / 2.0 },
        sy: if sideways { cy + h / 2.0 } else { cy - spread },
        sdx: if sideways { 0.0 } else { -1.0 },
        sdy: if sideways { -1.0 } else { 0.0 },
        ex: if sideways { cx + spread } else { cx + w / 2.0 },
        ey: if sideways { cy + h / 2.0 } else { cy + spread },
        edx: if sideways {
            -spread / len
        } else {
            -reach / len
        },
        edy: if sideways {
            -reach / len
        } else {
            -spread / len
        },
    };
    (path, lx, ly)
}
