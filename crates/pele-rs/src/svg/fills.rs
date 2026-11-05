//! Contrast and casings for edges crossing author fills, from Pele 1.13 svg/edges.ts.
use super::{edges::EdgePath, ink::ink, markers::marker, num};

pub(crate) struct Fill {
    pub bounds: [f64; 4],
    pub shape: String,
}
pub(crate) enum Coverage<'a> {
    Outside,
    Crossed,
    Inside(&'a Fill),
}

fn commands(d: &str) -> Vec<(u8, Vec<f64>)> {
    let bytes = d.as_bytes();
    let mut out: Vec<(u8, Vec<f64>)> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_uppercase() && c != b'E' {
            out.push((c, Vec::new()));
            i += 1;
        } else if c.is_ascii_digit() || matches!(c, b'-' | b'.') {
            let start = i;
            i += 1;
            while i < bytes.len()
                && (bytes[i].is_ascii_digit()
                    || matches!(bytes[i], b'.' | b'e' | b'E')
                    || (matches!(bytes[i], b'+' | b'-') && matches!(bytes[i - 1], b'e' | b'E')))
            {
                i += 1;
            }
            if let Some((_, args)) = out.last_mut() {
                args.push(d[start..i].parse().unwrap_or(f64::NAN));
            }
        } else {
            i += 1;
        }
    }
    out
}
fn points(d: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let (mut x, mut y) = (0.0, 0.0);
    for (cmd, args) in commands(d) {
        if matches!(cmd, b'H' | b'V') {
            for value in args {
                if cmd == b'H' {
                    x = value;
                } else {
                    y = value;
                }
                out.extend([x, y]);
            }
        } else if matches!(cmd, b'C' | b'Q') {
            let cubic = cmd == b'C';
            let step = if cubic { 6 } else { 4 };
            for p in args.chunks_exact(step) {
                let (x1, y1, x2, y2) = (p[0], p[1], p[2], p[3]);
                let (x3, y3) = if cubic { (p[4], p[5]) } else { (x2, y2) };
                for k in 1..=8 {
                    let t = k as f64 * 0.125;
                    let (mut ax, mut ay) = (x + (x1 - x) * t, y + (y1 - y) * t);
                    let (mut bx, mut by) = (x1 + (x2 - x1) * t, y1 + (y2 - y1) * t);
                    if cubic {
                        let (cx, cy) = (x2 + (x3 - x2) * t, y2 + (y3 - y2) * t);
                        ax += (bx - ax) * t;
                        ay += (by - ay) * t;
                        bx += (cx - bx) * t;
                        by += (cy - by) * t;
                    }
                    out.extend([ax + (bx - ax) * t, ay + (by - ay) * t]);
                }
                x = x3;
                y = y3;
            }
        } else if cmd == b'A' {
            for p in args.chunks_exact(7) {
                arc(&mut out, [x, y], p);
                x = p[5];
                y = p[6];
            }
        } else {
            for (i, p) in args.chunks_exact(2).enumerate() {
                if cmd == b'M' && i == 0 && !out.is_empty() {
                    out.extend([f64::NAN, f64::NAN]);
                }
                x = p[0];
                y = p[1];
                out.extend([x, y]);
            }
        }
    }
    out
}
fn arc(out: &mut Vec<f64>, from: [f64; 2], p: &[f64]) {
    let [x1, y1] = from;
    let (mut rx, mut ry, large, sweep, x2, y2) = (p[0], p[1], p[3], p[4], p[5], p[6]);
    let (hx, hy) = ((x1 - x2) / 2.0, (y1 - y2) / 2.0);
    if rx == 0.0 || ry == 0.0 || (hx == 0.0 && hy == 0.0) {
        out.extend([x2, y2]);
        return;
    }
    let scale = ((hx * hx) / (rx * rx) + (hy * hy) / (ry * ry))
        .sqrt()
        .max(1.0);
    rx *= scale;
    ry *= scale;
    let k = if large == sweep { -1.0 } else { 1.0 }
        * ((rx * rx * ry * ry - rx * rx * hy * hy - ry * ry * hx * hx)
            / (rx * rx * hy * hy + ry * ry * hx * hx))
            .max(0.0)
            .sqrt();
    let cx = (k * rx * hy) / ry + (x1 + x2) / 2.0;
    let cy = (-k * ry * hx) / rx + (y1 + y2) / 2.0;
    let from = ((y1 - cy) / ry).atan2((x1 - cx) / rx);
    let mut turn = ((y2 - cy) / ry).atan2((x2 - cx) / rx) - from;
    if sweep != 0.0 && turn < 0.0 {
        turn += 2.0 * std::f64::consts::PI;
    } else if sweep == 0.0 && turn > 0.0 {
        turn -= 2.0 * std::f64::consts::PI;
    }
    for i in 1..=8 {
        out.extend([
            cx + rx * (from + (turn * i as f64) / 8.0).cos(),
            cy + ry * (from + (turn * i as f64) / 8.0).sin(),
        ]);
    }
}
fn clips(x: f64, y: f64, dx: f64, dy: f64, bounds: [f64; 4]) -> bool {
    let [l, t, r, b] = bounds;
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for (p, q) in [(-dx, x - l), (dx, r - x), (-dy, y - t), (dy, b - y)] {
        if p == 0.0 {
            if q < 0.0 {
                hi = -1.0;
            }
        } else if p < 0.0 {
            lo = lo.max(q / p);
        } else {
            hi = hi.min(q / p);
        }
    }
    lo < hi
}
pub(crate) fn fill_under<'a>(d: &str, tips: &[f64], fills: &'a [Fill]) -> Coverage<'a> {
    if fills.is_empty() {
        return Coverage::Outside;
    }
    let points = points(d);
    let (mut minx, mut miny, mut maxx, mut maxy) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for p in points.chunks_exact(2).chain(tips.chunks_exact(2)) {
        if p[0].is_finite() && p[1].is_finite() {
            minx = minx.min(p[0]);
            miny = miny.min(p[1]);
            maxx = maxx.max(p[0]);
            maxy = maxy.max(p[1]);
        }
    }
    let mut holder = None;
    let mut least = f64::INFINITY;
    for fill in fills {
        let [l, t, r, b] = fill.bounds;
        if maxx < l + 0.5 || minx > r - 0.5 || maxy < t + 0.5 || miny > b - 0.5 {
            continue;
        }
        let touched = points.windows(4).step_by(2).any(|p| {
            p.iter().all(|n| n.is_finite())
                && clips(
                    p[0],
                    p[1],
                    p[2] - p[0],
                    p[3] - p[1],
                    [l + 0.5, t + 0.5, r - 0.5, b - 0.5],
                )
        }) || tips
            .chunks_exact(2)
            .any(|p| p[0] > l + 0.5 && p[0] < r - 0.5 && p[1] > t + 0.5 && p[1] < b - 0.5);
        if !touched {
            continue;
        }
        if minx < l || maxx > r || miny < t || maxy > b {
            return Coverage::Crossed;
        }
        let area = (r - l) * (b - t);
        if area < least {
            least = area;
            holder = Some(fill);
        }
    }
    holder.map_or(Coverage::Outside, Coverage::Inside)
}
pub(crate) fn edge_ink(fill: &Fill, line: &str) -> String {
    if line.contains("\"stroke:") || line.contains(";stroke:") {
        return String::new();
    }
    ink(&fill.shape, "").unwrap_or("").to_string()
}
pub(crate) fn over_fills(d: &str, tips: &[f64], fills: &[Fill], line: &str) -> Option<String> {
    match fill_under(d, tips, fills) {
        Coverage::Outside => None,
        Coverage::Crossed => Some(String::new()),
        Coverage::Inside(fill) => Some(edge_ink(fill, line)),
    }
}
pub(crate) fn hollow_on(markers: &str, fill: &Fill) -> String {
    let color = fill
        .shape
        .split(['"', ';'])
        .find_map(|s| s.strip_prefix("fill:"))
        .map(str::trim);
    color.map_or_else(
        || markers.to_string(),
        |c| markers.replace(" fill=\"var(--_bg)\"", &format!(" fill=\"{c}\"")),
    )
}
pub(crate) fn casing(d: &str, markers: &str, line: &str, thick: bool) -> String {
    let decl = |prop: &str| {
        line.split(['"', ';'])
            .find_map(|s| s.strip_prefix(prop).and_then(|s| s.strip_prefix(':')))
            .map(str::trim)
    };
    if matches!(decl("stroke"), Some("none" | "transparent")) {
        return String::new();
    }
    let width = if thick {
        Some("2.5")
    } else {
        decl("stroke-width")
    };
    let mut attrs = String::new();
    let mut style = String::new();
    if let Some(width) = width {
        let number = width.strip_suffix("px").unwrap_or(width);
        if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
            if let Some(n) = super::parse_float(number) {
                if n != 1.0 {
                    attrs = format!(" stroke-width=\"{}\"", num(n + 4.0));
                }
            }
        } else {
            style += &format!("stroke-width:calc({width} + 4px);");
        }
    }
    for prop in ["opacity", "stroke-opacity"] {
        if let Some(v) = decl(prop) {
            style += &format!("{prop}:{v};");
        }
    }
    let body = format!(
        "<path d=\"{d}\"/>{}",
        markers.replace(" class=\"pele-marker\"", "")
    );
    if attrs.is_empty() && style.is_empty() {
        body
    } else {
        format!(
            "<g{attrs}{}>{body}</g>",
            if style.is_empty() {
                String::new()
            } else {
                format!(" style=\"{style}\"")
            }
        )
    }
}
pub(crate) fn edge_casing(
    path: &EdgePath,
    start: &str,
    end: &str,
    line: &str,
    thick: bool,
) -> String {
    let markers = (marker(start, path.sx, path.sy, path.sdx, path.sdy, "var(--_bg)")
        + &marker(end, path.ex, path.ey, path.edx, path.edy, "var(--_bg)"))
        .replace("stroke=\"none\"", "stroke=\"var(--_bg)\"");
    casing(&path.d, &markers, line, thick)
}
pub(crate) fn casings_svg(casings: &str) -> String {
    if casings.is_empty() {
        String::new()
    } else {
        format!(
            "<g class=\"pele-casings\" fill=\"none\" stroke=\"var(--_bg)\" stroke-width=\"5\" stroke-linecap=\"round\" stroke-linejoin=\"round\" pointer-events=\"none\">{casings}</g>"
        )
    }
}
