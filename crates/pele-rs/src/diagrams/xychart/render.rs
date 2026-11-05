//! The drawing of an XY chart: a bar or a line over a category or a number axis.

use super::db::{Orientation, PlotKind, XAxis, XyChartDb};
use super::ticks::{tick_index, tick_step, tick_value, ticks_between};
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::cap_width;
use crate::error::{Error, ErrorCode};
use crate::preprocess::{Config, Value};
use crate::svg::{Accessible, esc, esc_text, label_svg, num, series_color, svg_document};
use crate::text::label::{Label, layout_label};
use crate::text::{Style, TextMeasurer};
use crate::util::{js_number, round};

/// The room a tick takes.
const TICK: f64 = 4.0;
/// The size of the mark of a series in the legend.
const SWATCH: f64 = 12.0;
/// The room one entry of the legend takes.
const ROW: f64 = 26.0;
/// The room between the plot and the legend.
const LEGEND_GAP: f64 = 24.0;
/// The largest value a coordinate may have.
const LIMIT: f64 = 1e300;

/// A mark on an axis: a place and the label that stands there.
struct Mark {
    at: f64,
    label: Label,
}

/// The widest of the labels of some marks.
fn widest(marks: &[Mark]) -> f64 {
    marks.iter().map(|m| m.label.width).fold(0.0, f64::max)
}

/// A boolean from the configuration, as pele reads it: anything but a boolean falls back to the
/// default.
fn on(conf: &Config, key: &str, fallback: bool) -> bool {
    match conf.get(key) {
        Some(Value::Bool(b)) => *b,
        _ => fallback,
    }
}

/// A size from the configuration: a value that is not a number, or is not positive, falls back
/// to the default, and one that is out of range is cut down to it.
fn dimension(conf: &Config, key: &str, fallback: f64) -> f64 {
    match conf.get(key) {
        Some(Value::Number(n)) if *n > 0.0 => round(n.clamp(100.0, 4000.0)),
        _ => fallback,
    }
}

/// A number that is not infinite, and not so large that the drawing cannot hold it.
fn finite(value: f64) -> Option<f64> {
    if value.is_finite() {
        Some(value.clamp(-LIMIT, LIMIT))
    } else {
        None
    }
}

/// The section of the configuration under a key, when it is one.
fn section<'a>(conf: &'a Config, key: &str) -> Option<&'a Config> {
    match conf.get(key) {
        Some(Value::Map(m)) => Some(m),
        _ => None,
    }
}

/// Writes a label with its left edge, center or right edge at x. A plain one-line label keeps
/// its place when the text is wider or narrower than measured.
fn aligned(label: &Label, x: f64, y: f64, anchor: &str) -> String {
    if let [line] = label.lines.as_slice() {
        if let [only] = line.as_slice() {
            if only.style.is_empty() {
                return format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"{anchor}\">{}</text>",
                    num(x),
                    num(y + label.size * 0.35),
                    esc(&only.text)
                );
            }
        }
    }
    let cx = match anchor {
        "start" => x + label.width / 2.0,
        "end" => x - label.width / 2.0,
        _ => x,
    };
    label_svg(label, cx, y, "")
}

/// A group of the chart, or nothing when it holds nothing.
fn group_svg(class: &str, font_size: f64, body: &str, extra: &str) -> String {
    if body.is_empty() {
        String::new()
    } else {
        format!("<g class=\"{class}\" font-size=\"{font_size}\"{extra}>{body}</g>")
    }
}

/// What is known about the x axis when its marks are placed.
struct XAxisInfo {
    /// Whether the axis holds categories rather than a range of numbers.
    is_band: bool,
    categories: Vec<String>,
    /// The labels of the categories, empty when they are not shown.
    labels: Vec<Label>,
    label_width: f64,
    label_height: f64,
    /// The ends of the range of a linear axis.
    min: Option<f64>,
    max: Option<f64>,
    /// The share of the axis one category or one point takes.
    slot: f64,
    /// The room the first and the last mark keep to the ends of the axis.
    inset: f64,
    horizontal: bool,
}

