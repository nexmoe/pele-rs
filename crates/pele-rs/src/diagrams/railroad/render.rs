//! The drawing of a railroad diagram: the rules, the track that joins their parts, and the
//! boxes that hold the parts. Ported from pele's `diagrams/railroad/render.ts`.

use super::db::{Node, RailroadModel};
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{tighten, title_room};
use crate::error::Error;
use crate::preprocess::{Config, Value};
use crate::svg::{Accessible, ROUNDED, esc, esc_text, label_svg, num, svg_document};
use crate::text::Style;
use crate::text::TextMeasurer;
use crate::text::entities::decode_entities;
use crate::text::label::layout_label;
use crate::util::round;

const LEAD: f64 = 12.0;
const HEAD_GAP: f64 = 6.0;
const RULE_GAP: f64 = 18.0;
const COUNT_SIZE: f64 = 12.0;
const ARROW: f64 = 7.0;
const MONO: &str = "var(--_fm)";

/// What a part of a rule is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Terminal,
    NonTerminal,
    Special,
    Sequence,
    Choice,
    Bypass,
    Loop,
    Folded,
}

/// A laid-out element. The track enters at the left edge and leaves at the right edge on one
/// baseline, with `up` above it and `down` below it.
#[derive(Debug)]
struct Shape {
    kind: Kind,
    width: f64,
    up: f64,
    down: f64,
    kids: Vec<Shape>,
    /// A leaf's text, or a loop's count.
    text: String,
    /// The name a non-terminal refers to.
    id: String,
    /// How far a bypass rises above the baseline, or a loop drops below it.
    reach: f64,
}

/// The measures the drawing is made with.
struct Metrics<'a> {
    measurer: &'a dyn TextMeasurer,
    size: f64,
    padding: f64,
    height: f64,
    /// Space between elements in a sequence, and between stacked branches.
    gap: f64,
    sep: f64,
    arc: f64,
}

/// The paths and the boxes of one rule.
#[derive(Default)]
struct Drawing {
    track: String,
    arrows: String,
    nodes: String,
}

impl Drawing {
    /// A straight piece of track.
    fn line(&mut self, x1: f64, y: f64, x2: f64) {
        if x2 > x1 {
            self.track
                .push_str(&format!("M{},{}H{}", num(x1), num(y), num(x2)));
        }
    }
}

/// The room a value of the settings takes.
fn option(config: &Config, key: &str, fallback: f64, max: f64) -> f64 {
    match config.get("railroad") {
        Some(Value::Map(map)) => match map.get(key) {
            Some(Value::Number(value)) if *value >= 0.0 => value.min(max),
            _ => fallback,
        },
        _ => fallback,
    }
}

/// The smallest even number that is not below the given one.
fn even(value: f64) -> f64 {
    (value / 2.0).ceil() * 2.0
}

