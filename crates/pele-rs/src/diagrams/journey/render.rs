//! User journey layout and SVG. Ported from pele's `diagrams/journey/render.ts`.

use std::collections::HashMap;
use std::fmt::Write;

use super::db::{JourneyDb, journey_actors};
use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::turn_to_fit;
use crate::diagrams::common::plain::{fit_label, plain_label};
use crate::svg::{Accessible, ROUNDED, esc_text, label_svg, num, series_color, svg_document};
use crate::text::Style;
use crate::text::label::Label;
use crate::util::{round, trim};

const PAD_X: f64 = 10.0;
const PAD_Y: f64 = 8.0;
const MIN_TEXT: f64 = 124.0;
const MAX_TEXT: f64 = 200.0;
const GAP: f64 = 12.0;
const BAND_PAD: f64 = 6.0;
const GUTTER: f64 = 24.0;
const LEVEL: f64 = 26.0;
const FACE: f64 = 11.0;
const DOT: f64 = 5.0;
const DOT_STEP: f64 = 13.0;
const LEGEND_GAP: f64 = 20.0;

const EYES: &str =
    "<circle cx=\"-3.6\" cy=\"-2.8\" r=\"1.3\"/><circle cx=\"3.6\" cy=\"-2.8\" r=\"1.3\"/>";

fn face(x: f64, y: f64, score: f64) -> String {
    let (mood, mouth) = if score > 3.0 {
        ("happy", "M-5,2A6,6 0 0 0 5,2")
    } else if score < 3.0 {
        ("sad", "M-5,6.5A6,6 0 0 1 5,6.5")
    } else {
        ("neutral", "M-4.5,4.5H4.5")
    };
    format!(
        "<g class=\"pele-rs-face pele-rs-face-{mood}\" transform=\"translate({},{})\"><circle r=\"{}\" fill=\"var(--_bg)\" stroke=\"var(--_l)\"/><g fill=\"var(--_l)\">{EYES}</g><path d=\"{mouth}\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\"/></g>",
        num(x),
        num(y),
        num(FACE)
    )
}

/// `Math.min(5, Math.max(1, score))`.
fn clamp_score(score: f64) -> f64 {
    5f64.min(1f64.max(score))
}

pub(crate) fn render_journey(db: &JourneyDb, input: &Input<'_>) -> Output {
    turn_to_fit(input.options, |down| {
        if down {
            vertical(db, input)
        } else {
            draw(db, input)
        }
    })
}

struct LegendItem<'a> {
    name: &'a str,
    label: Label,
    x: f64,
    y: f64,
    h: f64,
}

struct Run {
    start: usize,
    count: usize,
    label: Option<Label>,
}

