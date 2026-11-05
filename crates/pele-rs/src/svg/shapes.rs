//! Node shapes: their size around a label, their outline, and where edges meet them. Ported
//! from pele's `svg/shapes.ts`.

use std::f64::consts::{FRAC_1_SQRT_2, PI};
use std::fmt::Write;

use super::num;
use super::theme::rounded;
#[cfg(any(
    feature = "flowchart",
    feature = "state",
    feature = "class",
    feature = "er",
    feature = "agentflow",
))]
use crate::layout::compound::CNode;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ShapeSize {
    pub w: f64,
    pub h: f64,
    /// Vertical shift of the label from the shape's center.
    pub dy: f64,
}

pub(crate) const PAD_X: f64 = 18.0;
pub(crate) const PAD_Y: f64 = 12.0;
/// Kept clear at each end of a straight side, for the corner.
const CORNER: f64 = 12.0;

/// The shapes with a definition of their own; anything else is drawn as `rect`.
const NAMES: &[&str] = &[
    "rect",
    "rounded",
    "stadium",
    "fr-rect",
    "cyl",
    "circle",
    "dbl-circ",
    "ellipse",
    "diam",
    "hex",
    "odd",
    "lean-r",
    "lean-l",
    "trap-b",
    "trap-t",
    "text",
    "sm-circ",
    "f-circ",
    "fr-circ",
    "fork",
    "notch-rect",
    "lin-rect",
    "div-rect",
    "win-pane",
    "tri",
    "flip-tri",
    "sl-rect",
    "notch-pent",
    "hourglass",
    "delay",
    "h-cyl",
    "cross-circ",
    "st-rect",
    "doc",
    "flag",
    "datastore",
    "folder",
    "bucket",
    "console",
    "browser",
    "person",
    "bang",
    "cloud",
    "brace",
    "brace-r",
    "braces",
    "bolt",
    "lin-cyl",
    "curv-trap",
    "docs",
    "bow-rect",
    "tag-rect",
    "tag-doc",
    "lin-doc",
    "collapsedGroup",
];

fn known(shape: &str) -> &str {
    if NAMES.contains(&shape) {
        shape
    } else {
        "rect"
    }
}

/// How far an ellipse's outline has fallen away from its bounding box, `t` from the middle of
/// a side `half` long.
fn bulge(t: f64, half: f64, depth: f64) -> f64 {
    let k = (t.abs() / half).min(1.0);
    depth * (1.0 - (1.0 - k * k).sqrt())
}

fn round_inset(w: f64, h: f64, side: u8, t: f64) -> f64 {
    if side & 1 == 1 {
        bulge(t, h / 2.0, w / 2.0)
    } else {
        bulge(t, w / 2.0, h / 2.0)
    }
}

fn cap_ry(w: f64) -> f64 {
    12f64.min(6f64.max(w / 14.0))
}

fn box_size(tw: f64, th: f64) -> ShapeSize {
    ShapeSize {
        w: (tw + 2.0 * PAD_X).max(2.0 * PAD_X + 12.0),
        h: th + 2.0 * PAD_Y,
        dy: 0.0,
    }
}

fn tri_size(tw: f64, th: f64, flip: bool) -> ShapeSize {
    let w = (1.6 * tw + 40.0).max(80.0);
    let h = (th + 10.0) / (1.0 - (tw + 8.0) / w);
    let dy = h / 2.0 - 10.0 - th / 2.0;
    ShapeSize {
        w,
        h,
        dy: if flip { -dy } else { dy },
    }
}

fn path(d: &str, a: &str) -> String {
    format!("<path d=\"{d}\"{a}/>")
}

fn poly(points: &[f64], a: &str) -> String {
    let mut d = String::new();
    for (i, pair) in points.chunks(2).enumerate() {
        if i > 0 {
            d.push(' ');
        }
        let _ = write!(d, "{},{}", num(pair[0]), num(pair[1]));
    }
    format!("<polygon points=\"{d}\"{a}/>")
}

