//! Cluster (subgraph) titles that keep clear of the edges entering through the top border.
//! Ported from pele's `svg/cluster.ts`.

use std::collections::HashMap;

use super::{label_svg, num};
use crate::layout::compound::CNode;
use crate::text::label::Label;
use crate::util::round;

/// The x of every route point, by its rounded y: the edges that cross a cluster's top border.
pub(crate) type Crossings = HashMap<i64, Vec<f64>>;

pub(crate) fn mark_crossings(crossings: &mut Crossings, route: &[f64]) {
    for p in route.chunks(3) {
        crossings.entry(round(p[1]) as i64).or_default().push(p[0]);
    }
}

/// Picks where a cluster title sits along the top edge so that edges entering there do not
/// cross it.
pub(crate) fn cluster_title_x(c: &CNode, width: f64, crossings: &Crossings, pad: f64) -> f64 {
    if width == 0.0 {
        return c.x;
    }
    let top = c.y - c.h / 2.0;
    let left = c.x - c.w / 2.0;
    let half = width / 2.0;
    let candidates = [left + pad - 6.0 + half, c.x, left + c.w - pad + 6.0 - half];
    let Some(xs) = crossings.get(&(round(top) as i64)) else {
        return candidates[0];
    };
    let mut best = candidates[0];
    let mut best_gap = -1.0;
    for x in candidates {
        let mut gap = f64::INFINITY;
        for &cx in xs {
            if cx > left && cx < left + c.w {
                gap = gap.min((cx - x).abs() - half);
            }
        }
        if gap >= 6.0 {
            return x;
        }
        if gap > best_gap + 0.5 {
            best_gap = gap;
            best = x;
        }
    }
    best
}

/// Whether an edge that enters at the top runs through a title placed at `x`.
pub(crate) fn title_crossed(c: &CNode, width: f64, x: f64, crossings: &Crossings) -> bool {
    if width <= 0.0 {
        return false;
    }
    let Some(xs) = crossings.get(&(round(c.y - c.h / 2.0) as i64)) else {
        return false;
    };
    let left = c.x - c.w / 2.0;
    xs.iter()
        .any(|&cx| cx > left && cx < left + c.w && (cx - x).abs() < width / 2.0 + 4.0)
}

/// A title that an edge runs through, drawn over the edges on an outline in the color of the
/// cluster behind it. `depth` is how many clusters deep the title is.
pub(crate) fn struck_title(
    label: &Label,
    x: f64,
    y: f64,
    attrs: &str,
    id: &str,
    depth: u32,
) -> String {
    let fill = format!(
        "color-mix(in srgb,var(--_a) {}%,var(--_bg))",
        num(100.0 * (1.0 - 0.5f64.powi(depth.min(8) as i32)))
    );
    format!(
        "<g class=\"pele-rs-cluster-title\" data-id=\"{id}\">{}{}</g>",
        label_svg(
            label,
            x,
            y,
            &format!(
                " fill=\"none\" stroke=\"{fill}\" stroke-width=\"5\" stroke-linejoin=\"round\""
            )
        ),
        label_svg(label, x, y, attrs)
    )
}
