//! Drawing an architecture diagram.
//!
//! Mermaid draws an icon for every service, with its title beneath it. The icon is a square;
//! what is drawn in it is the host's icon of that name, or one of the five that are built in
//! (`database`, `server`, `disk`, `internet` and `cloud`), or the text of the service when it
//! was given as one. An icon nothing draws leaves the square empty.
//!
//! The grid the services are placed in is pele's; the drawing follows it.

use crate::diagram::{Input, Output};
use crate::diagrams::architecture::db::{ArchitectureModel, NodeKind};
use crate::diagrams::architecture::icons::builtin_icon;
use crate::diagrams::architecture::layout::place_on_grid;
use crate::diagrams::architecture::parser::Side;
use crate::diagrams::common::fit::{tighten, title_room};
use crate::error::Error;
use crate::preprocess::{Config, Value};
use crate::svg::{Accessible, ROUNDED, esc_text, icon_svg, label_svg, num, svg_document};
use crate::text::label::{Label, Span, layout_label};
use crate::text::{Style, TextMeasurer, icon_markup};

/// The side of the square of an icon.
const ICON: f64 = 48.0;
/// Between the square of a service and its title.
const LABEL_GAP: f64 = 4.0;
/// How far a line leaves a node before it turns.
const STUB: f64 = 12.0;
/// The radius of the corner of a bend.
const CORNER: f64 = 6.0;
/// The radius of the dot of a junction.
const DOT: f64 = 3.0;
/// The length of an arrowhead.
const ARROW: f64 = 8.0;

const DX: [f64; 4] = [-1.0, 1.0, 0.0, 0.0];
const DY: [f64; 4] = [0.0, 0.0, -1.0, 1.0];

/// A value of the settings of the diagram.
fn setting(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    let section = match config.get("architecture") {
        Some(Value::Map(map)) => map,
        _ => return fallback,
    };
    match section.get(key) {
        Some(Value::Number(n)) if n.is_finite() => n.max(min).min(max),
        _ => fallback,
    }
}

/// Maps the grid lines in use to consecutive indexes, so empty rows and columns take no room.
fn rank(values: &[i32]) -> Vec<(i32, usize)> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let mut out: Vec<(i32, usize)> = Vec::new();
    for value in sorted {
        if out.last().is_none_or(|&(last, _)| last != value) {
            out.push((value, out.len()));
        }
    }
    out
}

/// The index of a grid line.
fn rank_of(ranked: &[(i32, usize)], value: i32) -> usize {
    ranked
        .iter()
        .find(|(line, _)| *line == value)
        .map_or(0, |&(_, index)| index)
}

/// A place an edge leaves from or enters at.
struct Port {
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    /// The box the route has to stay out of when it cannot leave straight.
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    /// How far short of the port an arrowhead stops: the radius of a junction's dot.
    gap: f64,
    /// Set where a junction is reached through the border of its group: the line carries on to the
    /// junction.
    wire: bool,
}

