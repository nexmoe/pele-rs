//! The SVG of a C4 diagram: the shapes, the boundaries and the relations between them, drawn
//! as pele's `render.ts` draws them.

use std::collections::{HashMap, HashSet};

use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{tighten, title_room};
use crate::error::{Error, ErrorCode};
use crate::svg::markers::{ARROW, marker, marker_trim};
use crate::svg::shapes::draw_shape;
use crate::svg::{
    Accessible, ResolvedStyle, esc, esc_text, label_svg, num, resolve_style, rounded, shared_style,
    span_style, svg_document, text_style,
};
use crate::text::label::{Label, label_line, layout_label};
use crate::text::{Style, TextMeasurer};
use crate::url::{linked, sanitize_url};
use crate::{DiagramKind, Link, RenderOptions};

use super::db::Model;
use super::layout::{HEAD_LEFT, HEAD_TOP, layout_c4};
use super::route::{Form, Frame, Lanes, Route, cyl_ry, queue_rx, route};

const PAIR_SHIFT: f64 = 7.0;
const NARROW_GAP: f64 = 36.0;
const WIDEN: f64 = 24.0;
const MAX_OFFSET: f64 = 2000.0;
// A label may cover this much of a shape's edge, where there is no text.
const EDGE: f64 = 8.0;
// Places along a relation to try for its label, nearest the middle first.
const SLIDE: [f64; 7] = [0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.8];
// Above this many box comparisons, labels stay at the middle and lines are not steered around
// shapes.
const CARE_BUDGET: f64 = 2_000_000.0;

/// The style a diagram asks for, as the theme resolves it.
fn user_style(fill: Option<&str>, stroke: Option<&str>, color: Option<&str>) -> ResolvedStyle {
    let mut declarations: Vec<String> = Vec::new();
    if let Some(fill) = fill {
        declarations.push(format!("fill:{fill}"));
    }
    if let Some(stroke) = stroke {
        declarations.push(format!("stroke:{stroke}"));
    }
    if let Some(color) = color {
        declarations.push(format!("color:{color}"));
    }
    resolve_style(&declarations)
}

/// A link the diagram asks for, with its address made safe.
fn link(
    body: &str,
    url: Option<&str>,
    id: &str,
    links: &mut Vec<Link>,
    options: &RenderOptions,
) -> String {
    let Some(url) = url else {
        return body.to_string();
    };
    // Entity encoding has hidden any `#name;` in the address. Put it back, so that the check
    // sees what a browser would.
    let url = if url.contains("\u{00b6}\u{00df}") {
        url.replace("\u{fb02}\u{00b0}\u{00b0}", "#")
            .replace("\u{fb02}\u{00b0}", "#")
            .replace("\u{00b6}\u{00df}", ";")
    } else {
        url.to_string()
    };
    linked(body, &sanitize_url(&url), id, links, options, "")
}

/// Writes a label with every line starting at x. A line is anchored at its start and keeps its
/// spaces, so that where it begins does not depend on how its text was measured.
fn left_label_svg(label: &Label, x: f64, top: f64, attrs: &str) -> String {
    let shared = shared_style(label);
    let open = format!("<text{}", text_style(attrs, shared));
    let mut body = String::new();
    for (i, spans) in label.lines.iter().enumerate() {
        let cy = top + (i as f64 + 0.5) * label.line_height;
        // A line with an icon is written as a label of its own, which places the icon.
        if spans.iter().any(|span| span.icon.is_some()) {
            body += &label_svg(&label_line(label, i), x + label.widths[i] / 2.0, cy, attrs);
            continue;
        }
        let mut text = String::new();
        for span in spans {
            let own = span_style(span.style & !shared);
            if own.is_empty() {
                text += &esc(&span.text);
            } else {
                text += &format!("<tspan{own}>{}</tspan>", esc(&span.text));
            }
        }
        if !text.is_empty() {
            body += &format!(
                "{open} x=\"{}\" y=\"{}\" text-anchor=\"start\" xml:space=\"preserve\">{text}</text>",
                num(x),
                num(cy + label.size * 0.35)
            );
        }
    }
    body
}

