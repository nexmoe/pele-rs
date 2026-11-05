//! The drawing of a git graph: the lanes of the branches, the commits on them, and the
//! connectors between them. Ported from pele's `diagrams/git/render.ts`.

use super::db::{CommitKind, GitCommit, GitModel};
use super::parser::Direction;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::turn_to_fit;
use crate::diagrams::common::plain::plain_label;
use crate::error::Error;
use crate::preprocess::{Config, Value};
use crate::svg::{Accessible, ROUNDED, esc_text, label_svg, num, series_color, svg_document};
use crate::text::Style;
use crate::text::label::{Label, layout_label};
use crate::util::round;
use std::collections::HashMap;

const DOT: f64 = 7.0;
const STEP: f64 = 44.0;
const LANE: f64 = 44.0;
/// Shorter than a step, so a curve between lanes never runs through a commit on a lane it
/// crosses.
const CURVE: f64 = 36.0;
const GAP: f64 = 8.0;
const LABEL_GAP: f64 = 20.0;
const TAIL: f64 = 20.0;
const NOTCH: f64 = 7.0;
const TAG_GAP: f64 = 6.0;
const HALF: f64 = std::f64::consts::FRAC_1_SQRT_2;
const CLASSES: [&str; 5] = ["normal", "reverse", "highlight", "merge", "cherry-pick"];

/// A commit and the room it takes.
struct View<'a> {
    commit: &'a GitCommit,
    lane: usize,
    /// The place of the commit along the history.
    m: f64,
    label: Option<Label>,
    tags: Vec<Label>,
}

/// The room a value of the settings takes.
fn flag(config: &Config, key: &str, fallback: bool) -> bool {
    match config.get("gitGraph") {
        Some(Value::Map(map)) => match map.get(key) {
            Some(Value::Bool(value)) => *value,
            _ => fallback,
        },
        _ => fallback,
    }
}

/// The room a tag takes, with the notch that marks it.
fn tag_width(tag: &Label) -> f64 {
    NOTCH + 4.0 + tag.width + 6.0
}

/// The shape of a commit.
fn symbol(kind: CommitKind, x: f64, y: f64, color: &str) -> String {
    let at = format!("cx=\"{}\" cy=\"{}\"", num(x), num(y));
    if kind == CommitKind::Highlight {
        return format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\" fill=\"{color}\"/>",
            num(x - DOT - 1.0),
            num(y - DOT - 1.0),
            num(2.0 * DOT + 2.0),
            num(2.0 * DOT + 2.0)
        );
    }
    if kind == CommitKind::Merge {
        return format!(
            "<circle {at} r=\"{}\" fill=\"var(--_bg)\" stroke=\"{color}\" stroke-width=\"2\"/>\
             <circle {at} r=\"2.5\" fill=\"{color}\"/>",
            num(DOT - 1.0)
        );
    }
    let dot = format!("<circle {at} r=\"{}\" fill=\"{color}\"/>", num(DOT));
    if kind == CommitKind::Reverse {
        return dot
            + &format!(
                "<path d=\"M{},{}l6,6M{},{}l6,-6\" fill=\"none\" stroke=\"var(--_bg)\" \
                 stroke-width=\"1.5\" stroke-linecap=\"round\"/>",
                num(x - 3.0),
                num(y - 3.0),
                num(x - 3.0),
                num(y + 3.0)
            );
    }
    if kind == CommitKind::CherryPick {
        return dot
            + &format!(
                "<path d=\"M{},{}L{},{}L{},{}\" fill=\"none\" stroke=\"var(--_bg)\" \
                 stroke-linejoin=\"round\"/>\
                 <circle cx=\"{}\" cy=\"{}\" r=\"1.8\" fill=\"var(--_bg)\"/>\
                 <circle cx=\"{}\" cy=\"{}\" r=\"1.8\" fill=\"var(--_bg)\"/>",
                num(x - 2.6),
                num(y + 1.0),
                num(x + 0.8),
                num(y - 4.2),
                num(x + 2.6),
                num(y + 1.0),
                num(x - 2.6),
                num(y + 2.2),
                num(x + 2.6),
                num(y + 2.2)
            );
    }
    dot
}

