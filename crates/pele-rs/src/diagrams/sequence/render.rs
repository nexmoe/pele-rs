//! Sequence diagram layout and SVG. Ported from pele's `diagrams/sequence/render.ts`.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use super::actors::{ActorSize, DEFAULT_PAD_X, actor_kind, actor_size, draw_actor, is_figure};
use super::db::{LEFT_OF, MessageText, RIGHT_OF, SeqDb, line};
use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{tighten, title_room};
use crate::diagrams::common::literal::break_label;
use crate::preprocess::{Config, Value};
use crate::svg::markers::{ARROW, marker};
use crate::svg::{
    Accessible, ROUNDED, class_names, esc, esc_text, label_svg, num, resolve_style, rounded,
    svg_document, title_label,
};
use crate::text::Style;
use crate::text::label::{Label, layout_label};
use crate::util::{js_number, round};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Head {
    None,
    Arrow,
    Cross,
    Open,
    HalfSolid,
    HalfStick,
}

#[derive(Clone, Copy)]
struct LineStyle {
    dotted: bool,
    head: Head,
    /// Where the heads are: at the receiver, at the sender only (the reverse arrows), or both.
    at_start: bool,
    at_end: bool,
    bottom: bool,
}

fn line_style(kind: Option<i32>) -> Option<LineStyle> {
    let style = |dotted, head, ends: u8, bottom| LineStyle {
        dotted,
        head,
        at_start: ends != 1,
        at_end: ends != 0,
        bottom,
    };
    let k = kind?;
    Some(match k {
        line::SOLID => style(false, Head::Arrow, 1, false),
        line::DOTTED => style(true, Head::Arrow, 1, false),
        line::SOLID_CROSS => style(false, Head::Cross, 1, false),
        line::DOTTED_CROSS => style(true, Head::Cross, 1, false),
        line::SOLID_OPEN => style(false, Head::None, 1, false),
        line::DOTTED_OPEN => style(true, Head::None, 1, false),
        line::SOLID_POINT => style(false, Head::Open, 1, false),
        line::DOTTED_POINT => style(true, Head::Open, 1, false),
        line::BIDIRECTIONAL_SOLID => style(false, Head::Arrow, 2, false),
        line::BIDIRECTIONAL_DOTTED => style(true, Head::Arrow, 2, false),
        41..=48 | 51..=58 => {
            let dotted = k > 50;
            let base = if dotted { k - 10 } else { k };
            match base {
                line::SOLID_TOP => style(dotted, Head::HalfSolid, 1, false),
                line::SOLID_BOTTOM => style(dotted, Head::HalfSolid, 1, true),
                line::STICK_TOP => style(dotted, Head::HalfStick, 1, false),
                line::STICK_BOTTOM => style(dotted, Head::HalfStick, 1, true),
                line::SOLID_ARROW_TOP_REVERSE => style(dotted, Head::HalfSolid, 0, false),
                line::SOLID_ARROW_BOTTOM_REVERSE => style(dotted, Head::HalfSolid, 0, true),
                line::STICK_ARROW_TOP_REVERSE => style(dotted, Head::HalfStick, 0, false),
                _ => style(dotted, Head::HalfStick, 0, true),
            }
        }
        _ => return None,
    })
}

fn frame_kind(kind: Option<i32>) -> Option<&'static str> {
    Some(match kind? {
        line::LOOP_START => "loop",
        line::ALT_START => "alt",
        line::OPT_START => "opt",
        line::PAR_START | line::PAR_OVER_START => "par",
        line::CRITICAL_START => "critical",
        line::BREAK_START => "break",
        line::RECT_START => "rect",
        _ => return None,
    })
}

const MSG_PAD: f64 = 12.0;
const SELF_W: f64 = 28.0;
const SELF_H: f64 = 22.0;
const ACT_W: f64 = 10.0;
const ACT_STEP: f64 = 4.0;
const ACT_MIN: f64 = 14.0;
const NOTE_GAP: f64 = 10.0;
const NOTE_OVER: f64 = 16.0;
const FRAME_PAD: f64 = 20.0;
const FRAME_NEST: f64 = 10.0;
const TAB_H: f64 = 20.0;
const FRAME_TEXT: f64 = 240.0;
const BADGE_H: f64 = 16.0;
const CENTRAL_R: f64 = 4.0;
const DASH: &str = " stroke-dasharray=\"4 4\"";

pub(crate) fn is_line_message(kind: Option<i32>) -> bool {
    line_style(kind).is_some()
}

struct Numbering {
    /// The number each message line carries, whether or not it is shown.
    index: Vec<f64>,
    shown: Vec<bool>,
}

/// Counts message lines the way Mermaid's renderer does while it draws.
fn number_messages(db: &SeqDb, always: bool) -> Numbering {
    let n = db.messages.len();
    let mut index = vec![f64::NAN; n];
    let mut shown = vec![false; n];
    let mut current = 1.0;
    let mut step = 1.0;
    let mut visible = false;
    let truthy = |x: Option<f64>| x.filter(|&v| v != 0.0 && !v.is_nan());
    for (i, m) in db.messages.iter().enumerate() {
        if m.kind == Some(line::AUTONUMBER) {
            if let MessageText::Numbering {
                start,
                step: s,
                visible: v,
            } = m.message
            {
                current = truthy(start).unwrap_or(current);
                step = truthy(s).unwrap_or(step);
                visible = v;
            }
        } else if is_line_message(m.kind) {
            index[i] = current;
            shown[i] = visible || always;
            current = round((current + step) * 100.0) / 100.0;
        }
    }
    Numbering { index, shown }
}

struct Column<'a> {
    id: &'a str,
    actor: &'a super::db::SeqActor,
    kind: &'static str,
    label: Label,
    size: ActorSize,
    x: f64,
    create_at: Option<usize>,
    destroy_at: Option<usize>,
    top: f64,
    life_start: f64,
    life_end: f64,
    activations: Vec<f64>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum V {
    Line,
    SelfLoop,
    Note,
}

struct View {
    kind: V,
    a: usize,
    b: usize,
    label: Label,
    x: f64,
    w: f64,
}

