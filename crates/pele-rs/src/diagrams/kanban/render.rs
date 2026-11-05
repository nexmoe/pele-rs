//! Kanban board layout and SVG. Ported from pele's `diagrams/kanban/render.ts`.

use std::fmt::Write;

use super::db::KanbanDb;
use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::title_room;
use crate::options::Link;
use crate::preprocess::Value;
use crate::svg::{
    Accessible, CRITICAL, ROUNDED, class_names, esc, esc_text, icon_svg, label_svg, num,
    svg_document, text_style, title_label,
};
use crate::text::label::{Label, Span, label_line, layout_label, line_middle, with_heights};
use crate::text::{Style, TextMeasurer, icon_markup};
use crate::url::{link_url, rel_attr, safe_url};
use crate::util::{js_number, round, trim};

const COLUMN_GAP: f64 = 12.0;
const COLUMN_PAD: f64 = 8.0;
const CARD_PAD: f64 = 10.0;
const CARD_GAP: f64 = 8.0;
const META_GAP: f64 = 4.0;
const ICON_GAP: f64 = 6.0;

/// Mermaid names five priorities and gives Medium no color. Matching here ignores case. The
/// high ones are marked in the critical color and the low ones in the muted one, the nearer to
/// Medium the fainter. They are not series colors, as a theme may put those in any order.
fn priority_color(priority: &str) -> Option<String> {
    Some(match priority {
        "very high" => format!("stroke=\"{CRITICAL}\""),
        "high" => format!("stroke=\"{CRITICAL}\" opacity=\"0.5\""),
        "low" => "stroke=\"var(--_m)\" opacity=\"0.5\"".to_string(),
        "very low" => "stroke=\"var(--_m)\"".to_string(),
        _ => return None,
    })
}

/// Lays out text for a fixed width. A word too long for a line is broken between characters,
/// because a card cannot grow sideways to hold it.
fn fit_label(
    raw: Option<&str>,
    markdown: bool,
    measurer: &dyn TextMeasurer,
    size: f64,
    max: f64,
    base: Style,
) -> Label {
    let label = layout_label(raw, markdown, measurer, size, max, base);
    if label.width <= max {
        return label;
    }
    let mut lines: Vec<Vec<Span>> = Vec::new();
    let mut widths = Vec::new();
    let mut widest: f64 = 0.0;
    let mut line: Vec<Span> = Vec::new();
    let mut width = 0.0;
    let mut commit = |line: &mut Vec<Span>, width: &mut f64| {
        lines.push(std::mem::take(line));
        widths.push(*width);
        widest = widest.max(*width);
        *width = 0.0;
    };
    for (i, spans) in label.lines.iter().enumerate() {
        if label.widths[i] <= max {
            line = spans.clone();
            width = label.widths[i];
            commit(&mut line, &mut width);
            continue;
        }
        for span in spans {
            // An icon or math is not broken between characters.
            if !span.is_plain() {
                if width > 0.0 && width + span.width > max {
                    commit(&mut line, &mut width);
                }
                line.push(span.clone());
                width += span.width;
                continue;
            }
            let mut text = String::new();
            let mut from = width;
            for ch in span.text.chars() {
                let w = measurer.width(ch.encode_utf8(&mut [0; 4]), size, span.style);
                if width > 0.0 && width + w > max {
                    if !text.is_empty() {
                        line.push(Span::text(
                            std::mem::take(&mut text),
                            span.style,
                            width - from,
                        ));
                    }
                    commit(&mut line, &mut width);
                    from = 0.0;
                }
                text.push(ch);
                width += w;
            }
            if !text.is_empty() {
                line.push(Span::text(text, span.style, width - from));
            }
        }
        commit(&mut line, &mut width);
    }
    let height = lines.len() as f64 * label.line_height;
    let fitted = Label {
        lines,
        widths,
        width: widest,
        height,
        ..label
    };
    if fitted.heights.is_some() {
        with_heights(fitted)
    } else {
        fitted
    }
}