/// The box the drawing takes.
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn new() -> Self {
        Self {
            min_x: f64::INFINITY,
            min_y: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
        }
    }

    fn grow(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        if x0 < self.min_x {
            self.min_x = x0;
        }
        if y0 < self.min_y {
            self.min_y = y0;
        }
        if x1 > self.max_x {
            self.max_x = x1;
        }
        if y1 > self.max_y {
            self.max_y = y1;
        }
    }
}

/// Draws a git graph.
pub(crate) fn render_git(model: &GitModel, input: &Input<'_>) -> Result<Output, Error> {
    if model.direction != Direction::LeftToRight {
        return Ok(draw(model, input, model.direction));
    }
    Ok(turn_to_fit(input.options, |down| {
        draw(
            model,
            input,
            if down {
                Direction::TopToBottom
            } else {
                Direction::LeftToRight
            },
        )
    }))
}

/// Draws a git graph in one direction.
fn draw(model: &GitModel, input: &Input<'_>, direction: Direction) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let config = input.config;
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.75);
    let pad = options.padding.unwrap_or(8.0);

    let show_branches = flag(config, "showBranches", true);
    let show_commit_label = flag(config, "showCommitLabel", true);
    let rotate = flag(config, "rotateCommitLabel", true);
    let parallel = flag(config, "parallelCommits", false);
    let vertical = direction != Direction::LeftToRight;
    let flip = direction == Direction::BottomToTop;

    let lane_of: HashMap<&str, usize> = model
        .lanes
        .iter()
        .enumerate()
        .map(|(i, name)| (name.as_str(), i))
        .collect();
    let lane_count = model.lanes.len();
    let medium = round(size * 0.875);
    // Mermaid writes branch names, tags, and commit ids as plain text, so tags in them show as
    // written.
    let names: Vec<Label> = model
        .lanes
        .iter()
        .map(|name| {
            plain_label(
                if show_branches {
                    Some(name.as_str())
                } else {
                    None
                },
                measurer,
                medium,
                4000.0,
                Style::NONE,
            )
        })
        .collect();

    let mut commits: Vec<&GitCommit> = model.commits.values().collect();
    commits.sort_by_key(|commit| commit.seq);

    let mut views: Vec<View<'_>> = Vec::with_capacity(commits.len());
    let mut by_id: HashMap<&str, usize> = HashMap::new();
    for commit in commits {
        let labelled = show_commit_label
            && commit.kind != CommitKind::CherryPick
            && (commit.kind != CommitKind::Merge || commit.custom_id);
        let mut tags = Vec::new();
        for tag in &commit.tags {
            let label = plain_label(Some(tag.as_str()), measurer, small, 4000.0, Style::NONE);
            if !label.lines.is_empty() {
                tags.push(label);
            }
        }
        let label = if labelled {
            Some(plain_label(
                Some(commit.id.as_str()),
                measurer,
                small,
                4000.0,
                Style::NONE,
            ))
        } else {
            None
        };
        by_id.insert(commit.id.as_str(), views.len());
        views.push(View {
            commit,
            lane: lane_of.get(commit.branch.as_str()).copied().unwrap_or(0),
            m: 0.0,
            label,
            tags,
        });
    }

    // Room each commit needs: along the main axis on either side, and across it towards the
    // previous lane (`lo`) and the next one (`hi`). Tags sit on one side of a lane and ids on
    // the other.
    let mut half = vec![0.0; views.len()];
    let mut tag_half = vec![0.0; views.len()];
    let mut lo = vec![DOT; lane_count];
    let mut hi = vec![DOT; lane_count];
    for (k, view) in views.iter().enumerate() {
        let mut tags_main = 0.0;
        let mut tags_cross = 0.0;
        for tag in &view.tags {
            let w = tag_width(tag);
            if vertical {
                tags_main = f64::max(tags_main, tag.height);
                tags_cross += w + 4.0;
            } else {
                tags_main = f64::max(tags_main, w);
                tags_cross += tag.height + 3.0;
            }
        }
        let mut label_main = 0.0;
        let mut label_cross = 0.0;
        if let Some(label) = &view.label {
            if !label.lines.is_empty() {
                if rotate {
                    label_cross = 3.0 + HALF * (label.width + label.height);
                } else {
                    label_main = if vertical { label.height } else { label.width };
                    label_cross = 6.0 + if vertical { label.width } else { label.height };
                }
            }
        }
        half[k] = f64::max(DOT, f64::max(tags_main / 2.0, label_main / 2.0));
        tag_half[k] = tags_main / 2.0;
        let tag_side = DOT
            + if view.tags.is_empty() {
                0.0
            } else {
                TAG_GAP + tags_cross
            };
        let label_side = DOT + label_cross;
        lo[view.lane] = f64::max(lo[view.lane], if vertical { label_side } else { tag_side });
        hi[view.lane] = f64::max(hi[view.lane], if vertical { tag_side } else { label_side });
    }

    // In a left-to-right graph the tags stand over their commit. A connector that comes down
    // from a lane above joins the commit's lane this far before the commit, clear of the tags,
    // and the commit is placed far enough along to leave room for that.
    let mut lead = vec![0.0; views.len()];
    if !vertical {
        for (k, view) in views.iter().enumerate() {
            if tag_half[k] == 0.0 {
                continue;
            }
            for id in &view.commit.parents {
                if let Some(&parent) = by_id.get(id.as_str()) {
                    if views[parent].lane < view.lane {
                        lead[k] = tag_half[k] + 4.0;
                    }
                }
            }
        }
    }
    let space = |p: usize, k: usize| -> f64 {
        f64::max(
            STEP,
            f64::max(
                half[p] + half[k] + GAP,
                if lead[k] > 0.0 {
                    half[p] + CURVE + lead[k]
                } else {
                    0.0
                },
            ),
        )
    };

    for k in 0..views.len() {
        if !parallel {
            views[k].m = if k == 0 {
                0.0
            } else {
                views[k - 1].m + space(k - 1, k)
            };
            continue;
        }
        // Mermaid's `parallelCommits`: a commit sits one step after its furthest parent, so
        // branches that grow side by side share positions instead of taking turns.
        let mut m = 0.0;
        for id in &views[k].commit.parents {
            if let Some(&parent) = by_id.get(id.as_str()) {
                if parent >= k {
                    continue;
                }
                m = f64::max(m, views[parent].m + space(parent, k));
            }
        }
        views[k].m = m;
    }
    let end = views
        .iter()
        .fold(0.0_f64, |end, view| f64::max(end, view.m));

    let mut cross = vec![0.0; lane_count];
    for i in 0..lane_count {
        let name = &names[i];
        let reach = if name.lines.is_empty() {
            0.0
        } else if vertical {
            name.width / 2.0 + 8.0
        } else {
            name.height / 2.0 + 2.0
        };
        lo[i] = f64::max(lo[i], reach);
        hi[i] = f64::max(hi[i], reach);
        if i > 0 {
            cross[i] = cross[i - 1] + f64::max(LANE, hi[i - 1] + lo[i] + GAP);
        }
    }

    let px = |m: f64, c: f64| if vertical { c } else { m };
    let py = |m: f64, c: f64| {
        if vertical {
            if flip { -m } else { m }
        } else {
            c
        }
    };
    let point = |m: f64, c: f64| format!("{},{}", num(px(m, c)), num(py(m, c)));

    let mut bounds = Bounds::new();

    let mut branches = String::new();
    if show_branches {
        for i in 0..lane_count {
            let name = &names[i];
            let color = series_color(i);
            let c = cross[i];
            let w = name.width + 16.0;
            let h = name.height + 4.0;
            let from = if name.lines.is_empty() {
                -LABEL_GAP
            } else {
                -LABEL_GAP - if vertical { h } else { w }
            };
            let cx = px(from + if vertical { h } else { w } / 2.0, c);
            let cy = py(from + if vertical { h } else { w } / 2.0, c);
            bounds.grow(
                px(-LABEL_GAP, c),
                py(-LABEL_GAP, c),
                px(-LABEL_GAP, c),
                py(-LABEL_GAP, c),
            );
            bounds.grow(
                px(end + TAIL, c),
                py(end + TAIL, c),
                px(end + TAIL, c),
                py(end + TAIL, c),
            );
            branches.push_str(&format!(
                "<g class=\"pele-branch\" data-id=\"{}\"><path class=\"pele-lane\" \
                 d=\"M{}L{}\" stroke=\"{color}\" stroke-dasharray=\"2 4\"/>",
                esc_text(&model.lanes[i]),
                point(-LABEL_GAP, c),
                point(end + TAIL, c)
            ));
            if !name.lines.is_empty() {
                bounds.grow(cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0);
                branches.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{color}\"{ROUNDED}/>{}",
                    num(cx - w / 2.0),
                    num(cy - h / 2.0),
                    num(w),
                    num(h),
                    label_svg(
                        name,
                        cx,
                        cy,
                        &format!(" class=\"pele-label\"{}", { let ink = crate::palette::series_ink_fill(i); if ink.is_empty() { " fill=\"var(--_bg)\"".to_string() } else { ink } }),
                    )
                ));
            }
            branches.push_str("</g>");
        }
    }

    // Positions of the commits on each lane, to tell whether a connector may run along it.
    let mut taken: Vec<Vec<f64>> = vec![Vec::new(); lane_count];
    for view in &views {
        taken[view.lane].push(view.m);
    }
    if parallel {
        for list in &mut taken {
            list.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        }
    }
    let busy = |lane: usize, from: f64, to: f64| -> bool {
        let list = &taken[lane];
        let mut low = 0;
        let mut high = list.len();
        while low < high {
            let mid = (low + high) / 2;
            if list[mid] <= from {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        low < list.len() && list[low] < to
    };

    let mut curves = String::new();
    let mut lines = String::new();
    for (index, view) in views.iter().enumerate() {
        let commit = view.commit;
        let c2 = cross[view.lane];
        let m2 = view.m;
        for (j, id) in commit.parents.iter().enumerate() {
            let Some(&parent) = by_id.get(id.as_str()) else {
                continue;
            };
            if parent == index {
                continue;
            }
            let c1 = cross[views[parent].lane];
            let m1 = views[parent].m;
            if views[parent].lane == view.lane {
                lines.push_str(&format!(
                    "<path class=\"pele-edge\" d=\"M{}L{}\" stroke=\"{}\"/>",
                    point(m1, c1),
                    point(m2, c2),
                    series_color(view.lane)
                ));
                continue;
            }
            let merged = j > 0 && commit.kind == CommitKind::Merge;
            let picked = j > 0 && !merged;
            let mut d = format!("M{}", point(m1, c1));
            if m2 <= m1 {
                let mid = (m1 + m2) / 2.0;
                d.push_str(&format!(
                    "C{} {} {}",
                    point(mid, c1),
                    point(mid, c2),
                    point(m2, c2)
                ));
            } else if !merged && !busy(view.lane, m1, m2) {
                // Leaves the parent at once and runs along the child's lane.
                let s = f64::min(CURVE, m2 - m1);
                d.push_str(&format!(
                    "C{} {} {}L{}",
                    point(m1 + s / 2.0, c1),
                    point(m1 + s / 2.0, c2),
                    point(m1 + s, c2),
                    point(m2, c2)
                ));
            } else {
                // Turns in just before the child, or before its tags when it comes from their
                // side.
                let to = if views[parent].lane < view.lane && m2 - lead[index] > m1 {
                    m2 - lead[index]
                } else {
                    m2
                };
                let mut s = f64::min(CURVE, to - m1);
                let mut along = c1;
                if !merged || busy(views[parent].lane, m1, m2) {
                    // Neither lane is free to run along. Between two lanes, past the ids of one
                    // and before the tags of the next, a strip stays empty, and the connector
                    // takes that.
                    along = if c1 > c2 {
                        c2 + hi[view.lane] + GAP / 2.0
                    } else {
                        c2 - lo[view.lane] - GAP / 2.0
                    };
                    s = f64::min(CURVE, (to - m1) / 2.0);
                    d.push_str(&format!(
                        "C{} {} {}",
                        point(m1 + s / 2.0, c1),
                        point(m1 + s / 2.0, along),
                        point(m1 + s, along)
                    ));
                }
                d.push_str(&format!(
                    "L{}C{} {} {}",
                    point(to - s, along),
                    point(to - s / 2.0, along),
                    point(to - s / 2.0, c2),
                    point(to, c2)
                ));
                if to < m2 {
                    d.push_str(&format!("L{}", point(m2, c2)));
                }
            }
            curves.push_str(&format!(
                "<path class=\"pele-edge{}\" d=\"{d}\" stroke=\"{}\"{}/>",
                if merged {
                    " pele-edge-merge"
                } else if picked {
                    " pele-edge-cherry-pick"
                } else {
                    ""
                },
                series_color(if merged {
                    views[parent].lane
                } else {
                    view.lane
                }),
                if picked {
                    " stroke-dasharray=\"4 3\""
                } else {
                    ""
                }
            ));
        }
    }

    let mut commits_svg = String::new();
    for view in &views {
        let commit = view.commit;
        let x = px(view.m, cross[view.lane]);
        let y = py(view.m, cross[view.lane]);
        let kind = commit.custom_type.unwrap_or(commit.kind);
        bounds.grow(x - DOT - 1.0, y - DOT - 1.0, x + DOT + 1.0, y + DOT + 1.0);
        // Merges and cherry-picks carry a message Mermaid makes up; only a written one is worth
        // showing.
        let written = commit.kind.index() < CommitKind::Merge.index() && !commit.message.is_empty();
        let mut body = if written {
            format!("<title>{}</title>", esc_text(&commit.message))
        } else {
            String::new()
        };
        body.push_str(&symbol(kind, x, y, &series_color(view.lane)));

        if let Some(label) = &view.label {
            if !label.lines.is_empty() {
                let w = label.width;
                let h = label.height;
                let back = |cx: f64, cy: f64| -> String {
                    format!(
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" \
                         fill=\"var(--_bg)\"/>",
                        num(cx - w / 2.0 - 2.0),
                        num(cy - h / 2.0 + 2.0),
                        num(w + 4.0),
                        num(h - 4.0)
                    )
                };
                if rotate {
                    // The text ends beside the commit and trails back the way the history came.
                    let k = (HALF * h) / 2.0;
                    let ax = if vertical { x - DOT - 3.0 - k } else { x + 3.0 };
                    let ay = if vertical { y } else { y + DOT + 3.0 + k };
                    let reach = HALF * w + k;
                    bounds.grow(
                        ax - reach,
                        if flip { ay - reach } else { ay - k },
                        ax + k,
                        if flip { ay + k } else { ay + reach },
                    );
                    body.push_str(&format!(
                        "<g class=\"pele-commit-label\" transform=\"rotate({} {} {})\">{}{}</g>",
                        if flip { 45 } else { -45 },
                        num(ax),
                        num(ay),
                        back(ax - w / 2.0, ay),
                        label_svg(label, ax - w / 2.0, ay, "")
                    ));
                } else {
                    let cx = if vertical { x - DOT - 6.0 - w / 2.0 } else { x };
                    let cy = if vertical { y } else { y + DOT + 6.0 + h / 2.0 };
                    bounds.grow(cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0);
                    body.push_str(&format!(
                        "<g class=\"pele-commit-label\">{}{}</g>",
                        back(cx, cy),
                        label_svg(label, cx, cy, "")
                    ));
                }
            }
        }

        // Beside the lane the tags form a row; above it they stack, the first one furthest out.
        let mut offset = DOT + TAG_GAP;
        let mut t = if vertical {
            0
        } else {
            view.tags.len() as i64 - 1
        };
        while t >= 0 && t < view.tags.len() as i64 {
            let tag = &view.tags[t as usize];
            let w = tag_width(tag);
            let h = tag.height;
            let left = if vertical { x + offset } else { x - w / 2.0 };
            let cy = if vertical { y } else { y - offset - h / 2.0 };
            offset += if vertical { w + 4.0 } else { h + 3.0 };
            bounds.grow(left, cy - h / 2.0, left + w, cy + h / 2.0);
            body.push_str(&format!(
                "<g class=\"pele-tag\"><path d=\"M{},{}L{},{}H{}V{}H{}Z\" fill=\"var(--_s)\" \
                 stroke=\"var(--_b)\" stroke-linejoin=\"round\"/><circle cx=\"{}\" cy=\"{}\" \
                 r=\"1.5\" fill=\"var(--_b)\"/>{}</g>",
                num(left),
                num(cy),
                num(left + NOTCH),
                num(cy - h / 2.0),
                num(left + w),
                num(cy + h / 2.0),
                num(left + NOTCH),
                num(left + NOTCH),
                num(cy),
                label_svg(
                    tag,
                    left + NOTCH + 4.0 + tag.width / 2.0,
                    cy,
                    " fill=\"var(--_fg)\"",
                )
            ));
            t += if vertical { 1 } else { -1 };
        }

        let merge = if commit.kind == CommitKind::Merge && kind != CommitKind::Merge {
            " pele-commit-merge"
        } else {
            ""
        };
        commits_svg.push_str(&format!(
            "<g class=\"pele-commit pele-commit-{}{}\" data-id=\"{}\">{}</g>",
            CLASSES[kind.index()],
            merge,
            esc_text(&commit.id),
            body
        ));
    }

    if bounds.min_x > bounds.max_x {
        bounds.grow(0.0, 0.0, 0.0, 0.0);
    }
    let title = layout_label(
        model.title.as_deref(),
        false,
        measurer,
        size,
        4000.0,
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 16.0
    } else {
        0.0
    };
    let inner = f64::max(bounds.max_x - bounds.min_x, title.width);
    let width = (inner + 2.0 * pad).ceil();
    let height = (bounds.max_y - bounds.min_y + title_height + 2.0 * pad).ceil();
    let dx = pad - bounds.min_x + (inner - (bounds.max_x - bounds.min_x)) / 2.0;
    let dy = pad + title_height - bounds.min_y;

    let content = format!(
        "{}<g transform=\"translate({},{})\">{}{}{}</g>",
        label_svg(
            &title,
            width / 2.0,
            pad + title.height / 2.0,
            " class=\"pele-title\" font-weight=\"var(--_tw)\"",
        ),
        num(dx),
        num(dy),
        if branches.is_empty() {
            String::new()
        } else {
            format!(
                "<g class=\"pele-branches\" fill=\"none\" font-size=\"{medium}\">{branches}</g>"
            )
        },
        if curves.is_empty() && lines.is_empty() {
            String::new()
        } else {
            format!(
                "<g class=\"pele-edges\" fill=\"none\" stroke-width=\"2\" \
                 stroke-linecap=\"round\">{curves}{lines}</g>"
            )
        },
        if commits_svg.is_empty() {
            String::new()
        } else {
            format!(
                "<g class=\"pele-commits\" font-size=\"{small}\" fill=\"var(--_m)\">\
                 {commits_svg}</g>"
            )
        },
    );

    let acc = Accessible {
        title: model.acc_title.clone(),
        descr: model.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            crate::DiagramKind::GitGraph,
            width,
            height,
            size,
            options,
            &acc,
            &content,
        ),
        width,
        height,
        links: Vec::new(),
    }
}