fn draw(db: &JourneyDb, input: &Input<'_>) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let tiny = round(size * 0.75);
    let pad = options.padding.unwrap_or(8.0);
    let tasks = &db.tasks;
    let n = tasks.len();

    // `Task: 5:` names an actor with no name; Mermaid gives it a dot and an empty legend row.
    let names: Vec<&str> = journey_actors(tasks)
        .into_iter()
        .filter(|name| !name.is_empty())
        .collect();
    let actor: HashMap<&str, usize> = names.iter().enumerate().map(|(i, &n)| (n, i)).collect();
    let color = |name: &str| series_color(actor.get(name).copied().unwrap_or(0));

    // Cards widen, up to a point, for a word that would not fit; past that the word is cut.
    let mut text_w = MIN_TEXT;
    let mut labels: Vec<Label> = tasks
        .iter()
        .map(|t| plain_label(Some(&t.task), measurer, small, text_w, Style::NONE))
        .collect();
    let widest = labels.iter().fold(0f64, |w, l| w.max(l.width));
    if widest > text_w {
        text_w = widest.ceil().min(MAX_TEXT);
        labels = tasks
            .iter()
            .map(|t| fit_label(Some(&t.task), measurer, small, text_w, Style::NONE))
            .collect();
    }

    let card_w = text_w + 2.0 * PAD_X;
    let pitch = card_w + GAP;
    let plot_w = if n > 0 { n as f64 * pitch - GAP } else { 0.0 };

    let line_h = round(small * 1.5);
    let limit = plot_w.max(240.0);
    let mut legend: Vec<LegendItem> = Vec::new();
    let mut legend_w: f64 = 0.0;
    let mut legend_h: f64 = 0.0;
    let mut lx = 0.0;
    for &name in &names {
        let label = fit_label(
            Some(name),
            measurer,
            small,
            limit - 2.0 * DOT - 6.0,
            Style::NONE,
        );
        let w = 2.0 * DOT + 6.0 + label.width;
        if lx > 0.0 && lx + w > limit {
            lx = 0.0;
            legend_h += 4.0;
        }
        let y = if lx > 0.0 {
            legend.last().map_or(0.0, |item| item.y)
        } else {
            legend_h
        };
        let h = label.height.max(line_h);
        legend.push(LegendItem {
            name,
            label,
            x: lx,
            y,
            h,
        });
        lx += w;
        legend_w = legend_w.max(lx);
        legend_h = legend_h.max(y + h);
        lx += LEGEND_GAP;
    }

    let body_w = if n > 0 { GUTTER + plot_w } else { 0.0 };
    let title = fit_label(
        db.title.as_deref(),
        measurer,
        size,
        body_w.max(320.0),
        Style::BOLD,
    );
    let title_h = if title.height > 0.0 {
        title.height + 16.0
    } else {
        0.0
    };
    let width = body_w
        .max(title.width)
        .max(if n > 0 { GUTTER + legend_w } else { 0.0 });
    let left = pad + (width - body_w) / 2.0 + GUTTER;
    let top = pad + title_h;

    // A band covers a run of tasks that share a section, as in Mermaid; a section with no
    // tasks draws nothing.
    let mut runs: Vec<Run> = Vec::new();
    let mut band_h: f64 = 0.0;
    let mut i = 0;
    while i < n {
        let mut end = i + 1;
        while end < n && tasks[end].section == tasks[i].section {
            end += 1;
        }
        let mut label = None;
        if !tasks[i].section.is_empty() {
            let l = fit_label(
                Some(&tasks[i].section),
                measurer,
                small,
                (end - i) as f64 * pitch - GAP - 2.0 * PAD_X,
                Style::BOLD,
            );
            band_h = band_h.max(l.height.max(line_h) + 2.0 * BAND_PAD);
            label = Some(l);
        }
        runs.push(Run {
            start: i,
            count: end - i,
            label,
        });
        i = end;
    }

    let text_h = labels.iter().fold(line_h, |h, l| h.max(l.height));
    let dots_h = if names.is_empty() {
        0.0
    } else {
        2.0 * DOT + 6.0
    };
    let card_h = text_h + dots_h + 2.0 * PAD_Y;
    let cards_y = top + if band_h > 0.0 { band_h + BAND_PAD } else { 0.0 };
    let chart_top = cards_y + card_h + FACE + 16.0;
    let chart_bottom = chart_top + 4.0 * LEVEL;
    let right = left + plot_w;

    let mut body = String::new();
    let mut bottom = top;
    if n > 0 {
        let mut grid = String::new();
        let mut scale = String::new();
        for score in (1..=5).rev() {
            let y = chart_top + f64::from(5 - score) * LEVEL;
            let _ = write!(grid, "M{},{}H{}", num(left - 6.0), num(y), num(right));
            let _ = write!(
                scale,
                "<text x=\"{}\" y=\"{}\">{score}</text>",
                num(left - 12.0),
                num(y + tiny * 0.35)
            );
        }
        let _ = write!(
            body,
            "<path class=\"pele-rs-grid\" d=\"{grid}\" fill=\"none\" stroke=\"var(--_a)\"/><g class=\"pele-rs-scale\" font-size=\"{}\" text-anchor=\"end\" fill=\"var(--_m)\">{scale}</g>",
            num(tiny)
        );

        let mut line = String::new();
        let mut stems = String::new();
        let mut faces = String::new();
        let mut cards: Vec<String> = Vec::with_capacity(n);
        for (i, task) in tasks.iter().enumerate() {
            let x = left + i as f64 * pitch;
            let cx = x + card_w / 2.0;
            let score = task.score;
            let level = if score.is_nan() {
                3.0
            } else {
                clamp_score(score)
            };
            let y = chart_top + (5.0 - level) * LEVEL;
            let _ = write!(
                line,
                "{}{},{}",
                if i == 0 { 'M' } else { 'L' },
                num(cx),
                num(y)
            );
            let _ = write!(
                stems,
                "M{},{}V{}",
                num(cx),
                num(cards_y + card_h),
                num(y - FACE)
            );
            faces += &face(cx, y, score);

            let mut dots = String::new();
            let people: Vec<&str> = task
                .people
                .iter()
                .map(String::as_str)
                .filter(|n| !n.is_empty())
                .collect();
            let count = people.len() as f64;
            let step = DOT_STEP.min((card_w - 2.0 * PAD_X - 2.0 * DOT) / (count - 1.0).max(1.0));
            for (k, name) in people.iter().enumerate() {
                let _ = write!(
                    dots,
                    "<circle class=\"pele-rs-actor\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" stroke=\"var(--_s)\"><title>{}</title></circle>",
                    num(cx + (k as f64 - (count - 1.0) / 2.0) * step),
                    num(cards_y + card_h - PAD_Y - DOT),
                    num(DOT),
                    color(name),
                    esc_text(name)
                );
            }
            cards.push(format!(
                "<g class=\"pele-rs-node pele-rs-task\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{ROUNDED}/>{}{dots}</g>",
                esc_text(trim(&task.task)),
                num(x),
                num(cards_y),
                num(card_w),
                num(card_h),
                label_svg(
                    &labels[i],
                    cx,
                    cards_y + PAD_Y + text_h / 2.0,
                    &format!(" font-size=\"{}\"", num(small))
                )
            ));
        }
        let _ = write!(
            body,
            "<path class=\"pele-rs-stems\" d=\"{stems}\" fill=\"none\" stroke=\"var(--_b)\" stroke-dasharray=\"2 3\"/>"
        );
        if n > 1 {
            let _ = write!(
                body,
                "<path class=\"pele-rs-edge\" d=\"{line}\" fill=\"none\" stroke=\"var(--_l)\" stroke-linejoin=\"round\"/>"
            );
        }
        body += &faces;

        for run in &runs {
            let members = cards[run.start..run.start + run.count].concat();
            let Some(label) = &run.label else {
                body += &members;
                continue;
            };
            let x = left + run.start as f64 * pitch;
            let w = run.count as f64 * pitch - GAP;
            let _ = write!(
                body,
                "<g class=\"pele-rs-cluster pele-rs-section\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\"{ROUNDED}/>{}{members}</g>",
                esc_text(trim(&tasks[run.start].section)),
                num(x),
                num(top),
                num(w),
                num(band_h),
                label_svg(
                    label,
                    x + w / 2.0,
                    top + band_h / 2.0,
                    &format!(" font-size=\"{}\" font-weight=\"var(--_hw)\"", num(small))
                )
            );
        }
        bottom = chart_bottom + FACE;

        if !legend.is_empty() {
            let x0 = left + (plot_w - legend_w) / 2.0;
            let y0 = bottom + 16.0;
            let mut items = String::new();
            for item in &legend {
                let cy = y0 + item.y + item.h / 2.0;
                let _ = write!(
                    items,
                    "<g class=\"pele-rs-legend-item\" data-id=\"{}\"><circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>{}</g>",
                    esc_text(item.name),
                    num(x0 + item.x + DOT),
                    num(cy),
                    num(DOT),
                    color(item.name),
                    label_svg(
                        &item.label,
                        x0 + item.x + 2.0 * DOT + 6.0 + item.label.width / 2.0,
                        cy,
                        ""
                    )
                );
            }
            let _ = write!(
                body,
                "<g class=\"pele-rs-legend\" font-size=\"{}\">{items}</g>",
                num(small)
            );
            bottom = y0 + legend_h;
        }
    }

    let total_width = (width + 2.0 * pad).ceil();
    let total_height = (bottom.max(pad) + pad).ceil();
    let content = label_svg(
        &title,
        pad + width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-rs-title\" font-weight=\"var(--_tw)\"",
    ) + &body;
    output(db, input, total_width, total_height, size, &content)
}

