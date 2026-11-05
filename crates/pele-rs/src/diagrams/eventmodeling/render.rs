//! The drawing of an event modeling diagram: the lanes, the frames in them, and the relations
//! between the frames. Ported from pele's `diagrams/eventmodeling/render.ts`, whose layout
//! pele-rs keeps.
//!
//! Notes and given/when/then specifications are drawn as cards, matching Pele.

use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::error::Error;
use crate::svg::{
    Accessible, FONT_MONO, ROUNDED, esc, esc_text, label_svg, num, series_color, svg_document,
};
use crate::text::Style;
use crate::text::TextMeasurer;
use crate::text::entities::decode_entities;
use crate::text::label::{Label, layout_label};

use super::db::EventModel;
use super::parser::EntityKind;

const BOX_PAD_X: f64 = 12.0;
const BOX_PAD_Y: f64 = 10.0;
const BOX_MIN_WIDTH: f64 = 96.0;
const BOX_GAP: f64 = 16.0;
const STAGGER: f64 = 80.0;
const LANE_PAD: f64 = 12.0;
const LANE_GAP: f64 = 8.0;
const DATA_COLUMNS: usize = 48;
/// The length of an arrowhead.
const ARROW: f64 = 8.0;

/// The series slot of each kind of entity. A user interface has none and is drawn plain.
fn series(kind: EntityKind) -> Option<usize> {
    match kind {
        EntityKind::Command => Some(0),
        EntityKind::Event => Some(1),
        EntityKind::ReadModel => Some(2),
        EntityKind::Processor => Some(6),
        EntityKind::Ui => None,
    }
}

/// The place of a lane in the bands.
fn band_index(bands: &[Band], lane: usize) -> usize {
    bands
        .iter()
        .position(|band| band.lane == lane)
        .unwrap_or_default()
}

/// A lane of the drawing.
struct Band {
    /// The index of the lane in the model.
    lane: usize,
    /// The text of the lane, for its `data-id`.
    text: String,
    label: Label,
    /// Where the last frame of the lane ends.
    right: f64,
    y: f64,
    h: f64,
}