fn rect(w: f64, h: f64, a: &str, rx: &str) -> String {
    format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rx}{a}/>",
        num(-w / 2.0),
        num(-h / 2.0),
        num(w),
        num(h)
    )
}

pub(crate) fn shape_size(shape: &str, tw: f64, th: f64) -> ShapeSize {
    let fixed = |w, h| ShapeSize { w, h, dy: 0.0 };
    let line_h = th + 2.0 * PAD_Y;
    match known(shape) {
        "stadium" => ShapeSize {
            w: tw + line_h * 0.5 + 2.0 * PAD_X - 8.0,
            h: line_h,
            dy: 0.0,
        },
        "fr-rect" => ShapeSize {
            w: tw + 2.0 * PAD_X + 16.0,
            h: line_h,
            dy: 0.0,
        },
        "cyl" => {
            let w = (tw + 2.0 * PAD_X).max(56.0);
            let ry = cap_ry(w);
            ShapeSize {
                w,
                h: line_h + 2.0 * ry,
                dy: ry / 2.0,
            }
        }
        "circle" => {
            let d = ((tw * tw + th * th).sqrt() + 16.0).max(44.0);
            fixed(d, d)
        }
        "dbl-circ" => {
            let d = ((tw * tw + th * th).sqrt() + 26.0).max(54.0);
            fixed(d, d)
        }
        "ellipse" => ShapeSize {
            w: tw * 1.3 + 2.0 * PAD_X,
            h: th * 1.2 + 2.0 * PAD_Y,
            dy: 0.0,
        },
        "diam" => {
            let a = tw + 16.0;
            let b = th + 8.0;
            ShapeSize {
                w: a + 1.6 * b,
                h: a / 1.6 + b,
                dy: 0.0,
            }
        }
        "hex" | "lean-r" | "lean-l" | "trap-b" | "trap-t" => ShapeSize {
            w: tw + 2.0 * PAD_X + line_h / 2.0,
            h: line_h,
            dy: 0.0,
        },
        "odd" | "delay" => ShapeSize {
            w: tw + 2.0 * PAD_X + line_h / 4.0,
            h: line_h,
            dy: 0.0,
        },
        "text" => ShapeSize {
            w: tw + 8.0,
            h: th + 8.0,
            dy: 0.0,
        },
        "sm-circ" | "f-circ" => fixed(14.0, 14.0),
        "fr-circ" => fixed(18.0, 18.0),
        "fork" => fixed(72.0, 8.0),
        "lin-rect" => ShapeSize {
            w: tw + 2.0 * PAD_X + 8.0,
            h: line_h,
            dy: 0.0,
        },
        "div-rect" => ShapeSize {
            w: tw + 2.0 * PAD_X,
            h: line_h + 8.0,
            dy: 4.0,
        },
        "win-pane" | "st-rect" => ShapeSize {
            w: tw + 2.0 * PAD_X + 8.0,
            h: line_h + 8.0,
            dy: 4.0,
        },
        "tri" => tri_size(tw, th, false),
        "flip-tri" => tri_size(tw, th, true),
        "sl-rect" => ShapeSize {
            w: tw + 2.0 * PAD_X,
            h: line_h + 10.0,
            dy: 5.0,
        },
        "notch-pent" => ShapeSize {
            w: tw + 2.0 * PAD_X,
            h: line_h,
            dy: 0.0,
        },
        "hourglass" => fixed(32.0, 40.0),
        "h-cyl" => {
            let rx = 12f64.min(line_h / 4.0);
            ShapeSize {
                w: tw + 2.0 * PAD_X + 2.0 * rx,
                h: line_h,
                dy: 0.0,
            }
        }
        "cross-circ" => fixed(44.0, 44.0),
        "doc" | "tag-doc" => ShapeSize {
            w: tw + 2.0 * PAD_X,
            h: line_h + 10.0,
            dy: -5.0,
        },
        "lin-doc" => ShapeSize {
            w: tw + 2.0 * PAD_X + 8.0,
            h: line_h + 10.0,
            dy: -5.0,
        },
        "flag" => ShapeSize {
            w: tw + 2.0 * PAD_X,
            h: line_h + 16.0,
            dy: 0.0,
        },
        "folder" => ShapeSize {
            w: (tw + 2.0 * PAD_X).max(64.0),
            h: line_h + 8.0,
            dy: 4.0,
        },
        "bucket" => ShapeSize {
            w: tw + 2.0 * PAD_X + 20.0,
            h: line_h + 10.0,
            dy: 3.0,
        },
        "console" | "browser" => ShapeSize {
            w: (tw + 2.0 * PAD_X).max(72.0),
            h: line_h + 12.0,
            dy: 6.0,
        },
        "person" => ShapeSize {
            w: (tw + 2.0 * PAD_X).max(56.0),
            h: line_h + 26.0,
            dy: 13.0,
        },
        "bang" => ShapeSize {
            w: tw + 2.0 * PAD_X + 28.0,
            h: line_h + 24.0,
            dy: 0.0,
        },
        "cloud" => ShapeSize {
            w: tw + 2.0 * PAD_X + 32.0,
            h: line_h + 24.0,
            dy: 0.0,
        },
        "brace" | "brace-r" => ShapeSize {
            w: tw + 2.0 * PAD_X + 12.0,
            h: line_h,
            dy: 0.0,
        },
        "braces" => ShapeSize {
            w: tw + 2.0 * PAD_X + 24.0,
            h: line_h,
            dy: 0.0,
        },
        "bolt" => fixed(36.0, 56.0),
        "lin-cyl" => {
            let w = (tw + 2.0 * PAD_X).max(56.0);
            let ry = cap_ry(w);
            ShapeSize {
                w,
                h: line_h + 2.0 * ry + 8.0,
                dy: ry / 2.0 + 4.0,
            }
        }
        "curv-trap" => ShapeSize {
            w: tw + 2.0 * PAD_X + line_h * 0.6,
            h: line_h,
            dy: 0.0,
        },
        "docs" => ShapeSize {
            w: tw + 2.0 * PAD_X + 8.0,
            h: line_h + 18.0,
            dy: -1.0,
        },
        "bow-rect" => ShapeSize {
            w: tw + 2.0 * PAD_X + line_h * 0.4,
            h: line_h,
            dy: 0.0,
        },
        "collapsedGroup" => ShapeSize {
            w: tw + 2.0 * PAD_X + 8.0,
            h: line_h + 8.0,
            dy: -2.0,
        },
        _ => box_size(tw, th),
    }
}

