//! Gantt chart layout and SVG. Ported from pele's `diagrams/gantt/render.ts`.

use std::collections::HashMap;
use std::fmt::Write;

use super::axis::{auto_ticks, interval_ticks, time_format};
use super::dates::{Date, Unit, add, add_days};
use super::db::{GanttDb, Task};
use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::cap_width;
use crate::error::Error;
use crate::preprocess::{Config, Value};
use crate::svg::{
    Accessible, CRITICAL, ROUNDED, esc, esc_text, label_svg, num, resolve_style, svg_document,
};
use crate::text::label::{Label, layout_label};
use crate::text::{Style, TextMeasurer};
use crate::url::linked;
use crate::util::{js_number, round};

const WIDTH: f64 = 800.0;
const MIN_PLOT: f64 = 240.0;
const LABEL_GAP: f64 = 8.0;

fn gantt(config: &Config) -> Option<&Config> {
    match config.get("gantt") {
        Some(Value::Map(m)) => Some(m),
        _ => None,
    }
}

fn setting(config: &Config, key: &str, min: f64, max: f64) -> Option<f64> {
    match gantt(config)?.get(key) {
        Some(Value::Number(n)) if n.is_finite() => Some(n.max(min).min(max)),
        _ => None,
    }
}

fn text<'a>(config: &'a Config, key: &str) -> Option<&'a str> {
    match gantt(config)?.get(key) {
        Some(Value::String(s)) => Some(s),
        _ => None,
    }
}

fn flag(config: &Config, key: &str) -> bool {
    gantt(config).is_some_and(|g| g.get(key) == Some(&Value::Bool(true)))
}

