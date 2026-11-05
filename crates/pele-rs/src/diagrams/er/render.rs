//! ER diagram layout and SVG. Ported from pele's `diagrams/er/render.ts`, without math and
//! icons.

use std::collections::HashMap;
use std::fmt::Write;

use super::db::ErDb;
use super::graph::{ErGraphEdge, ErGraphNode, build_er_graph};
use super::markers::{MARKER_LENGTH, er_marker};
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{runs_across, turn_to_fit};
use crate::diagrams::common::generics::parse_generic_types;
use crate::layout::compound::{
    CEdge, CNode, Dir, cedge, cnode, compound_layout, direction, shift_layout,
};
use crate::layout::layered::LayeredOptions;
use crate::preprocess::{Config, Value};
use crate::svg::cluster::{Crossings, cluster_title_x, mark_crossings};
use crate::svg::edges::{edge_label_svg, route_path};
use crate::svg::{
    Accessible, ROUNDED, ResolvedStyle, class_names, esc, esc_text, icon_svg, label_svg, num,
    resolve_style, rounded, shared_style, span_style, svg_document, text_style, title_label,
};
use crate::text::Style;
use crate::text::entities::decode_entities;
use crate::text::label::{Label, has_math, label_line, layout_label, line_middle};
use crate::url::{link_url, rel_attr, sanitize_url};
use crate::util::{is_space, round};
use crate::{DiagramKind, Link};

const NODE_SEP: f64 = 48.0;
const EDGE_SEP: f64 = 20.0;
const RANK_SEP: f64 = 64.0;
const GROUP_PAD: f64 = 20.0;
const LOOP: f64 = 28.0;
const LOOP_BAND: f64 = 38.0;
const MIN_WIDTH: f64 = 96.0;
const BOX_PAD_X: f64 = 18.0;
const BOX_PAD_Y: f64 = 12.0;
const HEAD_PAD_Y: f64 = 9.0;
const CELL_PAD_X: f64 = 12.0;
const CELL_GAP: f64 = 16.0;
const ROW_PAD: f64 = 9.0;
const COMMENT_WRAP: f64 = 260.0;
const NO_WRAP: f64 = 1e6;
const PORT_GAP: f64 = 18.0;
const PORT_MARGIN: f64 = 14.0;
const BOX_ATTRS: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";

fn number_option(section: &Config, key: &str, fallback: f64) -> f64 {
    match section.get(key) {
        Some(&Value::Number(v)) if v > 0.0 => v.min(2000.0),
        _ => fallback,
    }
}

/// Type, name, keys, comment.
struct Row {
    cells: [Label; 4],
    h: f64,
}

struct View<'a> {
    node: &'a ErGraphNode<'a>,
    style: ResolvedStyle,
    label: Label,
    w: f64,
    h: f64,
    head_h: f64,
    rows: Vec<Row>,
    /// Left edge of each column, measured from the left edge of the box.
    cols: Vec<f64>,
    row_size: f64,
    key_size: f64,
    /// Whether self-relationships loop out of the bottom side rather than the right.
    below: bool,
}

/// A relationship from an entity to itself, with the stretch of the side it loops out of.
struct Loop {
    edge: usize,
    band: f64,
    at: f64,
}

