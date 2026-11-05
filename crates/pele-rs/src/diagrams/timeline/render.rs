//! Timeline layout and SVG. Ported from pele's `diagrams/timeline/render.ts`.

use std::fmt::Write;

use super::db::TimelineDb;
use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::turn_to_fit;
use crate::diagrams::common::plain::{fit_label, plain_label};
use crate::preprocess::Value;
use crate::svg::markers::{ARROW, marker};
use crate::svg::{Accessible, esc_text, label_svg, num, svg_document};
use crate::svg::{rounded, series_color};
use crate::text::Style;
use crate::text::label::Label;
use crate::util::{round, trim};

const PAD_X: f64 = 12.0;
const PAD_Y: f64 = 8.0;
const MIN_TEXT: f64 = 152.0;
const MAX_TEXT: f64 = 216.0;
const EVENT_TEXT_TD: f64 = 264.0;
const GAP: f64 = 20.0;
const AXIS: f64 = 20.0;
const SPINE: f64 = 14.0;
const EVENT_GAP: f64 = 8.0;
const SECTION_GAP: f64 = 8.0;
const TAIL: f64 = 12.0;
const ON_COLOR: &str = " fill=\"var(--_bg)\"";
const LINE: &str = " fill=\"none\" stroke=\"var(--_b)\"";

struct Group {
    /// The section's index, `None` for the periods before the first section.
    section: Option<usize>,
    periods: Vec<usize>,
}

fn rect(x: f64, y: f64, w: f64, h: f64, attrs: &str) -> String {
    format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>",
        num(x),
        num(y),
        num(w),
        num(h),
        rounded(attrs)
    )
}

fn dot(x: f64, y: f64, color: &str) -> String {
    format!(
        "<circle class=\"pele-dot\" cx=\"{}\" cy=\"{}\" r=\"4\" fill=\"{color}\" stroke=\"var(--_bg)\" stroke-width=\"2\"/>",
        num(x),
        num(y)
    )
}

pub(crate) fn render_timeline(db: &TimelineDb, input: &Input<'_>) -> Output {
    if db.direction == "TD" {
        draw(db, input, true)
    } else {
        turn_to_fit(input.options, |down| draw(db, input, down))
    }
}