/// Writes a label with every line starting at x, or ending there, and the first line's box
/// starting at y.
fn text_block(label: &Label, x: f64, y: f64, attrs: &str, end: bool) -> String {
    let mut out = String::new();
    for (i, line) in label.lines.iter().enumerate() {
        let cy = y + line_middle(label, i);
        match line.as_slice() {
            [only]
                if only.is_plain()
                    && !only.style.has(Style::SUB | Style::SUP | Style::UNDERLINE) =>
            {
                let _ = write!(
                    out,
                    "<text{} x=\"{}\" y=\"{}\"{}>{}</text>",
                    text_style(attrs, only.style),
                    num(x),
                    num(cy + label.size * 0.35),
                    if end { " text-anchor=\"end\"" } else { "" },
                    esc(&only.text)
                );
            }
            _ => {
                let width = label.widths[i];
                let cx = x + if end { -width } else { width } / 2.0;
                out += &label_svg(&label_line(label, i), cx, cy, attrs);
            }
        }
    }
    out
}

struct Card {
    label: Label,
    ticket: Label,
    assigned: Label,
    /// Height of the rows under the text that hold the ticket and the assignee.
    meta: f64,
    height: f64,
}

pub(crate) fn render_kanban(db: &KanbanDb, input: &Input<'_>) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let small = round(size * 0.875);
    let line = round(size * 1.5);
    let small_line = round(small * 1.5);
    let pad = options.padding.unwrap_or(8.0);
    let conf = match input.config.get("kanban") {
        Some(Value::Map(m)) => Some(m),
        _ => None,
    };
    let section_width = match conf.and_then(|c| c.get("sectionWidth")) {
        Some(Value::Number(n)) if *n > 0.0 => n.clamp(80.0, 2000.0),
        _ => 200.0,
    };
    let glyph = small * 1.25;
    let sections = &db.sections;
    // The icon of a node, when the host has it. One it has not takes no room.
    let icon_of = |name: Option<&str>| name.and_then(|name| icon_markup(measurer, name));
    // Math can't break across lines, so columns widen to hold the widest expression.
    let mut math_width = 0.0f64;
    let mut holds = |raw: &str, font_size: f64, reserved: f64| {
        if !raw.contains("$$") {
            return;
        }
        let label = layout_label(
            Some(raw),
            true,
            measurer,
            font_size,
            f64::INFINITY,
            Style::NONE,
        );
        for span in label.lines.iter().flatten() {
            if let Some(math) = &span.math {
                math_width = math_width.max(math.width() + reserved);
            }
        }
    };
    for section in sections {
        let count = measurer.width(&section.items.len().to_string(), small, Style::NONE);
        let icon = if icon_of(section.node.icon.as_deref()).is_some() {
            glyph + ICON_GAP
        } else {
            0.0
        };
        holds(
            &section.node.label,
            size,
            2.0 * COLUMN_PAD + 8.0 + count + 12.0 + icon,
        );
        for item in &section.items {
            let reserved = 2.0 * (COLUMN_PAD + CARD_PAD);
            let icon = if icon_of(item.icon.as_deref()).is_some() {
                glyph + ICON_GAP
            } else {
                0.0
            };
            holds(&item.label, small, reserved + icon);
            holds(item.ticket.as_deref().unwrap_or(""), small, reserved);
            holds(item.assigned.as_deref().unwrap_or(""), small, reserved);
        }
    }
    let own_width = section_width.max(math_width.ceil());
    // A board wider than the width the host has is folded into rows of columns, which then
    // share the row between them.
    let room = match options.max_width {
        Some(m) if m > 0.0 => m - 2.0 * pad,
        _ => f64::INFINITY,
    };
    let count = sections.len();
    let fits = count as f64 * (own_width + COLUMN_GAP) - COLUMN_GAP <= room;
    let per_row = if fits {
        count.max(1)
    } else {
        ((room + COLUMN_GAP) / (own_width + COLUMN_GAP))
            .floor()
            .max(1.0) as usize
    };
    let column_width = if fits {
        own_width
    } else {
        own_width
            .min(room)
            .max((((room + COLUMN_GAP) / per_row as f64).floor() - COLUMN_GAP).min(own_width * 2.0))
            .max(math_width.ceil())
    };
    let base_url = match conf.and_then(|c| c.get("ticketBaseUrl")) {
        Some(Value::String(s)) => s.as_str(),
        _ => "",
    };
    let card_width = column_width - 2.0 * COLUMN_PAD;
    let text_width = card_width - 2.0 * CARD_PAD;
    let mut links = Vec::new();

    let mut titles = Vec::new();
    let mut cards: Vec<Vec<Card>> = Vec::new();
    let mut head_height = line;
    let mut body_heights = Vec::new();
    for section in sections {
        let count_width = measurer.width(&section.items.len().to_string(), small, Style::NONE);
        let section_icon = icon_of(section.node.icon.as_deref()).is_some();
        let reserved = count_width + 12.0 + if section_icon { glyph + ICON_GAP } else { 0.0 };
        let title = fit_label(
            Some(&section.node.label),
            true,
            measurer,
            size,
            card_width - 8.0 - reserved,
            Style::BOLD,
        );
        head_height = head_height.max(title.height);
        titles.push(title);

        let mut list = Vec::new();
        let mut height = 0.0;
        for item in &section.items {
            let fit = |raw: Option<&str>, markdown, width| {
                fit_label(raw, markdown, measurer, small, width, Style::NONE)
            };
            let icon = if icon_of(item.icon.as_deref()).is_some() {
                glyph + ICON_GAP
            } else {
                0.0
            };
            let label = fit(Some(&item.label), true, text_width - icon);
            let ticket = fit(item.ticket.as_deref(), false, text_width);
            let assigned = fit(item.assigned.as_deref(), false, text_width);
            let meta = if ticket.width + assigned.width + 8.0 > text_width {
                ticket.height + assigned.height
            } else {
                ticket.height.max(assigned.height)
            };
            let card_height = 2.0 * CARD_PAD
                + label.height.max(small_line)
                + if meta != 0.0 { META_GAP + meta } else { 0.0 };
            list.push(Card {
                label,
                ticket,
                assigned,
                meta,
                height: card_height,
            });
            height += card_height + CARD_GAP;
        }
        cards.push(list);
        body_heights.push(height);
    }
    head_height += 2.0 * COLUMN_PAD;
    let mut row_heights = Vec::new();
    let mut row_tops = Vec::new();
    let mut y = 0.0;
    for row in body_heights.chunks(per_row) {
        let tallest = row.iter().fold(CARD_GAP, |a, &b| a.max(b));
        row_tops.push(y);
        row_heights.push(head_height + tallest);
        y += head_height + tallest + COLUMN_GAP;
    }
    let board_height = match (row_tops.last(), row_heights.last()) {
        (Some(top), Some(height)) => top + height,
        _ => 0.0,
    };

    let title = title_label(db.title.as_deref(), measurer, size, title_room(options));
    let title_height = if title.height > 0.0 {
        title.height + 16.0
    } else {
        0.0
    };
    let board_width =
        (count.min(per_row) as f64 * (column_width + COLUMN_GAP) - COLUMN_GAP).max(0.0);
    let width = board_width.max(title.width);
    let left = pad + (width - board_width) / 2.0;
    let edge = column_width - COLUMN_PAD - 4.0;

    let mut out = String::new();
    for (index, section) in sections.iter().enumerate() {
        let row = index / per_row;
        let mut body = format!(
            "<rect width=\"{}\" height=\"{}\" fill=\"var(--_s)\"{ROUNDED}/>",
            js_number(column_width),
            num(row_heights[row])
        );
        body += &text_block(
            &titles[index],
            COLUMN_PAD + 4.0,
            COLUMN_PAD,
            " class=\"pele-rs-cluster-label\" font-weight=\"var(--_hw)\"",
            false,
        );
        let _ = write!(
            body,
            "<text class=\"pele-rs-count\" x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"{}\" fill=\"var(--_m)\">{}</text>",
            js_number(edge),
            num(COLUMN_PAD + line / 2.0 + small * 0.35),
            js_number(small),
            section.items.len()
        );
        if let (Some(name), Some(markup)) = (
            section.node.icon.as_deref(),
            icon_of(section.node.icon.as_deref()),
        ) {
            let count_width = measurer.width(&section.items.len().to_string(), small, Style::NONE);
            body += &icon_svg(
                name,
                edge - count_width - ICON_GAP - glyph,
                COLUMN_PAD + (line - glyph) / 2.0,
                glyph,
                &markup,
            );
        }

        let mut y = head_height;
        for (item, card) in section.items.iter().zip(&cards[index]) {
            let priority = trim(item.priority.as_deref().unwrap_or("")).to_lowercase();
            let color = priority_color(&priority);
            let mut inner = format!(
                "<rect width=\"{}\" height=\"{}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{ROUNDED}/>",
                js_number(card_width),
                num(card.height)
            );
            if let Some(color) = &color {
                let _ = write!(
                    inner,
                    "<path class=\"pele-rs-priority\" d=\"M2.5,5V{}\" fill=\"none\" {color} stroke-width=\"3\" stroke-linecap=\"round\"/>",
                    num(card.height - 5.0)
                );
            }
            inner += &text_block(
                &card.label,
                CARD_PAD,
                CARD_PAD,
                " class=\"pele-rs-label\"",
                false,
            );
            if let (Some(name), Some(markup)) =
                (item.icon.as_deref(), icon_of(item.icon.as_deref()))
            {
                inner += &icon_svg(
                    name,
                    card_width - CARD_PAD - glyph,
                    CARD_PAD + (small_line - glyph) / 2.0,
                    glyph,
                    &markup,
                );
            }
            let bottom = card.height - CARD_PAD;
            if let Some(ticket) = item.ticket.as_deref() {
                let top = bottom - card.meta;
                let href = if base_url.is_empty() {
                    None
                } else {
                    link_url(
                        &safe_url(&base_url.replacen("#TICKET#", ticket, 1)),
                        options,
                    )
                };
                match href {
                    None => {
                        inner += &text_block(
                            &card.ticket,
                            CARD_PAD,
                            top,
                            " class=\"pele-rs-ticket\" fill=\"var(--_m)\"",
                            false,
                        );
                    }
                    Some(href) => {
                        let _ = write!(
                            inner,
                            "<a href=\"{}\" target=\"_blank\"{}>{}</a>",
                            esc(&href),
                            rel_attr(options),
                            text_block(
                                &card.ticket,
                                CARD_PAD,
                                top,
                                " class=\"pele-rs-ticket\" fill=\"var(--_c)\"",
                                false
                            )
                        );
                        links.push(Link {
                            id: item.id.clone(),
                            href,
                            internal: false,
                        });
                    }
                }
            }
            if item.assigned.is_some() {
                inner += &text_block(
                    &card.assigned,
                    card_width - CARD_PAD,
                    bottom - card.assigned.height,
                    " class=\"pele-rs-assigned\" fill=\"var(--_m)\"",
                    true,
                );
            }
            let marked = if color.is_some() {
                format!(" pele-rs-priority-{}", priority.replacen(' ', "-", 1))
            } else {
                String::new()
            };
            let _ = write!(
                body,
                "<g class=\"pele-rs-node pele-rs-card{marked}{}\" data-id=\"{}\" transform=\"translate({},{})\" font-size=\"{}\">{inner}</g>",
                class_names(item.css_classes.as_deref().unwrap_or("")),
                esc_text(&item.id),
                js_number(COLUMN_PAD),
                num(y),
                js_number(small)
            );
            y += card.height + CARD_GAP;
        }

        let _ = write!(
            out,
            "<g class=\"pele-rs-cluster pele-rs-column{}\" data-id=\"{}\" transform=\"translate({},{})\">{body}</g>",
            class_names(section.node.css_classes.as_deref().unwrap_or("")),
            esc_text(&section.node.id),
            num(left + (index % per_row) as f64 * (column_width + COLUMN_GAP)),
            num(pad + title_height + row_tops[row])
        );
    }

    let total_width = (width + 2.0 * pad).ceil();
    let total_height = (board_height + title_height + 2.0 * pad).ceil();
    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-rs-title\" font-weight=\"var(--_tw)\"",
    ) + &out;
    Output {
        svg: svg_document(
            DiagramKind::Kanban,
            total_width,
            total_height,
            size,
            options,
            &Accessible::default(),
            &content,
        ),
        width: total_width,
        height: total_height,
        links,
    }
}
