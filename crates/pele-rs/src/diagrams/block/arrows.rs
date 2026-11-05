//! The arrows of a block diagram: the blocks that point in one or two directions.
//! Ported from pele's `diagrams/block/arrows.ts`.

use crate::svg::num;

pub(crate) const RIGHT: u32 = 1;
pub(crate) const LEFT: u32 = 2;
pub(crate) const UP: u32 = 4;
pub(crate) const DOWN: u32 = 8;

/// How far a head reaches past each side of its shaft.
pub(crate) const OVERHANG: f64 = 7.0;
const MAX_DEPTH: f64 = 20.0;
const NUB: f64 = 12.0;

/// The size an arrow takes, and where its label sits.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ArrowSize {
    pub w: f64,
    pub h: f64,
    /// The shift of the label from the center, away from the heads.
    pub dx: f64,
    pub dy: f64,
}

/// The directions an arrow points in, as a mask of the sides.
pub(crate) fn arrow_mask(directions: &[String]) -> u32 {
    let mut mask = 0;
    for direction in directions {
        match direction.trim() {
            "right" => mask |= RIGHT,
            "left" => mask |= LEFT,
            "up" => mask |= UP,
            "down" => mask |= DOWN,
            "x" => mask |= RIGHT | LEFT,
            "y" => mask |= UP | DOWN,
            _ => {}
        }
    }
    mask
}

/// Whether the bit is set in the mask.
fn bit(mask: u32, flag: u32) -> f64 {
    if mask & flag != 0 { 1.0 } else { 0.0 }
}

/// The axis an arrow runs along: `x`, `y`, or neither when it points along both.
pub(crate) fn arrow_axis(mask: u32) -> &'static str {
    if mask != 0 && mask & (UP | DOWN) == 0 {
        return "x";
    }
    if mask != 0 && mask & (RIGHT | LEFT) == 0 {
        return "y";
    }
    ""
}

/// The size an arrow takes.
pub(crate) fn arrow_size(mask: u32, tw: f64, th: f64) -> ArrowSize {
    let axis = arrow_axis(mask);
    let across = bit(mask, LEFT) - bit(mask, RIGHT);
    let along = bit(mask, UP) - bit(mask, DOWN);
    if axis == "x" {
        let h = (th + 8.0).max(14.0) + 2.0 * OVERHANG;
        let depth = (h / 2.0).min(MAX_DEPTH);
        return ArrowSize {
            w: (tw + 12.0).max(16.0) + depth * if mask == (RIGHT | LEFT) { 2.0 } else { 1.0 },
            h,
            dx: (across * depth) / 2.0,
            dy: 0.0,
        };
    }
    if axis == "y" {
        let w = (tw + 16.0).max(14.0) + 2.0 * OVERHANG;
        let depth = (w / 2.0).min(MAX_DEPTH);
        return ArrowSize {
            w,
            h: (th + 8.0).max(16.0) + depth * if mask == (UP | DOWN) { 2.0 } else { 1.0 },
            dx: 0.0,
            dy: (along * depth) / 2.0,
        };
    }
    ArrowSize {
        w: (tw + 16.0).max(24.0) + NUB * (bit(mask, LEFT) + bit(mask, RIGHT)),
        h: (th + 8.0).max(24.0) + NUB * (bit(mask, UP) + bit(mask, DOWN)),
        dx: (across * NUB) / 2.0,
        dy: (along * NUB) / 2.0,
    }
}

/// The shape of an arrow.
pub(crate) fn draw_arrow(mask: u32, w: f64, h: f64, attrs: &str) -> String {
    let axis = arrow_axis(mask);
    let x = w / 2.0;
    let y = h / 2.0;
    let mut p: Vec<f64> = Vec::new();
    if axis == "x" {
        let depth = y.min(MAX_DEPTH);
        let s = y - OVERHANG;
        let x0 = if mask & LEFT != 0 { -x + depth } else { -x };
        let x1 = if mask & RIGHT != 0 { x - depth } else { x };
        p.extend([x0, -s, x1, -s]);
        if mask & RIGHT != 0 {
            p.extend([x1, -y, x, 0.0, x1, y]);
        }
        p.extend([x1, s, x0, s]);
        if mask & LEFT != 0 {
            p.extend([x0, y, -x, 0.0, x0, -y]);
        }
    } else if axis == "y" {
        let depth = x.min(MAX_DEPTH);
        let s = x - OVERHANG;
        let y0 = if mask & UP != 0 { -y + depth } else { -y };
        let y1 = if mask & DOWN != 0 { y - depth } else { y };
        p.extend([s, y0, s, y1]);
        if mask & DOWN != 0 {
            p.extend([x, y1, 0.0, y, -x, y1]);
        }
        p.extend([-s, y1, -s, y0]);
        if mask & UP != 0 {
            p.extend([-x, y0, 0.0, -y, x, y0]);
        }
    } else {
        let x0 = if mask & LEFT != 0 { -x + NUB } else { -x };
        let x1 = if mask & RIGHT != 0 { x - NUB } else { x };
        let y0 = if mask & UP != 0 { -y + NUB } else { -y };
        let y1 = if mask & DOWN != 0 { y - NUB } else { y };
        let cx = (x0 + x1) / 2.0;
        let cy = (y0 + y1) / 2.0;
        let kx = 10.0f64.min((x1 - x0) / 2.0);
        let ky = 10.0f64.min((y1 - y0) / 2.0);
        p.extend([x0, y0]);
        if mask & UP != 0 {
            p.extend([cx - kx, y0, cx, -y, cx + kx, y0]);
        }
        p.extend([x1, y0]);
        if mask & RIGHT != 0 {
            p.extend([x1, cy - ky, x, cy, x1, cy + ky]);
        }
        p.extend([x1, y1]);
        if mask & DOWN != 0 {
            p.extend([cx + kx, y1, cx, y, cx - kx, y1]);
        }
        p.extend([x0, y1]);
        if mask & LEFT != 0 {
            p.extend([x0, cy + ky, -x, cy, x0, cy - ky]);
        }
    }
    let mut points = String::new();
    for i in (0..p.len()).step_by(2) {
        if i > 0 {
            points.push(' ');
        }
        points.push_str(&num(p[i]));
        points.push(',');
        points.push_str(&num(p[i + 1]));
    }
    format!("<polygon points=\"{points}\" stroke-linejoin=\"round\"{attrs}/>")
}