/// Text is literal here: a terminal such as "<br>" or "*" is itself, not markup. Mermaid's
/// `#35;` codes still stand for characters, and escapes that would be invisible are written
/// out.
fn shown(text: &str) -> String {
    let text = if text.contains("¶ß") {
        decode_entities(&text.replace('&', "&amp;"))
    } else {
        text.to_string()
    };
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

/// A leaf of a rule.
fn leaf(kind: Kind, text: &str, id: &str, style: Style, m: &Metrics<'_>) -> Shape {
    let half = m.height / 2.0;
    let width = even(m.measurer.width(text, m.size, style) + 2.0 * m.padding).max(m.height);
    Shape {
        kind,
        width,
        up: half,
        down: half,
        kids: Vec::new(),
        text: text.to_string(),
        id: id.to_string(),
        reach: 0.0,
    }
}

/// The count a loop shows.
fn count(min: f64, max: Option<f64>) -> String {
    if max.is_none() && min <= 1.0 {
        return String::new();
    }
    let text = |value: f64| {
        if value.is_finite() {
            crate::util::js_number(value)
        } else {
            "∞".to_string()
        }
    };
    match max {
        Some(max) if max == min => format!("{}×", text(min)),
        None => format!("≥{}×", text(min)),
        Some(max) if min == 0.0 => format!("≤{}×", text(max)),
        Some(max) => format!("{}–{}×", text(min), text(max)),
    }
}

/// Where the next branch of a choice sits below the baseline, given the one above it.
fn below(offset: f64, above_down: f64, branch_up: f64, m: &Metrics<'_>) -> f64 {
    (offset + above_down + m.sep + branch_up).max(2.0 * m.arc)
}

/// A part that may be left out, drawn as a line that goes over the part.
fn bypass(kid: Shape, m: &Metrics<'_>) -> Shape {
    let reach = (kid.up + m.sep).max(2.0 * m.arc);
    Shape {
        kind: Kind::Bypass,
        width: kid.width + 4.0 * m.arc,
        up: reach,
        down: kid.down,
        kids: vec![kid],
        text: String::new(),
        id: String::new(),
        reach,
    }
}

/// The room a node takes around the parts below it.
fn around(node: &Node, m: &Metrics<'_>) -> f64 {
    match node {
        Node::Choice(_) | Node::Optional(_) => 4.0 * m.arc,
        Node::Repetition { min, .. } => {
            if *min == 0.0 {
                6.0 * m.arc
            } else {
                2.0 * m.arc
            }
        }
        Node::Sequence(_) => 6.0 * m.arc,
        _ => 0.0,
    }
}

/// Lays a rule out. The tree is walked without recursion, so that the depth of a rule is
/// bounded by memory and not by the stack.
fn measure(root: &Node, m: &Metrics<'_>, room: f64) -> Shape {
    let mut nodes: Vec<&Node> = vec![root];
    let mut budgets: Vec<f64> = vec![room];
    let mut lists: Vec<Vec<&Node>> = vec![root.parts()];
    let mut done: Vec<Vec<Shape>> = vec![Vec::new()];
    loop {
        let top = nodes.len() - 1;
        if done[top].len() < lists[top].len() {
            let child = lists[top][done[top].len()];
            let budget = (budgets[top] - around(nodes[top], m)).max(40.0);
            nodes.push(child);
            budgets.push(budget);
            lists.push(child.parts());
            done.push(Vec::new());
        } else {
            let results = std::mem::take(&mut done[top]);
            let value = build(nodes[top], results, budgets[top], m);
            if top == 0 {
                return value;
            }
            nodes.pop();
            budgets.pop();
            lists.pop();
            done.pop();
            done[top - 1].push(value);
        }
    }
}

/// A row of a sequence that was folded.
fn row(kids: Vec<Shape>, width: f64) -> Shape {
    let mut up: f64 = 0.0;
    let mut down: f64 = 0.0;
    for kid in &kids {
        up = up.max(kid.up);
        down = down.max(kid.down);
    }
    Shape {
        kind: Kind::Sequence,
        width,
        up,
        down,
        kids,
        text: String::new(),
        id: String::new(),
        reach: 0.0,
    }
}

/// Builds the shape of a node from the shapes of the parts below it.
fn build(node: &Node, kids: Vec<Shape>, room: f64, m: &Metrics<'_>) -> Shape {
    match node {
        Node::Terminal(value) => {
            let text = shown(value);
            let text = if !text.is_empty() && text.chars().all(|c| c == ' ') {
                "␣".repeat(text.chars().count())
            } else {
                text
            };
            leaf(Kind::Terminal, &text, "", Style::MONO, m)
        }
        Node::NonTerminal(name) => leaf(Kind::NonTerminal, &shown(name), name, Style::NONE, m),
        Node::Special(text) => leaf(
            Kind::Special,
            &format!("? {} ?", shown(text)),
            "",
            Style::ITALIC,
            m,
        ),
        Node::Sequence(_) => {
            let count = kids.len();
            let mut width = (count as f64 - 1.0).max(0.0) * m.gap;
            let mut up: f64 = 0.0;
            let mut down: f64 = 0.0;
            for kid in &kids {
                width += kid.width;
                up = up.max(kid.up);
                down = down.max(kid.down);
            }
            if width <= room || count < 2 {
                return Shape {
                    kind: Kind::Sequence,
                    width,
                    up,
                    down,
                    kids,
                    text: String::new(),
                    id: String::new(),
                    reach: 0.0,
                };
            }
            // The sequence is too wide: it is folded into rows that are drawn one under the
            // other, inside rails.
            let limit = (room - 6.0 * m.arc).max(40.0);
            let mut rows: Vec<Shape> = Vec::new();
            let mut line: Vec<Shape> = Vec::new();
            let mut line_width = 0.0;
            for kid in kids {
                if !line.is_empty() && line_width + m.gap + kid.width > limit {
                    rows.push(row(std::mem::take(&mut line), line_width));
                    line_width = 0.0;
                }
                line_width += if line.is_empty() { 0.0 } else { m.gap } + kid.width;
                line.push(kid);
            }
            rows.push(row(line, line_width));
            if rows.len() < 2 {
                let kids: Vec<Shape> = rows.into_iter().flat_map(|r| r.kids).collect();
                return Shape {
                    kind: Kind::Sequence,
                    width,
                    up,
                    down,
                    kids,
                    text: String::new(),
                    id: String::new(),
                    reach: 0.0,
                };
            }
            let mut folded_width: f64 = 0.0;
            let mut offset = 0.0;
            for (i, r) in rows.iter().enumerate() {
                folded_width = folded_width.max(r.width);
                if i > 0 {
                    offset += rows[i - 1].down + m.sep.max(2.0 * m.arc) + r.up;
                }
            }
            let last = rows.len() - 1;
            Shape {
                kind: Kind::Folded,
                width: folded_width + 6.0 * m.arc,
                up: rows[0].up,
                down: offset + rows[last].down,
                kids: rows,
                text: String::new(),
                id: String::new(),
                reach: 0.0,
            }
        }
        Node::Choice(_) => {
            let mut inner: f64 = 0.0;
            let mut offset = 0.0;
            for i in 0..kids.len() {
                inner = inner.max(kids[i].width);
                if i > 0 {
                    offset = below(offset, kids[i - 1].down, kids[i].up, m);
                }
            }
            let up = kids.first().map_or(0.0, |k| k.up);
            let down = kids.last().map_or(0.0, |k| offset + k.down);
            Shape {
                kind: Kind::Choice,
                width: inner + 4.0 * m.arc,
                up,
                down,
                kids,
                text: String::new(),
                id: String::new(),
                reach: 0.0,
            }
        }
        Node::Optional(_) => bypass(kids.into_iter().next().expect("an element"), m),
        Node::Repetition { min, max, .. } => {
            let kid = kids.into_iter().next().expect("an element");
            if *min == 1.0 && *max == Some(1.0) {
                return kid;
            }
            let text = count(*min, *max);
            let reach = (kid.down + m.sep).max(2.0 * m.arc);
            let label = if text.is_empty() {
                0.0
            } else {
                even(m.measurer.width(&text, COUNT_SIZE, Style::NONE))
            };
            let loop_shape = Shape {
                kind: Kind::Loop,
                width: kid.width.max(label) + 2.0 * m.arc,
                up: kid.up,
                down: reach
                    + if text.is_empty() {
                        ARROW / 2.0 + 1.0
                    } else {
                        COUNT_SIZE + 6.0
                    },
                kids: vec![kid],
                text: text.clone(),
                id: String::new(),
                reach,
            };
            if *min == 0.0 {
                bypass(loop_shape, m)
            } else {
                loop_shape
            }
        }
    }
}

/// Draws a laid-out rule.
fn place(root: Shape, left: f64, baseline: f64, out: &mut Drawing, m: &Metrics<'_>, stroke: &str) {
    let arc = m.arc;
    let box_top = m.height / 2.0;
    let text_shift = m.size * 0.35;
    let a = num(arc);
    let turn = |sweep: u8, dx: &str, dy: &str| format!("a{a},{a} 0 0 {sweep} {dx}{a},{dy}{a}");
    let east_to_south = turn(1, "", "");
    let south_to_east = turn(0, "", "");
    let east_to_north = turn(0, "", "-");
    let north_to_east = turn(1, "", "-");
    let loop_down = turn(0, "-", "");
    let loop_up = turn(0, "-", "-");

    let mut stack: Vec<(Shape, f64, f64)> = vec![(root, left, baseline)];
    while let Some((shape, x, y)) = stack.pop() {
        let right = x + shape.width;
        match shape.kind {
            Kind::Sequence => {
                let kids = shape.kids;
                let mut kx = right;
                for (i, kid) in kids.into_iter().enumerate().rev() {
                    kx -= kid.width;
                    stack.push((kid, kx, y));
                    if i > 0 {
                        out.line(kx - m.gap, y, kx);
                    }
                    kx -= m.gap;
                }
            }
            Kind::Folded => {
                let kids = shape.kids;
                let count = kids.len();
                let start = x + 2.0 * arc;
                out.line(x, y, start);
                // The rows sit one under the other, each below the one before it.
                let mut offsets = Vec::with_capacity(count);
                let mut offset = 0.0;
                for i in 0..count {
                    offsets.push(offset);
                    if i + 1 < count {
                        offset += kids[i].down + m.sep.max(2.0 * arc) + kids[i + 1].up;
                    }
                }
                for (i, kid) in kids.iter().enumerate() {
                    let ky = y + offsets[i];
                    if i + 1 < count {
                        let next_offset = offsets[i + 1];
                        let mid = y + offsets[i] + kid.down + m.sep.max(2.0 * arc) / 2.0;
                        out.track.push_str(&format!(
                            "M{},{}H{}V{}H{}V{}H{}",
                            num(start + kid.width),
                            num(ky),
                            num(right - 2.0 * arc),
                            num(mid),
                            num(x + arc),
                            num(y + next_offset),
                            num(start)
                        ));
                        out.arrows.push_str(&format!(
                            "M{},{}l{},{}l{},{}z",
                            num(start - ARROW),
                            num(y + next_offset - ARROW / 2.0),
                            num(ARROW),
                            num(ARROW / 2.0),
                            num(-ARROW),
                            num(ARROW / 2.0)
                        ));
                    } else {
                        out.track.push_str(&format!(
                            "M{},{}H{}V{}H{}",
                            num(start + kid.width),
                            num(ky),
                            num(right - arc),
                            num(y),
                            num(right)
                        ));
                    }
                }
                for (kid, offset) in kids.into_iter().zip(offsets).rev() {
                    stack.push((kid, start, y + offset));
                }
            }
            Kind::Choice => {
                let kids = shape.kids;
                let count = kids.len();
                let inner = shape.width - 4.0 * arc;
                let rail_in = x + arc;
                let rail_out = right - arc;
                let mut offset = 0.0;
                let mut offsets = Vec::with_capacity(count);
                for i in 0..count {
                    if i > 0 {
                        offset = below(offset, kids[i - 1].down, kids[i].up, m);
                    }
                    offsets.push(offset);
                }
                let mut places = Vec::with_capacity(count);
                for (i, kid) in kids.iter().enumerate() {
                    let ky = y + offsets[i];
                    let kx = x + 2.0 * arc + (inner - kid.width) / 2.0;
                    let last = i + 1 == count;
                    if i == 0 {
                        out.line(x, y, kx);
                        out.line(kx + kid.width, y, right);
                    } else {
                        out.track.push_str(&if last {
                            format!(
                                "M{},{}{}V{}{}{}M{},{}{}{}V{}{}",
                                num(x),
                                num(y),
                                east_to_south,
                                num(ky - arc),
                                south_to_east,
                                if kx > rail_in + arc {
                                    format!("H{}", num(kx))
                                } else {
                                    String::new()
                                },
                                num(kx + kid.width),
                                num(ky),
                                if kx > rail_in + arc {
                                    format!("H{}", num(rail_out - arc))
                                } else {
                                    String::new()
                                },
                                east_to_north,
                                num(y + arc),
                                north_to_east
                            )
                        } else {
                            format!(
                                "M{},{}{}{}M{},{}{}{}",
                                num(rail_in),
                                num(ky - arc),
                                south_to_east,
                                if kx > rail_in + arc {
                                    format!("H{}", num(kx))
                                } else {
                                    String::new()
                                },
                                num(kx + kid.width),
                                num(ky),
                                if kx > rail_in + arc {
                                    format!("H{}", num(rail_out - arc))
                                } else {
                                    String::new()
                                },
                                east_to_north
                            )
                        });
                    }
                    places.push((kx, ky));
                }
                for (kid, (kx, ky)) in kids.into_iter().zip(places).rev() {
                    stack.push((kid, kx, ky));
                }
            }
            Kind::Bypass => {
                let kid = shape.kids.into_iter().next().expect("an element");
                let kx = x + 2.0 * arc;
                out.line(x, y, kx);
                out.line(kx + kid.width, y, right);
                out.track.push_str(&format!(
                    "M{},{}{}V{}{}H{}{}V{}{}",
                    num(x),
                    num(y),
                    east_to_north,
                    num(y - shape.reach + arc),
                    north_to_east,
                    num(right - 2.0 * arc),
                    east_to_south,
                    num(y - arc),
                    south_to_east
                ));
                stack.push((kid, kx, y));
            }
            Kind::Loop => {
                let kid = shape.kids.into_iter().next().expect("an element");
                let kx = x + (shape.width - kid.width) / 2.0;
                let loop_y = y + shape.reach;
                let middle = x + shape.width / 2.0;
                out.line(x, y, kx);
                out.line(kx + kid.width, y, right);
                out.track.push_str(&format!(
                    "M{},{}{}V{}{}H{}{}V{}{}",
                    num(x + arc),
                    num(y),
                    loop_down,
                    num(loop_y - arc),
                    south_to_east,
                    num(right - arc),
                    east_to_north,
                    num(y + arc),
                    loop_up
                ));
                out.arrows.push_str(&format!(
                    "M{},{}l{},{}v{}z",
                    num(middle - ARROW / 2.0),
                    num(loop_y),
                    num(ARROW),
                    num(-ARROW / 2.0),
                    num(ARROW)
                ));
                if !shape.text.is_empty() {
                    out.nodes.push_str(&format!(
                        "<text class=\"pele-edge-label pele-count\" x=\"{}\" y=\"{}\" \
                         font-size=\"{}\" fill=\"var(--_m)\">{}</text>",
                        num(middle),
                        num(loop_y + COUNT_SIZE + 3.0),
                        num(COUNT_SIZE),
                        esc(&shape.text)
                    ));
                }
                stack.push((kid, kx, y));
            }
            _ => {
                let text = &shape.text;
                let space = if text.starts_with(' ') || text.ends_with(' ') || text.contains("  ") {
                    " xml:space=\"preserve\""
                } else {
                    ""
                };
                let rect = format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                    num(x),
                    num(y - box_top),
                    num(shape.width),
                    num(m.height)
                );
                let label = format!(
                    "<text x=\"{}\" y=\"{}\"{}",
                    num(x + shape.width / 2.0),
                    num(y + text_shift),
                    space
                );
                match shape.kind {
                    Kind::Terminal => out.nodes.push_str(&format!(
                        "<g class=\"pele-node pele-terminal\">{rect} rx=\"{}\" \
                         fill=\"var(--_s)\" stroke=\"var(--_b)\"{stroke}/>{label} \
                         font-family=\"{MONO}\">{}</text></g>",
                        num(box_top),
                        esc(text)
                    )),
                    Kind::NonTerminal => out.nodes.push_str(&format!(
                        "<g class=\"pele-node pele-nonterminal\" data-id=\"{}\">{rect} \
                         fill=\"var(--_bg)\" stroke=\"var(--_b)\"{stroke}{ROUNDED}/>{label}>{}</text></g>",
                        esc_text(&shape.id),
                        esc(text)
                    )),
                    _ => out.nodes.push_str(&format!(
                        "<g class=\"pele-node pele-special\">{rect} fill=\"var(--_bg)\" \
                         stroke=\"var(--_b)\" stroke-dasharray=\"4 3\"{stroke}{ROUNDED}/>{label} \
                         font-style=\"italic\"><tspan fill=\"var(--_m)\">? </tspan>{}\
                         <tspan fill=\"var(--_m)\"> ?</tspan></text></g>",
                        esc(&text[2..text.len() - 2])
                    )),
                }
            }
        }
    }
}