/// The area a label would cover.
fn covered(taken: &[f64], x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let mut area = 0.0;
    let mut i = 0;
    while i + 3 < taken.len() {
        let w = x1.min(taken[i + 2]) - x0.max(taken[i]);
        if w > 0.0 {
            let h = y1.min(taken[i + 3]) - y0.max(taken[i + 1]);
            if h > 0.0 {
                area += w * h;
            }
        }
        i += 4;
    }
    area
}

/// An offset a relation asks for, kept within what a diagram may move a label.
fn offset(value: Option<f64>) -> f64 {
    match value {
        Some(value) if value != 0.0 && !value.is_nan() => value.clamp(-MAX_OFFSET, MAX_OFFSET),
        _ => 0.0,
    }
}

/// Draws the diagram.
pub(crate) fn render_c4(model: &Model, input: &Input<'_>) -> Result<Output, Error> {
    let options = input.options;
    let failure = std::cell::RefCell::new(None);
    let empty = || Output {
        svg: String::new(),
        width: 0.0,
        height: 0.0,
        links: Vec::new(),
    };
    let out = tighten(options, |tight| {
        if !tight {
            return match draw(model, input, false, NARROW_GAP) {
                Ok((out, _)) => out,
                Err(error) => {
                    *failure.borrow_mut() = Some(error);
                    empty()
                }
            };
        }
        // A relation that found no room of its own is drawn again with more room between the
        // columns, until the drawing is wide enough or the room runs out.
        let (mut best, mut squeezed) = match draw(model, input, true, NARROW_GAP) {
            Ok(drawn) => drawn,
            Err(error) => {
                *failure.borrow_mut() = Some(error);
                return empty();
            }
        };
        let mut gap = NARROW_GAP + WIDEN;
        while squeezed > 0 && gap <= NARROW_GAP + 3.0 * WIDEN {
            match draw(model, input, true, gap) {
                Ok((wider, left)) => {
                    if left < squeezed {
                        best = wider;
                        squeezed = left;
                    }
                }
                Err(error) => {
                    *failure.borrow_mut() = Some(error);
                    return empty();
                }
            }
            gap += WIDEN;
        }
        best
    });
    if let Some(error) = failure.into_inner() {
        return Err(error);
    }
    Ok(out)
}