fn brace(x: f64, h: f64, dir: f64, line: &str) -> String {
    let y = h / 2.0;
    let k = 6.0 * dir;
    format!(
        "<path d=\"M{},{}q{},0 {},6V-6q0,6 {},6q{},0 {},6V{}q0,6 {},6\" fill=\"none\"{line}/>",
        num(x),
        num(-y),
        num(k),
        num(k),
        num(k),
        num(-k),
        num(-k),
        num(y - 6.0),
        num(-k)
    )
}

fn wave(w: f64, h: f64, a: &str) -> String {
    let x = w / 2.0;
    let top = -h / 2.0;
    let base = h / 2.0 - 10.0;
    let q = w / 4.0;
    format!(
        "<path d=\"M{},{}H{}V{}q{},-12 {},0t{},0Z\"{a}/>",
        num(-x),
        num(top),
        num(x),
        num(base),
        num(-q),
        num(-2.0 * q),
        num(-2.0 * q)
    )
}

/// The outline of a shape `w` by `h` centered on the origin. `a` holds the attributes of its
/// fill and outline, `line` those of lines drawn inside it.
pub(crate) fn draw_shape(shape: &str, w: f64, h: f64, a: &str, line: &str) -> String {
    let (x, y) = (w / 2.0, h / 2.0);
    let fill_none = |d: &str| format!("<path d=\"{d}\" fill=\"none\"{line}/>");
    match known(shape) {
        "rounded" => rect(w, h, a, &format!(" rx=\"{}\"", num(12f64.min(y)))),
        "stadium" => rect(w, h, a, &format!(" rx=\"{}\"", num(y))),
        "fr-rect" => {
            let fx = x - 8.0;
            rect(w, h, &rounded(a), "")
                + &fill_none(&format!(
                    "M{},{}V{}M{},{}V{}",
                    num(-fx),
                    num(-y),
                    num(y),
                    num(fx),
                    num(-y),
                    num(y)
                ))
        }
        "cyl" | "lin-cyl" => {
            let rx = x;
            let ry = cap_ry(w);
            let top = -y + ry;
            let bottom = y - ry;
            let body = format!(
                "<path d=\"M{},{}A{},{} 0 0 1 {},{}V{}A{},{} 0 0 1 {},{}Z\"{a}/>",
                num(-rx),
                num(top),
                num(rx),
                num(ry),
                num(rx),
                num(top),
                num(bottom),
                num(rx),
                num(ry),
                num(-rx),
                num(bottom)
            );
            let arc = |yy: f64| {
                format!(
                    "M{},{}A{},{} 0 0 0 {},{}",
                    num(-rx),
                    num(yy),
                    num(rx),
                    num(ry),
                    num(rx),
                    num(yy)
                )
            };
            if shape == "cyl" {
                body + &fill_none(&arc(top))
            } else {
                body + &fill_none(&(arc(top) + &arc(top + 8.0)))
            }
        }
        "circle" | "sm-circ" => format!("<circle r=\"{}\"{a}/>", num(x)),
        "dbl-circ" => format!(
            "<circle r=\"{}\"{a}/><circle r=\"{}\" fill=\"none\"{line}/>",
            num(x),
            num(x - 5.0)
        ),
        "f-circ" => format!("<circle r=\"{}\" fill=\"var(--_l)\"{line}/>", num(x)),
        "fr-circ" => format!(
            "<circle r=\"{}\"{a}/><circle r=\"{}\" fill=\"var(--_l)\"{line}/>",
            num(x),
            num(x - 4.0)
        ),
        "ellipse" => format!("<ellipse rx=\"{}\" ry=\"{}\"{a}/>", num(x), num(y)),
        "diam" => poly(&[0.0, -y, x, 0.0, 0.0, y, -x, 0.0], a),
        "hex" => {
            let m = h / 4.0;
            poly(
                &[-x + m, -y, x - m, -y, x, 0.0, x - m, y, -x + m, y, -x, 0.0],
                a,
            )
        }
        "odd" => poly(&[-x, -y, x, -y, x, y, -x, y, -x + h / 4.0, 0.0], a),
        "lean-r" => {
            let s = h / 4.0;
            poly(&[-x + s, -y, x, -y, x - s, y, -x, y], a)
        }
        "lean-l" => {
            let s = h / 4.0;
            poly(&[-x, -y, x - s, -y, x, y, -x + s, y], a)
        }
        "trap-b" => {
            let s = h / 4.0;
            poly(&[-x + s, -y, x - s, -y, x, y, -x, y], a)
        }
        "trap-t" => {
            let s = h / 4.0;
            poly(&[-x, -y, x, -y, x - s, y, -x + s, y], a)
        }
        "text" => rect(w, h, " fill=\"none\" stroke=\"none\"", ""),
        "fork" => rect(w, h, &format!(" fill=\"var(--_l)\"{line}"), " rx=\"2\""),
        "notch-rect" => {
            let c = 10.0;
            poly(&[-x + c, -y, x, -y, x, y, -x, y, -x, -y + c], a)
        }
        "lin-rect" => {
            rect(w, h, &rounded(a), "")
                + &fill_none(&format!("M{},{}V{}", num(-x + 8.0), num(-y), num(y)))
        }
        "div-rect" => {
            rect(w, h, &rounded(a), "")
                + &fill_none(&format!("M{},{}H{}", num(-x), num(-y + 8.0), num(x)))
        }
        "win-pane" => {
            rect(w, h, &rounded(a), "")
                + &fill_none(&format!(
                    "M{},{}V{}M{},{}H{}",
                    num(-x + 8.0),
                    num(-y),
                    num(y),
                    num(-x),
                    num(-y + 8.0),
                    num(x)
                ))
        }
        "tri" => poly(&[0.0, -y, x, y, -x, y], a),
        "flip-tri" => poly(&[-x, -y, x, -y, 0.0, y], a),
        "sl-rect" => poly(&[-x, -y + 10.0, x, -y, x, y, -x, y], a),
        "notch-pent" => {
            let c = 10.0;
            poly(
                &[-x + c, -y, x - c, -y, x, -y + c, x, y, -x, y, -x, -y + c],
                a,
            )
        }
        "hourglass" => poly(&[-x, -y, x, -y, -x, y, x, y], a),
        "delay" => {
            let r = y;
            path(
                &format!(
                    "M{},{}H{}A{},{} 0 0 1 {},{}H{}Z",
                    num(-x),
                    num(-r),
                    num(x - r),
                    num(r),
                    num(r),
                    num(x - r),
                    num(r),
                    num(-x)
                ),
                a,
            )
        }
        "h-cyl" => {
            let ry = y;
            let rx = 12f64.min(h / 4.0);
            let l = -x + rx;
            let r = x - rx;
            path(
                &format!(
                    "M{},{}H{}A{},{} 0 0 1 {},{}H{}A{},{} 0 0 1 {},{}Z",
                    num(l),
                    num(-ry),
                    num(r),
                    num(rx),
                    num(ry),
                    num(r),
                    num(ry),
                    num(l),
                    num(rx),
                    num(ry),
                    num(l),
                    num(-ry)
                ),
                a,
            ) + &fill_none(&format!(
                "M{},{}A{},{} 0 0 0 {},{}",
                num(r),
                num(-ry),
                num(rx),
                num(ry),
                num(r),
                num(ry)
            ))
        }
        "cross-circ" => {
            let k = x * FRAC_1_SQRT_2;
            format!("<circle r=\"{}\"{a}/>", num(x))
                + &fill_none(&format!(
                    "M{},{}L{},{}M{},{}L{},{}",
                    num(-k),
                    num(-k),
                    num(k),
                    num(k),
                    num(k),
                    num(-k),
                    num(-k),
                    num(k)
                ))
        }
        "st-rect" => {
            let mut out = String::new();
            for i in 0..3 {
                let _ = write!(
                    out,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>",
                    num(-x + f64::from(2 - i) * 4.0),
                    num(-y + f64::from(i) * 4.0),
                    num(w - 8.0),
                    num(h - 8.0),
                    rounded(a)
                );
            }
            out
        }
        "doc" => wave(w, h, a),
        "flag" => {
            let fy = y - 8.0;
            let q = w / 4.0;
            path(
                &format!(
                    "M{},{}q{},-10 {},0t{},0V{}q{},10 {},0t{},0Z",
                    num(-x),
                    num(-fy),
                    num(q),
                    num(2.0 * q),
                    num(2.0 * q),
                    num(fy),
                    num(-q),
                    num(-2.0 * q),
                    num(-2.0 * q)
                ),
                a,
            )
        }
        "datastore" => {
            rect(
                w,
                h,
                &a.replacen("stroke=\"var(--_b)\"", "stroke=\"none\"", 1),
                "",
            ) + &fill_none(&format!(
                "M{},{}H{}M{},{}H{}",
                num(-x),
                num(-y),
                num(x),
                num(-x),
                num(y),
                num(x)
            ))
        }
        "folder" => {
            let tab = (w * 0.4).min(48.0);
            path(
                &format!(
                    "M{},{}H{}L{},{}H{}V{}H{}Z",
                    num(-x),
                    num(-y),
                    num(-x + tab),
                    num(-x + tab + 8.0),
                    num(-y + 8.0),
                    num(x),
                    num(y),
                    num(-x)
                ),
                a,
            )
        }
        "bucket" => {
            let ry = 5.0;
            let taper = h / 6.0;
            path(
                &format!(
                    "M{},{}A{},{} 0 0 1 {},{}L{},{}A{},{} 0 0 1 {},{}Z",
                    num(-x),
                    num(-y + ry),
                    num(x),
                    num(ry),
                    num(x),
                    num(-y + ry),
                    num(x - taper),
                    num(y - ry),
                    num(x - taper),
                    num(ry),
                    num(-x + taper),
                    num(y - ry)
                ),
                a,
            ) + &fill_none(&format!(
                "M{},{}A{},{} 0 0 0 {},{}",
                num(-x),
                num(-y + ry),
                num(x),
                num(ry),
                num(x),
                num(-y + ry)
            ))
        }
        "console" => {
            rect(w, h, &rounded(a), "")
                + &fill_none(&format!(
                    "M{},{}H{}M{},{}l3,2.5l-3,2.5M{},{}h4",
                    num(-x),
                    num(-y + 12.0),
                    num(x),
                    num(-x + 7.0),
                    num(-y + 4.0),
                    num(-x + 13.0),
                    num(-y + 9.0)
                ))
        }
        "browser" => {
            let mut dots = String::new();
            for i in 0..3 {
                let _ = write!(
                    dots,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"1.5\" fill=\"none\"{line}/>",
                    num(-x + 8.0 + f64::from(i) * 7.0),
                    num(-y + 6.0)
                );
            }
            rect(w, h, &rounded(a), "")
                + &fill_none(&format!("M{},{}H{}", num(-x), num(-y + 12.0), num(x)))
                + &dots
        }
        "person" => {
            let top = -y;
            format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"12\"{a}/><circle cy=\"{}\" r=\"13\"{a}/>",
                num(-x),
                num(top + 26.0),
                num(w),
                num(h - 26.0),
                num(top + 14.0)
            )
        }
        "bang" => {
            let spikes = 14;
            let mut pts = Vec::with_capacity(spikes * 4);
            for i in 0..spikes * 2 {
                let angle = (PI * i as f64) / spikes as f64 - PI / 2.0;
                let k = if i % 2 == 0 { 1.0 } else { 0.82 };
                pts.push(angle.cos() * x * k);
                pts.push(angle.sin() * y * k);
            }
            poly(&pts, a)
        }
        "cloud" => {
            let r = y * 0.55;
            path(
                &format!(
                    "M{},{}A{},{} 0 0 1 {},{}A{},{} 0 0 1 {},{}A{},{} 0 0 1 {},{}A{},{} 0 0 1 {},{}Z",
                    num(-x + r),
                    num(y),
                    num(r),
                    num(r),
                    num(-x + r * 0.6),
                    num(-y * 0.1),
                    num(x * 0.42),
                    num(y * 0.7),
                    num(-x * 0.1),
                    num(-y * 0.72),
                    num(x * 0.4),
                    num(y * 0.62),
                    num(x * 0.62),
                    num(-y * 0.42),
                    num(r * 1.05),
                    num(r * 1.05),
                    num(x - r),
                    num(y)
                ),
                a,
            )
        }
        "brace" => {
            rect(w, h, " fill=\"none\" stroke=\"none\"", "") + &brace(-x + 12.0, h, -1.0, line)
        }
        "brace-r" => {
            rect(w, h, " fill=\"none\" stroke=\"none\"", "") + &brace(x - 12.0, h, 1.0, line)
        }
        "braces" => {
            rect(w, h, " fill=\"none\" stroke=\"none\"", "")
                + &brace(-x + 12.0, h, -1.0, line)
                + &brace(x - 12.0, h, 1.0, line)
        }
        "bolt" => poly(
            &[
                w * 0.15,
                -y,
                -x,
                h * 0.08,
                -w * 0.05,
                h * 0.08,
                -w * 0.15,
                y,
                x,
                -h * 0.08,
                w * 0.05,
                -h * 0.08,
            ],
            a,
        ),
        "curv-trap" => {
            let k = h * 0.3;
            path(
                &format!(
                    "M{},{}H{}A{},{} 0 0 1 {},{}H{}L{},0Z",
                    num(-x + k),
                    num(-y),
                    num(x - k),
                    num(k),
                    num(y),
                    num(x - k),
                    num(y),
                    num(-x + k),
                    num(-x)
                ),
                a,
            )
        }
        "docs" => {
            let mut out = String::new();
            for i in 0..3 {
                let _ = write!(
                    out,
                    "<g transform=\"translate({},{})\">{}</g>",
                    (1 - i) * 4,
                    (i - 1) * 4,
                    wave(w - 8.0, h - 8.0, a)
                );
            }
            out
        }
        "bow-rect" => {
            let k = h * 0.2;
            path(
                &format!(
                    "M{},{}H{}A{},{} 0 0 0 {},{}H{}A{},{} 0 0 1 {},{}Z",
                    num(-x + k),
                    num(-y),
                    num(x),
                    num(k),
                    num(y),
                    num(x),
                    num(y),
                    num(-x + k),
                    num(k),
                    num(y),
                    num(-x + k),
                    num(-y)
                ),
                a,
            )
        }
        "tag-rect" => {
            rect(w, h, &rounded(a), "")
                + &fill_none(&format!(
                    "M{},{}L{},{}",
                    num(x - 12.0),
                    num(y),
                    num(x),
                    num(y - 12.0)
                ))
        }
        "tag-doc" => {
            wave(w, h, a)
                + &fill_none(&format!(
                    "M{},{}L{},{}",
                    num(x - 12.0),
                    num(y - 14.0),
                    num(x),
                    num(y - 26.0)
                ))
        }
        "lin-doc" => {
            wave(w, h, a) + &fill_none(&format!("M{},{}V{}", num(-x + 8.0), num(-y), num(y - 6.0)))
        }
        "collapsedGroup" => format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>",
            num(-x + 6.0),
            num(-y + 8.0),
            num(w - 12.0),
            num(h - 8.0),
            rounded(a),
            num(-x),
            num(-y),
            num(w),
            num(h - 4.0),
            rounded(a)
        ),
        _ => rect(w, h, &rounded(a), ""),
    }
}

