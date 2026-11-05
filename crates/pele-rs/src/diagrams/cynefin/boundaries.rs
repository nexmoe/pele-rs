//! The curves of the frame of a cynefin diagram: the wavy lines between the domains and the
//! S-shaped cliff between Clear and Chaotic. The arithmetic is pele's own, ported from
//! `diagrams/cynefin/boundaries.ts`. A curve is a flat list of numbers: a start point, then six
//! numbers (two control points and an end point) per cubic segment.

use crate::svg::num;

/// The length of an arrowhead, and how far the line stops short of its end for it.
const ARROW: f64 = 8.0;

/// `Math.imul`: the product of two numbers, as an integer of 32 bits.
fn imul(a: i32, b: i32) -> i32 {
    a.wrapping_mul(b)
}

/// `ToInt32`: the number as an integer of 32 bits, wrapped around.
fn to_int32(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    let wrapped = x.trunc().rem_euclid(4294967296.0);
    let signed = if wrapped >= 2147483648.0 {
        wrapped - 4294967296.0
    } else {
        wrapped
    };
    signed as i32
}

/// mulberry32, one step.
fn seeded_random(seed: f64) -> f64 {
    let mut t = to_int32(seed + 1831565813.0);
    t = imul(t ^ ((t as u32) >> 15) as i32, t | 1);
    t ^= t.wrapping_add(imul(t ^ ((t as u32) >> 7) as i32, t | 61));
    ((t ^ ((t as u32) >> 14) as i32) as u32) as f64 / 4294967296.0
}

/// The hash of a string, as Mermaid derives the seed of the boundaries from it.
fn hash_string(text: &str) -> i32 {
    let mut hash: i32 = 0;
    for unit in text.encode_utf16() {
        hash = hash
            .wrapping_shl(5)
            .wrapping_sub(hash)
            .wrapping_add(i32::from(unit));
    }
    hash
}

/// A configured seed other than zero wins; otherwise the seed comes from the diagram's id.
pub(crate) fn resolve_seed(configured: f64, id: &str) -> f64 {
    if configured.is_finite() && configured != 0.0 {
        return configured;
    }
    f64::from(hash_string(id))
}

/// How a wavy line is drawn.
pub(crate) struct WaveOptions {
    pub segments: usize,
    pub vertical: bool,
    /// The multipliers Mermaid uses to vary the seed per point and per control point.
    pub point_step: f64,
    pub control_step: f64,
    pub control_shift: f64,
    pub pin_start: bool,
    pub pin_end: bool,
}

/// The fold: the line between Complex and Complicated.
pub(crate) const FOLD: WaveOptions = WaveOptions {
    segments: 7,
    vertical: true,
    point_step: 17.0,
    control_step: 31.0,
    control_shift: 7.0,
    pin_start: false,
    pin_end: false,
};

/// The horizon: the line between Complex and Chaotic, and between Complicated and Clear.
pub(crate) const HORIZON: WaveOptions = WaveOptions {
    segments: 7,
    vertical: false,
    point_step: 23.0,
    control_step: 37.0,
    control_shift: 11.0,
    pin_start: false,
    pin_end: false,
};

/// A wavy line along one axis from `from` to `to`, centered on `center` in the other axis.
/// Points jitter by up to the amplitude; a pinned end stays on the center line.
pub(crate) fn wave(
    from: f64,
    to: f64,
    center: f64,
    seed: f64,
    amplitude: f64,
    o: &WaveOptions,
) -> Vec<f64> {
    let step = (to - from) / o.segments as f64;
    let mut along: Vec<f64> = Vec::with_capacity(o.segments + 1);
    let mut across: Vec<f64> = Vec::with_capacity(o.segments + 1);
    for i in 0..=o.segments {
        let pinned = (i == 0 && o.pin_start) || (i == o.segments && o.pin_end);
        along.push(from + i as f64 * step);
        let jitter = seeded_random(seed + i as f64 * o.point_step);
        across.push(if pinned {
            center
        } else {
            center + jitter * amplitude * 2.0 - amplitude
        });
    }
    let mut out: Vec<f64> = Vec::with_capacity(2 + o.segments * 6);
    if o.vertical {
        out.extend_from_slice(&[across[0], along[0]]);
    } else {
        out.extend_from_slice(&[along[0], across[0]]);
    }
    for i in 0..o.segments {
        let mid = (along[i] + along[i + 1]) / 2.0;
        let jitter = seeded_random(seed + i as f64 * o.control_step + o.control_shift);
        let offset = amplitude * 1.5 * if i % 2 == 0 { 1.0 } else { -1.0 } * jitter;
        if o.vertical {
            out.extend_from_slice(&[
                across[i] + offset,
                mid,
                across[i + 1] - offset,
                mid,
                across[i + 1],
                along[i + 1],
            ]);
        } else {
            out.extend_from_slice(&[
                mid,
                across[i] + offset,
                mid,
                across[i + 1] - offset,
                along[i + 1],
                across[i + 1],
            ]);
        }
    }
    out
}