fn output(
    db: &JourneyDb,
    input: &Input<'_>,
    width: f64,
    height: f64,
    size: f64,
    content: &str,
) -> Output {
    let acc = Accessible {
        title: db.acc_title.clone(),
        descr: db.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            DiagramKind::Journey,
            width,
            height,
            size,
            input.options,
            &acc,
            content,
        ),
        width,
        height,
        links: Vec::new(),
    }
}

fn vertical(db: &JourneyDb, input: &Input<'_>) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let pad = options.padding.unwrap_or(8.0);
    let room = 140f64.max(options.max_width.unwrap_or(320.0) - 2.0 * pad);
    let title = fit_label(db.title.as_deref(), measurer, size, room, Style::BOLD);
    let names: Vec<&str> = journey_actors(&db.tasks)
        .into_iter()
        .filter(|name| !name.is_empty())
        .collect();
    let actors: HashMap<&str, usize> = names.iter().enumerate().map(|(i, &n)| (n, i)).collect();
    let mut y = pad
        + if title.height > 0.0 {
            title.height + 16.0
        } else {
            0.0
        };
    let mut section: Option<&str> = None;
    let mut body = String::new();
    let mut width = title.width;
    let count = db.tasks.len();
    for (index, task) in db.tasks.iter().enumerate() {
        if section != Some(task.section.as_str()) {
            section = Some(&task.section);
            let head = fit_label(
                Some(&task.section),
                measurer,
                size,
                room - 16.0,
                Style::BOLD,
            );
            if head.height > 0.0 {
                body += &label_svg(
                    &head,
                    pad + 8.0 + head.width / 2.0,
                    y + head.height / 2.0,
                    " class=\"pele-rs-cluster-label\" font-weight=\"var(--_hw)\"",
                );
                width = width.max(head.width + 16.0);
                y += head.height + 8.0;
            }
        }
        let label = fit_label(Some(&task.task), measurer, small, room - 72.0, Style::NONE);
        let people: Vec<(&str, Label)> = task
            .people
            .iter()
            .filter(|n| !n.is_empty())
            .map(|n| {
                (
                    n.as_str(),
                    fit_label(Some(n), measurer, small, room - 44.0, Style::NONE),
                )
            })
            .collect();
        let text_h = label.height.max(42.0);
        let mut h = 2.0 * PAD_Y + text_h;
        for (_, l) in &people {
            h += 16f64.max(l.height) + 4.0;
        }
        let w = room.max(label.width + 72.0);
        let _ = write!(
            body,
            "<g class=\"pele-rs-node pele-rs-journey-step\" data-id=\"{index}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{ROUNDED}/>",
            num(pad),
            num(y),
            num(w),
            num(h)
        );
        body += &label_svg(
            &label,
            pad + PAD_X + label.width / 2.0,
            y + PAD_Y + label.height / 2.0,
            " class=\"pele-rs-label\"",
        );
        let cx = pad + w - 28.0;
        body += &face(cx, y + PAD_Y + FACE, task.score);
        if !task.score.is_nan() {
            let _ = write!(
                body,
                "<text class=\"pele-rs-journey-score\" x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"middle\">{}</text>",
                num(cx),
                num(y + PAD_Y + 2.0 * FACE + small),
                num(small),
                num(clamp_score(task.score))
            );
        }
        let mut py = y + PAD_Y + text_h + 4.0;
        for (name, l) in &people {
            let ph = 16f64.max(l.height);
            let _ = write!(
                body,
                "<circle class=\"pele-rs-actor\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>",
                num(pad + 16.0),
                num(py + ph / 2.0),
                num(DOT),
                series_color(actors[name])
            );
            body += &label_svg(
                l,
                pad + 28.0 + l.width / 2.0,
                py + ph / 2.0,
                &format!(
                    " class=\"pele-rs-journey-actor\" font-size=\"{}\"",
                    num(small)
                ),
            );
            py += ph + 4.0;
        }
        body += "</g>";
        y += h;
        if index + 1 < count {
            let _ = write!(
                body,
                "<path class=\"pele-rs-edge\" d=\"M{},{}v16\" fill=\"none\" stroke=\"var(--_l)\"/>",
                num(pad + w / 2.0),
                num(y)
            );
        }
        y += 16.0;
        width = width.max(w);
    }
    let total_width = (width + 2.0 * pad).ceil();
    let height = ((pad + title.height).max(y - if count > 0 { 16.0 } else { 0.0 }) + pad).ceil();
    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-rs-title\" font-weight=\"var(--_tw)\"",
    ) + &body;
    output(db, input, total_width, height, size, &content)
}
