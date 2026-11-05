//! The layout of a venn diagram: one circle per set, as far apart as their overlaps ask, and
//! the roomiest point of every region. The layout is pele's own, ported from its
//! `diagrams/venn/layout.ts`; Mermaid draws its circles with another library.

use std::collections::{HashMap, HashSet};

use super::db::{VennSubset, js_cmp};

/// A circle of the diagram.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Circle {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub r: f64,
}

/// Where the content of a region goes: the point inside all of `inside` and outside every other
/// circle that is farthest from their outlines, and how far that is. A margin of 0 or less means
/// the circles as placed leave the region empty.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Spot {
    pub x: f64,
    pub y: f64,
    pub margin: f64,
}

const TAU: f64 = std::f64::consts::PI * 2.0;

/// For every overlap of three or more sets, adds the pairwise overlaps it implies and the diagram
/// does not state, a quarter of the smaller set each, so that the circles are drawn overlapping.
/// Returns the list it was given when there is nothing to add.
pub(crate) fn ensure_pairwise_subsets(subsets: &[VennSubset]) -> Vec<VennSubset> {
    let mut present: HashSet<Vec<String>> = HashSet::new();
    let mut sizes: HashMap<String, f64> = HashMap::new();
    for subset in subsets {
        let mut sets = subset.sets.clone();
        sets.sort_by(|a, b| js_cmp(a, b));
        present.insert(sets);
        if subset.sets.len() == 1 {
            sizes.insert(subset.sets[0].clone(), subset.size);
        }
    }
    let mut added: Vec<VennSubset> = Vec::new();
    for subset in subsets {
        if subset.sets.len() < 3 {
            continue;
        }
        let mut members = subset.sets.clone();
        members.sort_by(|a, b| js_cmp(a, b));
        for i in 0..members.len() - 1 {
            for j in (i + 1)..members.len() {
                let pair = vec![members[i].clone(), members[j].clone()];
                if !present.insert(pair.clone()) {
                    continue;
                }
                let size = match (sizes.get(&pair[0]), sizes.get(&pair[1])) {
                    (Some(a), Some(b)) => a.min(*b) / 4.0,
                    _ => 2.5,
                };
                added.push(VennSubset {
                    sets: pair,
                    size,
                    label: Some(String::new()),
                });
            }
        }
    }
    if added.is_empty() {
        subsets.to_vec()
    } else {
        subsets.iter().cloned().chain(added).collect()
    }
}

/// The area two circles share when their centres are `d` apart.
pub(crate) fn lens_area(r1: f64, r2: f64, d: f64) -> f64 {
    if d >= r1 + r2 {
        return 0.0;
    }
    let small = r1.min(r2);
    if d <= (r1 - r2).abs() {
        return std::f64::consts::PI * small * small;
    }
    let a1 = r1 * r1 * ((d * d + r1 * r1 - r2 * r2) / (2.0 * d * r1)).acos();
    let a2 = r2 * r2 * ((d * d + r2 * r2 - r1 * r1) / (2.0 * d * r2)).acos();
    let a3 = 0.5
        * ((-d + r1 + r2) * (d + r1 - r2) * (d - r1 + r2) * (d + r1 + r2))
            .max(0.0)
            .sqrt();
    a1 + a2 - a3
}

