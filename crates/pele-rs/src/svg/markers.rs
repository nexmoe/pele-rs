//! Edge end markers. Ported from pele's `svg/edges.ts`.

use super::num;

/// The length of an arrowhead.
pub(crate) const ARROW: f64 = 8.0;

/// How far the line stops short of its end to leave room for a marker.
pub(crate) fn marker_trim(kind: &str) -> f64 {
    match kind {
        "arrow_point" => ARROW - 1.0,
        "arrow_circle" => ARROW,
        "arrow_cross" => 4.0,
        _ => 0.0,
    }
}

/// Draws a marker whose tip is at (x, y), pointing along (dx, dy). Types are Mermaid's:
/// `arrow_point`, `arrow_circle`, `arrow_cross`; anything else draws nothing.
pub(crate) fn marker(kind: &str, x: f64, y: f64, dx: f64, dy: f64, color: &str) -> String {
    let nx = -dy;
    let ny = dx;
    match kind {
        "arrow_point" => {
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
        "arrow_circle" => format!(
            "<circle class=\"pele-rs-marker\" cx=\"{}\" cy=\"{}\" r=\"3.5\" fill=\"{color}\" stroke=\"none\"/>",
            num(x - dx * 4.0),
            num(y - dy * 4.0)
        ),
        "arrow_cross" => {
            let cx = x - dx * 4.0;
            let cy = y - dy * 4.0;
            let a = 3.5;
            let (ux, uy) = ((dx + nx) * a, (dy + ny) * a);
            let (vx, vy) = ((dx - nx) * a, (dy - ny) * a);
            format!(
                "<path class=\"pele-rs-marker\" d=\"M{},{}L{},{}M{},{}L{},{}\"/>",
                num(cx - ux),
                num(cy - uy),
                num(cx + ux),
                num(cy + uy),
                num(cx - vx),
                num(cy - vy),
                num(cx + vx),
                num(cy + vy)
            )
        }
        _ => String::new(),
    }
}