/// Distance from the bounding box to the outline on a side: 0 top, 1 right, 2 bottom, 3 left.
/// `t` is how far along the side from its middle, to the right or down.
pub(crate) fn shape_inset(shape: &str, w: f64, h: f64, side: u8, t: f64) -> f64 {
    let odd = side & 1 == 1;
    match shape {
        "circle" | "dbl-circ" | "ellipse" | "sm-circ" | "f-circ" | "fr-circ" | "cross-circ" => {
            round_inset(w, h, side, t)
        }
        "stadium" if odd => bulge(t, h / 2.0, h / 2.0),
        "cyl" | "lin-cyl" if !odd => bulge(t, w / 2.0, cap_ry(w)),
        "diam" if odd => (t.abs() * w) / h,
        "diam" => (t.abs() * h) / w,
        "hex" if odd => t.abs() / 2.0,
        "odd" if side == 3 => h / 4.0,
        "lean-r" | "lean-l" | "trap-b" | "trap-t" if odd => h / 8.0,
        "tri" | "flip-tri" | "bolt" if odd => w / 4.0,
        "sl-rect" if side == 0 => 5.0,
        "hourglass" if odd => w / 2.0,
        "doc" | "docs" | "tag-doc" | "lin-doc" if side == 2 => 10.0,
        "flag" if !odd => 8.0,
        "bucket" if odd => h / 12.0,
        "bang" => 3.0,
        "cloud" if odd => 2.0,
        "cloud" => 1.0,
        "bow-rect" if side == 1 => h * 0.2,
        _ => 0.0,
    }
}