/// A frame, placed.
struct Placed {
    /// The frame of the model this is.
    frame: usize,
    name: Label,
    data: Vec<String>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Splits text into lines of at most `columns` characters, keeping its own line breaks.
fn hard_wrap(text: &str, columns: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in decode_entities(text).split('\n') {
        let chars: Vec<char> = line.chars().collect();
        if chars.len() <= columns {
            out.push(line.to_string());
            continue;
        }
        for chunk in chars.chunks(columns) {
            out.push(chunk.iter().collect());
        }
    }
    out
}

/// The width of the widest of the rows.
fn widest(rows: &[String], size: f64, measurer: &dyn TextMeasurer, style: Style) -> f64 {
    rows.iter()
        .map(|row| measurer.width(row, size, style))
        .fold(0.0, f64::max)
}

/// Writes the rows as text, each on its own line.
fn lines(rows: &[String], x: f64, y: f64, step: f64, attrs: &str) -> String {
    let mut body = String::new();
    for (i, row) in rows.iter().enumerate() {
        if row.is_empty() {
            continue;
        }
        body += &format!(
            "<tspan x=\"{}\" y=\"{}\">{}</tspan>",
            num(x),
            num(y + i as f64 * step),
            esc(row)
        );
    }
    if body.is_empty() {
        return String::new();
    }
    format!("<text{attrs} xml:space=\"preserve\">{body}</text>")
}

/// An arrowhead whose tip is at (x, y), pointing along (dx, dy).
fn marker(x: f64, y: f64, dx: f64, dy: f64) -> String {
    crate::svg::markers::marker("arrow_point", x, y, dx, dy, "var(--_l)")
}

/// Draws an event modeling diagram.
pub(crate) fn render_event_model(model: &EventModel, input: &Input<'_>) -> Result<Output, Error> {
    Ok(crate::diagrams::common::fit::turn_to_fit(
        input.options,
        |down| {
            if down {
                vertical_event_model(model, input)
            } else {
                horizontal_event_model(model, input)
            }
        },
    ))
}

fn horizontal_event_model(model: &EventModel, input: &Input<'_>) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let name_size = (size * 0.875).round();
    let small = (size * 0.8125).round();
    let mono = (size * 0.75).round();
    let mono_step = (mono * 1.4).round();
    let pad = options.padding.unwrap_or(8.0);

    // The label of every lane, and the column they take.
    let mut bands: Vec<Band> = Vec::new();
    let mut label_column = 0.0_f64;
    for lane in &model.lanes {
        let label = layout_label(
            Some(&lane.label),
            false,
            measurer,
            small,
            4000.0,
            Style::BOLD,
        );
        label_column = label_column.max(label.width);
        bands.push(Band {
            lane: lane.index,
            text: lane.label.clone(),
            label,
            right: f64::NEG_INFINITY,
            y: 0.0,
            h: 0.0,
        });
    }
    label_column += 2.0 * LANE_PAD;

    // Time runs left to right. A frame in a new lane starts under the end of the one before it,
    // so the arrow between them can drop straight down; a frame in the same lane follows on.
    let mut placed: Vec<Placed> = Vec::new();
    let mut previous: Option<(usize, f64, f64)> = None;
    for (index, item) in model.boxes.iter().enumerate() {
        let band = band_index(&bands, item.lane);
        let name = layout_label(
            Some(&item.name),
            false,
            measurer,
            name_size,
            4000.0,
            Style::BOLD,
        );
        let data = item
            .data
            .as_deref()
            .map(|data| hard_wrap(data, DATA_COLUMNS))
            .unwrap_or_default();
        let w = BOX_MIN_WIDTH
            .max(name.width.max(widest(&data, mono, measurer, Style::MONO)) + 2.0 * BOX_PAD_X);
        let h = 2.0 * BOX_PAD_Y
            + name.height
            + if data.is_empty() {
                0.0
            } else {
                6.0 + data.len() as f64 * mono_step
            };
        let mut x = 0.0;
        if let Some((previous_lane, previous_x, previous_w)) = previous {
            let stagger = previous_x + (previous_w / 2.0).max(previous_w - STAGGER);
            x = if previous_lane == item.lane {
                bands[band].right + BOX_GAP
            } else {
                stagger.max(bands[band].right + BOX_GAP)
            };
        }
        bands[band].right = x + w;
        bands[band].h = bands[band].h.max(h);
        placed.push(Placed {
            frame: index,
            name,
            data,
            x,
            y: 0.0,
            w,
            h,
        });
        previous = Some((item.lane, x, w));
    }

    let content_right = placed
        .iter()
        .map(|item| item.x + item.w)
        .fold(0.0_f64, f64::max);
    let lane_width = label_column + content_right + LANE_PAD;

    let title = layout_label(
        model.title.as_deref(),
        false,
        measurer,
        size,
        4000.0,
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let left = pad;
    let top = pad + title_height;

    let mut y = top;
    let mut lanes_svg = String::new();
    for band in &mut bands {
        band.y = y;
        band.h += 2.0 * LANE_PAD;
        lanes_svg += &format!(
            "<g class=\"pele-cluster pele-em-lane\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" \
             fill=\"var(--_s)\"{ROUNDED}/>{}</g>",
            esc_text(&band.text),
            num(left),
            num(y),
            num(lane_width),
            num(band.h),
            label_svg(
                &band.label,
                left + LANE_PAD + band.label.width / 2.0,
                y + band.h / 2.0,
                &format!(
                    " class=\"pele-cluster-label\" font-size=\"{}\" font-weight=\"var(--_hw)\" \
                     fill=\"var(--_m)\"",
                    num(small)
                )
            )
        );
        y += band.h + LANE_GAP;
    }
    let mut bottom = if bands.is_empty() { top } else { y - LANE_GAP };

    let mut nodes = String::new();
    for item in &mut placed {
        let item_box = &model.boxes[item.frame];
        let band = band_index(&bands, item_box.lane);
        item.x += left + label_column;
        item.y = bands[band].y + (bands[band].h - item.h) / 2.0;
        let frame = format!(
            "x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
            num(item.x),
            num(item.y),
            num(item.w),
            num(item.h)
        );
        let shape = match series(item_box.kind) {
            None => format!("<rect {frame} fill=\"var(--_bg)\" stroke=\"var(--_b)\"{ROUNDED}/>"),
            Some(slot) => {
                let color = series_color(slot);
                format!(
                    "<rect {frame} fill=\"var(--_bg)\"{ROUNDED}/><rect {frame} fill=\"{color}\" \
                     fill-opacity=\"0.22\" stroke=\"{color}\"{ROUNDED}/>"
                )
            }
        };
        let name_y = item.y + BOX_PAD_Y + item.name.height / 2.0;
        let text = label_svg(
            &item.name,
            item.x + item.w / 2.0,
            name_y,
            &format!(
                " class=\"pele-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
                num(name_size)
            ),
        ) + &lines(
            &item.data,
            item.x + BOX_PAD_X,
            item.y + BOX_PAD_Y + item.name.height + 6.0 + mono_step / 2.0 + mono * 0.35,
            mono_step,
            &format!(
                " class=\"pele-em-data\" font-family=\"{FONT_MONO}\" font-size=\"{}\"",
                num(mono)
            ),
        );
        nodes += &format!(
            "<g class=\"pele-node pele-em-{}{}\" data-id=\"{}\">{shape}{text}</g>",
            item_box.kind.name(),
            if item_box.reset { " pele-em-reset" } else { "" },
            esc_text(&item_box.id)
        );
    }

    let mut edges = String::new();
    let trim = ARROW - 1.0;
    for relation in &model.relations {
        let Some(source) = placed
            .iter()
            .find(|item| model.boxes[item.frame].id == relation.source)
        else {
            continue;
        };
        let Some(target) = placed
            .iter()
            .find(|item| model.boxes[item.frame].id == relation.target)
        else {
            continue;
        };
        if source.frame == target.frame {
            continue;
        }
        let (d, head) = if model.boxes[source.frame].lane == model.boxes[target.frame].lane {
            // Along the lane, from the side of one box to the side of the other.
            let forward = target.x >= source.x;
            let sx = if forward {
                source.x + source.w
            } else {
                source.x
            };
            let tx = if forward {
                target.x
            } else {
                target.x + target.w
            };
            let sy = source.y + source.h / 2.0;
            let ty = target.y + target.h / 2.0;
            let dir = if tx >= sx { 1.0 } else { -1.0 };
            (
                format!(
                    "M{},{}L{},{}",
                    num(sx),
                    num(sy),
                    num(tx - dir * trim),
                    num(ty)
                ),
                marker(tx, ty, dir, 0.0),
            )
        } else {
            // Between lanes, straight down or up where the boxes overlap, else in an S.
            let down = target.y > source.y;
            let from = source.x.max(target.x);
            let to = (source.x + source.w).min(target.x + target.w);
            let inset = |w: f64| 24.0_f64.min(w / 4.0);
            let (sx, tx) = if to - from >= 16.0 {
                let middle = (from + to) / 2.0;
                (middle, middle)
            } else if target.x > source.x {
                (
                    source.x + source.w - inset(source.w),
                    target.x + inset(target.w),
                )
            } else {
                (
                    source.x + inset(source.w),
                    target.x + target.w - inset(target.w),
                )
            };
            let sy = if down { source.y + source.h } else { source.y };
            let ty = if down { target.y } else { target.y + target.h };
            let dir = if down { 1.0 } else { -1.0 };
            let end = ty - dir * trim;
            let middle = (sy + end) / 2.0;
            let d = if sx == tx {
                format!("M{},{}V{}", num(sx), num(sy), num(end))
            } else {
                format!(
                    "M{},{}C{},{} {},{} {},{}",
                    num(sx),
                    num(sy),
                    num(sx),
                    num(middle),
                    num(tx),
                    num(middle),
                    num(tx),
                    num(end)
                )
            };
            (d, marker(tx, ty, 0.0, dir))
        };
        edges += &format!(
            "<g class=\"pele-edge\" data-id=\"{}-{}\"><path d=\"{d}\" fill=\"none\" \
             stroke=\"var(--_l)\"/>{head}</g>",
            esc_text(&relation.source),
            esc_text(&relation.target)
        );
    }

    let small_step = (small * 1.5).round();
    let mut cards = String::new();
    let mut cards_right = 0.0_f64;
    let mut card = |kind: &str, heading: &str, frame: &str, rows: Vec<String>| {
        let name = model
            .boxes
            .iter()
            .find(|b| b.id == frame)
            .map_or(String::new(), |b| format!(" {}", b.name));
        let head = layout_label(
            Some(&format!("{heading} · {frame}{name}")),
            false,
            measurer,
            small,
            4000.0,
            Style::BOLD,
        );
        let w = head.width.max(widest(&rows, small, measurer, Style::NONE)) + 2.0 * BOX_PAD_X;
        let h = 2.0 * BOX_PAD_Y + head.height + rows.len() as f64 * small_step;
        let cy = bottom
            + if cards.is_empty() && bands.is_empty() {
                0.0
            } else {
                8.0
            };
        cards += &format!(
            "<g class=\"pele-em-{kind}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{ROUNDED}/>{}{}</g>",
            esc_text(frame),
            num(left),
            num(cy),
            num(w),
            num(h),
            label_svg(
                &head,
                left + BOX_PAD_X + head.width / 2.0,
                cy + BOX_PAD_Y + head.height / 2.0,
                &format!(" font-size=\"{}\" font-weight=\"var(--_hw)\"", num(small))
            ),
            lines(
                &rows,
                left + BOX_PAD_X,
                cy + BOX_PAD_Y + head.height + small_step / 2.0 + small * 0.35,
                small_step,
                &format!(" font-size=\"{}\"", num(small))
            )
        );
        bottom = cy + h;
        cards_right = cards_right.max(w);
    };
    for note in &model.notes {
        card("note", "Note", &note.frame, hard_wrap(&note.text, 80));
    }
    for spec in &model.specifications {
        card(
            "specification",
            "Given / When / Then",
            &spec.frame,
            spec_rows(spec)
                .into_iter()
                .flat_map(|s| hard_wrap(&s, 80))
                .collect(),
        );
    }
    let width = (if bands.is_empty() { 0.0 } else { lane_width })
        .max(cards_right)
        .max(title.width);
    let total_width = (width + 2.0 * pad).ceil();
    let total_height = (bottom + pad).ceil();
    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &lanes_svg
        + &edges
        + &nodes
        + &cards;

    let acc = Accessible {
        title: model.acc_title.clone(),
        descr: model.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            DiagramKind::EventModeling,
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

fn spec_rows(spec: &super::db::Specification) -> Vec<String> {
    let mut rows = vec![format!("Given  {}", spec.given.join(", "))];
    if !spec.when.is_empty() {
        rows.push(format!("When  {}", spec.when.join(", ")));
    }
    rows.push(format!("Then  {}", spec.then.join(", ")));
    rows
}

fn vertical_event_model(model: &EventModel, input: &Input<'_>) -> Output {
    use crate::diagrams::common::literal::{break_label, left_label_svg, literal_label};
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let small = (size * 0.875).round();
    let mono = (size * 0.75).round();
    let pad = options.padding.unwrap_or(8.0);
    let room = 160.0_f64.max(options.max_width.unwrap_or(320.0) - 2.0 * pad);
    let index_of = |id: &str| model.boxes.iter().position(|b| b.id == id);
    let mut passed = vec![false; model.boxes.len()];
    let mut runs: Vec<(usize, usize, usize)> = Vec::new();
    for r in &model.relations {
        let (Some(s), Some(t)) = (index_of(&r.source), index_of(&r.target)) else {
            continue;
        };
        if s == t {
            continue;
        }
        if s.abs_diff(t) < 2 {
            passed[s.max(t)] = true;
            continue;
        }
        if let Some(run) = runs.iter_mut().find(|run| run.0 == t) {
            run.1 = run.1.min(s);
            run.2 = run.2.max(s);
        } else {
            runs.push((t, s.min(t), s.max(t)));
        }
    }
    runs.sort_by_key(|run| run.1);
    let mut rail_ends: Vec<usize> = Vec::new();
    let mut rail_of = vec![0; model.boxes.len()];
    for (target, from, to) in runs {
        let rail = rail_ends
            .iter()
            .position(|&end| end < from)
            .unwrap_or(rail_ends.len());
        if rail == rail_ends.len() {
            rail_ends.push(to);
        } else {
            rail_ends[rail] = to;
        }
        rail_of[target] = rail;
    }
    let gutter = 24.0 + 8.0 * rail_ends.len().saturating_sub(1) as f64;
    let card_width = 120.0_f64.max(room - gutter);
    let text_room = card_width - 2.0 * BOX_PAD_X;
    let title = layout_label(
        model.title.as_deref(),
        false,
        measurer,
        size,
        room,
        Style::BOLD,
    );
    let mut y = pad
        + if title.height > 0.0 {
            title.height + 12.0
        } else {
            0.0
        };
    let mut bottom = pad + title.height;
    let mut width = title.width.max(card_width + gutter);
    let mut nodes = String::new();
    let mut edges = String::new();
    let mut cards = String::new();
    let mut placed: Vec<(f64, f64, f64, f64)> = Vec::new();
    let text = |raw: &str, font_size: f64, style: Style| {
        literal_label(&decode_entities(raw), measurer, font_size, text_room, style)
    };
    for (index, b) in model.boxes.iter().enumerate() {
        let lane_room = text_room - if passed[index] { 20.0 } else { 0.0 };
        let lane_text = &model
            .lanes
            .iter()
            .find(|l| l.index == b.lane)
            .unwrap()
            .label;
        let lane = break_label(
            layout_label(
                Some(lane_text),
                false,
                measurer,
                small,
                lane_room,
                Style::BOLD,
            ),
            measurer,
            lane_room,
        );
        let name = layout_label(
            Some(&b.name),
            false,
            measurer,
            small,
            text_room - 36.0,
            Style::BOLD,
        );
        let data = b
            .data
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|raw| text(raw, mono, Style::MONO));
        let w = card_width
            .max(lane.width + 2.0 * BOX_PAD_X)
            .max(name.width + 2.0 * BOX_PAD_X + 36.0)
            .max(data.as_ref().map_or(0.0, |l| l.width) + 2.0 * BOX_PAD_X);
        let h = 2.0 * BOX_PAD_Y + name.height + data.as_ref().map_or(0.0, |l| l.height + 6.0);
        let cy = y + lane.height + 12.0;
        let frame = format!(
            "x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
            num(pad),
            num(cy),
            num(w),
            num(h)
        );
        nodes += &format!(
            "<g class=\"pele-cluster pele-em-lane\" data-id=\"{}\">{}<g class=\"pele-node pele-em-{}{}\" data-id=\"{}\"><rect {frame} fill=\"var(--_bg)\" stroke=\"var(--_b)\"{ROUNDED}/>",
            esc_text(lane_text),
            left_label_svg(
                &lane,
                pad + BOX_PAD_X,
                y,
                &format!(
                    " class=\"pele-cluster-label\" font-size=\"{}\" font-weight=\"var(--_hw)\" fill=\"var(--_m)\"",
                    num(small)
                )
            ),
            b.kind.name(),
            if b.reset { " pele-em-reset" } else { "" },
            esc_text(&b.id)
        );
        if let Some(series) = series(b.kind) {
            let color = series_color(series);
            nodes += &format!(
                "<rect {frame} fill=\"{color}\" fill-opacity=\"0.22\" stroke=\"{color}\"{ROUNDED}/>"
            );
        }
        nodes += &label_svg(
            &name,
            pad + BOX_PAD_X + name.width / 2.0,
            cy + BOX_PAD_Y + name.height / 2.0,
            &format!(
                " class=\"pele-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
                num(small)
            ),
        );
        nodes += &format!(
            "<text class=\"pele-em-frame\" x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"{}\" fill=\"var(--_m)\">{}</text>",
            num(pad + w - BOX_PAD_X),
            num(cy + BOX_PAD_Y + small),
            num(mono),
            esc(&b.id)
        );
        if let Some(data) = data {
            nodes += &lines(
                &label_rows(&data),
                pad + BOX_PAD_X,
                cy + BOX_PAD_Y + name.height + 6.0 + data.line_height / 2.0 + mono * 0.35,
                data.line_height,
                &format!(
                    " class=\"pele-em-data\" font-family=\"{FONT_MONO}\" font-size=\"{}\"",
                    num(mono)
                ),
            );
        }
        nodes += "</g></g>";
        placed.push((cy, h, w, lane.width));
        width = width.max(w + gutter);
        bottom = cy + h;
        y = bottom + 28.0;
    }
    for relation in &model.relations {
        let (Some(si), Some(ti)) = (index_of(&relation.source), index_of(&relation.target)) else {
            continue;
        };
        if si == ti {
            continue;
        }
        let (s, t) = (placed[si], placed[ti]);
        let down = t.0 > s.0;
        let dir = if down { 1.0 } else { -1.0 };
        let sy = if down { s.0 + s.1 } else { s.0 };
        let ty = if down { t.0 } else { t.0 + t.1 };
        let (d, head) = if si.abs_diff(ti) == 1 {
            let cx =
                pad + (s.2.min(t.2) / 2.0).max(BOX_PAD_X + if down { t.3 } else { s.3 } + 16.0);
            (
                format!("M{},{}V{}", num(cx), num(sy), num(ty - dir * (ARROW - 1.0))),
                marker(cx, ty, 0.0, dir),
            )
        } else {
            let rail = pad + width - gutter + 12.0 + 8.0 * rail_of[ti] as f64;
            let from = s.0 + s.1 / 2.0 - 5.0;
            let to = t.0 + t.1 / 2.0 + 5.0;
            (
                format!(
                    "M{},{}H{}V{}H{}",
                    num(pad + s.2),
                    num(from),
                    num(rail),
                    num(to),
                    num(pad + t.2 + ARROW - 1.0)
                ),
                marker(pad + t.2, to, -1.0, 0.0),
            )
        };
        edges += &format!(
            "<g class=\"pele-edge\" data-id=\"{}-{}\"><path d=\"{d}\" fill=\"none\" stroke=\"var(--_l)\"/>{head}</g>",
            esc_text(&relation.source),
            esc_text(&relation.target)
        );
    }
    let mut card = |kind: &str, heading: &str, frame: &str, raw: &str| {
        let name = model
            .boxes
            .iter()
            .find(|b| b.id == frame)
            .map_or(String::new(), |b| format!(" {}", b.name));
        let head = layout_label(
            Some(&format!("{heading} · {frame}{name}")),
            false,
            measurer,
            small,
            text_room,
            Style::BOLD,
        );
        let body = text(raw, small, Style::NONE);
        let w = card_width
            .max(head.width + 2.0 * BOX_PAD_X)
            .max(body.width + 2.0 * BOX_PAD_X);
        let h = 2.0 * BOX_PAD_Y + head.height + body.height;
        cards += &format!(
            "<g class=\"pele-em-{kind}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{ROUNDED}/>{}{}</g>",
            esc_text(frame),
            num(pad),
            num(y),
            num(w),
            num(h),
            label_svg(
                &head,
                pad + BOX_PAD_X + head.width / 2.0,
                y + BOX_PAD_Y + head.height / 2.0,
                &format!(" font-size=\"{}\" font-weight=\"var(--_hw)\"", num(small))
            ),
            lines(
                &label_rows(&body),
                pad + BOX_PAD_X,
                y + BOX_PAD_Y + head.height + body.line_height / 2.0 + small * 0.35,
                body.line_height,
                &format!(" font-size=\"{}\"", num(small))
            )
        );
        bottom = y + h;
        y = bottom + 8.0;
        width = width.max(w);
    };
    for note in &model.notes {
        card("note", "Note", &note.frame, &note.text);
    }
    for spec in &model.specifications {
        card(
            "specification",
            "Given / When / Then",
            &spec.frame,
            &spec_rows(spec).join("\n"),
        );
    }
    let total_width = (width + 2.0 * pad).ceil();
    let height = (bottom + pad).ceil();
    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &edges
        + &nodes
        + &cards;
    Output {
        svg: svg_document(
            DiagramKind::EventModeling,
            total_width,
            height,
            size,
            options,
            &Accessible {
                title: model.acc_title.clone(),
                descr: model.acc_descr.clone(),
            },
            &content,
        ),
        width: total_width,
        height,
        links: Vec::new(),
    }
}
fn label_rows(label: &Label) -> Vec<String> {
    label
        .lines
        .iter()
        .map(|line| line.iter().map(|span| span.text.as_str()).collect())
        .collect()
}