/// Writes a measured label starting at x, with the baseline of its first line at y. A line that
/// holds math is written as a label of its own, and an icon is written beside the text.
fn text_left(label: &Label, x: f64, y: f64, attrs: &str) -> String {
    let lines = &label.lines;
    if lines.is_empty() {
        return String::new();
    }
    if let [line] = lines.as_slice() {
        if let [only] = line.as_slice() {
            if only.is_plain() && only.style.is_empty() {
                return format!(
                    "<text{attrs} x=\"{}\" y=\"{}\">{}</text>",
                    num(x),
                    num(y),
                    esc(&only.text)
                );
            }
        }
    }
    let shared = shared_style(label);
    let shift = label.size * 0.35;
    let top = y - shift - label.line_height / 2.0;
    let mut body = String::new();
    let mut extra = String::new();
    for (i, line) in lines.iter().enumerate() {
        let middle = top + line_middle(label, i);
        if has_math(line) {
            extra += &label_svg(
                &label_line(label, i),
                x + label.widths[i] / 2.0,
                middle,
                attrs,
            );
            continue;
        }
        let y = middle + shift;
        let has_icon = line.iter().any(|span| span.icon.is_some());
        let mut at = x;
        let mut placed = false;
        for span in line {
            if let Some(icon) = &span.icon {
                let side = label.size * 1.1;
                extra += &icon_svg(&icon.name, at, middle - side / 2.0, side, &icon.markup);
            } else if !span.text.is_empty() {
                let pos = if has_icon || !placed {
                    format!(" x=\"{}\" y=\"{}\"", num(at), num(y))
                } else {
                    String::new()
                };
                let _ = write!(
                    body,
                    "<tspan{pos}{}>{}</tspan>",
                    span_style(span.style & !shared),
                    esc(&span.text)
                );
                placed = true;
            }
            at += span.width;
        }
    }
    format!(
        "<text{} xml:space=\"preserve\">{body}</text>{extra}",
        text_style(attrs, shared)
    )
}

/// Mermaid reads `~` pairs as angle brackets. They are written as entities so the label parser
/// does not take `<int>` for a tag.
fn cell_text(raw: &str) -> String {
    let converted = parse_generic_types(raw);
    if converted == raw {
        raw.to_string()
    } else {
        converted.replace('<', "&lt;").replace('>', "&gt;")
    }
}

/// `classes.replace(/^default\s?/, '')`.
fn without_default(classes: &str) -> &str {
    match classes.strip_prefix("default") {
        Some(rest) => match rest.chars().next() {
            Some(c) if is_space(c) => &rest[c.len_utf8()..],
            _ => rest,
        },
        None => classes,
    }
}

pub(crate) fn render_er(db: &ErDb, input: &Input<'_>) -> Output {
    if runs_across(Some(db.direction)) {
        turn_to_fit(input.options, |down| draw(db, input, down))
    } else {
        draw(db, input, false)
    }
}