/// How much of a side of the shape edges may spread along, when the flow runs down or, if
/// `sideways`, across.
pub(crate) fn shape_span(shape: &str, w: f64, h: f64, sideways: bool) -> f64 {
    let along = if sideways { h } else { w };
    let flat = along - 2.0 * CORNER;
    let flat_across = |cut: f64| {
        if sideways {
            0.0
        } else {
            w - 2.0 * cut - 2.0 * CORNER
        }
    };
    let span = match shape {
        "rect" | "rounded" | "fr-rect" | "text" | "notch-rect" | "lin-rect" | "div-rect"
        | "win-pane" | "notch-pent" | "datastore" | "console" | "browser" | "tag-rect" => flat,
        "stadium" => {
            if sideways {
                h * 0.6
            } else {
                w - h
            }
        }
        "cyl" => {
            if sideways {
                h - 2.0 * CORNER - 12.0
            } else {
                w * 0.6
            }
        }
        "lin-cyl" => {
            if sideways {
                h - 2.0 * CORNER - 20.0
            } else {
                w * 0.6
            }
        }
        "circle" | "dbl-circ" | "ellipse" | "sm-circ" | "f-circ" | "fr-circ" | "cross-circ"
        | "diam" => along * 0.6,
        "hex" => {
            if sideways {
                h * 0.5
            } else {
                w - h / 2.0 - CORNER
            }
        }
        "odd" => flat_across(0.0),
        "lean-r" | "lean-l" | "trap-b" | "trap-t" => flat_across(h / 4.0),
        "fork" => along - 8.0,
        "st-rect" => along - 2.0 * CORNER - 8.0,
        _ => 0.0,
    };
    span.max(0.0)
}