/// `raw.replace(/<br\s*\/?>/gi, ' ')`.
fn break_to_space(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    let b = lower.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    let mut from = 0;
    while i < b.len() {
        if b[i..].starts_with(b"<br") {
            let mut k = i + 3;
            while k < b.len() && crate::util::is_space(b[k] as char) && b[k].is_ascii() {
                k += 1;
            }
            if b.get(k) == Some(&b'/') {
                k += 1;
            }
            if b.get(k) == Some(&b'>') {
                out.push_str(&raw[from..i]);
                out.push(' ');
                i = k + 1;
                from = i;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&raw[from..]);
    out
}

/// A task label sits on one row, so only its first line is kept.
fn one_line(raw: &str, measurer: &dyn TextMeasurer, size: f64) -> Label {
    let label = layout_label(
        Some(&break_to_space(raw)),
        false,
        measurer,
        size,
        4000.0,
        Style::NONE,
    );
    if label.lines.len() <= 1 {
        return label;
    }
    Label {
        lines: vec![label.lines[0].clone()],
        widths: vec![label.widths[0]],
        width: label.widths[0],
        height: label.line_height,
        ..label
    }
}

/// Whether scheduling gave the task a start and an end. One that waits on a task that never
/// resolves has neither.
fn scheduled(task: &Task) -> bool {
    task.start_time.is_some_and(Date::valid) && task.end_time.is_some_and(Date::valid)
}

fn start(task: &Task) -> f64 {
    task.start_time.map_or(f64::NAN, Date::time)
}

fn end(task: &Task) -> f64 {
    task.end_time.map_or(f64::NAN, Date::time)
}

/// Mermaid's compact mode: within a section, a task goes on the first row that is free when
/// it starts.
fn pack_rows(tasks: &[&Task], offset: usize, row_of: &mut HashMap<*const Task, usize>) -> usize {
    let mut sorted: Vec<&Task> = tasks.iter().copied().filter(|t| scheduled(t)).collect();
    sorted.sort_by(|a, b| start(a).total_cmp(&start(b)).then(a.order.cmp(&b.order)));
    let mut free: Vec<f64> = Vec::new();
    for task in sorted {
        let s = start(task);
        let mut row = 0;
        while row < free.len() && s < free[row] {
            row += 1;
        }
        if row == free.len() {
            free.push(end(task));
        } else {
            free[row] = end(task);
        }
        row_of.insert(std::ptr::from_ref(task), offset + row);
    }
    let mut rows = free.len();
    for &task in tasks {
        row_of.entry(std::ptr::from_ref(task)).or_insert_with(|| {
            rows += 1;
            offset + rows - 1
        });
    }
    rows.max(1)
}

struct Bar<'a> {
    task: &'a Task,
    label: Label,
    row: usize,
    x0: f64,
    x1: f64,
    label_x: f64,
    inside: bool,
}

pub(crate) fn render_gantt(db: &mut GanttDb, input: &Input<'_>) -> Result<Output, Error> {
    let options = input.options;
    let config = input.config;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let pad = options.padding.unwrap_or(8.0);
    let now = options.now.filter(|n| n.is_finite()).unwrap_or(0.0);
    let acc = Accessible {
        title: db.acc_title.clone(),
        descr: db.acc_descr.clone(),
    };

    let label_size = setting(config, "fontSize", 4.0, 72.0).unwrap_or(round(size * 0.8125));
    let section_size = setting(config, "sectionFontSize", 4.0, 72.0).unwrap_or(label_size);
    let tick_size = round(size * 0.75);
    let bar_height = setting(config, "barHeight", 4.0, 400.0).unwrap_or(round(label_size * 1.85));
    let bar_gap = setting(config, "barGap", 0.0, 400.0).unwrap_or(round(bar_height / 4.0));
    let row_height = bar_height + bar_gap;
    let section_styles = round(setting(config, "numberSectionStyles", 1.0, 1000.0).unwrap_or(4.0));
    let compact = db.display_mode == "compact";
    let top_axis = db.top_axis || flag(config, "topAxis");

    db.schedule(now)?;
    // Taken out so that marking excluded days, which counts its work on the model, can borrow
    // it while the bars hold on to their tasks.
    let tasks = std::mem::take(&mut db.tasks);
    let row_tasks: Vec<&Task> = tasks.iter().filter(|t| !t.vert).collect();
    let mut verts: Vec<&Task> = tasks.iter().filter(|t| t.vert && scheduled(t)).collect();

    let mut categories: HashMap<&str, usize> = HashMap::new();
    for task in &row_tasks {
        let next = categories.len();
        categories.entry(task.section.as_str()).or_insert(next);
    }

    let mut row_of: HashMap<*const Task, usize> = HashMap::new();
    let mut row_section: Vec<&str> = Vec::new();
    if compact {
        let mut order: Vec<&str> = Vec::new();
        let mut groups: HashMap<&str, Vec<&Task>> = HashMap::new();
        for &task in &row_tasks {
            let group = groups.entry(task.section.as_str()).or_default();
            if group.is_empty() {
                order.push(task.section.as_str());
            }
            group.push(task);
        }
        for section in order {
            let rows = pack_rows(&groups[section], row_section.len(), &mut row_of);
            row_section.extend(std::iter::repeat_n(section, rows));
        }
    } else {
        for &task in &row_tasks {
            row_of.insert(std::ptr::from_ref(task), row_section.len());
            row_section.push(task.section.as_str());
        }
    }
    let row_count = row_section.len();

    let mut first = f64::INFINITY;
    let mut stop = f64::NEG_INFINITY;
    for task in tasks.iter().filter(|t| scheduled(t)) {
        first = first.min(start(task));
        stop = stop.max(end(task));
    }
    let start_time = first;
    let has_plot = start_time <= stop;

    let title = layout_label(
        db.title.as_deref(),
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
    let title_svg = |width: f64| {
        label_svg(
            &title,
            width / 2.0,
            pad + title.height / 2.0,
            " class=\"pele-rs-title\" font-weight=\"var(--_tw)\"",
        )
    };

    if !has_plot {
        let width = (title.width.max(1.0) + 2.0 * pad).ceil();
        let height = (title.height.max(1.0) + 2.0 * pad).ceil();
        let content = title_svg(width);
        return Ok(Output {
            svg: svg_document(
                DiagramKind::Gantt,
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
        });
    }

    let mut section_labels: HashMap<&str, Label> = HashMap::new();
    let mut section_width: f64 = 0.0;
    for &section in categories.keys() {
        let label = layout_label(
            Some(section),
            false,
            measurer,
            section_size,
            180.0,
            Style::NONE,
        );
        section_width = section_width.max(label.width);
        section_labels.insert(section, label);
    }

    let axis_format = if !db.axis_format.is_empty() {
        db.axis_format.as_str()
    } else if db.date_format == "D" {
        "%d"
    } else {
        text(config, "axisFormat").unwrap_or("%Y-%m-%d")
    };
    let tick_label = |t: f64| time_format(axis_format, Date::new(t));
    let sample_width = measurer
        .width(&tick_label(start_time), tick_size, Style::NONE)
        .max(measurer.width(&tick_label(stop), tick_size, Style::NONE));

    let left = pad
        + setting(config, "leftPadding", 0.0, 2000.0).unwrap_or_else(|| {
            (if section_width > 0.0 {
                section_width + 24.0
            } else {
                0.0
            })
            .max(sample_width / 2.0)
        });
    let right_padding = setting(config, "rightPadding", 0.0, 2000.0)
        .unwrap_or_else(|| 16f64.max(sample_width / 2.0));
    let target = cap_width(
        options,
        setting(config, "useWidth", 100.0, 20000.0).unwrap_or(WIDTH),
        280.0,
    );
    let plot_width = MIN_PLOT.max(target - left - right_padding - pad);
    let right = left + plot_width;
    let scale = if stop > start_time {
        plot_width / (stop - start_time)
    } else {
        0.0
    };
    let x = |t: f64| {
        if scale == 0.0 {
            left + plot_width / 2.0
        } else {
            left + (t - start_time) * scale
        }
    };

    let tick_height = round(tick_size * 1.75);
    let top_padding = setting(config, "topPadding", 0.0, 2000.0);
    let grid_start = setting(config, "gridLineStartPadding", 0.0, 2000.0);
    let head_height = pad + title_height + if top_axis { tick_height } else { 0.0 };
    let plot_top = head_height.max(top_padding.unwrap_or(0.0));
    // Mermaid starts its grid lines above the first row by the difference between these two
    // settings. Here that only uses room a larger `topPadding` leaves free.
    let grid_lead = match (top_padding, grid_start) {
        (Some(top), Some(grid)) => (top - grid).min(plot_top - head_height).max(0.0),
        _ => 0.0,
    };
    let plot_height = row_count.max(1) as f64 * row_height;
    let plot_bottom = plot_top + plot_height;

    // Bars and where their labels go: inside when they fit, otherwise beside the bar on the
    // side with room.
    let mut bars = Vec::new();
    let mut extent = right + right_padding;
    for &task in &row_tasks {
        let label = one_line(&task.task, measurer, label_size);
        let row = row_of[&(std::ptr::from_ref(task))];
        if !scheduled(task) {
            let label_x = left + LABEL_GAP + label.width / 2.0;
            extent = extent.max(label_x + label.width / 2.0);
            bars.push(Bar {
                task,
                label,
                row,
                x0: f64::NAN,
                x1: f64::NAN,
                label_x,
                inside: false,
            });
            continue;
        }
        let mut x0 = x(start(task));
        let render_end = task.render_end_time.map_or(end(task), Date::time);
        let mut x1 = x(render_end).max(x0 + if task.milestone { 0.0 } else { 2.0 });
        if task.milestone {
            let mid = x0 + (x(end(task)) - x0) / 2.0;
            x0 = mid - bar_height / 2.0;
            x1 = mid + bar_height / 2.0;
        }
        let inside = !task.milestone && label.width + 2.0 * LABEL_GAP <= x1 - x0;
        let mut label_x = (x0 + x1) / 2.0;
        if !inside {
            let fits_right = x1 + LABEL_GAP + label.width <= right + right_padding;
            let fits_left = x0 - LABEL_GAP - label.width >= left;
            label_x = if fits_right || !fits_left {
                x1 + LABEL_GAP + label.width / 2.0
            } else {
                x0 - LABEL_GAP - label.width / 2.0
            };
            extent = extent.max(label_x + label.width / 2.0);
        }
        bars.push(Bar {
            task,
            label,
            row,
            x0,
            x1,
            label_x,
            inside,
        });
    }

    let interval = db
        .tick_interval
        .as_deref()
        .filter(|i| !i.is_empty())
        .or(text(config, "tickInterval"))
        .unwrap_or("");
    let weekday = db.weekday.or(text(config, "weekday")).unwrap_or("sunday");
    let count = 2f64.max(10f64.min((plot_width / (sample_width + 24.0)).floor()));
    let ticks = interval_ticks(
        interval,
        weekday,
        start_time,
        stop,
        (plot_width / 3.0).ceil(),
    )
    .unwrap_or_else(|| auto_ticks(start_time, stop, count));

    let mut grid = String::new();
    let mut axis_bottom = String::new();
    let mut axis_top = String::new();
    let mut last_right = f64::NEG_INFINITY;
    let mut last_label = String::new();
    let bottom_y = plot_bottom + tick_height / 2.0 + 2.0;
    let top_y = plot_top - grid_lead - tick_height / 2.0 - 2.0;
    for &t in &ticks {
        let tx = x(t);
        let _ = write!(
            grid,
            "M{},{}V{}",
            num(tx),
            num(plot_top - grid_lead),
            num(plot_bottom)
        );
        let label = tick_label(t);
        let width = measurer.width(&label, tick_size, Style::NONE);
        // A label is skipped when it would run into the one before it or only repeat it; its
        // grid line stays.
        if tx - width / 2.0 < last_right + 8.0 || label == last_label {
            continue;
        }
        last_right = tx + width / 2.0;
        extent = extent.max(last_right);
        let body = format!("text-anchor=\"middle\">{}</text>", esc(&label));
        last_label = label;
        let _ = write!(
            axis_bottom,
            "<text x=\"{}\" y=\"{}\" {body}",
            num(tx),
            num(bottom_y + tick_size * 0.35)
        );
        if top_axis {
            let _ = write!(
                axis_top,
                "<text x=\"{}\" y=\"{}\" {body}",
                num(tx),
                num(top_y + tick_size * 0.35)
            );
        }
    }

    // Vertical markers take no row. Their labels go under the axis, on as many lines as it
    // takes to keep them apart.
    let mut vert_lines = String::new();
    let mut vert_labels = String::new();
    let mut vert_ends: Vec<f64> = Vec::new();
    let vert_top = plot_bottom + tick_height + 4.0;
    let vert_line = round(label_size * 1.5);
    verts.sort_by(|a, b| start(a).total_cmp(&start(b)));
    for task in verts {
        let vx = x(start(task));
        let label = one_line(&task.task, measurer, label_size);
        let cx = (pad + label.width / 2.0).max(vx);
        let mut line = 0;
        while line < vert_ends.len() && cx - label.width / 2.0 < vert_ends[line] + 8.0 {
            line += 1;
        }
        let right_end = cx + label.width / 2.0;
        if line == vert_ends.len() {
            vert_ends.push(right_end);
        } else {
            vert_ends[line] = right_end;
        }
        extent = extent.max(right_end);
        let id = esc_text(&task.id);
        let _ = write!(
            vert_lines,
            "<path class=\"pele-rs-vert\" data-id=\"{id}\" d=\"M{},{}V{}\"/>",
            num(vx),
            num(plot_top - grid_lead),
            num(plot_bottom)
        );
        vert_labels += &label_svg(
            &label,
            cx,
            vert_top + line as f64 * vert_line + vert_line / 2.0,
            &format!(" class=\"pele-rs-vert-label\" data-id=\"{id}\""),
        );
    }

    let width = (extent.max(pad + title.width) + pad).ceil();
    let height = (plot_bottom
        + tick_height
        + if vert_ends.is_empty() {
            0.0
        } else {
            4.0 + vert_ends.len() as f64 * vert_line
        }
        + pad)
        .ceil();

    // Section bands, one per run of rows that share a section, shaded on alternate sections.
    let mut bands = String::new();
    let mut section_out = String::new();
    let mut row = 0;
    while row < row_count {
        let section = row_section[row];
        let mut end_row = row + 1;
        while end_row < row_count && row_section[end_row] == section {
            end_row += 1;
        }
        let index = categories[section] as f64 % section_styles;
        let y = plot_top + row as f64 * row_height;
        let h = (end_row - row) as f64 * row_height;
        if categories.len() > 1 || !section.is_empty() {
            let _ = write!(
                bands,
                "<rect class=\"pele-rs-section pele-rs-section-{}\" x=\"0\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                js_number(index),
                num(y),
                js_number(width),
                num(h),
                if index % 2.0 == 0.0 {
                    "var(--_s)"
                } else {
                    "none"
                }
            );
        }
        let label = &section_labels[section];
        section_out += &label_svg(
            label,
            pad + 8.0 + label.width / 2.0,
            y + h / 2.0,
            &format!(
                " class=\"pele-rs-section-label\" data-id=\"{}\"",
                esc_text(section)
            ),
        );
        row = end_row;
    }

    // Excluded days, as Mermaid marks them: a day at a time from the first task's start, and
    // not at all when the chart spans more than five years.
    let mut excluded = String::new();
    if (!db.excludes.is_empty() || !db.includes.is_empty())
        && add(Date::new(start_time), 6.0, Unit::Y).time() > stop
    {
        let mut from: Option<Date> = None;
        let mut to: Option<Date> = None;
        let flush = |from: &mut Option<Date>, to: &mut Option<Date>, excluded: &mut String| {
            let (Some(f), Some(t)) = (*from, *to) else {
                return;
            };
            let mut day_start = f;
            day_start.set_hours(0.0, 0.0, 0.0, 0.0);
            let mut day_end = t;
            day_end.set_hours(23.0, 59.0, 59.0, 999.0);
            let ex0 = left.max(x(day_start.time()));
            let ex1 = right.min(x(day_end.time()));
            if ex1 > ex0 {
                let _ = write!(
                    excluded,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                    num(ex0),
                    num(plot_top),
                    num(ex1 - ex0),
                    num(plot_height)
                );
            }
            *from = None;
            *to = None;
        };
        let mut day = Date::new(start_time);
        while day.time() <= stop {
            if db.is_invalid_date(day)? {
                from.get_or_insert(day);
                to = Some(day);
            } else {
                flush(&mut from, &mut to, &mut excluded);
            }
            day = add_days(day, 1.0);
        }
        flush(&mut from, &mut to, &mut excluded);
    }

    let mut links = Vec::new();
    let mut tasks_out = String::new();
    for bar in &bars {
        let task = bar.task;
        let cy = plot_top + bar.row as f64 * row_height + row_height / 2.0;
        let state = if task.active {
            "active"
        } else if task.done {
            "done"
        } else {
            ""
        };
        let mut classes = "pele-rs-task".to_string();
        if !state.is_empty() {
            classes += &format!(" pele-rs-{state}");
        }
        if task.crit {
            classes += " pele-rs-crit";
        }
        if task.milestone {
            classes += " pele-rs-milestone";
        }
        if task.classes.iter().any(|c| c == "clickable") {
            classes += " pele-rs-clickable";
        }

        let mut shape = String::new();
        let mut text_fill = "";
        if bar.x0.is_nan() {
            classes += " pele-rs-unscheduled";
            text_fill = " fill=\"var(--_m)\"";
        } else {
            // A critical task takes the critical color in place of the accent. Done, it keeps
            // the fill of finished work and only its border is in that color.
            let color = if task.crit { CRITICAL } else { "var(--_c)" };
            let paint = match state {
                "done" => "fill=\"var(--_a)\"".to_string(),
                "active" => format!("fill=\"{color}\" fill-opacity=\"0.25\""),
                _ => format!("fill=\"{color}\""),
            };
            let stroke = format!(
                "stroke=\"{}\"",
                if state == "done" && !task.crit {
                    "var(--_b)"
                } else {
                    color
                }
            );
            if task.milestone {
                let cx = (bar.x0 + bar.x1) / 2.0;
                let r = bar_height / 2.0;
                shape = format!(
                    "<path d=\"M{},{}L{},{}L{},{}L{},{}Z\" {paint} {stroke} stroke-linejoin=\"round\"/>",
                    num(cx),
                    num(cy - r),
                    num(cx + r),
                    num(cy),
                    num(cx),
                    num(cy + r),
                    num(cx - r),
                    num(cy)
                );
            } else {
                shape = format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" {paint} {stroke}{ROUNDED}/>",
                    num(bar.x0),
                    num(cy - bar_height / 2.0),
                    num(bar.x1 - bar.x0),
                    num(bar_height)
                );
            }
            if bar.inside {
                text_fill = if state.is_empty() {
                    " fill=\"var(--_bg)\""
                } else {
                    ""
                };
            } else if state == "done" {
                text_fill = " fill=\"var(--_m)\"";
            }
        }

        let mut body = shape
            + &label_svg(
                &bar.label,
                bar.label_x,
                cy,
                &format!(" class=\"pele-rs-label\"{text_fill}"),
            );
        if let Some(url) = db.links.get(&task.id) {
            body = linked(&body, url, &task.id, &mut links, options, "");
        }
        let _ = write!(
            tasks_out,
            "<g class=\"{classes}\" data-id=\"{}\">{body}</g>",
            esc_text(&task.id)
        );
    }

    let mut today = String::new();
    if db.today_marker != "off" && now >= start_time && now <= stop && stop > start_time {
        let tx = x(now);
        let top = plot_top - grid_lead;
        let style = if db.today_marker.is_empty() {
            String::new()
        } else {
            resolve_style(&db.today_marker.split(',').collect::<Vec<_>>()).line
        };
        let _ = write!(
            today,
            "<g class=\"pele-rs-today\"><path d=\"M{},{}V{}\" fill=\"none\" stroke=\"var(--_c)\" stroke-width=\"2\"{style}/>",
            num(tx),
            num(top),
            num(plot_bottom)
        );
        if style.is_empty() {
            let _ = write!(
                today,
                "<path d=\"M{},{}H{}L{},{}Z\" fill=\"var(--_c)\"/>",
                num(tx - 4.0),
                num(top - 5.0),
                num(tx + 4.0),
                num(tx),
                num(top + 1.0)
            );
        }
        today += "</g>";
    }

    let mut axis_line = format!("M{},{}H{}", num(left), num(plot_bottom), num(right));
    if top_axis {
        let _ = write!(
            axis_line,
            "M{},{}H{}",
            num(left),
            num(plot_top - grid_lead),
            num(right)
        );
    }
    let mut content = title_svg(width);
    if !bands.is_empty() {
        let _ = write!(content, "<g class=\"pele-rs-sections\">{bands}</g>");
    }
    if !excluded.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-excluded\" fill=\"var(--_a)\">{excluded}</g>"
        );
    }
    let _ = write!(
        content,
        "<g class=\"pele-rs-grid\" fill=\"none\"><path d=\"{grid}\" stroke=\"var(--_b)\" stroke-dasharray=\"1 3\"/><path d=\"{axis_line}\" stroke=\"var(--_b)\"/></g>"
    );
    if !vert_lines.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-verts\" fill=\"none\" stroke=\"var(--_l)\" stroke-width=\"1.5\" stroke-dasharray=\"4 3\">{vert_lines}</g>"
        );
    }
    content += &today;
    let _ = write!(
        content,
        "<g class=\"pele-rs-tasks\" font-size=\"{}\">{tasks_out}</g><g class=\"pele-rs-section-labels\" font-size=\"{}\">{section_out}</g><g class=\"pele-rs-axis\" font-size=\"{}\" fill=\"var(--_m)\">{axis_bottom}{axis_top}</g>",
        num(label_size),
        num(section_size),
        js_number(tick_size)
    );
    if !vert_labels.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-vert-labels\" font-size=\"{}\">{vert_labels}</g>",
            num(label_size)
        );
    }

    Ok(Output {
        svg: svg_document(
            DiagramKind::Gantt,
            width,
            height,
            size,
            options,
            &acc,
            &content,
        ),
        width,
        height,
        links,
    })
}