/// An orthogonal route from one port to another, each left along its side's normal.
fn route(s: &Port, e: &Port) -> Vec<f64> {
    let (sx, sy, sdx, sdy) = (s.x, s.y, s.dx, s.dy);
    let (ex, ey, edx, edy) = (e.x, e.y, e.dx, e.dy);
    let (s1x, s1y) = (sx + sdx * STUB, sy + sdy * STUB);
    let (e1x, e1y) = (ex + edx * STUB, ey + edy * STUB);
    if sdx * edx + sdy * edy == 0.0 {
        let cx = if sdx != 0.0 { ex } else { sx };
        let cy = if sdx != 0.0 { sy } else { ey };
        if (cx - sx) * sdx + (cy - sy) * sdy > 0.0 && (cx - ex) * edx + (cy - ey) * edy > 0.0 {
            return vec![sx, sy, cx, cy, ex, ey];
        }
        return if sdx != 0.0 {
            vec![sx, sy, s1x, s1y, s1x, e1y, e1x, e1y, ex, ey]
        } else {
            vec![sx, sy, s1x, s1y, e1x, s1y, e1x, e1y, ex, ey]
        };
    }
    let sideways = sdx != 0.0;
    let offset = if sideways { ey - sy } else { ex - sx };
    if sdx == -edx && sdy == -edy {
        let ahead = (ex - sx) * sdx + (ey - sy) * sdy;
        if ahead > 0.0 {
            if offset.abs() < 0.5 {
                return vec![sx, sy, ex, ey];
            }
            // Turn midway between the two boxes, which is clear of both titles, when there is
            // such a place.
            if sideways {
                let lo = if sdx > 0.0 { s.x1 } else { e.x1 };
                let hi = if sdx > 0.0 { e.x0 } else { s.x0 };
                let mx = if lo < hi {
                    (lo + hi) / 2.0
                } else {
                    (sx + ex) / 2.0
                };
                return vec![sx, sy, mx, sy, mx, ey, ex, ey];
            }
            let lo = if sdy > 0.0 { s.y1 } else { e.y1 };
            let hi = if sdy > 0.0 { e.y0 } else { s.y0 };
            let my = if lo < hi {
                (lo + hi) / 2.0
            } else {
                (sy + ey) / 2.0
            };
            return vec![sx, sy, sx, my, ex, my, ex, ey];
        }
    } else if offset.abs() >= 1.0 {
        if sideways {
            let x = if sdx > 0.0 {
                sx.max(ex) + STUB
            } else {
                sx.min(ex) - STUB
            };
            return vec![sx, sy, x, sy, x, ey, ex, ey];
        }
        let y = if sdy > 0.0 {
            sy.max(ey) + STUB
        } else {
            sy.min(ey) - STUB
        };
        return vec![sx, sy, sx, y, ex, y, ex, ey];
    }
    // The straight way is blocked by one of the two nodes: go between them, or around both.
    if sideways {
        let lane = if s.y1 + STUB <= e.y0 {
            (s.y1 + e.y0) / 2.0
        } else if e.y1 + STUB <= s.y0 {
            (e.y1 + s.y0) / 2.0
        } else {
            s.y1.max(e.y1) + STUB
        };
        return vec![sx, sy, s1x, sy, s1x, lane, e1x, lane, e1x, ey, ex, ey];
    }
    let lane = if s.x1 + STUB <= e.x0 {
        (s.x1 + e.x0) / 2.0
    } else if e.x1 + STUB <= s.x0 {
        (e.x1 + s.x0) / 2.0
    } else {
        s.x1.max(e.x1) + STUB
    };
    vec![sx, sy, sx, s1y, lane, s1y, lane, e1y, ex, e1y, ex, ey]
}

/// Drops repeated points and joins runs that continue in a straight line.
fn simplify(p: &[f64]) -> Vec<f64> {
    let mut out = vec![p[0], p[1]];
    let mut i = 2;
    while i < p.len() {
        let (x, y) = (p[i], p[i + 1]);
        let n = out.len();
        let same_x = (x - out[n - 2]).abs() < 0.01;
        let same_y = (y - out[n - 1]).abs() < 0.01;
        if same_x && same_y {
            i += 2;
            continue;
        }
        if n >= 4
            && ((same_x && (out[n - 4] - out[n - 2]).abs() < 0.01)
                || (same_y && (out[n - 3] - out[n - 1]).abs() < 0.01))
        {
            out[n - 2] = x;
            out[n - 1] = y;
        } else {
            out.push(x);
            out.push(y);
        }
        i += 2;
    }
    out
}

/// The path of a route, with its corners rounded.
fn path_data(p: &[f64], start_trim: f64, end_trim: f64) -> String {
    let n = p.len() / 2;
    let mut xs: Vec<f64> = p.iter().step_by(2).copied().collect();
    let mut ys: Vec<f64> = p.iter().skip(1).step_by(2).copied().collect();
    let trim = |xs: &mut Vec<f64>, ys: &mut Vec<f64>, at: usize, toward: usize, amount: f64| {
        let len = (xs[toward] - xs[at]).hypot(ys[toward] - ys[at]);
        let t = amount.min((len - 1.0).max(0.0));
        if len > 0.0 && t > 0.0 {
            xs[at] += ((xs[toward] - xs[at]) / len) * t;
            ys[at] += ((ys[toward] - ys[at]) / len) * t;
        }
    };
    trim(&mut xs, &mut ys, 0, 1, start_trim);
    trim(&mut xs, &mut ys, n - 1, n - 2, end_trim);
    let mut d = format!("M{},{}", num(xs[0]), num(ys[0]));
    for i in 1..n - 1 {
        let before = (xs[i] - xs[i - 1]).hypot(ys[i] - ys[i - 1]);
        let after = (xs[i + 1] - xs[i]).hypot(ys[i + 1] - ys[i]);
        let r = CORNER.min(before / 2.0).min(after / 2.0);
        if r < 0.5 {
            d += &format!("L{},{}", num(xs[i]), num(ys[i]));
            continue;
        }
        let in_x = xs[i] - ((xs[i] - xs[i - 1]) / before) * r;
        let in_y = ys[i] - ((ys[i] - ys[i - 1]) / before) * r;
        let out_x = xs[i] + ((xs[i + 1] - xs[i]) / after) * r;
        let out_y = ys[i] + ((ys[i + 1] - ys[i]) / after) * r;
        d += &format!(
            "L{},{}Q{},{} {},{}",
            num(in_x),
            num(in_y),
            num(xs[i]),
            num(ys[i]),
            num(out_x),
            num(out_y)
        );
    }
    d += &format!("L{},{}", num(xs[n - 1]), num(ys[n - 1]));
    d
}