#[derive(Clone)]
struct Frame {
    kind: &'static str,
    x0: f64,
    x1: f64,
    top: f64,
    title: Label,
    tab_w: f64,
    fill: String,
    dividers: Vec<usize>,
    divider_y: Vec<f64>,
    divider_labels: Vec<Label>,
}

struct BoxRun {
    boxed: usize,
    first: usize,
    last: usize,
    title: Label,
    grow: f64,
}

fn option(conf: &Config, key: &str, fallback: f64) -> f64 {
    match conf.get(key) {
        Some(&Value::Number(v)) if (0.0..=2000.0).contains(&v) => v,
        _ => fallback,
    }
}

fn half(x: f64, y: f64, dir: f64, up: f64, stick: bool) -> String {
    let bx = num(x - dir * ARROW);
    if stick {
        return format!(
            "<path class=\"pele-rs-marker\" d=\"M{},{}L{bx},{}\"/>",
            num(x),
            num(y),
            num(y + up * 5.0)
        );
    }
    format!(
        "<path class=\"pele-rs-marker\" d=\"M{},{}L{bx},{}V{}Z\" fill=\"var(--_l)\" stroke=\"none\"/>",
        num(x),
        num(y),
        num(y + up * 4.5),
        num(y)
    )
}

/// Draws the head whose tip is at (x, y) and points along `dir`. `up` is the side a half head
/// sits on.
fn head(kind: Head, x: f64, y: f64, dir: f64, up: f64) -> String {
    match kind {
        Head::Arrow => marker("arrow_point", x, y, dir, 0.0, "var(--_l)"),
        Head::Cross => marker("arrow_cross", x, y, dir, 0.0, "var(--_l)"),
        Head::Open => format!(
            "<path class=\"pele-rs-marker\" d=\"M{},{}L{},{}L{},{}\" stroke-linejoin=\"round\"/>",
            num(x - dir * ARROW),
            num(y - 4.5),
            num(x),
            num(y),
            num(x - dir * ARROW),
            num(y + 4.5)
        ),
        Head::HalfSolid => half(x, y, dir, up, false),
        Head::HalfStick => half(x, y, dir, up, true),
        Head::None => String::new(),
    }
}

fn trim(kind: Head) -> f64 {
    match kind {
        Head::Arrow => ARROW - 1.0,
        Head::Cross => 4.0,
        _ => 0.0,
    }
}

pub(crate) fn render_sequence(db: &SeqDb, input: &Input<'_>) -> Output {
    tighten(input.options, |tight| draw(db, input, tight))
}