/// How far apart two circles sit for their overlap to have the given area. The area falls as the
/// distance grows, so bisection finds it.
pub(crate) fn distance_for_overlap(r1: f64, r2: f64, area: f64) -> f64 {
    let mut low = (r1 - r2).abs();
    let mut high = r1 + r2;
    if area <= 0.0 || area.is_nan() {
        return high;
    }
    if area >= std::f64::consts::PI * r1.min(r2).powi(2) {
        return low;
    }
    for _ in 0..48 {
        let middle = (low + high) / 2.0;
        if lens_area(r1, r2, middle) > area {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) / 2.0
}

/// How far a point is from the nearest outline it must stay within or without. Gives up, with
/// some value below `floor`, as soon as the point cannot reach `floor`.
fn margin(
    circles: &[Circle],
    inside: &[usize],
    outside: &[usize],
    x: f64,
    y: f64,
    floor: f64,
) -> f64 {
    let mut best = f64::INFINITY;
    for &i in inside {
        let c = &circles[i];
        let dx = x - c.x;
        let dy = y - c.y;
        let room = c.r - (dx * dx + dy * dy).sqrt();
        if room < best {
            if room < floor {
                return room;
            }
            best = room;
        }
    }
    for &i in outside {
        let c = &circles[i];
        let dx = x - c.x;
        let dy = y - c.y;
        let room = (dx * dx + dy * dy).sqrt() - c.r;
        if room < best {
            if room < floor {
                return room;
            }
            best = room;
        }
    }
    best
}

/// The roomiest point found so far, and how far it is from the middle of the region.
struct Search {
    x: f64,
    y: f64,
    margin: f64,
    off: f64,
}

/// Finds the roomiest point of a region by sampling the smallest of its circles on a grid and then
/// walking uphill from the best sample. Ties go to the point nearest the middle of the region, so
/// a symmetric arrangement gets symmetric labels.
pub(crate) fn find_spot(circles: &[Circle], inside: &[usize], outside: &[usize]) -> Spot {
    let Some(&first) = inside.first() else {
        return Spot {
            x: 0.0,
            y: 0.0,
            margin: 0.0,
        };
    };
    let mut cx = 0.0f64;
    let mut cy = 0.0f64;
    let mut smallest = &circles[first];
    for &i in inside {
        let c = &circles[i];
        cx += c.x / inside.len() as f64;
        cy += c.y / inside.len() as f64;
        if c.r < smallest.r {
            smallest = c;
        }
    }
    let r = smallest.r;
    let mut search = Search {
        x: cx,
        y: cy,
        margin: margin(circles, inside, outside, cx, cy, f64::NEG_INFINITY),
        off: 0.0,
    };
    {
        let consider = |px: f64, py: f64, s: &mut Search| -> bool {
            let m = margin(circles, inside, outside, px, py, s.margin - r * 1e-9);
            if m <= s.margin - r * 1e-9 || m.is_nan() {
                return false;
            }
            let distance = (px - cx).hypot(py - cy);
            if m > s.margin + r * 1e-9 || distance < s.off - r * 1e-9 {
                s.margin = m;
                s.off = distance;
                s.x = px;
                s.y = py;
                return true;
            }
            false
        };
        const GRID: f64 = 10.0;
        let mut gy = -GRID;
        while gy <= GRID {
            let mut gx = -GRID;
            while gx <= GRID {
                if gx * gx + gy * gy <= GRID * GRID {
                    consider(
                        smallest.x + (gx * r) / GRID,
                        smallest.y + (gy * r) / GRID,
                        &mut search,
                    );
                }
                gx += 1.0;
            }
            gy += 1.0;
        }
        let mut step = r / GRID;
        for _ in 0..40 {
            if step <= r * 1e-6 {
                break;
            }
            let mut moved = false;
            for k in 0..8 {
                let angle = (k as f64 * TAU) / 8.0;
                let px = search.x + angle.cos() * step;
                let py = search.y + angle.sin() * step;
                if consider(px, py, &mut search) {
                    moved = true;
                }
            }
            if !moved {
                step /= 2.0;
            }
        }
    }
    Spot {
        x: search.x,
        y: search.y,
        margin: search.margin,
    }
}

/// Places one circle per set, with areas in proportion to the sizes of the sets and each pair as
/// far apart as its overlap asks: one, two and three sets exactly, more by relaxing a ring of
/// circles toward those distances. Lengths are in units of the square root of a size.
pub(crate) fn layout_circles(subsets: &[VennSubset]) -> Vec<Circle> {
    // The ids of the sets, in the order they are first named, and their sizes.
    let mut ids: Vec<String> = Vec::new();
    let mut sizes: HashMap<String, f64> = HashMap::new();
    for subset in subsets {
        for id in &subset.sets {
            if !sizes.contains_key(id) {
                sizes.insert(id.clone(), 10.0);
                ids.push(id.clone());
            }
        }
    }
    for subset in subsets {
        if subset.sets.len() == 1 {
            sizes.insert(subset.sets[0].clone(), subset.size);
        }
    }
    let mut largest = 0.0;
    for size in sizes.values() {
        if size.is_finite() && *size > largest {
            largest = *size;
        }
    }
    if largest == 0.0 {
        largest = 10.0;
    }
    let widest = (largest / std::f64::consts::PI).sqrt();

    let mut index: HashMap<String, usize> = HashMap::new();
    let mut circles: Vec<Circle> = Vec::new();
    for id in &ids {
        index.insert(id.clone(), circles.len());
        let size = sizes[id];
        // A set too small to hold its name, or with no size at all, still gets a circle that shows.
        let r = if size.is_finite() && size > 0.0 {
            (size / std::f64::consts::PI).sqrt()
        } else {
            0.0
        };
        circles.push(Circle {
            id: id.clone(),
            x: 0.0,
            y: 0.0,
            r: r.max(widest * 0.3),
        });
    }
    let n = circles.len();
    if n < 2 {
        return circles;
    }

    // The overlap each pair was given, by the indexes of its circles.
    let mut overlaps: HashMap<usize, f64> = HashMap::new();
    for subset in ensure_pairwise_subsets(subsets) {
        if subset.sets.len() != 2 {
            continue;
        }
        let (Some(a), Some(b)) = (
            index.get(&subset.sets[0]).copied(),
            index.get(&subset.sets[1]).copied(),
        ) else {
            continue;
        };
        if a != b {
            overlaps.insert(a.min(b) * n + a.max(b), subset.size);
        }
    }
    // How far apart each overlapping pair should be. Pairs that do not overlap only have to keep a
    // gap.
    let mut wanted: HashMap<usize, f64> = HashMap::new();
    for (&key, &area) in &overlaps {
        let a = key / n;
        let b = key % n;
        wanted.insert(key, distance_for_overlap(circles[a].r, circles[b].r, area));
    }
    let gap = widest * 0.15;
    let distance = |a: usize, b: usize| -> f64 {
        wanted
            .get(&(a * n + b))
            .copied()
            .unwrap_or(circles[a].r + circles[b].r + gap)
    };

    if n == 2 {
        circles[1].x = distance(0, 1);
        return circles;
    }

    if n == 3 {
        // Two sets side by side and the third below, the corners of a triangle whose sides are the
        // three distances. An overlap of all three has to exist, so the triangle shrinks until it
        // does.
        let mut ab = distance(0, 1);
        let mut ac = distance(0, 2);
        let mut bc = distance(1, 2);
        let triple = subsets
            .iter()
            .any(|subset| subset.sets.iter().collect::<HashSet<_>>().len() == 3);
        for _ in 0..40 {
            let x = if ab > 0.0 {
                (ab * ab + ac * ac - bc * bc) / (2.0 * ab)
            } else {
                0.0
            };
            circles[1].x = ab;
            circles[2].x = (-ac).max(ac.min(x));
            circles[2].y = (ac * ac - x * x).max(0.0).sqrt();
            if !triple || find_spot(&circles, &[0, 1, 2], &[]).margin > widest * 0.12 {
                break;
            }
            ab *= 0.94;
            ac *= 0.94;
            bc *= 0.94;
        }
        return circles;
    }

    let mut total = 0.0;
    for c in &circles {
        total += 2.0 * c.r + gap;
    }
    let rounds = 300.min((3e6 / (n * n) as f64).floor() as usize);
    if n <= 64 {
        let ring = total / TAU;
        for (i, c) in circles.iter_mut().enumerate() {
            let angle = (i as f64 * TAU) / n as f64 - std::f64::consts::PI / 2.0;
            c.x = angle.cos() * ring;
            c.y = angle.sin() * ring;
        }
    } else {
        let columns = (n as f64).sqrt().ceil() as usize;
        let cell = 2.0 * widest + gap;
        for (i, c) in circles.iter_mut().enumerate() {
            c.x = (i % columns) as f64 * cell;
            c.y = (i / columns) as f64 * cell;
        }
    }
    let mut dx = vec![0.0; n];
    let mut dy = vec![0.0; n];
    let mut pulls = vec![0usize; n];
    for _ in 0..rounds {
        dx.fill(0.0);
        dy.fill(0.0);
        pulls.fill(0);
        for a in 0..n {
            for b in (a + 1)..n {
                let overlap = wanted.get(&(a * n + b)).copied();
                let apart = circles[a].r + circles[b].r + gap;
                let mut ux = circles[b].x - circles[a].x;
                let mut uy = circles[b].y - circles[a].y;
                let mut length = ux.hypot(uy);
                if overlap.is_none() && length >= apart {
                    continue;
                }
                if length < 1e-9 {
                    ux = (a as f64 + b as f64).cos();
                    uy = (a as f64 + b as f64).sin();
                    length = 1.0;
                }
                let target = overlap.unwrap_or(apart);
                let move_by = (length - target) / 2.0 / length;
                dx[a] += ux * move_by;
                dy[a] += uy * move_by;
                dx[b] -= ux * move_by;
                dy[b] -= uy * move_by;
                pulls[a] += 1;
                pulls[b] += 1;
            }
        }
        for i in 0..n {
            if pulls[i] == 0 {
                continue;
            }
            circles[i].x += dx[i] / pulls[i] as f64;
            circles[i].y += dy[i] / pulls[i] as f64;
        }
    }
    circles
}

/// A corner of a region: where two outlines cross.
struct Corner {
    x: f64,
    y: f64,
    /// The two circles whose outlines cross there.
    a: usize,
    b: usize,
}

/// Whether a point is within every circle but the two that are skipped.
fn within(circles: &[Circle], x: f64, y: f64, skip_a: usize, skip_b: usize) -> bool {
    for (i, c) in circles.iter().enumerate() {
        if i == skip_a || i == skip_b {
            continue;
        }
        if (x - c.x).hypot(y - c.y) > c.r * (1.0 + 1e-9) {
            return false;
        }
    }
    true
}

/// The outline of the region all the given circles share, as an SVG path, or nothing if they
/// share none.
pub(crate) fn region_path<F: Fn(f64) -> String>(circles: &[Circle], format: F) -> String {
    let whole = |c: &Circle| -> String {
        format!(
            "M{},{}A{},{} 0 1 1 {},{}A{},{} 0 1 1 {},{}Z",
            format(c.x - c.r),
            format(c.y),
            format(c.r),
            format(c.r),
            format(c.x + c.r),
            format(c.y),
            format(c.r),
            format(c.r),
            format(c.x - c.r),
            format(c.y)
        )
    };
    if circles.len() == 1 {
        return whole(&circles[0]);
    }
    // The corners of the region: where two outlines cross inside all the other circles.
    let mut corners: Vec<Corner> = Vec::new();
    for i in 0..circles.len() {
        for j in (i + 1)..circles.len() {
            let a = &circles[i];
            let b = &circles[j];
            let d = (b.x - a.x).hypot(b.y - a.y);
            if d >= a.r + b.r || d <= (a.r - b.r).abs() || d == 0.0 {
                continue;
            }
            let along = (a.r * a.r - b.r * b.r + d * d) / (2.0 * d);
            let up = (a.r * a.r - along * along).max(0.0).sqrt();
            let mx = a.x + (b.x - a.x) * along / d;
            let my = a.y + (b.y - a.y) * along / d;
            for sign in [1.0, -1.0] {
                let x = mx + sign * up * (b.y - a.y) / d;
                let y = my - sign * up * (b.x - a.x) / d;
                if within(circles, x, y, i, j) {
                    corners.push(Corner { x, y, a: i, b: j });
                }
            }
        }
    }
    if corners.len() < 2 {
        // No corners: either one circle lies inside all the others, or there is nothing shared.
        let mut smallest_at = 0;
        for (i, c) in circles.iter().enumerate() {
            if c.r < circles[smallest_at].r {
                smallest_at = i;
            }
        }
        let smallest = &circles[smallest_at];
        for (i, c) in circles.iter().enumerate() {
            if i == smallest_at {
                continue;
            }
            if (c.x - smallest.x).hypot(c.y - smallest.y) + smallest.r > c.r * (1.0 + 1e-9) {
                return String::new();
            }
        }
        return whole(smallest);
    }
    let mut cx = 0.0;
    let mut cy = 0.0;
    for p in &corners {
        cx += p.x / corners.len() as f64;
        cy += p.y / corners.len() as f64;
    }
    corners.sort_by(|p, q| {
        let a = (p.y - cy).atan2(p.x - cx);
        let b = (q.y - cy).atan2(q.x - cx);
        a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut d = String::new();
    for i in 0..corners.len() {
        let from = &corners[i];
        let to = &corners[(i + 1) % corners.len()];
        if i == 0 {
            d = format!("M{},{}", format(from.x), format(from.y));
        }
        // The side between two corners is an arc of a circle that passes through both; of those,
        // the one whose arc stays inside the others.
        let mut arc: Option<usize> = None;
        let mut large = 0;
        let mut narrowest = f64::INFINITY;
        for &c in &[from.a, from.b] {
            if c != to.a && c != to.b {
                continue;
            }
            let circle = &circles[c];
            let start = (from.y - circle.y).atan2(from.x - circle.x);
            let mut sweep = (to.y - circle.y).atan2(to.x - circle.x) - start;
            if sweep <= 0.0 {
                sweep += TAU;
            }
            let mx = circle.x + circle.r * (start + sweep / 2.0).cos();
            let my = circle.y + circle.r * (start + sweep / 2.0).sin();
            let width = (mx - (from.x + to.x) / 2.0).hypot(my - (from.y + to.y) / 2.0);
            if width < narrowest {
                narrowest = width;
                arc = Some(c);
                large = i32::from(sweep > std::f64::consts::PI);
            }
        }
        match arc {
            Some(c) => {
                let circle = &circles[c];
                d += &format!(
                    "A{},{} 0 {} 1 {},{}",
                    format(circle.r),
                    format(circle.r),
                    large,
                    format(to.x),
                    format(to.y)
                );
            }
            None => d += &format!("L{},{}", format(to.x), format(to.y)),
        }
    }
    d + "Z"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subset(sets: &[&str], size: f64) -> VennSubset {
        VennSubset {
            sets: sets.iter().map(|s| s.to_string()).collect(),
            size,
            label: None,
        }
    }

    #[test]
    fn adds_the_pairs_of_a_three_set_overlap() {
        let subsets = vec![subset(&["A", "B", "C"], 10.0)];
        let out = ensure_pairwise_subsets(&subsets);
        assert_eq!(out.len(), 4);
        assert_eq!(out[1].sets, ["A", "B"]);
        assert_eq!(out[1].size, 2.5);
    }

    #[test]
    fn leaves_a_two_set_overlap_alone() {
        let subsets = vec![subset(&["A", "B"], 10.0)];
        assert_eq!(ensure_pairwise_subsets(&subsets).len(), 1);
    }

    #[test]
    fn places_two_circles_apart() {
        let circles = layout_circles(&[subset(&["A"], 10.0), subset(&["B"], 10.0)]);
        assert_eq!(circles.len(), 2);
        assert!(circles[1].x > 0.0);
    }

    #[test]
    fn measures_the_area_of_a_lens() {
        // Two circles of radius 1 whose centres are one apart share about two fifths of a
        // circle.
        let area = lens_area(1.0, 1.0, 1.0);
        assert!(area > 1.2 && area < 1.3, "{area}");
    }
}