/// The length of an arrowhead, and how far the line stops short of its end.
const TRIM: f64 = ARROW - 1.0;

/// An arrowhead whose tip is at (x, y), pointing along (dx, dy).
fn marker(x: f64, y: f64, dx: f64, dy: f64, color: &str) -> String {
    let nx = -dy;
    let ny = dx;
    let bx = x - dx * ARROW;
    let by = y - dy * ARROW;
    let half = ARROW * 0.42;
    format!(
        "<path class=\"pele-marker\" d=\"M{},{}L{},{}L{},{}Z\" fill=\"{color}\" stroke=\"none\"/>",
        num(x),
        num(y),
        num(bx + nx * half),
        num(by + ny * half),
        num(bx - nx * half),
        num(by - ny * half)
    )
}

/// An edge label on a patch of the background, so the line does not run through the text.
fn edge_label_svg(id: &str, label: &Label, x: f64, y: f64) -> String {
    let w = label.width + 8.0;
    let h = label.height;
    format!(
        "<g class=\"pele-edge-label\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" \
         height=\"{}\" rx=\"3\" fill=\"var(--_bg)\"/>{}</g>",
        esc_text(id),
        num(x - w / 2.0),
        num(y - h / 2.0),
        num(w),
        num(h),
        label_svg(label, x, y, "")
    )
}

/// A square for an icon: the host's drawing of it, or one of the built-in ones, or nothing.
fn icon_slot(measurer: &dyn TextMeasurer, name: &str, x: f64, y: f64, side: f64) -> String {
    let markup = icon_markup(measurer, name)
        .map(|markup| markup.to_string())
        .or_else(|| builtin_icon(name))
        .unwrap_or_default();
    icon_svg(name, x, y, side, &markup)
}

/// Cuts a label down to a box, ending what is left with an ellipsis.
fn fit(label: &Label, max_width: f64, max_lines: usize, measurer: &dyn TextMeasurer) -> Label {
    let cut = label.lines.len() > max_lines;
    if !cut && label.width <= max_width {
        return label.clone();
    }
    let dots = measurer.width("…", label.size, Style::NONE);
    let mut lines: Vec<Vec<Span>> = Vec::new();
    let mut widths: Vec<f64> = Vec::new();
    let mut widest = 0.0f64;
    for i in 0..max_lines.min(label.lines.len()) {
        let mut line = label.lines[i].clone();
        let mut width = label.widths[i];
        if width > max_width || (cut && i == max_lines - 1) {
            loop {
                let Some(span) = line.last_mut() else {
                    line.push(Span::text("…".to_string(), Style::NONE, dots));
                    width = dots;
                    break;
                };
                let rest = width - span.width;
                if span.is_plain() && rest + dots < max_width {
                    // The most characters that fit with the ellipsis after them.
                    let chars: Vec<char> = span.text.chars().collect();
                    let (mut lo, mut hi) = (0usize, chars.len());
                    while lo < hi {
                        let mid = (lo + hi).div_ceil(2);
                        let head: String = chars[..mid].iter().collect();
                        if rest + measurer.width(&head, label.size, span.style) + dots <= max_width
                        {
                            lo = mid;
                        } else {
                            hi = mid - 1;
                        }
                    }
                    let head: String = chars[..lo].iter().collect();
                    span.text = format!("{}…", crate::util::trim_end(&head));
                    span.width = measurer.width(&span.text, label.size, span.style);
                    width = rest + span.width;
                    break;
                }
                line.pop();
                width = rest;
            }
        }
        lines.push(line);
        widths.push(width);
        widest = widest.max(width);
    }
    let height = lines.len() as f64 * label.line_height;
    Label {
        lines,
        widths,
        width: widest,
        height,
        ..label.clone()
    }
}

/// Draws the diagram.
pub(crate) fn render_architecture(
    model: &ArchitectureModel,
    input: &Input<'_>,
) -> Result<Output, Error> {
    let options = input.options;
    Ok(tighten(options, |tight| draw(model, input, tight)))
}