impl XAxisInfo {
    /// Whether the axis has a range of numbers of its own.
    fn ranged(&self) -> bool {
        match (self.min, self.max) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        }
    }

    /// The place of the i-th of n marks along the axis, as a share of its length.
    fn place(&self, i: usize, n: usize) -> f64 {
        if self.is_band {
            (i as f64 + 0.5) * self.slot
        } else {
            let at = if n > 1 {
                i as f64 / (n as f64 - 1.0)
            } else if self.ranged() {
                0.0
            } else {
                0.5
            };
            self.inset + at * (1.0 - 2.0 * self.inset)
        }
    }

    /// The labelled places of the category axis. Labels that would run into each other are
    /// thinned out.
    fn marks(&self, length: f64, small: f64, measurer: &dyn TextMeasurer) -> Vec<Mark> {
        let mut marks = Vec::new();
        if self.is_band {
            let need = if self.labels.is_empty() {
                8.0
            } else if self.horizontal {
                self.label_height
            } else {
                self.label_width + 12.0
            };
            let every = (need / (length * self.slot)).ceil().max(1.0) as usize;
            let mut i = 0;
            while i < self.categories.len() {
                let label = self.labels.get(i).cloned().unwrap_or_default();
                marks.push(Mark {
                    at: (i as f64 + 0.5) * self.slot,
                    label,
                });
                i += every;
            }
        } else if self.ranged() {
            let (x0, x1) = (self.min.unwrap_or_default(), self.max.unwrap_or_default());
            let spacing = if self.horizontal { 32.0 } else { 80.0 };
            let step = tick_step((x1 - x0).abs(), (length / spacing).floor().clamp(2.0, 10.0));
            for value in ticks_between(x0.min(x1), x0.max(x1), step) {
                let at = self.inset + ((value - x0) / (x1 - x0)) * (1.0 - 2.0 * self.inset);
                marks.push(Mark {
                    at,
                    label: layout_label(
                        Some(&js_number(value)),
                        false,
                        measurer,
                        small,
                        4000.0,
                        Style::NONE,
                    ),
                });
            }
        } else if let Some(x0) = self.min {
            marks.push(Mark {
                at: 0.5,
                label: layout_label(
                    Some(&js_number(x0)),
                    false,
                    measurer,
                    small,
                    4000.0,
                    Style::NONE,
                ),
            });
        }
        marks
    }
}

/// The labelled places of the value axis. The range grows to whole steps when it comes from the
/// data.
fn value_marks(
    lo: &mut f64,
    hi: &mut f64,
    exact: bool,
    length: f64,
    spacing: f64,
    small: f64,
    measurer: &dyn TextMeasurer,
) -> Vec<Mark> {
    let step = tick_step(
        (*hi - *lo).abs(),
        (length / spacing).floor().clamp(2.0, 10.0),
    );
    if !exact {
        *lo = tick_value(step, (tick_index(step, *lo) + 1e-9).floor());
        *hi = tick_value(step, (tick_index(step, *hi) - 1e-9).ceil());
    }
    ticks_between(lo.min(*hi), lo.max(*hi), step)
        .into_iter()
        .map(|value| Mark {
            at: (value - *lo) / (*hi - *lo),
            label: layout_label(
                Some(&js_number(value)),
                false,
                measurer,
                small,
                4000.0,
                Style::NONE,
            ),
        })
        .collect()
}