/// The cliff: a steep S from (cx, top) down to (cx, bottom).
pub(crate) fn cliff(cx: f64, top: f64, bottom: f64, amplitude: f64) -> Vec<f64> {
    let h = bottom - top;
    vec![
        cx,
        top,
        cx + amplitude,
        top + h * 0.2,
        cx - amplitude * 1.5,
        top + h * 0.55,
        cx + amplitude * 0.5,
        top + h * 0.75,
        cx - amplitude,
        top + h * 0.85,
        cx + amplitude * 0.3,
        top + h * 0.95,
        cx,
        bottom,
    ]
}

/// Path commands that continue from the curve's start point to its end.
pub(crate) fn forward(curve: &[f64]) -> String {
    let mut d = String::new();
    let mut i = 2;
    while i + 5 < curve.len() {
        d.push_str(&format!(
            "C{},{} {},{} {},{}",
            num(curve[i]),
            num(curve[i + 1]),
            num(curve[i + 2]),
            num(curve[i + 3]),
            num(curve[i + 4]),
            num(curve[i + 5]),
        ));
        i += 6;
    }
    d
}

/// Path commands that continue from the curve's end point back to its start.
pub(crate) fn backward(curve: &[f64]) -> String {
    let mut d = String::new();
    let mut at = curve.len() as isize - 6;
    while at >= 2 {
        let i = at as usize;
        d.push_str(&format!(
            "C{},{} {},{} {},{}",
            num(curve[i + 2]),
            num(curve[i + 3]),
            num(curve[i]),
            num(curve[i + 1]),
            num(curve[i - 2]),
            num(curve[i - 1]),
        ));
        at -= 6;
    }
    d
}

/// The path of a curve, from its start to its end.
pub(crate) fn curve_path(curve: &[f64]) -> String {
    format!("M{},{}{}", num(curve[0]), num(curve[1]), forward(curve))
}

/// The path of the ellipse in the middle of the frame.
pub(crate) fn ellipse_path(cx: f64, cy: f64, rx: f64, ry: f64) -> String {
    let r = format!("{},{}", num(rx), num(ry));
    format!(
        "M{},{}A{r} 0 1,1 {},{}A{r} 0 1,1 {},{}Z",
        num(cx - rx),
        num(cy),
        num(cx + rx),
        num(cy),
        num(cx - rx),
        num(cy),
    )
}

/// The arrowhead of a transition, whose tip is at (x, y) and points along (dx, dy).
pub(crate) fn arrow_head(x: f64, y: f64, dx: f64, dy: f64, color: &str) -> String {
    let nx = -dy;
    let ny = dx;
    let bx = x - dx * ARROW;
    let by = y - dy * ARROW;
    let half = ARROW * 0.42;
    format!(
        "<path class=\"pele-rs-marker\" d=\"M{},{}L{},{}L{},{}Z\" fill=\"{color}\" stroke=\"none\"/>",
        num(x),
        num(y),
        num(bx + nx * half),
        num(by + ny * half),
        num(bx - nx * half),
        num(by - ny * half)
    )
}

/// How far the line of a transition stops short of its end to leave room for the arrowhead.
pub(crate) const MARKER_TRIM: f64 = ARROW - 1.0;