pub(crate) fn shape_has_label(shape: &str) -> bool {
    !matches!(
        shape,
        "sm-circ" | "f-circ" | "fr-circ" | "fork" | "hourglass" | "cross-circ" | "bolt"
    )
}

/// The side of a node's layout box that a route end lies on: 0 top, 1 right, 2 bottom, 3 left,
/// or `None`.
#[cfg(any(
    feature = "flowchart",
    feature = "state",
    feature = "class",
    feature = "er",
    feature = "agentflow",
))]
pub(crate) fn end_side(route: &[f64], at: usize, c: &CNode) -> Option<u8> {
    let dx = route[at] - c.x;
    let dy = route[at + 1] - c.y;
    if (dy.abs() - c.h / 2.0).abs() < 0.5 {
        return Some(if dy < 0.0 { 0 } else { 2 });
    }
    if (dx.abs() - c.w / 2.0).abs() < 0.5 {
        return Some(if dx < 0.0 { 3 } else { 1 });
    }
    None
}

/// Moves a route end from a node's layout box onto the outline of the shape drawn in it.
#[cfg(any(
    feature = "flowchart",
    feature = "state",
    feature = "class",
    feature = "er",
    feature = "agentflow",
))]
pub(crate) fn inset_route(route: &mut [f64], at: usize, shape: &str, w: f64, h: f64, c: &CNode) {
    if c.is_group {
        return;
    }
    match end_side(route, at, c) {
        Some(side @ (0 | 2)) => {
            let amount = shape_inset(shape, w, h, side, route[at] - c.x) + (c.h - h) / 2.0;
            route[at + 1] += if side == 0 { amount } else { -amount };
        }
        Some(side) => {
            let amount = shape_inset(shape, w, h, side, route[at + 1] - c.y) + (c.w - w) / 2.0;
            route[at] += if side == 3 { amount } else { -amount };
        }
        None => {}
    }
}