/// Draws an XY chart.
pub(crate) fn render_xychart(db: &XyChartDb, input: &Input<'_>) -> Result<Output, Error> {
    if db.plots.is_empty() {
        return Err(Error::new(
            ErrorCode::Semantic,
            "No Plot to render, please provide a plot with some data",
        ));
    }
    let empty = Config::new();
    let chart = match input.config.get("xyChart") {
        Some(Value::Map(m)) => m,
        _ => &empty,
    };
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let tiny = round(size * 0.75);
    let pad = options.padding.unwrap_or(8.0);
    let natural = dimension(chart, "width", 700.0);
    let width = cap_width(options, natural, 240.0);
    // A narrower chart is not as much shorter, so that the plot keeps room for its marks.
    let height = round(dimension(chart, "height", 500.0) * (width / natural).max(0.6));
    let horizontal = match db.orientation {
        Some(Orientation::Horizontal) => true,
        Some(Orientation::Vertical) => false,
        None => {
            matches!(chart.get("chartOrientation"), Some(Value::String(s)) if s == "horizontal")
        }
    };
    // The value of a bar is always written past its end, which is what showDataLabelOutsideBar
    // asks for in Mermaid. Inside, whether it fits changes from bar to bar and with the width of
    // the chart.
    let show_values = on(chart, "showDataLabel", false);

    let text = |raw: Option<&str>, font_size: f64, max: f64| -> Label {
        layout_label(raw, false, measurer, font_size, max, Style::NONE)
    };
    let line_height = text(Some("0"), small, 4000.0).height;
    let tiny_height = text(Some("0"), tiny, 4000.0).height;

    let title = layout_label(
        if on(chart, "showTitle", true) {
            db.title.as_deref()
        } else {
            None
        },
        false,
        measurer,
        size,
        width - 2.0 * pad,
        Style::BOLD,
    );
    let title_space = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };

    let mut legend: Vec<(usize, Label)> = Vec::new();
    let mut legend_width: f64 = 0.0;
    if on(chart, "showLegend", true) {
        for (index, plot) in db.plots.iter().enumerate() {
            if plot.title.is_empty() {
                continue;
            }
            let label = text(Some(&plot.title), small, width * 0.3);
            legend_width = legend_width.max(label.width + SWATCH + 8.0);
            legend.push((index, label));
        }
    }
    let legend_space = if legend.is_empty() {
        0.0
    } else {
        legend_width + LEGEND_GAP
    };

    // The value range: as written, or taken from the data, where bars stand on zero and the
    // range grows to whole steps.
    let bars = db
        .plots
        .iter()
        .filter(|plot| plot.kind == PlotKind::Bar)
        .count();
    let mut lo: f64 = db.y_axis.min;
    let mut hi: f64 = db.y_axis.max;
    let exact = db.has_set_y_axis && lo.is_finite() && hi.is_finite() && lo != hi;
    if exact {
        lo = finite(lo).unwrap_or_default();
        hi = finite(hi).unwrap_or_default();
    } else {
        lo = f64::INFINITY;
        hi = f64::NEG_INFINITY;
        for plot in &db.plots {
            for (_, raw) in &plot.data.entries {
                let Some(value) = raw.and_then(finite) else {
                    continue;
                };
                if value < lo {
                    lo = value;
                }
                if value > hi {
                    hi = value;
                }
            }
        }
        if lo > hi {
            lo = 0.0;
            hi = 0.0;
        }
        if bars > 0 {
            lo = lo.min(0.0);
            hi = hi.max(0.0);
        }
        if lo == hi {
            if lo == 0.0 {
                hi = 1.0;
            } else {
                lo -= lo.abs() / 2.0;
                hi += hi.abs() / 2.0;
            }
        }
    }

    // Places along the category axis are fractions of its length. Categories and bars sit in the
    // middle of equal slots; the points of a line on a numeric axis run from end to end.
    let mut count: usize = 1;
    if let XAxis::Band { categories, .. } = &db.x_axis {
        count = count.max(categories.len());
    } else {
        for plot in &db.plots {
            count = count.max(plot.data.entries.len());
        }
    }
    let slot = 1.0 / count as f64;
    let inset = if matches!(db.x_axis, XAxis::Band { .. }) || bars > 0 {
        slot / 2.0
    } else {
        0.0
    };
    let (x0, x1) = match &db.x_axis {
        XAxis::Band { .. } => (None, None),
        XAxis::Linear { min, max, .. } => (finite(*min), finite(*max)),
    };

    let category_conf = section(chart, "xAxis").unwrap_or(&empty);
    let value_conf = section(chart, "yAxis").unwrap_or(&empty);
    let left_conf = if horizontal {
        category_conf
    } else {
        value_conf
    };
    let bottom_conf = if horizontal {
        value_conf
    } else {
        category_conf
    };
    let left_title = text(
        if on(left_conf, "showTitle", true) {
            if horizontal {
                Some(db.x_axis.title())
            } else {
                Some(db.y_axis.title.as_str())
            }
        } else {
            None
        },
        small,
        height * 0.8,
    );
    let bottom_title = text(
        if on(bottom_conf, "showTitle", true) {
            if horizontal {
                Some(db.y_axis.title.as_str())
            } else {
                Some(db.x_axis.title())
            }
        } else {
            None
        },
        small,
        width * 0.8,
    );
    let left_tick = if on(left_conf, "showTick", true) {
        TICK
    } else {
        0.0
    };
    let bottom_tick = if on(bottom_conf, "showTick", true) {
        TICK
    } else {
        0.0
    };
    let left_labels = on(left_conf, "showLabel", true);
    let bottom_labels = on(bottom_conf, "showLabel", true);

    let wrap_at = if horizontal {
        width * 0.3
    } else {
        ((width - 2.0 * pad - 64.0 - legend_space) * slot - 8.0).max(48.0)
    };
    let (categories, labels): (Vec<String>, Vec<Label>) = match &db.x_axis {
        XAxis::Band { categories, .. } if on(category_conf, "showLabel", true) => (
            categories.clone(),
            categories
                .iter()
                .map(|category| text(Some(category), small, wrap_at))
                .collect(),
        ),
        XAxis::Band { categories, .. } => (categories.clone(), Vec::new()),
        XAxis::Linear { .. } => (Vec::new(), Vec::new()),
    };
    let label_width = labels.iter().map(|l| l.width).fold(0.0, f64::max);
    let label_height = labels.iter().map(|l| l.height).fold(0.0, f64::max);
    let axis = XAxisInfo {
        is_band: matches!(db.x_axis, XAxis::Band { .. }),
        categories,
        labels,
        label_width,
        label_height,
        min: x0,
        max: x1,
        slot,
        inset,
        horizontal,
    };

    // Room past the end of the value axis for what is written beside a point or a bar.
    let mut point_label_width: f64 = 0.0;
    let mut value_width: f64 = 0.0;
    for plot in &db.plots {
        for label in plot.point_labels.iter().flatten() {
            point_label_width = point_label_width.max(measurer.width(label, tiny, Style::NONE));
        }
        if show_values && horizontal && plot.kind == PlotKind::Bar {
            for (_, value) in &plot.data.entries {
                let text = value.and_then(finite).map(js_number).unwrap_or_default();
                value_width = value_width.max(measurer.width(&text, tiny, Style::NONE));
            }
        }
    }

    let bottom_label_height = if !bottom_labels {
        0.0
    } else if horizontal || !axis.is_band {
        line_height
    } else {
        label_height
    };
    let bottom_space = bottom_tick
        + if bottom_label_height > 0.0 {
            bottom_label_height + 4.0
        } else {
            0.0
        }
        + if bottom_title.height > 0.0 {
            bottom_title.height + 4.0
        } else {
            0.0
        };
    let headroom = if horizontal {
        0.0
    } else if point_label_width > 0.0 || show_values {
        tiny_height + 4.0
    } else if left_labels {
        line_height / 2.0
    } else {
        0.0
    };
    let top = pad + title_space + headroom;
    let plot_height = (height - pad - bottom_space - top).max(1.0);
    let bottom = top + plot_height;

    let left_marks = if horizontal {
        axis.marks(plot_height, small, measurer)
    } else {
        value_marks(&mut lo, &mut hi, exact, plot_height, 56.0, small, measurer)
    };
    let left_label_width = if left_labels {
        widest(&left_marks)
    } else {
        0.0
    };
    let left =
        pad + if left_title.height > 0.0 {
            left_title.height + 6.0
        } else {
            0.0
        } + if left_label_width > 0.0 {
            left_label_width + 6.0
        } else {
            0.0
        } + left_tick;
    let beside = f64::max(
        if point_label_width > 0.0 {
            point_label_width + 12.0
        } else {
            0.0
        },
        if value_width > 0.0 {
            value_width + 8.0
        } else {
            0.0
        },
    );
    let reach = if horizontal {
        (width * 0.25).min(beside)
    } else {
        0.0
    };
    let mut plot_width = (width - pad - left - legend_space - reach).max(1.0);
    let bottom_marks = if horizontal {
        value_marks(&mut lo, &mut hi, exact, plot_width, 80.0, small, measurer)
    } else {
        axis.marks(plot_width, small, measurer)
    };
    if bottom_labels && legend_space + reach == 0.0 {
        let mut over: f64 = 0.0;
        for mark in &bottom_marks {
            over = f64::max(
                over,
                mark.at * plot_width + mark.label.width / 2.0 - plot_width,
            );
        }
        plot_width = (plot_width - over).max(1.0);
    }
    let right = left + plot_width;

    let px = |c: f64, v: f64| {
        if horizontal {
            left + v * plot_width
        } else {
            left + c * plot_width
        }
    };
    let py = |c: f64, v: f64| {
        if horizontal {
            top + c * plot_height
        } else {
            bottom - v * plot_height
        }
    };
    let scale = |value: f64| (value - lo) / (hi - lo);
    let clamp = |v: f64| v.clamp(0.0, 1.0);

    // A line across the plot at a place on the value axis.
    let rule = |v: f64| {
        if horizontal {
            format!("M{},{}V{}", num(px(0.0, v)), num(top), num(bottom))
        } else {
            format!("M{},{}H{}", num(left), num(py(0.0, v)), num(right))
        }
    };

    let mut grid = String::new();
    for mark in if horizontal {
        &bottom_marks
    } else {
        &left_marks
    } {
        grid += &rule(mark.at);
    }

    let mut axis_path = String::new();
    let mut labels = String::new();
    if on(left_conf, "showAxisLine", true) {
        axis_path += &format!("M{},{}V{}", num(left), num(top), num(bottom));
    }
    if on(bottom_conf, "showAxisLine", true) {
        axis_path += &format!("M{},{}H{}", num(left), num(bottom), num(right));
    }
    for mark in &left_marks {
        let y = if horizontal {
            top + mark.at * plot_height
        } else {
            bottom - mark.at * plot_height
        };
        if left_tick > 0.0 {
            axis_path += &format!("M{},{}h{}", num(left), num(y), num(-left_tick));
        }
        if left_labels {
            labels += &aligned(&mark.label, left - left_tick - 6.0, y, "end");
        }
    }
    for mark in &bottom_marks {
        let at = left + mark.at * plot_width;
        if bottom_tick > 0.0 {
            axis_path += &format!("M{},{}v{}", num(at), num(bottom), num(bottom_tick));
        }
        if bottom_labels {
            labels += &aligned(
                &mark.label,
                at,
                bottom + bottom_tick + 4.0 + mark.label.height / 2.0,
                "middle",
            );
        }
    }

    let span = if horizontal { plot_height } else { plot_width } * slot;
    let group = span * 0.7;
    let thickness = group / bars.max(1) as f64;
    let gap = if bars > 1 {
        (thickness * 0.2).min(2.0)
    } else {
        0.0
    };
    let base = clamp(scale(0.0));
    // Bars that go both ways get a line to stand on.
    if bars > 0 && base > 0.0 && base < 1.0 {
        axis_path += &rule(base);
    }
    // Lines are drawn over the bars wherever they were declared, as a bar would hide part of one
    // under it. A series keeps the color of its place in the source.
    let mut series = String::new();
    let mut lines = String::new();
    let mut outside = String::new();
    let mut point_labels = String::new();
    let mut bar: usize = 0;
    for (index, plot) in db.plots.iter().enumerate() {
        let color = series_color(index);
        let id = if plot.title.is_empty() {
            String::new()
        } else {
            format!(" data-id=\"{}\"", esc_text(&plot.title))
        };
        // A series declared before the categories can hold more values than there are
        // categories.
        let n = if axis.is_band {
            plot.data.entries.len().min(count)
        } else {
            plot.data.entries.len()
        };
        if plot.kind == PlotKind::Bar {
            let offset =
                (bar as f64 - (bars as f64 - 1.0) / 2.0) * thickness - (thickness - gap) / 2.0;
            bar += 1;
            let mut rects = String::new();
            for i in 0..n {
                let Some(value) = plot.data.entries[i].1.and_then(finite) else {
                    continue;
                };
                let c = axis.place(i, n);
                let v = clamp(scale(value));
                let from = base.min(v);
                let to = base.max(v);
                let bx = if horizontal {
                    px(c, from)
                } else {
                    px(c, 0.0) + offset
                };
                let by = if horizontal {
                    py(c, 0.0) + offset
                } else {
                    py(c, to)
                };
                let bw = if horizontal {
                    (to - from) * plot_width
                } else {
                    thickness - gap
                };
                let bh = if horizontal {
                    thickness - gap
                } else {
                    (to - from) * plot_height
                };
                rects += &format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                    num(bx),
                    num(by),
                    num(bw),
                    num(bh)
                );
                if !show_values {
                    continue;
                }
                let label = js_number(value);
                let w = measurer.width(&label, tiny, Style::NONE);
                // Towards the end of the bar: 1 when it runs up or right from the baseline, -1 the
                // other way.
                let out = if v >= base { 1.0 } else { -1.0 };
                let mut tx = bx + bw / 2.0;
                let mut ty = by + bh / 2.0;
                let mut anchor = "middle";
                if horizontal {
                    if bh < tiny {
                        continue;
                    }
                    tx = if out > 0.0 { bx + bw } else { bx } + out * 6.0;
                    anchor = if out > 0.0 { "start" } else { "end" };
                } else {
                    if w + 2.0 > span / bars as f64 {
                        continue;
                    }
                    ty = if out > 0.0 { by } else { by + bh } - out * (tiny_height / 2.0 + 2.0);
                }
                outside += &format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"{anchor}\">{}</text>",
                    num(tx),
                    num(ty + tiny * 0.35),
                    esc(&label)
                );
            }
            series += &format!(
                "<g class=\"pele-rs-xychart-series pele-rs-xychart-bars\"{id} fill=\"{color}\">{rects}</g>"
            );
            continue;
        }

        // A line is cut where it leaves the value range, so it stays inside the plot.
        let mut d = String::new();
        let mut dots = String::new();
        let mut pen = false;
        let mut pc = 0.0;
        let mut pv = f64::NAN;
        for i in 0..n {
            let Some(value) = plot.data.entries[i].1.and_then(finite) else {
                pv = f64::NAN;
                continue;
            };
            let c = axis.place(i, n);
            let v = scale(value);
            let visible = (0.0..=1.0).contains(&v);
            if pv.is_nan() {
                if visible {
                    d += &format!("M{},{}", num(px(c, v)), num(py(c, v)));
                }
                pen = visible;
            } else {
                let dv = v - pv;
                let mut t0: f64 = 0.0;
                let mut t1: f64 = 1.0;
                if dv != 0.0 {
                    let a = -pv / dv;
                    let b = (1.0 - pv) / dv;
                    t0 = t0.max(a.min(b));
                    t1 = t1.min(a.max(b));
                } else if !visible {
                    t0 = 2.0;
                }
                if t0 <= t1 {
                    let c0 = pc + (c - pc) * t0;
                    let v0 = pv + dv * t0;
                    let c1 = pc + (c - pc) * t1;
                    let v1 = pv + dv * t1;
                    if !pen || t0 > 0.0 {
                        d += &format!("M{},{}", num(px(c0, v0)), num(py(c0, v0)));
                    }
                    d += &format!("L{},{}", num(px(c1, v1)), num(py(c1, v1)));
                }
                pen = visible;
            }
            pc = c;
            pv = v;
            if !visible {
                continue;
            }
            let cx = px(c, v);
            let cy = py(c, v);
            if span >= 12.0 {
                dots += &format!("<circle cx=\"{}\" cy=\"{}\" r=\"3\"/>", num(cx), num(cy));
            }
            let label = text(
                plot.point_labels
                    .as_ref()
                    .and_then(|l| l.get(i))
                    .map(String::as_str),
                tiny,
                4000.0,
            );
            if label.height > 0.0 {
                let half = label.width / 2.0;
                let placed = if horizontal {
                    aligned(&label, cx + 10.0, cy, "start")
                } else {
                    aligned(
                        &label,
                        cx.min(right - half).max(left + half),
                        cy - 6.0 - label.height / 2.0,
                        "middle",
                    )
                };
                point_labels.push_str(&placed);
            }
        }
        lines += &format!(
            "<g class=\"pele-rs-xychart-series pele-rs-xychart-line\"{id} fill=\"{color}\">{}{dots}</g>",
            if d.is_empty() {
                String::new()
            } else {
                format!(
                    "<path d=\"{d}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"2\" \
                     stroke-linecap=\"round\" stroke-linejoin=\"round\"/>"
                )
            }
        );
    }

    let mut titles = String::new();
    if left_title.height > 0.0 {
        titles += &format!(
            "<g transform=\"translate({},{}) rotate(-90)\">{}</g>",
            num(pad + left_title.height / 2.0),
            num(top + plot_height / 2.0),
            label_svg(&left_title, 0.0, 0.0, "")
        );
    }
    titles += &label_svg(
        &bottom_title,
        left + plot_width / 2.0,
        height - pad - bottom_title.height / 2.0,
        "",
    );

    let mut legend_out = String::new();
    let mut legend_height: f64 = 0.0;
    for (_, label) in &legend {
        legend_height += ROW.max(label.height + 6.0);
    }
    let mut ly: f64 = top + ((plot_height - legend_height) / 2.0).max(0.0);
    let lx = right + reach + LEGEND_GAP;
    for (index, label) in &legend {
        let row = ROW.max(label.height + 6.0);
        if ly + row > height {
            break;
        }
        let y = ly + row / 2.0;
        let color = series_color(*index);
        let mark = if db.plots[*index].kind == PlotKind::Bar {
            format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{SWATCH}\" height=\"{SWATCH}\" rx=\"2\" fill=\"{color}\"/>",
                num(lx),
                num(y - SWATCH / 2.0)
            )
        } else {
            format!(
                "<path d=\"M{},{}h{SWATCH}\" stroke=\"{color}\" stroke-width=\"2\" \
                 stroke-linecap=\"round\"/>",
                num(lx),
                num(y)
            )
        };
        legend_out += &format!(
            "<g class=\"pele-rs-xychart-legend-item\" data-id=\"{}\">{mark}{}</g>",
            esc_text(&db.plots[*index].title),
            label_svg(label, lx + SWATCH + 8.0 + label.width / 2.0, y, "")
        );
        ly += row;
    }

    // The grid lies under the series, the axis over them.
    let content = format!(
        "{}{}<g class=\"pele-rs-xychart-plot\">{series}{lines}</g>{}{}{}{}{}{}",
        label_svg(
            &title,
            width / 2.0,
            pad + title.height / 2.0,
            " class=\"pele-rs-title\" font-weight=\"var(--_tw)\""
        ),
        if grid.is_empty() {
            String::new()
        } else {
            format!("<path class=\"pele-rs-xychart-grid\" d=\"{grid}\" stroke=\"var(--_a)\"/>")
        },
        if axis_path.is_empty() {
            String::new()
        } else {
            format!(
                "<path class=\"pele-rs-xychart-axis\" d=\"{axis_path}\" fill=\"none\" \
                 stroke=\"var(--_b)\"/>"
            )
        },
        group_svg(
            "pele-rs-xychart-axis-labels",
            small,
            &labels,
            " fill=\"var(--_m)\""
        ),
        group_svg("pele-rs-xychart-axis-titles", small, &titles, ""),
        group_svg("pele-rs-xychart-data-labels", tiny, &outside, ""),
        group_svg("pele-rs-xychart-point-labels", tiny, &point_labels, ""),
        group_svg("pele-rs-xychart-legend", small, &legend_out, ""),
    );

    let acc = Accessible {
        title: db.acc_title.clone(),
        descr: db.acc_descr.clone(),
    };
    Ok(Output {
        svg: svg_document(
            crate::DiagramKind::XyChart,
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
    })
}
