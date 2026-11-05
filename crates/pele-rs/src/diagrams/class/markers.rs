//! The markers at the ends of class relations. Ported from pele's `diagrams/class/markers.ts`.

use super::graph::EndMarker;
use crate::svg::markers::{marker, marker_trim};
use crate::svg::num;

const TRIANGLE: f64 = 12.0;
const DIAMOND: f64 = 16.0;
const HALF: f64 = 6.0;
const RING: f64 = 5.0;

/// How far the line stops short of the node so that it meets the back of the marker.
pub(crate) fn class_marker_trim(end: EndMarker) -> f64 {
    match end {
        EndMarker::Extension => TRIANGLE,
        EndMarker::Composition | EndMarker::Aggregation => DIAMOND,
        EndMarker::Lollipop => 2.0 * RING,
        EndMarker::Dependency => marker_trim("arrow_point"),
        EndMarker::None => 0.0,
    }
}

/// Draws an end marker whose tip is at (x, y), pointing along (dx, dy).
pub(crate) fn class_marker(
    end: EndMarker,
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    color: &str,
) -> String {
    let nx = -dy;
    let ny = dx;
    match end {
        EndMarker::Extension => {
            let bx = x - dx * TRIANGLE;
            let by = y - dy * TRIANGLE;
            format!(
                "<path class=\"pele-marker\" d=\"M{},{}L{},{}L{},{}Z\" stroke-linejoin=\"round\"/>",
                num(x),
                num(y),
                num(bx + nx * HALF),
                num(by + ny * HALF),
                num(bx - nx * HALF),
                num(by - ny * HALF)
            )
        }
        EndMarker::Composition | EndMarker::Aggregation => {
            let mx = x - (dx * DIAMOND) / 2.0;
            let my = y - (dy * DIAMOND) / 2.0;
            let half = HALF - 1.0;
            let fill = if end == EndMarker::Composition {
                format!(" fill=\"{color}\"")
            } else {
                String::new()
            };
            format!(
                "<path class=\"pele-marker\" d=\"M{},{}L{},{}L{},{}L{},{}Z\" stroke-linejoin=\"round\"{fill}/>",
                num(x),
                num(y),
                num(mx + nx * half),
                num(my + ny * half),
                num(x - dx * DIAMOND),
                num(y - dy * DIAMOND),
                num(mx - nx * half),
                num(my - ny * half)
            )
        }
        EndMarker::Lollipop => format!(
            "<circle class=\"pele-marker\" cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
            num(x - dx * RING),
            num(y - dy * RING),
            num(RING)
        ),
        EndMarker::Dependency => marker("arrow_point", x, y, dx, dy, color),
        EndMarker::None => String::new(),
    }
}