/// `turned` draws a diagram that runs across as one that runs down.
fn draw(db: &ErDb, input: &Input<'_>, turned: bool) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let empty = Config::new();
    let conf = match input.config.get("er") {
        Some(Value::Map(m)) => m,
        _ => &empty,
    };
    let graph = build_er_graph(db);
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let pad = options.padding.unwrap_or(8.0);

    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, node) in graph.nodes.iter().enumerate() {
        index.insert(&node.id, i);
    }

    let mut cedges: Vec<CEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut drawn: Vec<&ErGraphEdge<'_>> = Vec::new();
    // Self-relationships per node, in the order the nodes first get one.
    let mut loops: Vec<(usize, Vec<Loop>)> = Vec::new();
    let mut loop_of: Vec<Option<(usize, usize)>> = Vec::new();
    for edge in &graph.edges {
        let (Some(&src), Some(&dst)) = (index.get(edge.start), index.get(edge.end)) else {
            continue;
        };
        let label = layout_label(Some(edge.label), true, measurer, small, 200.0, Style::NONE);
        let mut at = None;
        if src == dst {
            let k = match loops.iter().position(|(node, _)| *node == src) {
                Some(k) => k,
                None => {
                    loops.push((src, Vec::new()));
                    loops.len() - 1
                }
            };
            at = Some((k, loops[k].1.len()));
            loops[k].1.push(Loop {
                edge: drawn.len(),
                band: 0.0,
                at: 0.0,
            });
        }
        loop_of.push(at);
        drawn.push(edge);
        cedges.push(cedge(
            src,
            dst,
            1,
            if label.width > 0.0 {
                label.width + 12.0
            } else {
                0.0
            },
            if label.height > 0.0 {
                label.height + 4.0
            } else {
                0.0
            },
        ));
        edge_labels.push(label);
    }

    // The direction of the level a node is laid out in: its nearest subgraph that sets one, or
    // the diagram's.
    let horizontal = |i: usize| -> bool {
        let mut at = i;
        // Subgraphs can name each other as members, so the walk up is bounded.
        for _ in 0..graph.nodes.len() {
            let Some(up) = graph.nodes[at]
                .parent_id
                .and_then(|p| index.get(p).copied())
            else {
                break;
            };
            if let Some(dir) = graph.nodes[up].dir {
                return dir == "LR" || dir == "RL";
            }
            at = up;
        }
        db.direction == "LR" || db.direction == "RL"
    };

    let mut views: Vec<View<'_>> = Vec::with_capacity(graph.nodes.len());
    let mut cnodes: Vec<CNode> = Vec::with_capacity(graph.nodes.len());
    for (i, node) in graph.nodes.iter().enumerate() {
        let style = if node.css_styles.is_empty() {
            resolve_style(&node.css_compiled_styles)
        } else {
            let mut all = node.css_compiled_styles.clone();
            all.extend(node.css_styles.iter().cloned());
            resolve_style(&all)
        };
        let mut base = Style::NONE;
        if style.bold {
            base |= Style::BOLD;
        }
        if style.italic {
            base |= Style::ITALIC;
        }
        let name_size = style.font_size.unwrap_or(size);
        let row_size = style.font_size.unwrap_or(small);
        let key_size = style.font_size.unwrap_or_else(|| round(size * 0.75));
        let label = if node.is_group {
            layout_label(Some(node.label), true, measurer, name_size, 4000.0, base)
        } else {
            let name = if node.alias.is_empty() {
                node.label
            } else {
                node.alias
            };
            layout_label(Some(name), true, measurer, name_size, 240.0, base)
        };
        let own: &mut [Loop] = loops
            .iter_mut()
            .find(|(n, _)| *n == i)
            .map_or(&mut [], |(_, l)| l.as_mut_slice());
        // Loops leave by a side the layout keeps free of other edges: the right when ranks run
        // down, the bottom when they run across.
        let below = !own.is_empty() && !node.is_group && horizontal(i);
        let mut view = View {
            node,
            style,
            label,
            w: 0.0,
            h: 0.0,
            head_h: 0.0,
            rows: Vec::new(),
            cols: Vec::new(),
            row_size,
            key_size,
            below,
        };
        let mut reach: f64 = 0.0;
        let mut bands = 0.0;
        for lp in own.iter_mut() {
            let label = &edge_labels[lp.edge];
            lp.band = LOOP_BAND.max(if below {
                label.width + 12.0
            } else {
                label.height + 6.0
            });
            bands += lp.band;
            reach = reach.max(if below { label.height } else { label.width });
        }
        let min_w = MIN_WIDTH.max(if below { bands + 8.0 } else { 0.0 });
        let mut c;
        if node.is_group {
            c = cnode(0.0, 0.0);
            c.is_group = true;
            c.dir = node.dir.map(|d| direction(Some(d)));
            c.pad_x = GROUP_PAD;
            c.pad_top = if view.label.height > 0.0 {
                view.label.height + 16.0
            } else {
                GROUP_PAD
            };
            c.pad_bottom = GROUP_PAD;
            c.min_w = (view.label.width + 2.0 * GROUP_PAD - 12.0).max(MIN_WIDTH);
        } else {
            let name_h = view.label.height.max(round(name_size * 1.5));
            if node.attributes.is_empty() {
                view.w = (view.label.width + 2.0 * BOX_PAD_X).max(min_w);
                view.h = (name_h + 2.0 * BOX_PAD_Y).max(if below { 0.0 } else { bands + 8.0 });
            } else {
                let row_line = round(row_size * 1.5);
                let mut widths = [0.0f64; 4];
                for attribute in node.attributes {
                    let cells = [
                        layout_label(
                            Some(&cell_text(&attribute.kind)),
                            true,
                            measurer,
                            row_size,
                            NO_WRAP,
                            base,
                        ),
                        layout_label(
                            Some(&cell_text(&attribute.name)),
                            true,
                            measurer,
                            row_size,
                            NO_WRAP,
                            base,
                        ),
                        layout_label(
                            Some(&attribute.keys.join(", ")),
                            false,
                            measurer,
                            key_size,
                            NO_WRAP,
                            base,
                        ),
                        layout_label(
                            Some(&cell_text(&attribute.comment)),
                            true,
                            measurer,
                            row_size,
                            COMMENT_WRAP,
                            base,
                        ),
                    ];
                    let mut h = row_line;
                    for k in 0..4 {
                        widths[k] = widths[k].max(cells[k].width);
                        h = h.max(cells[k].height);
                    }
                    view.rows.push(Row {
                        cells,
                        h: h + ROW_PAD,
                    });
                    view.h += h + ROW_PAD;
                }
                let mut columns = 0.0;
                let mut inner = 0.0;
                for &w in &widths {
                    if w == 0.0 {
                        continue;
                    }
                    inner += w + if columns > 0.0 { CELL_GAP } else { 0.0 };
                    columns += 1.0;
                }
                view.w = (inner + 2.0 * CELL_PAD_X)
                    .max(view.label.width + 2.0 * BOX_PAD_X)
                    .max(min_w);
                // A box wider than its table shares the spare width between the columns.
                let spare = if columns > 0.0 {
                    (view.w - inner - 2.0 * CELL_PAD_X) / columns
                } else {
                    0.0
                };
                let mut x = CELL_PAD_X;
                for &w in &widths {
                    view.cols.push(x);
                    if w > 0.0 {
                        x += w + spare + CELL_GAP;
                    }
                }
                view.head_h = name_h + 2.0 * HEAD_PAD_Y;
                view.h += view.head_h;
            }
            // The layout keeps a node centered in its box, so room for loops is added on both
            // sides.
            let room = if own.is_empty() {
                0.0
            } else {
                2.0 * (MARKER_LENGTH + LOOP * 0.75 + 10.0 + reach)
            };
            c = if below {
                cnode(view.w, view.h + room)
            } else {
                cnode(view.w + room, view.h)
            };
        }
        c.seq = views.len() as f64;
        views.push(view);
        cnodes.push(c);
    }
    for (i, node) in graph.nodes.iter().enumerate() {
        cnodes[i].parent = node
            .parent_id
            .and_then(|p| index.get(p))
            .map_or(-1, |&k| k as i64);
    }

    let root_dir = if turned {
        Dir::TB
    } else {
        direction(Some(db.direction))
    };
    let layout_options = LayeredOptions {
        node_sep: number_option(conf, "nodeSpacing", NODE_SEP),
        edge_sep: EDGE_SEP,
        rank_sep: number_option(conf, "rankSpacing", RANK_SEP),
        port_sep: 24.0,
        ..LayeredOptions::default()
    };
    let layout = compound_layout(&mut cnodes, &mut cedges, root_dir, &layout_options);

    spread_ends(&mut cedges, &cnodes, &views);

    // A subgraph gets its size from the layout, so its loops hang outside it and may widen the
    // diagram.
    let mut layout_width = layout.width;
    for (i, own) in &loops {
        let c = &cnodes[*i];
        if !c.is_group {
            continue;
        }
        let mut reach: f64 = 0.0;
        for lp in own {
            reach = reach.max(edge_labels[lp.edge].width);
        }
        layout_width =
            layout_width.max(c.x + c.w / 2.0 + MARKER_LENGTH + LOOP * 0.75 + 10.0 + reach);
    }

    let title = title_label(db.title.as_deref(), measurer, size, 4000.0);
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let inner = layout_width.max(title.width);
    let ox = pad + (inner - layout_width) / 2.0;
    let oy = pad + title_height;
    shift_layout(&mut cnodes, &mut cedges, ox, oy);
    let width = (inner + 2.0 * pad).ceil();
    let height = (layout.height + title_height + 2.0 * pad).ceil();
    let mut links: Vec<Link> = Vec::new();

    let mut crossings: Crossings = HashMap::new();
    if cnodes.iter().any(|c| c.is_group) {
        for e in &cedges {
            mark_crossings(&mut crossings, &e.route);
        }
    }

    let mut clusters = String::new();
    let mut nodes_out = String::new();
    for (i, view) in views.iter_mut().enumerate() {
        let c = &cnodes[i];
        let node = view.node;
        let classes = class_names(without_default(&node.css_classes));
        if c.is_group {
            view.w = c.w;
            view.h = c.h;
            let x = c.x - c.w / 2.0;
            let y = c.y - c.h / 2.0;
            let title_x = cluster_title_x(c, view.label.width, &crossings, GROUP_PAD);
            let _ = write!(
                clusters,
                "<g class=\"pele-rs-cluster{classes}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"{}/>{}</g>",
                esc_text(&node.id),
                num(x),
                num(y),
                num(c.w),
                num(c.h),
                rounded(&view.style.shape),
                label_svg(
                    &view.label,
                    title_x,
                    y + 8.0 + view.label.height / 2.0,
                    &format!(
                        " class=\"pele-rs-cluster-label\" fill=\"var(--_m)\"{}",
                        view.style.text
                    )
                )
            );
            continue;
        }

        let (w, h) = (view.w, view.h);
        let x0 = -w / 2.0;
        let y0 = -h / 2.0;
        let text = &view.style.text;
        let shape = format!(
            "x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
            num(x0),
            num(y0),
            num(w),
            num(h)
        );
        let mut name = label_svg(
            &view.label,
            0.0,
            if view.rows.is_empty() {
                0.0
            } else {
                y0 + view.head_h / 2.0
            },
            &format!(" class=\"pele-rs-label\"{text}"),
        );
        let display = decode_entities(if node.alias.is_empty() {
            node.label
        } else {
            node.alias
        });
        if format!("{classes} ").contains(" internal-link ") && !name.is_empty() {
            // The name ends up in an href, so it gets the same check as a URL.
            let safe = link_url(&sanitize_url(&display), options).is_some();
            let target = esc(&display);
            let href = if safe {
                format!(" href=\"{target}\"{}", rel_attr(options))
            } else {
                String::new()
            };
            name = format!("<a class=\"internal-link\"{href} data-href=\"{target}\">{name}</a>");
            links.push(Link {
                id: node.label.to_string(),
                href: display.clone(),
                internal: true,
            });
        }

        let body = if view.rows.is_empty() {
            format!(
                "<rect {shape}{}/>{name}",
                rounded(&format!("{BOX_ATTRS}{}", view.style.shape))
            )
        } else {
            let row_line = round(view.row_size * 1.5);
            let line_color = view
                .style
                .stroke
                .as_deref()
                .map_or_else(|| "var(--_b)".to_string(), esc);
            let mut stripes = String::new();
            let mut cells = String::new();
            let mut y = y0 + view.head_h;
            let last = view.rows.len() - 1;
            for (r, row) in view.rows.iter().enumerate() {
                if r % 2 == 0 {
                    let span = format!("x=\"{}\" width=\"{}\"", num(x0), num(w));
                    if r == last {
                        // The last row follows the box's rounded bottom corners: a rounded
                        // rect, squared off on top.
                        let _ = write!(
                            stripes,
                            "<rect {span} y=\"{}\" height=\"{}\"{ROUNDED}/><rect {span} y=\"{}\" height=\"{}\"/>",
                            num(y),
                            num(row.h),
                            num(y),
                            num(row.h / 2.0)
                        );
                    } else {
                        let _ = write!(
                            stripes,
                            "<rect {span} y=\"{}\" height=\"{}\"/>",
                            num(y),
                            num(row.h)
                        );
                    }
                }
                let baseline = y + ROW_PAD / 2.0 + row_line / 2.0 + view.row_size * 0.35;
                let [kind, attribute, keys, comment] = &row.cells;
                cells += &text_left(
                    kind,
                    x0 + view.cols[0],
                    baseline,
                    &format!(" class=\"pele-rs-er-type\" fill=\"var(--_m)\"{text}"),
                );
                cells += &text_left(
                    attribute,
                    x0 + view.cols[1],
                    baseline,
                    &format!(" class=\"pele-rs-er-name\"{text}"),
                );
                cells += &text_left(
                    keys,
                    x0 + view.cols[2],
                    baseline,
                    &format!(
                        " class=\"pele-rs-er-keys\" font-size=\"{}\" fill=\"var(--_m)\"{text}",
                        num(view.key_size)
                    ),
                );
                cells += &text_left(
                    comment,
                    x0 + view.cols[3],
                    baseline,
                    &format!(" class=\"pele-rs-er-comment\" fill=\"var(--_m)\"{text}"),
                );
                y += row.h;
            }
            let divider_y = num(y0 + view.head_h);
            // Over a fill the diagram chose, the shading is a tint of that fill. The opacity
            // is on the group because the two parts of the last row overlap.
            format!(
                "<rect {shape}{}/><g class=\"pele-rs-er-stripes\" fill=\"var(--_bg)\"{}>{stripes}</g><path class=\"pele-rs-er-divider\" d=\"M{},{divider_y}H{}\" stroke=\"{line_color}\"/><rect {shape} fill=\"none\" stroke=\"var(--_b)\"{}/>{name}<g class=\"pele-rs-er-attributes\" font-size=\"{}\">{cells}</g>",
                rounded(&format!("{BOX_ATTRS}{}", view.style.shape)),
                if view.style.shape.contains("fill:") {
                    " opacity=\"0.6\""
                } else {
                    ""
                },
                num(x0),
                num(x0 + w),
                rounded(&view.style.line),
                num(view.row_size)
            )
        };
        let _ = write!(
            nodes_out,
            "<g class=\"pele-rs-node pele-rs-entity{classes}\" data-id=\"{}\" transform=\"translate({},{})\">{body}</g>",
            esc_text(node.label),
            num(c.x),
            num(c.y)
        );
    }

    // Self-relationships share the side they loop out of, squeezed together if it is too short
    // for them.
    for (i, own) in &mut loops {
        let view = &views[*i];
        let total: f64 = own.iter().map(|lp| lp.band).sum();
        let side = if view.below { view.w } else { view.h };
        let scale = 1f64.min((side - 8.0).max(0.0) / total);
        let mut at = (-total * scale) / 2.0;
        for lp in own.iter_mut() {
            lp.band *= scale;
            lp.at = at + lp.band / 2.0;
            at += lp.band;
        }
    }

    let mut edges_out = String::new();
    let mut labels_out = String::new();
    for i in 0..cedges.len() {
        let edge = drawn[i];
        let label = &edge_labels[i];
        let (d, start, end);
        if let Some((k, j)) = loop_of[i] {
            let lp = &loops[k].1[j];
            let ce = &cedges[i];
            let c = &cnodes[ce.src];
            let view = &views[ce.src];
            let spread = (lp.band / 2.0 - 6.0).clamp(1.0, 13.0);
            let far = MARKER_LENGTH + LOOP * 0.75 + 6.0;
            let (label_x, label_y);
            if view.below {
                let x = c.x + lp.at;
                let y = c.y + view.h / 2.0;
                let turn = num(y + MARKER_LENGTH);
                let out = num(y + MARKER_LENGTH + LOOP);
                d = format!(
                    "M{},{}V{turn}C{},{out} {},{out} {},{turn}V{}",
                    num(x - spread),
                    num(y),
                    num(x - spread),
                    num(x + spread),
                    num(x + spread),
                    num(y)
                );
                start = er_marker(edge.arrow_start, x - spread, y, 0.0, -1.0);
                end = er_marker(edge.arrow_end, x + spread, y, 0.0, -1.0);
                label_x = x;
                label_y = y + far + label.height / 2.0;
            } else {
                let x = c.x + view.w / 2.0;
                let y = c.y + lp.at;
                let turn = num(x + MARKER_LENGTH);
                let out = num(x + MARKER_LENGTH + LOOP);
                d = format!(
                    "M{},{}H{turn}C{out},{} {out},{} {turn},{}H{}",
                    num(x),
                    num(y - spread),
                    num(y - spread),
                    num(y + spread),
                    num(y + spread),
                    num(x)
                );
                start = er_marker(edge.arrow_start, x, y - spread, -1.0, 0.0);
                end = er_marker(edge.arrow_end, x, y + spread, -1.0, 0.0);
                label_x = x + far + label.width / 2.0;
                label_y = y;
            }
            let ce = &mut cedges[i];
            ce.label_x = label_x;
            ce.label_y = label_y;
        } else {
            let ce = &cedges[i];
            let mut route = straighten(&ce.route);
            if route.len() < 6 {
                continue;
            }
            inset(&mut route, 0, &views[ce.src], &cnodes[ce.src]);
            let last = route.len() - 3;
            inset(&mut route, last, &views[ce.dst], &cnodes[ce.dst]);
            // A curve stops short of each box so that the marker sits on a straight run of
            // line.
            let dx = (route[last] - route[0]).abs();
            let dy = (route[last + 1] - route[1]).abs();
            let direct =
                route.len() == 6 && (dx < 0.01 || dy < 0.01 || dx.hypot(dy) <= 3.0 * MARKER_LENGTH);
            let stub = if direct { 0.0 } else { MARKER_LENGTH };
            let path = route_path(&route, None, stub, stub, None, None);
            d = if stub > 0.0 {
                format!(
                    "M{},{}L{}L{},{}",
                    num(path.sx),
                    num(path.sy),
                    &path.d[1..],
                    num(path.ex),
                    num(path.ey)
                )
            } else {
                path.d.clone()
            };
            start = er_marker(edge.arrow_start, path.sx, path.sy, path.sdx, path.sdy);
            end = er_marker(edge.arrow_end, path.ex, path.ey, path.edx, path.edy);
        }
        let _ = write!(
            edges_out,
            "<g class=\"pele-rs-edge pele-rs-er-{}\" data-id=\"{}\"><path d=\"{d}\"{}/>{start}{end}</g>",
            if edge.dashed {
                "non-identifying"
            } else {
                "identifying"
            },
            esc_text(&edge.id),
            if edge.dashed {
                " stroke-dasharray=\"4 3\""
            } else {
                ""
            }
        );
        if label.width > 0.0 {
            let ce = &cedges[i];
            labels_out += &edge_label_svg(&edge.id, label, ce.label_x, ce.label_y, "");
        }
    }

    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-rs-title\" font-weight=\"var(--_tw)\"",
    );
    if !clusters.is_empty() {
        let _ = write!(content, "<g class=\"pele-rs-clusters\">{clusters}</g>");
    }
    if !edges_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges_out}</g>"
        );
    }
    if !labels_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"pele-rs-edge-labels\" font-size=\"{}\">{labels_out}</g>",
            num(small)
        );
    }
    let _ = write!(content, "<g class=\"pele-rs-nodes\">{nodes_out}</g>");

    let acc = Accessible {
        title: db.acc_title.clone(),
        descr: db.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            DiagramKind::Er,
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
    }
}