fn draw(db: &SeqDb, input: &Input<'_>, tight: bool) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let empty = Config::new();
    let conf = match input.config.get("sequence") {
        Some(Value::Map(m)) => m,
        _ => &empty,
    };
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let tiny = round(size * 0.75);
    let pad = options.padding.unwrap_or(8.0);
    let mirror = !matches!(conf.get("mirrorActors"), Some(Value::Bool(false)));
    let right_angles = matches!(conf.get("rightAngles"), Some(Value::Bool(true)));
    let actor_margin =
        option(conf, "actorMargin", 40.0).min(if tight { 8.0 } else { f64::INFINITY });
    let min_w = option(conf, "width", 64.0).min(if tight { 48.0 } else { f64::INFINITY });
    let min_h = option(conf, "height", round(size * 1.5) + 20.0);
    let wrap_w = option(conf, "width", 160.0).min(if tight {
        32f64.max(
            (options.max_width.unwrap_or(320.0) - 2.0 * pad) / (db.actors.len().max(1) as f64)
                - actor_margin
                - 24.0,
        )
    } else {
        f64::INFINITY
    });
    // Mermaid's default of 35 corresponds to the default gap here; other values move it by the
    // difference.
    let msg_gap = 4f64.max(14.0 + option(conf, "messageMargin", 35.0) - 35.0);
    let note_pad = option(conf, "noteMargin", 10.0).min(if tight { 6.0 } else { f64::INFINITY });
    let box_pad = option(conf, "boxMargin", 10.0).min(if tight { 6.0 } else { f64::INFINITY });
    let align = match conf.get("messageAlign") {
        Some(Value::String(s)) if s == "left" => -1,
        Some(Value::String(s)) if s == "right" => 1,
        _ => 0,
    };
    let msg_pad = if tight { 6.0 } else { MSG_PAD };
    let note_over = if tight { 8.0 } else { NOTE_OVER };
    let frame_pad = if tight { 12.0 } else { FRAME_PAD };
    let frame_nest = if tight { 6.0 } else { FRAME_NEST };
    let messages = &db.messages;
    let count = messages.len();
    let numbering = number_messages(
        db,
        matches!(conf.get("showSequenceNumbers"), Some(Value::Bool(true))),
    );
    let text = |raw: &str, font_size: f64, wrap: bool, width: f64| -> Label {
        let label = layout_label(
            Some(raw),
            false,
            measurer,
            font_size,
            if tight || wrap { width } else { f64::INFINITY },
            Style::NONE,
        );
        if tight {
            break_label(label, measurer, 120f64.max(width))
        } else {
            label
        }
    };

    let used: Option<HashSet<Option<&str>>> =
        matches!(conf.get("hideUnusedParticipants"), Some(Value::Bool(true))).then(|| {
            messages
                .iter()
                .flat_map(|m| [m.from.as_deref(), m.to.as_deref()])
                .collect()
        });

    let mut next_line = vec![None; count + 1];
    for i in (0..count).rev() {
        next_line[i] = if is_line_message(messages[i].kind) {
            Some(i)
        } else {
            next_line[i + 1]
        };
    }

    let mut cols: Vec<Column<'_>> = Vec::new();
    let mut col_of: HashMap<&str, usize> = HashMap::new();
    for (id, actor) in &db.actors {
        if used
            .as_ref()
            .is_some_and(|u| !u.contains(&Some(id.as_str())))
        {
            continue;
        }
        let kind = actor_kind(&actor.kind);
        let label = text(&actor.description, size, actor.wrap, wrap_w);
        let mut create_at = db.created.get(id).and_then(|&c| next_line[c.min(count)]);
        if create_at.is_some_and(|c| messages[c].to.as_deref() != Some(id)) {
            create_at = None;
        }
        let mut destroy_at = db.destroyed.get(id).and_then(|&c| next_line[c.min(count)]);
        if destroy_at.is_some_and(|d| {
            messages[d].to.as_deref() != Some(id) && messages[d].from.as_deref() != Some(id)
        }) {
            destroy_at = None;
        }
        col_of.insert(id, cols.len());
        let size = actor_size(
            kind,
            &label,
            min_w,
            min_h,
            if tight { 8.0 } else { DEFAULT_PAD_X },
        );
        cols.push(Column {
            id,
            actor,
            kind,
            label,
            size,
            x: 0.0,
            create_at,
            destroy_at,
            top: 0.0,
            life_start: 0.0,
            life_end: -1.0,
            activations: Vec::new(),
        });
    }
    let n_cols = cols.len();

    // Widths between lifelines: every message and note says how far apart the lifelines it
    // touches must be.
    let mut needs: HashMap<(usize, usize), f64> = HashMap::new();
    let mut need = |lo: i64, hi: i64, width: f64| {
        if lo < 0 || hi >= n_cols as i64 {
            return;
        }
        let entry = needs.entry((lo as usize, hi as usize)).or_insert(0.0);
        if *entry < width {
            *entry = width;
        }
    };
    let badge_width = |i: usize| {
        if numbering.shown[i] {
            BADGE_H.max(measurer.width(&js_number(numbering.index[i]), tiny, Style::NONE) + 8.0)
        } else {
            0.0
        }
    };

    let mut views: Vec<Option<View>> = (0..count).map(|_| None).collect();
    for (i, m) in messages.iter().enumerate() {
        let (Some(a), Some(b)) = (
            m.from.as_deref().and_then(|f| col_of.get(f).copied()),
            m.to.as_deref().and_then(|t| col_of.get(t).copied()),
        ) else {
            continue;
        };
        let MessageText::Text(message) = &m.message else {
            continue;
        };
        let (ai, bi) = (a as i64, b as i64);
        if m.kind == Some(line::NOTE) {
            let label = text(message, small, m.wrap, wrap_w);
            let w = label.width + 2.0 * note_pad;
            match m.placement {
                Some(LEFT_OF) => need(ai - 1, ai, w + 2.0 * NOTE_GAP + ACT_W),
                Some(RIGHT_OF) => need(ai, ai + 1, w + 2.0 * NOTE_GAP + ACT_W),
                _ if a == b => {
                    need(ai - 1, ai, w / 2.0 + NOTE_GAP);
                    need(ai, ai + 1, w / 2.0 + NOTE_GAP);
                }
                _ => need(ai.min(bi), ai.max(bi), w - 2.0 * note_over),
            }
            views[i] = Some(View {
                kind: V::Note,
                a,
                b,
                label,
                x: 0.0,
                w,
            });
        } else if is_line_message(m.kind) {
            let label = text(message, small, m.wrap, wrap_w);
            let badge = badge_width(i);
            if a == b {
                need(
                    ai,
                    ai + 1,
                    SELF_W + 8.0 + label.width + 16.0 + ACT_W + badge,
                );
                views[i] = Some(View {
                    kind: V::SelfLoop,
                    a,
                    b,
                    label,
                    x: 0.0,
                    w: 0.0,
                });
            } else {
                let created = if cols[b].create_at == Some(i) {
                    cols[b].size.w / 2.0
                } else {
                    0.0
                };
                let central = if m.central != 0 { 4.0 * CENTRAL_R } else { 0.0 };
                let lw = if tight {
                    label.width.min(16f64.max(wrap_w - 16.0))
                } else {
                    label.width
                };
                need(
                    ai.min(bi),
                    ai.max(bi),
                    lw + 2.0 * msg_pad + ACT_W + badge + created + central,
                );
                views[i] = Some(View {
                    kind: V::Line,
                    a,
                    b,
                    label,
                    x: 0.0,
                    w: 0.0,
                });
            }
        }
    }

    let mut runs: Vec<BoxRun> = Vec::new();
    for (i, col) in cols.iter().enumerate() {
        let Some(boxed) = col.actor.boxed else {
            continue;
        };
        if let Some(run) = runs.last_mut() {
            if run.boxed == boxed && run.last + 1 == i {
                run.last = i;
                continue;
            }
        }
        let b = &db.boxes[boxed];
        runs.push(BoxRun {
            boxed,
            first: i,
            last: i,
            title: text(b.name.as_deref().unwrap_or(""), small, b.wrap, wrap_w),
            grow: 0.0,
        });
    }

    let mut gaps = vec![0.0f64; n_cols.saturating_sub(1)];
    for i in 0..gaps.len() {
        let (left, right) = (&cols[i], &cols[i + 1]);
        let boxed = if left.actor.boxed != right.actor.boxed {
            (if left.actor.boxed.is_some() {
                box_pad
            } else {
                0.0
            }) + if right.actor.boxed.is_some() {
                box_pad
            } else {
                0.0
            }
        } else {
            0.0
        };
        gaps[i] = (left.size.w / 2.0 + right.size.w / 2.0 + actor_margin + boxed)
            .max(needs.get(&(i, i + 1)).copied().unwrap_or(0.0));
    }
    // A message across several lifelines widens each gap it crosses by an equal share of what
    // is missing.
    let mut spans: Vec<(usize, usize, f64)> = needs
        .iter()
        .filter(|(k, _)| k.1 - k.0 > 1)
        .map(|(k, &w)| (k.0, k.1, w))
        .collect();
    spans.sort_by(|p, q| (p.1 - p.0).cmp(&(q.1 - q.0)).then(p.0.cmp(&q.0)));
    for (lo, hi, width) in spans {
        let total: f64 = gaps[lo..hi].iter().sum();
        if total >= width {
            continue;
        }
        let share = (width - total) / (hi - lo) as f64;
        for g in &mut gaps[lo..hi] {
            *g += share;
        }
    }
    let place = |cols: &mut Vec<Column<'_>>, gaps: &[f64]| {
        for i in 1..cols.len() {
            cols[i].x = cols[i - 1].x + gaps[i - 1];
        }
    };
    place(&mut cols, &gaps);
    for run in &mut runs {
        let inner = cols[run.last].x + cols[run.last].size.w / 2.0
            - (cols[run.first].x - cols[run.first].size.w / 2.0);
        run.grow = 0f64.max((run.title.width - inner) / 2.0);
        if run.grow == 0.0 {
            continue;
        }
        if run.first > 0 {
            gaps[run.first - 1] += run.grow;
        }
        if run.last + 1 < n_cols {
            gaps[run.last] += run.grow;
        }
    }
    place(&mut cols, &gaps);

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let extend = |min_x: &mut f64, max_x: &mut f64, x0: f64, x1: f64| {
        if x0 < *min_x {
            *min_x = x0;
        }
        if x1 > *max_x {
            *max_x = x1;
        }
    };
    for col in &cols {
        extend(
            &mut min_x,
            &mut max_x,
            col.x - col.size.w / 2.0,
            col.x + col.size.w / 2.0,
        );
    }
    for run in &runs {
        extend(
            &mut min_x,
            &mut max_x,
            cols[run.first].x - cols[run.first].size.w / 2.0 - box_pad - run.grow,
            cols[run.last].x + cols[run.last].size.w / 2.0 + box_pad + run.grow,
        );
    }

    // Horizontal extents of every block, from what it holds.
    let mut frames: Vec<Option<usize>> = vec![None; count];
    let mut frame_list: Vec<Frame> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    let span_all = || {
        if n_cols == 0 {
            (0.0, 120.0)
        } else {
            (cols[0].x - frame_pad, cols[n_cols - 1].x + frame_pad)
        }
    };
    let no_label = Label::default();
    for i in 0..count {
        let m = &messages[i];
        let kind = m.kind;
        if let Some(fk) = frame_kind(kind) {
            let fill = match (&m.message, fk) {
                (MessageText::Text(t), "rect") => t.clone(),
                _ => String::new(),
            };
            frames[i] = Some(frame_list.len());
            open.push(frame_list.len());
            frame_list.push(Frame {
                kind: fk,
                x0: f64::INFINITY,
                x1: f64::NEG_INFINITY,
                top: 0.0,
                title: no_label.clone(),
                tab_w: 0.0,
                fill,
                dividers: vec![i],
                divider_y: Vec::new(),
                divider_labels: Vec::new(),
            });
        } else if matches!(
            kind,
            Some(line::ALT_ELSE | line::PAR_AND | line::CRITICAL_OPTION)
        ) {
            if let Some(&f) = open.last() {
                frame_list[f].dividers.push(i);
            }
        } else if matches!(
            kind,
            Some(
                line::LOOP_END
                    | line::ALT_END
                    | line::OPT_END
                    | line::PAR_END
                    | line::CRITICAL_END
                    | line::BREAK_END
                    | line::RECT_END
            )
        ) {
            let Some(f) = open.pop() else { continue };
            frames[i] = Some(f);
            if frame_list[f].x0 > frame_list[f].x1 {
                let (x0, x1) = span_all();
                frame_list[f].x0 = x0;
                frame_list[f].x1 = x1;
            }
            if frame_list[f].kind != "rect" {
                let tab_w = measurer.width(frame_list[f].kind, tiny, Style::BOLD) + 14.0;
                frame_list[f].tab_w = tab_w;
                // A block grows to the right to give its texts up to FRAME_TEXT of width.
                for k in 0..frame_list[f].dividers.len() {
                    let raw = match &messages[frame_list[f].dividers[k]].message {
                        MessageText::Text(t) => t.clone(),
                        MessageText::Numbering { .. } => String::new(),
                    };
                    let tab = if k == 0 { tab_w + 8.0 } else { 0.0 };
                    let room = frame_list[f].x1 - frame_list[f].x0 - 16.0 - tab;
                    let label = if raw.is_empty() {
                        no_label.clone()
                    } else {
                        let limit = if tight {
                            FRAME_TEXT.min(wrap_w)
                        } else {
                            FRAME_TEXT
                        };
                        text(&format!("[{raw}]"), small, true, room.max(limit))
                    };
                    if label.width > room {
                        frame_list[f].x1 += label.width - room;
                    }
                    if k == 0 {
                        frame_list[f].title = label;
                    } else {
                        frame_list[f].divider_labels.push(label);
                    }
                }
            }
            let (fx0, fx1) = (frame_list[f].x0, frame_list[f].x1);
            extend(&mut min_x, &mut max_x, fx0, fx1);
            if let Some(&parent) = open.last() {
                frame_list[parent].x0 = frame_list[parent].x0.min(fx0 - frame_nest);
                frame_list[parent].x1 = frame_list[parent].x1.max(fx1 + frame_nest);
            }
        } else if let Some(view) = views[i].as_mut() {
            let (a, b) = (&cols[view.a], &cols[view.b]);
            let (x0, x1);
            match view.kind {
                V::Note => {
                    if m.placement == Some(LEFT_OF) {
                        view.x = a.x - NOTE_GAP - view.w;
                    } else if m.placement == Some(RIGHT_OF) {
                        view.x = a.x + NOTE_GAP;
                    } else if view.a == view.b {
                        view.x = a.x - view.w / 2.0;
                    } else {
                        let lo = a.x.min(b.x) - note_over;
                        let hi = a.x.max(b.x) + note_over;
                        if tight || m.wrap {
                            if let MessageText::Text(t) = &m.message {
                                view.label =
                                    text(t, small, true, wrap_w.max(hi - lo - 2.0 * note_pad));
                            }
                        }
                        view.w = (hi - lo).max(view.label.width + 2.0 * note_pad);
                        view.x = (lo + hi - view.w) / 2.0;
                    }
                    x0 = view.x;
                    x1 = view.x + view.w;
                }
                V::SelfLoop => {
                    x0 = a.x;
                    x1 = a.x + ACT_W + SELF_W + 8.0 + view.label.width + badge_width(i);
                }
                V::Line => {
                    let mut lo = a.x.min(b.x);
                    let mut hi = a.x.max(b.x);
                    if tight || m.wrap {
                        if let MessageText::Text(t) = &m.message {
                            view.label =
                                text(t, small, true, wrap_w.max(hi - lo - 2.0 * msg_pad - ACT_W));
                        }
                    }
                    if b.create_at == Some(i) {
                        if b.x > a.x {
                            hi += b.size.w / 2.0;
                        } else {
                            lo -= b.size.w / 2.0;
                        }
                    }
                    x0 = lo;
                    x1 = hi;
                }
            }
            extend(&mut min_x, &mut max_x, x0, x1);
            if let Some(&f) = open.last() {
                frame_list[f].x0 = frame_list[f].x0.min(x0 - frame_pad);
                frame_list[f].x1 = frame_list[f].x1.max(x1 + frame_pad);
            }
        }
    }

    // Vertical placement, top to bottom, drawing as it goes.
    let box_title_h = runs.iter().map(|r| r.title.height).fold(0.0, f64::max);
    let box_top = if runs.is_empty() {
        0.0
    } else {
        box_pad
            + if box_title_h > 0.0 {
                box_title_h + 4.0
            } else {
                0.0
            }
    };
    let row_h = cols
        .iter()
        .filter(|c| c.create_at.is_none())
        .map(|c| c.size.h)
        .fold(0.0, f64::max);
    let head_bottom = box_top + row_h;
    for col in &mut cols {
        col.top = head_bottom - col.size.h;
        col.life_start = head_bottom;
    }

    let xs: Vec<f64> = cols.iter().map(|c| c.x).collect();
    // Whether a lifeline runs between two x positions, where text would be struck through.
    let crosses = |x0: f64, x1: f64| {
        let lo = xs.partition_point(|&x| x <= x0);
        lo < xs.len() && xs[lo] < x1
    };
    let backed = |label: &Label, cx: f64, cy: f64, attrs: &str| -> String {
        if label.width == 0.0 {
            return String::new();
        }
        let l = cx - label.width / 2.0;
        let back = if crosses(l, l + label.width) {
            format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" fill=\"var(--_bg)\" stroke=\"none\"/>",
                num(l - 3.0),
                num(cy - label.height / 2.0 + 2.0),
                num(label.width + 6.0),
                num(label.height - 4.0)
            )
        } else {
            String::new()
        };
        back + &label_svg(label, cx, cy, attrs)
    };
    let left_edge = |col: &Column<'_>, extra: usize| {
        if col.activations.len() + extra > 0 {
            col.x - ACT_W / 2.0
        } else {
            col.x
        }
    };
    let right_edge = |col: &Column<'_>, extra: usize| {
        let depth = col.activations.len() + extra;
        if depth > 0 {
            col.x + ACT_W / 2.0 + (depth - 1) as f64 * ACT_STEP
        } else {
            col.x
        }
    };

    // Shaded blocks and activations close from the inside out; they are written in reverse, so
    // that the inner ones end up on top.
    let mut rects_out: Vec<String> = Vec::new();
    let mut activations_out: Vec<String> = Vec::new();
    let mut frames_out = String::new();
    let mut edges_out = String::new();
    let mut labels_out = String::new();
    let mut notes_out = String::new();
    let mut actors_out = String::new();
    let mut crosses_out = String::new();
    let end_activation = |col: &mut Column<'_>,
                          end: f64,
                          out: &mut Vec<String>,
                          min_x: &mut f64,
                          max_x: &mut f64|
     -> f64 {
        let start = col.activations.pop().unwrap_or(end);
        let bottom = end.max(start + ACT_MIN);
        let x = col.x - ACT_W / 2.0 + col.activations.len() as f64 * ACT_STEP;
        *min_x = min_x.min(x);
        *max_x = max_x.max(x + ACT_W);
        out.push(format!(
            "<rect class=\"pele-rs-activation\" data-id=\"{}\" x=\"{}\" y=\"{}\" width=\"{ACT_W}\" height=\"{}\" rx=\"2\"/>",
            esc_text(col.id),
            num(x),
            num(start),
            num(bottom - start)
        ));
        bottom
    };
    // The lifeline of a destroyed participant ends in a cross a little below the message that
    // ends it.
    let destroy = |col: &mut Column<'_>, line_y: f64, out: &mut String| -> f64 {
        let cx = col.x;
        let cy = line_y + 16.0;
        col.life_end = cy;
        let _ = write!(
            out,
            "<path class=\"pele-rs-destroy\" data-id=\"{}\" d=\"M{},{}L{},{}M{},{}L{},{}\"/>",
            esc_text(col.id),
            num(cx - 6.0),
            num(cy - 6.0),
            num(cx + 6.0),
            num(cy + 6.0),
            num(cx + 6.0),
            num(cy - 6.0),
            num(cx - 6.0),
            num(cy + 6.0)
        );
        cy + 6.0
    };
    let actor_svg = |col: &Column<'_>, top: f64, footer: bool| -> String {
        let class = col
            .actor
            .properties
            .iter()
            .find(|(k, _)| k == "class")
            .and_then(|(_, v)| match v {
                Value::String(s) => Some(class_names(s)),
                _ => None,
            })
            .unwrap_or_default();
        format!(
            "<g class=\"pele-rs-node pele-rs-actor pele-rs-actor-{}{}{class}\" data-id=\"{}\">{}</g>",
            col.kind,
            if footer { " pele-rs-actor-footer" } else { "" },
            esc_text(col.id),
            draw_actor(col.kind, col.x, top, col.size, &col.label)
        )
    };
    let badge = |i: usize, cx: f64, cy: f64, w: f64| -> String {
        format!(
            "<g class=\"pele-rs-sequence-number\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{BADGE_H}\" rx=\"{}\" fill=\"var(--_l)\" \
             stroke=\"none\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" fill=\"var(--_bg)\" stroke=\"none\">{}</text></g>",
            num(cx - w / 2.0),
            num(cy - BADGE_H / 2.0),
            num(w),
            num(BADGE_H / 2.0),
            num(cx),
            num(cy + tiny * 0.35),
            num(tiny),
            esc(&js_number(numbering.index[i]))
        )
    };

    let mut y = head_bottom + 6.0;
    let mut stack: Vec<usize> = Vec::new();
    for i in 0..count {
        let m = &messages[i];
        let kind = m.kind;
        let frame = frames[i];
        let col_from = m.from.as_deref().and_then(|f| col_of.get(f).copied());
        if kind == Some(line::ACTIVE_START) {
            if let Some(c) = col_from {
                cols[c].activations.push(y);
            }
        } else if kind == Some(line::ACTIVE_END) {
            if let Some(c) = col_from {
                if !cols[c].activations.is_empty() {
                    y = y.max(end_activation(
                        &mut cols[c],
                        y,
                        &mut activations_out,
                        &mut min_x,
                        &mut max_x,
                    ));
                }
            }
        } else if let Some(f) = frame.filter(|&f| frame_list[f].dividers[0] == i) {
            stack.push(f);
            let fr = &mut frame_list[f];
            if fr.kind == "rect" {
                fr.top = y + 8.0;
                y = fr.top;
            } else {
                fr.top = y + 12.0;
                y = fr.top + TAB_H.max(fr.title.height + 2.0);
            }
        } else if let Some(f) = frame {
            stack.pop();
            let bottom = y + 12.0;
            let fr = &frame_list[f];
            let x = num(fr.x0);
            let w = num(fr.x1 - fr.x0);
            if fr.kind == "rect" {
                let fill = if fr.fill.is_empty() {
                    String::new()
                } else {
                    resolve_style(&[format!("fill:{}", fr.fill)]).shape
                };
                rects_out.push(format!(
                    "<rect class=\"pele-rs-rect\" x=\"{x}\" y=\"{}\" width=\"{w}\" height=\"{}\"{}/>",
                    num(fr.top),
                    num(bottom - fr.top),
                    rounded(&(if fill.is_empty() { " fill-opacity=\"0.5\"".to_string() } else { String::new() } + &fill))
                ));
            } else {
                let mut inner = format!(
                    "<rect x=\"{x}\" y=\"{}\" width=\"{w}\" height=\"{}\" fill=\"none\"{ROUNDED}/>",
                    num(fr.top),
                    num(bottom - fr.top)
                );
                for (k, &dy) in fr.divider_y.iter().enumerate() {
                    let label = &fr.divider_labels[k];
                    let _ = write!(inner, "<path d=\"M{x},{}H{}\"{DASH}/>", num(dy), num(fr.x1));
                    inner += &backed(
                        label,
                        fr.x0 + 8.0 + label.width / 2.0,
                        dy + 2.0 + label.height / 2.0,
                        " class=\"pele-rs-frame-label\" fill=\"var(--_m)\" stroke=\"none\"",
                    );
                }
                let _ = write!(
                    inner,
                    "<rect x=\"{x}\" y=\"{}\" width=\"{}\" height=\"{TAB_H}\" fill=\"var(--_s)\"{ROUNDED}/><text class=\"pele-rs-frame-kind\" \
                     x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" font-weight=\"var(--_hw)\" fill=\"var(--_m)\" stroke=\"none\">{}</text>",
                    num(fr.top),
                    num(fr.tab_w),
                    num(fr.x0 + fr.tab_w / 2.0),
                    num(fr.top + TAB_H / 2.0 + tiny * 0.35),
                    num(tiny),
                    fr.kind
                );
                inner += &backed(
                    &fr.title,
                    fr.x0 + fr.tab_w + 8.0 + fr.title.width / 2.0,
                    fr.top + TAB_H.max(fr.title.height) / 2.0,
                    " class=\"pele-rs-frame-label\" fill=\"var(--_m)\" stroke=\"none\"",
                );
                let _ = write!(
                    frames_out,
                    "<g class=\"pele-rs-frame pele-rs-frame-{}\">{inner}</g>",
                    fr.kind
                );
            }
            y = bottom;
        } else if matches!(
            kind,
            Some(line::ALT_ELSE | line::PAR_AND | line::CRITICAL_OPTION)
        ) {
            if let Some(&f) = stack.last() {
                let fr = &mut frame_list[f];
                let label_h = fr
                    .divider_labels
                    .get(fr.divider_y.len())
                    .map_or(0.0, |l| l.height);
                fr.divider_y.push(y + 12.0);
                y += 12.0 + if label_h > 0.0 { label_h } else { 0.0 };
            }
        } else if let Some(view) = views[i].as_ref().filter(|v| v.kind == V::Note) {
            let a = &cols[view.a];
            let h = view.label.height + 2.0 * 4f64.max(note_pad - 4.0);
            let top = y + 10.0;
            let mut x = view.x;
            if m.placement == Some(RIGHT_OF) {
                x += right_edge(a, 0) - a.x;
            } else if m.placement == Some(LEFT_OF) {
                x += left_edge(a, 0) - a.x;
            }
            extend(&mut min_x, &mut max_x, x, x + view.w);
            let _ = write!(
                notes_out,
                "<g class=\"pele-rs-note\" data-id=\"i{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{ROUNDED}/>{}</g>",
                m.id,
                num(x),
                num(top),
                num(view.w),
                num(h),
                label_svg(
                    &view.label,
                    x + view.w / 2.0,
                    top + h / 2.0,
                    " class=\"pele-rs-label\" stroke=\"none\" fill=\"var(--_fg)\""
                )
            );
            y = top + h;
        } else if let Some(view) = views[i].as_ref() {
            let Some(st) = line_style(kind) else { continue };
            let (ai, bi) = (view.a, view.b);
            let label = &view.label;
            let bw = badge_width(i);
            // The activation a `+` starts belongs to this message, so the arrow stops at its edge.
            let mut starting = 0;
            for next in &messages[i + 1..] {
                let t = next.kind;
                if t == Some(line::ACTIVE_START) {
                    if next.from == m.to {
                        starting += 1;
                    }
                } else if !matches!(
                    t,
                    Some(
                        line::ACTIVE_END
                            | line::CENTRAL_CONNECTION
                            | line::CENTRAL_CONNECTION_REVERSE
                    )
                ) {
                    break;
                }
            }
            let up = if st.bottom { 1.0 } else { -1.0 };
            let attrs = if st.dotted { DASH } else { "" };
            let cc = m.central;
            let circle_to = cc == line::CENTRAL_CONNECTION || cc == line::CENTRAL_CONNECTION_DUAL;
            let circle_from =
                cc == line::CENTRAL_CONNECTION_REVERSE || cc == line::CENTRAL_CONNECTION_DUAL;
            let circle = |cx: f64, cy: f64| {
                format!(
                    "<circle class=\"pele-rs-central\" cx=\"{}\" cy=\"{}\" r=\"{CENTRAL_R}\" fill=\"var(--_l)\" stroke=\"none\"/>",
                    num(cx),
                    num(cy)
                )
            };
            let tail_at_end = st.at_start && !st.at_end;
            let head_trim = trim(st.head);
            let mut inner = String::new();
            let line_y;

            if view.kind == V::SelfLoop {
                let a = &cols[ai];
                let top = y + msg_gap + 0f64.max((label.height - SELF_H) / 2.0);
                let bottom = top + SELF_H;
                let mut x0 = if circle_from {
                    a.x + CENTRAL_R
                } else {
                    right_edge(a, 0)
                };
                let mut x1 = if circle_to {
                    a.x + CENTRAL_R
                } else {
                    right_edge(a, starting)
                };
                let out = x0.max(x1) + SELF_W + bw / 2.0;
                if circle_from {
                    inner += &circle(a.x, top);
                }
                if circle_to {
                    inner += &circle(a.x, bottom);
                }
                if bw > 0.0 {
                    inner += &badge(
                        i,
                        if tail_at_end { x1 } else { x0 },
                        if tail_at_end { bottom } else { top },
                        bw,
                    );
                    if tail_at_end {
                        x1 += bw / 2.0;
                    } else {
                        x0 += bw / 2.0;
                    }
                }
                // The loop leaves to the right and comes back heading left, which turns a half
                // head over.
                let heads = (if st.at_start {
                    head(st.head, x0, top, -1.0, up)
                } else {
                    String::new()
                }) + &(if st.at_end {
                    head(st.head, x1, bottom, -1.0, -up)
                } else {
                    String::new()
                });
                let from = num(x0 + if st.at_start { head_trim } else { 0.0 });
                let to = num(x1 + if st.at_end { head_trim } else { 0.0 });
                let d = if right_angles {
                    format!("M{from},{}H{}V{}H{to}", num(top), num(out), num(bottom))
                } else {
                    format!(
                        "M{from},{}H{}A6,6 0 0 1 {},{}V{}A6,6 0 0 1 {},{}H{to}",
                        num(top),
                        num(out - 6.0),
                        num(out),
                        num(top + 6.0),
                        num(bottom - 6.0),
                        num(out - 6.0),
                        num(bottom)
                    )
                };
                inner = format!("<path d=\"{d}\"{attrs}/>{heads}{inner}");
                extend(&mut min_x, &mut max_x, a.x, out + 8.0 + label.width);
                if label.width > 0.0 {
                    let _ = write!(
                        labels_out,
                        "<g class=\"pele-rs-edge-label\" data-id=\"i{}\">{}</g>",
                        m.id,
                        label_svg(
                            label,
                            out + 8.0 + label.width / 2.0,
                            (top + bottom) / 2.0,
                            ""
                        )
                    );
                }
                // A label taller than the loop reaches below it, so the next row starts under it.
                line_y = bottom.max((top + bottom) / 2.0 + label.height / 2.0);
            } else {
                let (ax, bx) = (cols[ai].x, cols[bi].x);
                let dir = if bx > ax { 1.0 } else { -1.0 };
                let creating = cols[bi].create_at == Some(i);
                let b_size = cols[bi].size;
                line_y = y
                    + msg_gap
                    + (if label.height > 0.0 {
                        label.height + 2.0
                    } else {
                        6.0
                    })
                    .max(if creating {
                        b_size.h / 2.0 - msg_gap + 6.0
                    } else {
                        0.0
                    });
                let mut x0 = if dir > 0.0 {
                    right_edge(&cols[ai], 0)
                } else {
                    left_edge(&cols[ai], 0)
                };
                let mut x1 = if dir > 0.0 {
                    left_edge(&cols[bi], starting)
                } else {
                    right_edge(&cols[bi], starting)
                };
                if creating {
                    let b = &mut cols[bi];
                    b.top = line_y - b.size.h / 2.0;
                    b.life_start = b.top + b.size.h;
                    x1 = b.x
                        - dir
                            * if is_figure(b.kind) {
                                18.0
                            } else {
                                b.size.w / 2.0 + 1.0
                            };
                }
                if circle_from {
                    inner += &circle(ax, line_y);
                    x0 = ax + dir * CENTRAL_R;
                }
                if circle_to {
                    inner += &circle(bx, line_y);
                    x1 = bx - dir * CENTRAL_R;
                }
                if bw > 0.0 {
                    // The number sits on the lifeline at the tail of the arrow, past a
                    // connection circle if there is one.
                    if tail_at_end {
                        let cx = if circle_to {
                            x1 - dir * (bw / 2.0 + 2.0)
                        } else {
                            x1
                        };
                        inner += &badge(i, cx, line_y, bw);
                        x1 = cx - (dir * bw) / 2.0;
                    } else {
                        let cx = if circle_from {
                            x0 + dir * (bw / 2.0 + 2.0)
                        } else {
                            x0
                        };
                        inner += &badge(i, cx, line_y, bw);
                        x0 = cx + (dir * bw) / 2.0;
                    }
                }
                let heads = (if st.at_start {
                    head(st.head, x0, line_y, -dir, up * dir)
                } else {
                    String::new()
                }) + &(if st.at_end {
                    head(st.head, x1, line_y, dir, up * dir)
                } else {
                    String::new()
                });
                let from = x0 + if st.at_start { dir * head_trim } else { 0.0 };
                let to = x1 - if st.at_end { dir * head_trim } else { 0.0 };
                inner = format!(
                    "<path d=\"M{},{}H{}\"{attrs}/>{heads}{inner}",
                    num(from),
                    num(line_y),
                    num(to)
                );
                if label.width > 0.0 {
                    let l = x0.min(x1);
                    let r = x0.max(x1);
                    let cx = match align {
                        -1 => l + msg_pad + label.width / 2.0,
                        1 => r - msg_pad - label.width / 2.0,
                        _ => (l + r) / 2.0,
                    };
                    extend(
                        &mut min_x,
                        &mut max_x,
                        cx - label.width / 2.0,
                        cx + label.width / 2.0,
                    );
                    let _ = write!(
                        labels_out,
                        "<g class=\"pele-rs-edge-label\" data-id=\"i{}\">{}</g>",
                        m.id,
                        backed(label, cx, line_y - 4.0 - label.height / 2.0, "")
                    );
                }
                if creating {
                    y = line_y + b_size.h / 2.0 - 6.0;
                }
            }
            let _ = write!(
                edges_out,
                "<g class=\"pele-rs-edge pele-rs-message\" data-id=\"i{}\">{inner}</g>",
                m.id
            );
            y = y.max(line_y);
            if cols[ai].destroy_at == Some(i) {
                y = y.max(destroy(&mut cols[ai], line_y, &mut crosses_out));
            }
            if cols[bi].destroy_at == Some(i) && bi != ai {
                y = y.max(destroy(&mut cols[bi], line_y, &mut crosses_out));
            }
        }
    }

    y += 20.0;
    for col in &mut cols {
        while !col.activations.is_empty() {
            end_activation(col, y, &mut activations_out, &mut min_x, &mut max_x);
        }
    }
    let mut bottom = y;
    if mirror {
        let mut foot_h = 0f64;
        for col in &cols {
            if col.life_end >= 0.0 {
                continue;
            }
            foot_h = foot_h.max(col.size.h);
            actors_out += &actor_svg(col, y, true);
        }
        bottom = y + foot_h;
    }
    let mut lifelines = String::new();
    for col in &cols {
        actors_out += &actor_svg(col, col.top, false);
        let end = if col.life_end >= 0.0 { col.life_end } else { y };
        if end > col.life_start {
            let _ = write!(
                lifelines,
                "<path class=\"pele-rs-lifeline\" data-id=\"{}\" d=\"M{},{}V{}\"/>",
                esc_text(col.id),
                num(col.x),
                num(col.life_start),
                num(end)
            );
        }
    }

    let mut boxes_out = String::new();
    for run in &runs {
        let b = &db.boxes[run.boxed];
        let x0 = cols[run.first].x - cols[run.first].size.w / 2.0 - box_pad - run.grow;
        let x1 = cols[run.last].x + cols[run.last].size.w / 2.0 + box_pad + run.grow;
        let fill = if b.fill == "transparent" {
            String::new()
        } else {
            resolve_style(&[format!("fill:{}", b.fill)]).shape
        };
        let _ = write!(
            boxes_out,
            "<g class=\"pele-rs-cluster pele-rs-box\"{}><rect x=\"{}\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\" stroke=\"var(--_b)\"{}/>{}</g>",
            b.name
                .as_deref()
                .filter(|n| !n.is_empty())
                .map_or(String::new(), |n| format!(" data-id=\"{}\"", esc_text(n))),
            num(x0),
            num(x1 - x0),
            num(bottom + box_pad),
            rounded(
                &(if fill.is_empty() {
                    " fill-opacity=\"0.5\"".to_string()
                } else {
                    String::new()
                } + &fill)
            ),
            label_svg(
                &run.title,
                (x0 + x1) / 2.0,
                box_pad / 2.0 + 2.0 + run.title.height / 2.0,
                " class=\"pele-rs-cluster-label\" fill=\"var(--_m)\""
            )
        );
    }
    if !runs.is_empty() {
        bottom += box_pad;
    }

    let mut min_y = 0.0;
    let mut title_out = String::new();
    let room = title_room(options);
    let plain_title = title_label(
        db.title.as_deref(),
        measurer,
        size,
        if tight { room } else { f64::INFINITY },
    );
    let title = if tight {
        break_label(plain_title, measurer, 120f64.max(room))
    } else {
        plain_title
    };
    if min_x > max_x {
        extend(&mut min_x, &mut max_x, 0.0, title.width.max(40.0));
    }
    if title.width > 0.0 {
        let cx = (min_x + max_x) / 2.0;
        extend(
            &mut min_x,
            &mut max_x,
            cx - title.width / 2.0,
            cx + title.width / 2.0,
        );
        min_y = -title.height - 12.0;
        title_out = label_svg(
            &title,
            cx,
            min_y + title.height / 2.0,
            " class=\"pele-rs-title\" font-weight=\"var(--_tw)\"",
        );
    }

    let width = (max_x - min_x + 2.0 * pad).ceil();
    let height = (bottom - min_y + 2.0 * pad).ceil();
    let small_size = num(small);
    let mut content = format!(
        "<g transform=\"translate({},{})\">{title_out}",
        num(pad - min_x),
        num(pad - min_y)
    );
    if !boxes_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-boxes\" font-size=\"{small_size}\">{boxes_out}</g>"
        );
    }
    if !rects_out.is_empty() {
        rects_out.reverse();
        let _ = write!(
            content,
            "<g class=\"pele-rs-rects\" fill=\"var(--_a)\">{}</g>",
            rects_out.join("")
        );
    }
    if !lifelines.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-lifelines\" fill=\"none\" stroke=\"var(--_b)\">{lifelines}</g>"
        );
    }
    if !activations_out.is_empty() {
        activations_out.reverse();
        let _ = write!(
            content,
            "<g class=\"pele-rs-activations\" fill=\"var(--_s)\" stroke=\"var(--_b)\">{}</g>",
            activations_out.join("")
        );
    }
    if !frames_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-frames\" font-size=\"{small_size}\" stroke=\"var(--_b)\">{frames_out}</g>"
        );
    }
    if !edges_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges_out}</g>"
        );
    }
    if !crosses_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-destroys\" fill=\"none\" stroke=\"var(--_l)\" stroke-width=\"1.5\" stroke-linecap=\"round\">{crosses_out}</g>"
        );
    }
    if !labels_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-edge-labels\" font-size=\"{small_size}\">{labels_out}</g>"
        );
    }
    if !notes_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-notes\" font-size=\"{small_size}\" fill=\"var(--_a)\" stroke=\"var(--_b)\">{notes_out}</g>"
        );
    }
    if !actors_out.is_empty() {
        let _ = write!(content, "<g class=\"pele-rs-nodes\">{actors_out}</g>");
    }
    content.push_str("</g>");
    let acc = Accessible {
        title: db.acc_title.clone(),
        descr: db.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            DiagramKind::Sequence,
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