/// The place of everything of the diagram.
struct View {
    /// The label of each node.
    labels: Vec<Label>,
    /// The size of each node: its icon, and the title beneath it.
    box_w: Vec<f64>,
    box_h: Vec<f64>,
    /// The column and the row of each node, and the width and the height of each of them.
    node_col: Vec<usize>,
    node_row: Vec<usize>,
    col_w: Vec<f64>,
    row_h: Vec<f64>,
    /// The place of each column and row.
    x: Vec<f64>,
    y: Vec<f64>,
    /// The room around each group.
    ins_l: Vec<f64>,
    ins_r: Vec<f64>,
    ins_t: Vec<f64>,
    ins_b: Vec<f64>,
    head_w: Vec<f64>,
    head_h: Vec<f64>,
    /// The label of each group.
    group_labels: Vec<Label>,
    /// The columns and the rows each group holds.
    c0: Vec<usize>,
    c1: Vec<usize>,
    r0: Vec<usize>,
    r1: Vec<usize>,
    /// The label of each edge.
    edge_labels: Vec<Label>,
}

fn draw(model: &ArchitectureModel, input: &Input<'_>, tight: bool) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let icon_size = setting(input.config, "iconSize", ICON, 8.0, 512.0);
    let size = setting(
        input.config,
        "fontSize",
        options.font_size.unwrap_or(16.0),
        4.0,
        128.0,
    );
    let group_pad = setting(input.config, "padding", 20.0, 0.0, 256.0).min(if tight {
        12.0
    } else {
        f64::INFINITY
    });
    let small = (size * 0.875).round();
    let pad = options.padding.unwrap_or(8.0);

    let nodes: Vec<_> = model.nodes.values().collect();
    let groups: Vec<_> = model.groups.values().collect();
    let n = nodes.len();
    let g = groups.len();
    let grid = place_on_grid(model);

    // A service is its icon square with the title beneath; a junction takes the same room.
    let wrap = if tight {
        icon_size.max(64.0)
    } else {
        (icon_size * 2.25).max(96.0)
    };
    let mut labels: Vec<Label> = Vec::with_capacity(n);
    let mut box_w: Vec<f64> = Vec::with_capacity(n);
    let mut box_h: Vec<f64> = Vec::with_capacity(n);
    for node in &nodes {
        let title = if node.kind == NodeKind::Service {
            node.title.as_deref()
        } else {
            None
        };
        let label = layout_label(title, true, measurer, size, wrap, Style::NONE);
        box_w.push(icon_size.max(2.0 * (label.width / 2.0).ceil()));
        box_h.push(
            icon_size
                + if label.height > 0.0 {
                    LABEL_GAP + label.height
                } else {
                    0.0
                },
        );
        labels.push(label);
    }

    let mut col_values: Vec<i32> = (0..n).map(|i| grid.col[i]).collect();
    let mut row_values: Vec<i32> = (0..n).map(|i| grid.row[i]).collect();
    for gi in 0..g {
        if !grid.group_empty[gi] {
            continue;
        }
        col_values.push(grid.group_col[gi]);
        row_values.push(grid.group_row[gi]);
    }
    let col_of = rank(&col_values);
    let row_of = rank(&row_values);
    let c = col_of.len();
    let r = row_of.len();
    let mut col_w = vec![0.0_f64; c];
    let mut row_h = vec![0.0_f64; r];
    let gap = if tight {
        (2.0 * STUB).max(24.0)
    } else {
        (icon_size * 0.75).round().max(40.0)
    };
    let mut gap_x = vec![gap; c];
    let mut gap_y = vec![gap; r];
    let mut node_col = vec![0_usize; n];
    let mut node_row = vec![0_usize; n];
    for i in 0..n {
        let col = rank_of(&col_of, grid.col[i]);
        let row = rank_of(&row_of, grid.row[i]);
        node_col[i] = col;
        node_row[i] = row;
        col_w[col] = col_w[col].max(box_w[i]);
        row_h[row] = row_h[row].max(box_h[i]);
    }
    for gi in 0..g {
        if !grid.group_empty[gi] {
            continue;
        }
        let col = rank_of(&col_of, grid.group_col[gi]);
        let row = rank_of(&row_of, grid.group_row[gi]);
        col_w[col] = col_w[col].max(icon_size);
        row_h[row] = row_h[row].max(icon_size / 2.0);
    }

    // An edge label needs the gap its edge crosses to be wide enough to hold it.
    let e = model.edges.len();
    let mut edge_labels: Vec<Label> = Vec::with_capacity(e);
    let mut ends: Vec<i32> = vec![-1; e * 2];
    let index: std::collections::HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();
    for (k, edge) in model.edges.iter().enumerate() {
        let a = index
            .get(edge.lhs_id.as_str())
            .copied()
            .map_or(-1, |i| i as i32);
        let b = index
            .get(edge.rhs_id.as_str())
            .copied()
            .map_or(-1, |i| i as i32);
        ends[k * 2] = a;
        ends[k * 2 + 1] = b;
        let label = layout_label(
            edge.title.as_deref(),
            true,
            measurer,
            small,
            160.0,
            Style::NONE,
        );
        edge_labels.push(label);
        let label = &edge_labels[k];
        if label.width == 0.0 || a < 0 || b < 0 {
            continue;
        }
        let (a, b) = (a as usize, b as usize);
        if node_col[a] != node_col[b] {
            let col = node_col[a].min(node_col[b]);
            gap_x[col] = gap_x[col].max(label.width.ceil() + 28.0);
        } else {
            col_w[node_col[a]] = col_w[node_col[a]].max(2.0 * (label.width / 2.0).ceil() + 8.0);
            if node_row[a] != node_row[b] {
                let row = node_row[a].min(node_row[b]);
                gap_y[row] = gap_y[row].max(label.height.ceil() + 24.0);
            }
        }
    }

    // Room around each group, kept on the rows and columns at its edges. A group that shares an
    // edge with a group inside it stands that much further out. Children come after their parents,
    // so walking backwards settles every group before the one that holds it.
    let mut group_labels: Vec<Label> = Vec::with_capacity(g);
    let mut c0 = vec![0_usize; g];
    let mut c1 = vec![0_usize; g];
    let mut r0 = vec![0_usize; g];
    let mut r1 = vec![0_usize; g];
    let mut ins_l = vec![0.0_f64; g];
    let mut ins_r = vec![0.0_f64; g];
    let mut ins_t = vec![0.0_f64; g];
    let mut ins_b = vec![0.0_f64; g];
    let mut head_w = vec![0.0_f64; g];
    let mut head_h = vec![0.0_f64; g];
    let mut col_l = vec![0.0_f64; c];
    let mut col_r = vec![0.0_f64; c];
    let mut row_t = vec![0.0_f64; r];
    let mut row_b = vec![0.0_f64; r];
    let badge = (size * 1.25).round();
    for gi in 0..g {
        c0[gi] = rank_of(&col_of, grid.group_col[gi]);
        c1[gi] = rank_of(&col_of, grid.group_col[gi] + grid.group_cols[gi] - 1);
        r0[gi] = rank_of(&row_of, grid.group_row[gi]);
        r1[gi] = rank_of(&row_of, grid.group_row[gi] + grid.group_rows[gi] - 1);
        let label = layout_label(
            groups[gi].title.as_deref(),
            true,
            measurer,
            size,
            if tight {
                (title_room(options) / 2.0).max(80.0)
            } else {
                320.0
            },
            Style::NONE,
        );
        let icon = if groups[gi].icon.is_some() {
            badge
        } else {
            0.0
        };
        head_w[gi] = label.width
            + icon
            + if label.width > 0.0 && icon > 0.0 {
                6.0
            } else {
                0.0
            };
        head_h[gi] = label.height.max(icon);
        group_labels.push(label);
    }
    for gi in (0..g).rev() {
        let left = group_pad + ins_l[gi];
        let mut right = group_pad + ins_r[gi];
        let top = (if head_h[gi] > 0.0 {
            group_pad.max(head_h[gi] + 12.0)
        } else {
            group_pad
        }) + ins_t[gi];
        let bottom = group_pad + ins_b[gi];
        if head_w[gi] > 0.0 {
            let mut inner = left + right;
            for col in c0[gi]..=c1[gi] {
                inner += col_w[col];
                if col < c1[gi] {
                    inner += col_r[col] + gap_x[col] + col_l[col + 1];
                }
            }
            if head_w[gi] + 20.0 > inner {
                right += (head_w[gi] + 20.0 - inner).ceil();
            }
        }
        ins_l[gi] = left;
        ins_r[gi] = right;
        ins_t[gi] = top;
        ins_b[gi] = bottom;
        col_l[c0[gi]] = col_l[c0[gi]].max(left);
        col_r[c1[gi]] = col_r[c1[gi]].max(right);
        row_t[r0[gi]] = row_t[r0[gi]].max(top);
        row_b[r1[gi]] = row_b[r1[gi]].max(bottom);
        let p = grid.group_parent[gi];
        if p < 0 {
            continue;
        }
        let p = p as usize;
        if c0[p] == c0[gi] {
            ins_l[p] = ins_l[p].max(left);
        }
        if c1[p] == c1[gi] {
            ins_r[p] = ins_r[p].max(right);
        }
        if r0[p] == r0[gi] {
            ins_t[p] = ins_t[p].max(top);
        }
        if r1[p] == r1[gi] {
            ins_b[p] = ins_b[p].max(bottom);
        }
    }

    let mut x = vec![0.0_f64; c];
    let mut y = vec![0.0_f64; r];
    let mut inner_w = 0.0;
    for col in 0..c {
        x[col] = inner_w + col_l[col];
        inner_w = x[col] + col_w[col] + col_r[col];
        if col < c - 1 {
            inner_w += gap_x[col];
        }
    }
    let mut inner_h = 0.0;
    for row in 0..r {
        y[row] = inner_h + row_t[row];
        inner_h = y[row] + row_h[row] + row_b[row];
        if row < r - 1 {
            inner_h += gap_y[row];
        }
    }
    // What is drawn, which an edge that has to go around a node can push past the grid.
    let mut min_x = 0.0_f64;
    let mut min_y = 0.0_f64;
    let mut max_x = inner_w;
    let mut max_y = inner_h;

    let view = View {
        labels,
        box_w,
        box_h,
        node_col,
        node_row,
        col_w,
        row_h,
        x,
        y,
        ins_l,
        ins_r,
        ins_t,
        ins_b,
        head_w,
        head_h,
        group_labels,
        c0,
        c1,
        r0,
        r1,
        edge_labels,
    };

    let left = |gi: usize| view.x[view.c0[gi]] - view.ins_l[gi];
    let right = |gi: usize| view.x[view.c1[gi]] + view.col_w[view.c1[gi]] + view.ins_r[gi];
    let top = |gi: usize| view.y[view.r0[gi]] - view.ins_t[gi];
    let bottom = |gi: usize| view.y[view.r1[gi]] + view.row_h[view.r1[gi]] + view.ins_b[gi];
    let center_x = |i: usize| view.x[view.node_col[i]] + view.col_w[view.node_col[i]] / 2.0;

    let port = |i: usize, side: Side, via_group: bool| -> Port {
        let side = side.index();
        let (dx, dy) = (DX[side], DY[side]);
        let cx = center_x(i);
        let y = view.y[view.node_row[i]];
        let half = icon_size / 2.0;
        let (mut px, mut py) = (cx, y + half);
        let (mut x0, mut y0) = (cx - view.box_w[i] / 2.0, y);
        let (mut x1, mut y1) = (cx + view.box_w[i] / 2.0, y + view.box_h[i]);
        let mut gap = 0.0;
        let mut wire = false;
        if nodes[i].kind == NodeKind::Junction {
            x0 = cx;
            x1 = cx;
            y0 = py;
            y1 = py;
            gap = DOT;
        } else if dx != 0.0 {
            px = cx + dx * half;
        } else if dy < 0.0 {
            py = y;
        } else {
            py = y
                + view.box_h[i]
                + if view.labels[i].height > 0.0 {
                    2.0
                } else {
                    0.0
                };
        }
        let gi = if via_group { grid.node_parent[i] } else { -1 };
        if gi >= 0 {
            let gi = gi as usize;
            x0 = left(gi);
            x1 = right(gi);
            y0 = top(gi);
            y1 = bottom(gi);
            gap = 0.0;
            wire = nodes[i].kind == NodeKind::Junction;
            if dx != 0.0 {
                px = if dx < 0.0 { x0 } else { x1 };
            } else if dy < 0.0 {
                py = y0;
            } else {
                py = y1;
            }
        }
        Port {
            x: px,
            y: py,
            dx,
            dy,
            x0,
            y0,
            x1,
            y1,
            gap,
            wire,
        }
    };

    let mut edges_out = String::new();
    let mut labels_out = String::new();
    // Vertical runs of the edges, to keep group titles out from under them.
    let mut runs: Vec<f64> = Vec::new();
    let trim_length = TRIM;
    for (k, edge) in model.edges.iter().enumerate() {
        let (a, b) = (ends[k * 2], ends[k * 2 + 1]);
        if a < 0 || b < 0 {
            continue;
        }
        let (a, b) = (a as usize, b as usize);
        let mut s = port(a, edge.lhs_dir, edge.lhs_group);
        let mut e = port(b, edge.rhs_dir, edge.rhs_group);
        if s.x == e.x && s.y == e.y && s.dx == e.dx {
            // An edge from a side back to the same side: part the two ends so it shows as a loop.
            s.x -= 8.0 * s.dy.abs();
            s.y -= 8.0 * s.dx.abs();
            e.x += 8.0 * s.dy.abs();
            e.y += 8.0 * s.dx.abs();
        }
        let mut routed = route(&s, &e);
        if s.wire {
            routed.insert(0, center_x(a));
            routed.insert(1, view.y[view.node_row[a]] + icon_size / 2.0);
        }
        if e.wire {
            routed.push(center_x(b));
            routed.push(view.y[view.node_row[b]] + icon_size / 2.0);
        }
        let points = simplify(&routed);
        if points.len() < 4 {
            continue;
        }
        let id = esc_text(&format!("{}-{}", edge.lhs_id, edge.rhs_id));
        edges_out += &format!(
            "<g class=\"pele-edge\" data-id=\"{id}\"><path d=\"{}\"/>",
            path_data(
                &points,
                if edge.lhs_into && !s.wire {
                    trim_length + s.gap
                } else {
                    0.0
                },
                if edge.rhs_into && !e.wire {
                    trim_length + e.gap
                } else {
                    0.0
                },
            )
        );
        if edge.lhs_into {
            edges_out += &marker(
                s.x + s.dx * s.gap,
                s.y + s.dy * s.gap,
                -s.dx,
                -s.dy,
                "var(--_l)",
            );
        }
        if edge.rhs_into {
            edges_out += &marker(
                e.x + e.dx * e.gap,
                e.y + e.dy * e.gap,
                -e.dx,
                -e.dy,
                "var(--_l)",
            );
        }
        edges_out += "</g>";

        let mut best = 0_usize;
        let mut best_length = -1.0;
        let mut flat = 0_usize;
        let mut flat_length = -1.0;
        let label = &view.edge_labels[k];
        let mut i = 0;
        while i < points.len() {
            min_x = min_x.min(points[i] - 4.0);
            max_x = max_x.max(points[i] + 4.0);
            min_y = min_y.min(points[i + 1] - 4.0);
            max_y = max_y.max(points[i + 1] + 4.0);
            i += 2;
        }
        let mut i = 0;
        while i + 3 < points.len() {
            let horizontal = (points[i + 1] - points[i + 3]).abs() < 0.01;
            let length = (points[i] - points[i + 2]).abs() + (points[i + 1] - points[i + 3]).abs();
            if !horizontal {
                runs.push(points[i]);
                runs.push(points[i + 1].min(points[i + 3]));
                runs.push(points[i + 1].max(points[i + 3]));
            }
            if length > best_length {
                best = i;
                best_length = length;
            }
            if horizontal && length > flat_length {
                flat = i;
                flat_length = length;
            }
            i += 2;
        }
        if label.width > 0.0 {
            let at = if flat_length >= label.width + 8.0 {
                flat
            } else {
                best
            };
            let mut lx = (points[at] + points[at + 2]) / 2.0;
            let mut ly = (points[at + 1] + points[at + 3]) / 2.0;
            let w = label.width + 8.0;
            let upright = (points[at] - points[at + 2]).abs() < 0.01;
            // On a short run next to a node the label would land on the node, so it moves off to
            // the side.
            for p in [&s, &e] {
                if lx + w / 2.0 <= p.x0
                    || lx - w / 2.0 >= p.x1
                    || ly + label.height / 2.0 <= p.y0
                    || ly - label.height / 2.0 >= p.y1
                {
                    continue;
                }
                if upright {
                    lx = if lx >= (p.x0 + p.x1) / 2.0 {
                        p.x1 + w / 2.0 + 4.0
                    } else {
                        p.x0 - w / 2.0 - 4.0
                    };
                } else {
                    ly = if ly >= (p.y0 + p.y1) / 2.0 {
                        p.y1 + label.height / 2.0 + 4.0
                    } else {
                        p.y0 - label.height / 2.0 - 4.0
                    };
                }
            }
            min_x = min_x.min(lx - w / 2.0);
            max_x = max_x.max(lx + w / 2.0);
            min_y = min_y.min(ly - label.height / 2.0);
            max_y = max_y.max(ly + label.height / 2.0);
            labels_out +=
                &edge_label_svg(&format!("{}-{}", edge.lhs_id, edge.rhs_id), label, lx, ly);
        }
    }

    let mut clusters = String::new();
    for (gi, group) in groups.iter().enumerate() {
        let x = left(gi);
        let y = top(gi);
        let w = right(gi) - x;
        let label = &view.group_labels[gi];
        let mut head = String::new();
        if view.head_w[gi] > 0.0 {
            // The title starts at the left, or after the edges that come down through that corner.
            let band = y + view.head_h[gi] + 12.0;
            let mut crossings: Vec<f64> = Vec::new();
            let mut i = 0;
            while i < runs.len() {
                if runs[i + 1] < band && runs[i + 2] > y && runs[i] > x && runs[i] < x + w {
                    crossings.push(runs[i]);
                }
                i += 3;
            }
            crossings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            crossings.push(x + w - 4.0);
            let mut start = x + 10.0;
            for crossing in crossings {
                if crossing - 6.0 - start >= view.head_w[gi] {
                    break;
                }
                start = start.max(crossing + 6.0);
            }
            if start + view.head_w[gi] > x + w - 4.0 {
                start = x + 10.0;
            }
            let cy = y + 6.0 + view.head_h[gi] / 2.0;
            let mut text_x = start;
            if let Some(icon) = &group.icon {
                head += &icon_slot(measurer, icon, start, cy - badge / 2.0, badge);
                text_x += badge + 6.0;
            }
            head += &label_svg(
                label,
                text_x + label.width / 2.0,
                cy,
                " class=\"pele-cluster-label\" fill=\"var(--_m)\"",
            );
        }
        clusters += &format!(
            "<g class=\"pele-cluster\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" \
             height=\"{}\" fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"{ROUNDED}/>{}</g>",
            esc_text(&group.id),
            num(x),
            num(y),
            num(w),
            num(bottom(gi) - y),
            head
        );
    }

    let mut nodes_out = String::new();
    let half = icon_size / 2.0;
    let glyph = icon_size * 0.5;
    let text_size = (size * 0.75).round().max(6.0);
    for (i, node) in nodes.iter().enumerate() {
        let at = format!(
            "data-id=\"{}\" transform=\"translate({},{})\"",
            esc_text(&node.id),
            num(center_x(i)),
            num(view.y[view.node_row[i]])
        );
        if node.kind == NodeKind::Junction {
            nodes_out += &format!(
                "<g class=\"pele-node pele-junction\" {at}><circle cy=\"{}\" r=\"{}\" \
                 fill=\"var(--_l)\"/></g>",
                num(half),
                num(DOT)
            );
            continue;
        }
        // What is in the square: the icon of the service, or its text.
        let mut inner = String::new();
        if let Some(icon) = &node.icon {
            inner = icon_slot(
                measurer,
                icon,
                -glyph / 2.0,
                (icon_size - glyph) / 2.0,
                glyph,
            );
        } else if let Some(text) = &node.icon_text {
            let label = layout_label(
                Some(text),
                false,
                measurer,
                text_size,
                icon_size - 8.0,
                Style::NONE,
            );
            let lines = (((icon_size - 4.0)
                / if label.line_height == 0.0 {
                    1.0
                } else {
                    label.line_height
                })
            .floor())
            .max(1.0) as usize;
            inner = label_svg(
                &fit(&label, icon_size - 8.0, lines, measurer),
                0.0,
                half,
                &format!(" class=\"pele-icon-text\" font-size=\"{}\"", num(text_size)),
            );
        }
        let label = &view.labels[i];
        nodes_out += &format!(
            "<g class=\"pele-node pele-service\" {at}><rect x=\"{}\" width=\"{}\" height=\"{}\" \
             fill=\"var(--_s)\" stroke=\"var(--_b)\"{ROUNDED}/>{inner}{}</g>",
            num(-half),
            num(icon_size),
            num(icon_size),
            label_svg(
                label,
                0.0,
                icon_size + LABEL_GAP + label.height / 2.0,
                " class=\"pele-label\""
            )
        );
    }

    let title = layout_label(
        model.title.as_deref(),
        false,
        measurer,
        size,
        if tight { title_room(options) } else { 4000.0 },
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let width = ((max_x - min_x).max(title.width) + 2.0 * pad).ceil();
    let height = (max_y - min_y + title_height + 2.0 * pad).ceil();
    let ox = ((width - (max_x - min_x)) / 2.0 - min_x).round();
    let oy = (pad + title_height - min_y).round();
    let mut body = String::new();
    if !clusters.is_empty() {
        body += &format!("<g class=\"pele-clusters\">{clusters}</g>");
    }
    if !edges_out.is_empty() {
        body += &format!(
            "<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\" \
             stroke-linejoin=\"round\">{edges_out}</g>"
        );
    }
    if !labels_out.is_empty() {
        body += &format!("<g class=\"pele-edge-labels\" font-size=\"{small}\">{labels_out}</g>");
    }
    if !nodes_out.is_empty() {
        body += &format!("<g class=\"pele-nodes\">{nodes_out}</g>");
    }
    let acc = Accessible {
        title: model.acc_title.clone(),
        descr: model.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            crate::DiagramKind::Architecture,
            width,
            height,
            size,
            options,
            &acc,
            &format!(
                "{}{}",
                label_svg(
                    &title,
                    width / 2.0,
                    pad + title.height / 2.0,
                    " class=\"pele-title\" font-weight=\"var(--_tw)\"",
                ),
                if body.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<g transform=\"translate({},{})\">{body}</g>",
                        num(ox),
                        num(oy)
                    )
                }
            ),
        ),
        width,
        height,
        links: Vec::new(),
    }
}