/// Draws a railroad diagram.
pub(crate) fn render_railroad(model: &RailroadModel, input: &Input<'_>) -> Result<Output, Error> {
    let options = input.options;
    Ok(tighten(options, |tight| draw(model, input, tight)))
}

/// Draws a railroad diagram, with room to spare or as tightly as it fits.
fn draw(model: &RailroadModel, input: &Input<'_>, tight: bool) -> Output {
    let options = input.options;
    let config = input.config;
    let base = options.font_size.unwrap_or(16.0);
    let font_size = option(config, "fontSize", 0.0, 200.0);
    let size = if font_size != 0.0 {
        font_size
    } else {
        round(base * 0.875)
    };
    let head_size = if font_size != 0.0 { font_size } else { base };
    let padding = option(config, "padding", -1.0, 200.0);
    let line_height = round(size * 1.5);
    let m = Metrics {
        measurer: input.measurer,
        size,
        padding: if padding < 0.0 { 12.0 } else { padding },
        height: even(line_height + if padding < 0.0 { 9.0 } else { 2.0 * padding }),
        gap: {
            let gap = option(config, "horizontalSeparation", 16.0, 500.0);
            if tight { gap.min(12.0) } else { gap }
        },
        sep: option(config, "verticalSeparation", 10.0, 500.0),
        arc: {
            let arc = option(config, "arcRadius", 10.0, 200.0);
            if tight { arc.min(6.0) } else { arc }
        },
    };
    let radius = match config.get("railroad") {
        Some(Value::Map(map)) => match map.get("showMarkers") {
            Some(Value::Bool(false)) => 0.0,
            _ => option(config, "markerRadius", 4.0, 50.0),
        },
        _ => option(config, "markerRadius", 4.0, 50.0),
    };
    let stroke_width = option(config, "strokeWidth", 0.0, 20.0);
    let stroke = if stroke_width > 0.0 {
        format!(" stroke-width=\"{}\"", num(stroke_width))
    } else {
        String::new()
    };
    let pad = options.padding.unwrap_or(8.0);

    let title = layout_label(
        model.title.as_deref(),
        false,
        m.measurer,
        base,
        if tight { title_room(options) } else { 4000.0 },
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 16.0
    } else {
        0.0
    };
    let head_height = round(head_size * 1.5);

    let mut rules = String::new();
    let mut width = title.width;
    let mut top = pad + title_height;
    let mut bottom = pad + title.height;
    for rule in &model.rules {
        let room = if tight {
            (options.max_width.unwrap_or(320.0) - 2.0 * pad - 4.0 * radius - 2.0 * LEAD - 1.0)
                .max(80.0)
        } else {
            f64::INFINITY
        };
        let shape = measure(&rule.definition, &m, room);
        let shape_down = shape.down;
        let name = shown(&rule.name);
        let baseline = top + head_height + HEAD_GAP + shape.up;
        let start = pad + 2.0 * radius;
        let left = start + LEAD;
        let end = left + shape.width + LEAD;
        let mut out = Drawing::default();
        out.line(start, baseline, left);
        out.line(left + shape.width, baseline, end);
        place(shape, left, baseline, &mut out, &m, &stroke);
        let markers = if radius > 0.0 {
            format!(
                "<circle class=\"pele-marker pele-start\" cx=\"{}\" cy=\"{}\" r=\"{}\" \
                 fill=\"var(--_l)\"/><circle class=\"pele-marker pele-end\" cx=\"{}\" \
                 cy=\"{}\" r=\"{}\" fill=\"var(--_l)\"/>",
                num(pad + radius),
                num(baseline),
                num(radius),
                num(end + radius),
                num(baseline),
                num(radius)
            )
        } else {
            String::new()
        };
        rules.push_str(&format!(
            "<g class=\"pele-rule\" data-id=\"{}\">\
             <text class=\"pele-rule-name\" x=\"{}\" y=\"{}\" font-size=\"{}\" \
             font-weight=\"var(--_hw)\" text-anchor=\"start\">{}</text>\
             <path class=\"pele-edge pele-track\" d=\"{}\" fill=\"none\" \
             stroke=\"var(--_l)\"{}/>{}{}{}</g>",
            esc_text(&rule.name),
            num(pad),
            num(top + head_height / 2.0 + head_size * 0.35),
            num(head_size),
            esc(&name),
            out.track,
            stroke,
            if out.arrows.is_empty() {
                String::new()
            } else {
                format!(
                    "<path class=\"pele-marker\" d=\"{}\" fill=\"var(--_l)\"/>",
                    out.arrows
                )
            },
            markers,
            out.nodes
        ));
        width = width.max(end + 2.0 * radius - pad).max(m.measurer.width(
            &name,
            head_size,
            Style::BOLD,
        ));
        bottom = baseline + shape_down;
        top = bottom + m.sep + RULE_GAP;
    }

    let total_width = (width + 2.0 * pad).ceil();
    let total_height = (bottom + pad).ceil();
    let content = format!(
        "{}{}",
        label_svg(
            &title,
            total_width / 2.0,
            pad + title.height / 2.0,
            " class=\"pele-title\" font-weight=\"var(--_tw)\""
        ),
        if rules.is_empty() {
            String::new()
        } else {
            format!(
                "<g class=\"pele-rules\" transform=\"translate(0.5,0.5)\" font-size=\"{}\" \
                 text-anchor=\"middle\">{}</g>",
                num(size),
                rules
            )
        }
    );
    let acc = Accessible {
        title: model.acc_title.clone(),
        descr: model.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            crate::DiagramKind::Railroad,
            total_width,
            total_height,
            base,
            options,
            &acc,
            &content,
        ),
        width: total_width,
        height: total_height,
        links: Vec::new(),
    }
}