fn draw(db: &TimelineDb, input: &Input<'_>, vertical: bool) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let pad = options.padding.unwrap_or(8.0);
    let multicolor = !matches!(
        input.config.get("timeline"),
        Some(Value::Map(m)) if m.get("disableMulticolor") == Some(&Value::Bool(true))
    );
    let periods = &db.periods;
    let has_sections = !db.sections.is_empty();

    // Mermaid draws nothing for periods that come before the first section; here they lead,
    // in a neutral color.
    let mut groups: Vec<Group> = (0..db.sections.len())
        .map(|s| Group {
            section: Some(s),
            periods: Vec::new(),
        })
        .collect();
    let mut leading = Group {
        section: None,
        periods: Vec::new(),
    };
    for (i, period) in periods.iter().enumerate() {
        match period.section_index {
            Some(s) => groups[s].periods.push(i),
            None => leading.periods.push(i),
        }
    }
    if !leading.periods.is_empty() {
        groups.insert(0, leading);
    }

    // Columns widen, up to a point, for a word that would not fit; past that the word is cut.
    // Running down, the two columns share the width the host has.
    let room = match options.max_width {
        Some(w) if vertical && w > 0.0 => w - 2.0 * pad,
        _ => f64::INFINITY,
    };
    let taken = AXIS + SPINE * 2.0 + 4.0 * PAD_X;
    let period_text = MIN_TEXT.min(80f64.max(round((room - taken) * 0.38)));
    let event_text = EVENT_TEXT_TD.min(110f64.max(room - taken - period_text));
    let mut text_w = period_text;
    let event_text_w = |text_w: f64| if vertical { event_text } else { text_w };
    let mut period_labels: Vec<Label> = periods
        .iter()
        .map(|p| plain_label(Some(&p.text), measurer, size, text_w, Style::NONE))
        .collect();
    let mut event_labels: Vec<Vec<Label>> = periods
        .iter()
        .map(|p| {
            p.events
                .iter()
                .map(|e| plain_label(Some(e), measurer, small, event_text_w(text_w), Style::NONE))
                .collect()
        })
        .collect();
    let mut widest: f64 = 0.0;
    for label in &period_labels {
        widest = widest.max(label.width);
    }
    if !vertical {
        for label in event_labels.iter().flatten() {
            widest = widest.max(label.width);
        }
    }
    let mut overflow = widest > text_w;
    if vertical {
        for label in event_labels.iter().flatten() {
            overflow |= label.width > event_text;
        }
    }
    if overflow {
        text_w = widest
            .ceil()
            .max(period_text)
            .min(if room == f64::INFINITY {
                MAX_TEXT
            } else {
                period_text
            });
        period_labels = periods
            .iter()
            .map(|p| fit_label(Some(&p.text), measurer, size, text_w, Style::NONE))
            .collect();
        event_labels = periods
            .iter()
            .map(|p| {
                p.events
                    .iter()
                    .map(|e| fit_label(Some(e), measurer, small, event_text_w(text_w), Style::NONE))
                    .collect()
            })
            .collect();
    }

    let line_height = round(size * 1.5);
    let col_w = text_w + 2.0 * PAD_X;
    let event_w = event_text_w(text_w) + 2.0 * PAD_X;
    let chip_height = |label: &Label| label.height.max(line_height) + 2.0 * PAD_Y;
    let event_height = |label: &Label| label.height.max(round(small * 1.5)) + 2.0 * PAD_Y;
    let series_of = |group: &Group, period: usize| {
        group.section.or({
            if has_sections {
                None
            } else {
                Some(if multicolor { period } else { 0 })
            }
        })
    };
    let on_color = |series: Option<usize>| {
        let ink = series.map_or_else(String::new, crate::palette::series_ink_fill);
        if ink.is_empty() {
            ON_COLOR.to_string()
        } else {
            ink
        }
    };
    let color_of = |group: &Group, period: usize| match group.section {
        Some(s) => series_color(s),
        None if has_sections => "var(--_m)".to_string(),
        None => series_color(if multicolor { period } else { 0 }),
    };

    let has_events = event_labels.iter().any(|labels| !labels.is_empty());
    let slots: usize = groups.iter().map(|g| g.periods.len().max(1)).sum();

    let pitch = col_w + GAP;
    let drawn = !periods.is_empty();
    let body_w = if vertical {
        col_w
            + if has_events {
                AXIS + SPINE * 2.0 + event_w
            } else if drawn {
                AXIS + TAIL
            } else {
                0.0
            }
    } else if slots > 0 {
        slots as f64 * pitch - GAP + if drawn { TAIL + ARROW } else { 0.0 }
    } else {
        0.0
    };
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
    let width = body_w.max(title.width);
    let left = pad + (width - body_w) / 2.0;
    let top = pad + title_h;

    let section_label = |section: usize, w: f64| {
        fit_label(
            Some(&db.sections[section]),
            measurer,
            size,
            w - 2.0 * PAD_X,
            Style::BOLD,
        )
    };
    let header = |section: usize, label: &Label, x: f64, y: f64, w: f64, h: f64| {
        rect(x, y, w, h, &format!(" fill=\"{}\"", series_color(section)))
            + &label_svg(
                label,
                x + w / 2.0,
                y + h / 2.0,
                &format!(" font-weight=\"var(--_hw)\"{}", on_color(Some(section))),
            )
    };
    let period_svg = |label: &Label, series: Option<usize>, color: &str, x: f64, y: f64, h: f64| {
        rect(x, y, col_w, h, &format!(" fill=\"{color}\""))
            + &label_svg(label, x + col_w / 2.0, y + h / 2.0, &on_color(series))
    };
    let event_svg = |label: &Label, color: &str, x: f64, y: f64, h: f64| {
        format!(
            "<g class=\"pele-node pele-event\">{}{}</g>",
            rect(
                x,
                y,
                event_w,
                h,
                &format!(" fill=\"{color}\" fill-opacity=\"0.16\"")
            ),
            label_svg(
                label,
                x + event_w / 2.0,
                y + h / 2.0,
                &format!(" font-size=\"{}\"", num(small))
            )
        )
    };
    let open = |group: &Group| match group.section {
        Some(s) => format!(
            "<g class=\"pele-cluster pele-section\" data-id=\"{}\">",
            esc_text(trim(&db.sections[s]))
        ),
        None => String::new(),
    };
    let open_period = |i: usize| {
        format!(
            "<g class=\"pele-node pele-period\" data-id=\"{}\">",
            esc_text(trim(&periods[i].text))
        )
    };

    let mut body = String::new();
    let height = if vertical {
        let axis_x = left + col_w + AXIS;
        let spine_x = axis_x + SPINE;
        let events_x = spine_x + SPINE;
        let mut y = top;
        for group in &groups {
            body += &open(group);
            if let Some(s) = group.section {
                let label = section_label(s, body_w);
                let h = chip_height(&label);
                body += &header(s, &label, left, y, body_w, h);
                y += h + SECTION_GAP + 4.0;
            }
            for &i in &group.periods {
                let color = color_of(group, i);
                let h = chip_height(&period_labels[i]);
                // A period and its first event share one center line, so the line between
                // them runs straight.
                let first = event_labels[i].first().map_or(h, event_height);
                let cy = y + h.max(first) / 2.0;
                let mut d = format!("M{},{}H{}", num(left + col_w), num(cy), num(axis_x));
                let mut cards = String::new();
                let mut ey = cy - first / 2.0;
                let mut last = cy;
                for label in &event_labels[i] {
                    let eh = event_height(label);
                    last = ey + eh / 2.0;
                    let _ = write!(d, "M{},{}H{}", num(spine_x), num(last), num(events_x));
                    cards += &event_svg(label, &color, events_x, ey, eh);
                    ey += eh + EVENT_GAP;
                }
                if !cards.is_empty() {
                    let _ = write!(
                        d,
                        "M{},{}H{}V{}",
                        num(axis_x),
                        num(cy),
                        num(spine_x),
                        num(last)
                    );
                }
                let _ = write!(
                    body,
                    "{}{}<path class=\"pele-edge\" d=\"{d}\"{LINE}/>{}{cards}</g>",
                    open_period(i),
                    period_svg(
                        &period_labels[i],
                        series_of(group, i),
                        &color,
                        left,
                        cy - h / 2.0,
                        h
                    ),
                    dot(axis_x, cy, &color)
                );
                y = (cy + h / 2.0).max(ey - EVENT_GAP) + GAP;
            }
            if group.periods.is_empty() {
                y += GAP - SECTION_GAP - 4.0;
            }
            if group.section.is_some() {
                body += "</g>";
            }
        }
        if !groups.is_empty() {
            y -= GAP;
        }
        if drawn {
            let tip = y + TAIL + ARROW;
            body = format!(
                "<path class=\"pele-axis\" d=\"M{},{}V{}\" fill=\"none\" stroke=\"var(--_l)\"/>{}{body}",
                num(axis_x),
                num(top),
                num(tip - ARROW + 1.0),
                marker("arrow_point", axis_x, tip, 0.0, 1.0, "var(--_l)")
            );
            y = tip;
        }
        y + pad
    } else {
        let mut header_h: f64 = 0.0;
        let section_labels: Vec<Option<Label>> = groups
            .iter()
            .map(|group| {
                let s = group.section?;
                let label = section_label(s, group.periods.len().max(1) as f64 * pitch - GAP);
                header_h = header_h.max(chip_height(&label));
                Some(label)
            })
            .collect();
        let mut chip_h: f64 = 0.0;
        for label in &period_labels {
            chip_h = chip_h.max(chip_height(label));
        }
        let periods_y = top
            + if has_sections {
                header_h + SECTION_GAP
            } else {
                0.0
            };
        let axis_y = periods_y + chip_h + if chip_h > 0.0 { AXIS } else { 0.0 };
        let events_y = axis_y + AXIS;
        let mut bottom = if drawn { axis_y + TAIL } else { top + header_h };
        let mut slot = 0;
        for (g, group) in groups.iter().enumerate() {
            let span = group.periods.len().max(1);
            let gx = left + slot as f64 * pitch;
            body += &open(group);
            if let (Some(s), Some(label)) = (group.section, &section_labels[g]) {
                body += &header(s, label, gx, top, span as f64 * pitch - GAP, header_h);
            }
            for (k, &i) in group.periods.iter().enumerate() {
                let color = color_of(group, i);
                let x = gx + k as f64 * pitch;
                let cx = x + col_w / 2.0;
                let mut d = format!(
                    "M{},{}V{}",
                    num(cx),
                    num(periods_y + chip_h),
                    num(if event_labels[i].is_empty() {
                        axis_y
                    } else {
                        events_y
                    })
                );
                let mut cards = String::new();
                let mut ey = events_y;
                for label in &event_labels[i] {
                    let eh = event_height(label);
                    if ey > events_y {
                        let _ = write!(d, "M{},{}V{}", num(cx), num(ey - EVENT_GAP), num(ey));
                    }
                    cards += &event_svg(label, &color, x, ey, eh);
                    ey += eh + EVENT_GAP;
                }
                bottom = bottom.max(ey - EVENT_GAP);
                let _ = write!(
                    body,
                    "{}{}<path class=\"pele-edge\" d=\"{d}\"{LINE}/>{}{cards}</g>",
                    open_period(i),
                    period_svg(
                        &period_labels[i],
                        series_of(group, i),
                        &color,
                        x,
                        periods_y,
                        chip_h
                    ),
                    dot(cx, axis_y, &color)
                );
            }
            if group.section.is_some() {
                body += "</g>";
            }
            slot += span;
        }
        if drawn {
            let tip = left + body_w;
            body = format!(
                "<path class=\"pele-axis\" d=\"M{},{}H{}\" fill=\"none\" stroke=\"var(--_l)\"/>{}{body}",
                num(left),
                num(axis_y),
                num(tip - ARROW + 1.0),
                marker("arrow_point", tip, axis_y, 1.0, 0.0, "var(--_l)")
            );
        }
        bottom + pad
    };

    let total_width = (width + 2.0 * pad).ceil();
    let total_height = height.max(pad * 2.0).ceil();
    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &body;
    let acc = Accessible {
        title: db.acc_title.clone(),
        descr: db.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            DiagramKind::Timeline,
            total_width,
            total_height,
            size,
            options,
            &acc,
            &content,
        ),
        width: total_width,
        height: total_height,
        links: Vec::new(),
    }
}