/// Draws the diagram once, and says how many relations found no room of their own.
fn draw(model: &Model, input: &Input<'_>, tight: bool, gap: f64) -> Result<(Output, usize), Error> {
    let options = input.options;
    let size = options.font_size.unwrap_or(16.0);
    let small = (size * 0.8125).round();
    let measurer: &dyn TextMeasurer = input.measurer;
    let pad = options.padding.unwrap_or(8.0);
    let mut links: Vec<Link> = Vec::new();
    let room = if tight {
        Some(80.0f64.max((options.max_width.unwrap_or(320.0)) - 2.0 * pad))
    } else {
        None
    };
    let mut layout = layout_c4(model, measurer, size, small, room, gap);
    let root = layout.order[0];

    let mut min_x = 0.0f64;
    let mut min_y = 0.0f64;
    let mut max_x: f64 = layout.groups[root].frame.w;
    let mut max_y: f64 = layout.groups[root].frame.h;

    let mut ends: Vec<Frame> = Vec::new();
    let mut pairs: HashMap<usize, HashSet<usize>> = HashMap::new();
    for rel in &model.rels {
        let from = if rel.from_attr.is_some() {
            None
        } else {
            model.get_c4_shape(&rel.from)
        };
        let to = if rel.to_attr.is_some() {
            None
        } else {
            model.get_c4_shape(&rel.to)
        };
        let a = from.and_then(|found| layout.boxes.get(&found)).cloned();
        let b = to.and_then(|found| layout.boxes.get(&found)).cloned();
        let (Some(a), Some(b)) = (a, b) else {
            return Err(Error::new(
                ErrorCode::Semantic,
                format!(
                    "C4 relation \"{}\" -> \"{}\" references an unknown shape or boundary",
                    rel.from, rel.to
                ),
            ));
        };
        pairs.entry(a.id).or_default().insert(b.id);
        ends.push(a);
        ends.push(b);
    }

    // What a relation's label should not cover: shapes, arrowheads, boundary headings, and the
    // labels placed before it.
    let mut nodes: Vec<Frame> = Vec::new();
    let mut taken: Vec<f64> = Vec::new();
    for &group in &layout.order {
        for &node_at in &layout.groups[group].nodes {
            let frame = &layout.nodes[node_at].frame;
            nodes.push(frame.clone());
            taken.extend_from_slice(&[
                frame.x + EDGE,
                frame.y + EDGE,
                frame.x + frame.w - EDGE,
                frame.y + frame.h - EDGE,
            ]);
        }
    }
    let careful = (model.rels.len() * (nodes.len() + layout.order.len() + 3 * model.rels.len()))
        as f64
        <= CARE_BUDGET;

    // Every line is drawn before any label is placed, so that labels can keep off the
    // arrowheads.
    let mut casings = String::new();
    let fills: Vec<_> = layout
        .order
        .iter()
        .copied()
        .filter(|&g| g != root)
        .filter_map(|g| {
            let group = &layout.groups[g];
            let frame = &group.frame;
            let shape = user_style(
                model.boundaries[group.boundary].element.bg_color.as_deref(),
                None,
                None,
            )
            .shape;
            crate::svg::ink::paints(&shape).then_some(crate::svg::fills::Fill {
                bounds: [frame.x, frame.y, frame.x + frame.w, frame.y + frame.h],
                shape,
            })
        })
        .collect();
    let mut edges_out = String::new();
    let trim = marker_trim("arrow_point");
    let mut lanes = Lanes::default();
    for &group in &layout.order {
        if group != root {
            lanes.frame(&layout.groups[group].frame);
        }
    }
    let mut routes: Vec<Route> = Vec::new();
    for (index, rel) in model.rels.iter().enumerate() {
        let a = &ends[index * 2];
        let b = &ends[index * 2 + 1];
        let style = user_style(None, rel.line_color.as_deref(), None);
        let mut color = style
            .stroke
            .as_deref()
            .map(esc)
            .unwrap_or_else(|| "var(--_l)".to_string());
        let start_arrow = rel.kind == "birel" || rel.kind == "rel_b";
        let end_arrow = rel.kind != "rel_b";
        let reverse = a.id != b.id
            && pairs
                .get(&b.id)
                .is_some_and(|targets| targets.contains(&a.id));
        let line = route(
            a,
            b,
            if reverse { PAIR_SHIFT } else { 0.0 },
            if careful { Some(&nodes) } else { None },
            &mut lanes,
            if start_arrow { trim } else { 0.0 },
            if end_arrow { trim } else { 0.0 },
        );
        min_x = min_x.min(line.x0.min(line.x1));
        min_y = min_y.min(line.y0.min(line.y1));
        max_x = max_x.max(line.x0.max(line.x1));
        max_y = max_y.max(line.y0.max(line.y1));
        if start_arrow && careful {
            taken.extend_from_slice(&[
                line.sx - ARROW,
                line.sy - ARROW,
                line.sx + ARROW,
                line.sy + ARROW,
            ]);
        }
        if end_arrow && careful {
            taken.extend_from_slice(&[
                line.ex - ARROW,
                line.ey - ARROW,
                line.ex + ARROW,
                line.ey + ARROW,
            ]);
        }
        let over = crate::svg::fills::over_fills(
            &line.d,
            &[line.sx, line.sy, line.ex, line.ey],
            &fills,
            &style.line,
        );
        if over.as_deref() == Some("") {
            let markers = (if start_arrow {
                marker(
                    "arrow_point",
                    line.sx,
                    line.sy,
                    line.sdx,
                    line.sdy,
                    "var(--_bg)",
                )
            } else {
                String::new()
            }) + &if end_arrow {
                marker(
                    "arrow_point",
                    line.ex,
                    line.ey,
                    line.edx,
                    line.edy,
                    "var(--_bg)",
                )
            } else {
                String::new()
            };
            casings += &crate::svg::fills::casing(
                &line.d,
                &markers.replace("stroke=\"none\"", "stroke=\"var(--_bg)\""),
                &style.line,
                false,
            );
        }
        if let Some(over) = over.as_ref().filter(|s| !s.is_empty()) {
            color = over.clone();
        }
        let over_attr = over
            .filter(|s| !s.is_empty())
            .map_or_else(String::new, |s| format!(" stroke=\"{s}\""));
        let mut arrows = String::new();
        if start_arrow {
            arrows += &marker("arrow_point", line.sx, line.sy, line.sdx, line.sdy, &color);
        }
        if end_arrow {
            arrows += &marker("arrow_point", line.ex, line.ey, line.edx, line.edy, &color);
        }
        edges_out += &format!(
            "<g class=\"pele-edge\" data-id=\"{}\"{over_attr}{}><path d=\"{}\"/>{}</g>",
            esc_text(&format!("{}-{}", rel.from, rel.to)),
            style.line,
            line.d,
            arrows
        );
        routes.push(line);
    }

    // A boundary's heading goes at the left, the middle or the right of its top edge: the first
    // of them no line crosses.
    for &group in &layout.order {
        let head_w = layout.groups[group].head_w;
        if !careful || head_w == 0.0 {
            continue;
        }
        let frame = layout.groups[group].frame.clone();
        let top = frame.y + HEAD_TOP;
        let bottom = frame.y + layout.groups[group].head - super::route::LANE;
        let mut fewest = f64::INFINITY;
        for x in [
            HEAD_LEFT,
            (frame.w - head_w) / 2.0,
            frame.w - HEAD_LEFT - head_w,
        ] {
            let mut count = 0usize;
            for line in &routes {
                if super::route::crosses(
                    &line.points,
                    frame.x + x - 4.0,
                    top,
                    frame.x + x + head_w + 4.0,
                    bottom,
                ) {
                    count += 1;
                }
            }
            if (count as f64) < fewest {
                fewest = count as f64;
                layout.groups[group].head_x = x;
            }
            if count == 0 {
                break;
            }
        }
        taken.extend_from_slice(&[
            frame.x + layout.groups[group].head_x - 4.0,
            top,
            frame.x + layout.groups[group].head_x + head_w + 4.0,
            bottom,
        ]);
    }

    let mut labels_out = String::new();
    for (index, rel) in model.rels.iter().enumerate() {
        let line = &routes[index];
        let id = format!("{}-{}", rel.from, rel.to);
        let style = user_style(None, None, rel.text_color.as_deref());
        let text = if model.c4_type.as_deref() == Some("C4Dynamic") {
            format!("{}: {}", index + 1, rel.label)
        } else {
            rel.label.clone()
        };
        let label = layout_label(Some(&text), false, measurer, small, line.room, Style::NONE);
        let techn_text = rel
            .techn
            .as_ref()
            .filter(|t| !t.is_empty())
            .map(|t| format!("[{t}]"));
        let techn = layout_label(
            techn_text.as_deref(),
            false,
            measurer,
            small,
            line.room,
            Style::ITALIC,
        );
        let lw = label.width.max(techn.width) + super::route::LABEL_PAD;
        let lh = label.height + techn.height;
        if lh == 0.0 {
            continue;
        }

        // The label sits on the line, or just beside it when another relation runs the opposite
        // way.
        let beside = line.nx.abs() * (lw / 2.0) + line.ny.abs() * (lh / 2.0) + 2.0;
        let aside = if line.lean != 0.0 {
            (beside - 14.0).max(0.0).min(line.lean)
        } else {
            beside
        };
        // Manual offsets are tuned to the original spacing. Reposition compact labels beside their
        // routed arrows so an offset cannot leave them floating outside a boundary.
        let mut dx = line.nx * aside + if tight { 0.0 } else { offset(rel.offset_x) };
        let mut dy = line.ny * aside + if tight { 0.0 } else { offset(rel.offset_y) };
        let mut at = 0.5;
        if careful {
            // Slide along the line to the spot that covers least. Offsets from the diagram are
            // kept unless a spot on the line itself covers less than any they lead to: they are
            // often tuned for Mermaid's spacing.
            let mut least = f64::INFINITY;
            let moved = dx != line.nx * aside || dy != line.ny * aside;
            let mut pass = 0;
            while pass < if moved { 2 } else { 1 } && least > 0.0 {
                let px = if pass == 1 { line.nx * aside } else { dx };
                let py = if pass == 1 { line.ny * aside } else { dy };
                for &t in &SLIDE {
                    let cx = line.x0 + (line.x1 - line.x0) * t + px;
                    let cy = line.y0 + (line.y1 - line.y0) * t + py;
                    let area = covered(
                        &taken,
                        cx - lw / 2.0,
                        cy - lh / 2.0,
                        cx + lw / 2.0,
                        cy + lh / 2.0,
                    );
                    if area < least {
                        least = area;
                        at = t;
                        dx = px;
                        dy = py;
                    }
                    if area == 0.0 {
                        break;
                    }
                }
                pass += 1;
            }
        }
        let cx = line.x0 + (line.x1 - line.x0) * at + dx;
        let top = line.y0 + (line.y1 - line.y0) * at + dy - lh / 2.0;
        if careful {
            taken.extend_from_slice(&[cx - lw / 2.0, top, cx + lw / 2.0, top + lh]);
        }
        min_x = min_x.min(cx - lw / 2.0);
        min_y = min_y.min(top);
        max_x = max_x.max(cx + lw / 2.0);
        max_y = max_y.max(top + lh);
        let mut body = String::new();
        if let Some(descr) = rel.descr.as_ref().filter(|d| !d.is_empty()) {
            body += &format!("<title>{}</title>", esc_text(descr));
        }
        body += &format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" fill=\"var(--_bg)\"/>",
            num(cx - lw / 2.0),
            num(top),
            num(lw),
            num(lh)
        );
        body += &link(
            &(label_svg(&label, cx, top + label.height / 2.0, &style.text)
                + &label_svg(
                    &techn,
                    cx,
                    top + label.height + techn.height / 2.0,
                    &format!(" class=\"pele-c4-techn\" fill=\"var(--_m)\"{}", style.text),
                )),
            rel.link.as_deref(),
            &id,
            &mut links,
            options,
        );
        labels_out += &format!(
            "<g class=\"pele-edge-label\" data-id=\"{}\">{body}</g>",
            esc_text(&id)
        );
    }

    let mut clusters = String::new();
    let mut nodes_out = String::new();
    for &group in &layout.order {
        if group != root {
            let boundary = &model.boundaries[layout.groups[group].boundary];
            let style = user_style(
                boundary.element.bg_color.as_deref(),
                boundary.element.border_color.as_deref(),
                boundary.element.font_color.as_deref(),
            );
            let color = crate::svg::ink::ink_fill(&style, None);
            let muted = crate::svg::ink::ink_fill(&style, Some("var(--_m)"));
            let frame = &layout.groups[group].frame;
            let x = frame.x + layout.groups[group].head_x;
            let mut y = frame.y + HEAD_TOP;
            let labels = &layout.groups[group].labels;
            let mut text = left_label_svg(
                &labels[0],
                x,
                y,
                &format!(
                    " class=\"pele-cluster-label\" font-weight=\"var(--_hw)\"{color}{}",
                    style.text
                ),
            );
            y += labels[0].height;
            text += &left_label_svg(
                &labels[1],
                x,
                y,
                &format!(
                    " class=\"pele-c4-type\" font-size=\"{small}\"{muted}{}",
                    style.text
                ),
            );
            y += labels[1].height;
            text += &left_label_svg(
                &labels[2],
                x,
                y,
                &format!(
                    " class=\"pele-c4-descr\" font-size=\"{small}\"{muted}{}",
                    style.text
                ),
            );
            clusters += &format!(
                "<g class=\"pele-cluster {}\" data-id=\"{}\">",
                if boundary.node_type.is_some() {
                    "pele-c4-node"
                } else {
                    "pele-c4-boundary"
                },
                esc_text(&boundary.element.alias)
            );
            clusters += &format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"var(--_b)\"{}{}/>",
                num(frame.x),
                num(frame.y),
                num(frame.w),
                num(frame.h),
                if boundary.node_type.is_some() {
                    ""
                } else {
                    " stroke-dasharray=\"6 4\""
                },
                rounded(&style.shape)
            );
            clusters += &link(
                &text,
                boundary.element.link.as_deref(),
                &boundary.element.alias,
                &mut links,
                options,
            );
            clusters += "</g>";
        }
        for &node_at in &layout.groups[group].nodes {
            let node = &layout.nodes[node_at];
            let shape = &model.shapes[node.shape];
            let external = shape.kind.starts_with("external");
            let style = user_style(
                shape.element.bg_color.as_deref(),
                shape.element.border_color.as_deref(),
                shape.element.font_color.as_deref(),
            );
            let stroke = format!(
                " stroke=\"var(--_b)\"{}",
                if external {
                    " stroke-dasharray=\"4 3\""
                } else {
                    ""
                }
            );
            let dx = if node.form == Form::HCyl {
                -queue_rx(node.frame.h) / 2.0
            } else {
                0.0
            };
            let dy = if node.form == Form::Person {
                super::route::HEAD / 2.0
            } else if node.form == Form::Cyl {
                cyl_ry(node.frame.w) / 2.0
            } else {
                0.0
            };
            let color = crate::svg::ink::ink_fill(&style, None);
            let muted = format!(
                "{}{}",
                crate::svg::ink::ink_fill(&style, Some("var(--_m)")),
                style.text
            );
            let mut y = dy - node.text_h / 2.0;
            let mut text = label_svg(
                &node.labels[0],
                dx,
                y + node.labels[0].height / 2.0,
                &format!(
                    " class=\"pele-c4-type\" font-size=\"{}\"{muted}",
                    small - 1.0
                ),
            );
            y += node.labels[0].height;
            text += &label_svg(
                &node.labels[1],
                dx,
                y + node.labels[1].height / 2.0,
                &format!(
                    " class=\"pele-label\" font-weight=\"var(--_hw)\"{color}{}",
                    style.text
                ),
            );
            y += node.labels[1].height;
            text += &label_svg(
                &node.labels[2],
                dx,
                y + node.labels[2].height / 2.0,
                &format!(" class=\"pele-c4-techn\" font-size=\"{small}\"{muted}"),
            );
            y += node.labels[2].height + 4.0;
            text += &label_svg(
                &node.labels[3],
                dx,
                y + node.labels[3].height / 2.0,
                &format!(
                    " class=\"pele-c4-descr\" font-size=\"{small}\"{color}{}",
                    style.text
                ),
            );
            nodes_out += &format!(
                "<g class=\"pele-node pele-shape-{} pele-c4-{}{}\" data-id=\"{}\" transform=\"translate({},{})\">",
                node.form.as_str(),
                shape.kind,
                if external { " pele-c4-external" } else { "" },
                esc_text(&shape.element.alias),
                num(node.frame.x + node.frame.w / 2.0),
                num(node.frame.y + node.frame.h / 2.0)
            );
            let body = format!(
                "{}{}",
                draw_shape(
                    node.form.as_str(),
                    node.frame.w,
                    node.frame.h,
                    &format!(
                        " fill=\"var({})\"{stroke}{}",
                        if external { "--_bg" } else { "--_s" },
                        style.shape
                    ),
                    &format!("{stroke}{}", style.line),
                ),
                text
            );
            nodes_out += &link(
                &body,
                shape.element.link.as_deref(),
                &shape.element.alias,
                &mut links,
                options,
            );
            nodes_out += "</g>";
        }
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
    let inner = (max_x - min_x).max(title.width);
    let width = (inner + 2.0 * pad).ceil();
    let height = (max_y - min_y + title_height + 2.0 * pad).ceil();
    let ox = pad - min_x + (inner - (max_x - min_x)) / 2.0;
    let oy = pad + title_height - min_y;

    let content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &format!("<g transform=\"translate({},{})\">", num(ox), num(oy))
        + &if clusters.is_empty() {
            String::new()
        } else {
            format!("<g class=\"pele-clusters\">{clusters}</g>")
                + &crate::svg::fills::casings_svg(&casings)
        }
        + &if edges_out.is_empty() {
            String::new()
        } else {
            format!(
                "<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" \
                 stroke-linecap=\"round\">{edges_out}</g>"
            )
        }
        + &if nodes_out.is_empty() {
            String::new()
        } else {
            format!("<g class=\"pele-nodes\">{nodes_out}</g>")
        }
        + &if labels_out.is_empty() {
            String::new()
        } else {
            format!("<g class=\"pele-edge-labels\" font-size=\"{small}\">{labels_out}</g>")
        }
        + "</g>";

    let acc = Accessible {
        title: model.acc_title.clone(),
        descr: model.acc_descr.clone(),
    };
    let output = Output {
        svg: svg_document(
            DiagramKind::C4,
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
    };
    Ok((output, lanes.squeezed))
}