fn straighten(route: &[f64]) -> Vec<f64> {
    let mut out = route[..3.min(route.len())].to_vec();
    let mut k = 3;
    while k < route.len() {
        let prev = out.len() - 3;
        let flat = |axis: usize| {
            (out[prev + axis] - route[k + axis]).abs() < 0.01
                && (route[k + axis] - route[k + 3 + axis]).abs() < 0.01
        };
        if k + 3 < route.len() && (flat(0) || flat(1)) {
            k += 3;
            continue;
        }
        out.extend_from_slice(&route[k..k + 3]);
        k += 3;
    }
    out
}

/// One end of an edge on a side of a box.
struct End {
    edge: usize,
    at: usize,
    toward: f64,
}

/// The layout brings every edge that ends on one side of a box to the middle of that side,
/// where their markers would cover each other. This spaces them along the side, in the order
/// they fan out.
fn spread_ends(cedges: &mut [CEdge], cnodes: &[CNode], views: &[View<'_>]) {
    let mut sides: Vec<(usize, Vec<End>)> = Vec::new();
    let mut side_at: HashMap<usize, usize> = HashMap::new();
    let mut add = |node: usize, edge: usize, route: &[f64], at: usize, next: usize| {
        let c = &cnodes[node];
        let dx = route[at] - c.x;
        let dy = route[at + 1] - c.y;
        let side = if (dy.abs() - c.h / 2.0).abs() < 0.5 {
            if dy < 0.0 { 0 } else { 2 }
        } else if (dx.abs() - c.w / 2.0).abs() < 0.5 {
            if dx < 0.0 { 3 } else { 1 }
        } else {
            return;
        };
        let key = node * 4 + side;
        let end = End {
            edge,
            at,
            toward: route[next + (side & 1)],
        };
        match side_at.get(&key) {
            Some(&k) => sides[k].1.push(end),
            None => {
                side_at.insert(key, sides.len());
                sides.push((key, vec![end]));
            }
        }
    };
    for (j, e) in cedges.iter().enumerate() {
        if e.src == e.dst || e.route.len() < 6 {
            continue;
        }
        let last = e.route.len() - 3;
        add(e.src, j, &e.route, 0, 3);
        add(e.dst, j, &e.route, last, last - 3);
    }
    for (key, ends) in &mut sides {
        if ends.len() < 2 {
            continue;
        }
        let node = *key >> 2;
        let vertical = *key & 1 == 1;
        let length = match (cnodes[node].is_group, vertical) {
            (true, true) => cnodes[node].h,
            (true, false) => cnodes[node].w,
            (false, true) => views[node].h,
            (false, false) => views[node].w,
        };
        let count = ends.len() as f64;
        let step = PORT_GAP
            .min((length - 2.0 * PORT_MARGIN) / (count - 1.0))
            .max(0.0);
        let center = if vertical {
            cnodes[node].y
        } else {
            cnodes[node].x
        };
        ends.sort_by(|a, b| {
            a.toward
                .partial_cmp(&b.toward)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (i, end) in ends.iter().enumerate() {
            cedges[end.edge].route[end.at + usize::from(vertical)] =
                center + (i as f64 - (count - 1.0) / 2.0) * step;
        }
    }
}

/// An entity with a self-relationship is laid out larger than it is drawn; this moves a route
/// end from the side of that larger box onto the entity's own side.
fn inset(route: &mut [f64], at: usize, view: &View<'_>, c: &CNode) {
    if c.is_group {
        return;
    }
    let dx = route[at] - c.x;
    let dy = route[at + 1] - c.y;
    if c.w != view.w && (dx.abs() - c.w / 2.0).abs() < 0.5 {
        route[at] -= sign(dx) * (c.w - view.w) / 2.0;
    }
    if c.h != view.h && (dy.abs() - c.h / 2.0).abs() < 0.5 {
        route[at + 1] -= sign(dy) * (c.h - view.h) / 2.0;
    }
}

/// `Math.sign`.
fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        x
    }
}
