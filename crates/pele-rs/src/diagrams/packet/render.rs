//! The drawing of a packet diagram: the rows of fields, the bits they hold, and the title under
//! them. Ported from pele's `diagrams/packet/render.ts`.

use std::collections::HashMap;

use super::db::{PacketBlock, PacketModel, packet_option};
use super::text::{Fitted, fit_text, literal_label};
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{title_room, turn_to_fit};
use crate::preprocess::{Config, Value};
use crate::svg::{Accessible, ROUNDED, esc, label_svg, num, svg_document};
use crate::text::Style;
use crate::text::entities::decode_entities;
use crate::text::label::{Label, layout_label};
use crate::util::round;

/// Draws a packet diagram.
pub(crate) fn render_packet(model: &PacketModel, config: &Config, input: &Input<'_>) -> Output {
    turn_to_fit(input.options, |down| draw(model, config, input, down))
}

/// Whether the bit numbers are drawn.
fn show_bits(config: &Config) -> bool {
    !matches!(
        config.get("packet"),
        Some(Value::Map(map)) if matches!(map.get("showBits"), Some(Value::Bool(false)))
    )
}

/// Whether the rows are drawn from their highest bit down to their lowest.
fn descending(config: &Config) -> bool {
    matches!(
        config.get("packet"),
        Some(Value::Map(map)) if matches!(map.get("bitOrder"), Some(Value::String(order)) if order == "descending")
    )
}

/// Draws the packet, either across the width it needs or down the width the host has.
fn draw(model: &PacketModel, config: &Config, input: &Input<'_>, down: bool) -> Output {
    let options = input.options;
    let size = options.font_size.unwrap_or(16.0);
    let label_size = round(size * 0.875);
    let bit_size = round(size * 0.625);
    let measurer = input.measurer;
    let pad = options.padding.unwrap_or(8.0);

    let row_height = packet_option(config, "rowHeight", 32.0, 1.0, 400.0);
    let mut bit_width = packet_option(config, "bitWidth", 32.0, 1.0, 400.0);
    let padding_x = packet_option(config, "paddingX", 5.0, 0.0, 400.0);
    let padding_y = packet_option(config, "paddingY", 5.0, 0.0, 400.0);
    let show_bits = show_bits(config);
    let descending = descending(config);
    let mut bits_per_row = model.bits_per_row;
    let mut rows = model.rows.clone();
    if down {
        let available = options.max_width.unwrap_or(320.0) - 2.0 * pad;
        let count = (available / bit_width.min(18.0)).floor().max(1.0);
        // A byte is the shortest row. Below that every label is one character to a line, and a
        // long packet is a row to a bit.
        bits_per_row = bits_per_row.min((8.0f64).max(2f64.powf(count.log2().floor())));
        let last = model
            .rows
            .last()
            .and_then(|row| row.last())
            .map_or(0.0, |block| block.end);
        if ((last + 1.0) / bits_per_row).ceil() > 10_000.0 {
            bits_per_row = model.bits_per_row;
        }
        bit_width = bit_width.min((18.0f64).max(available / bits_per_row));
        // The model has cut each field at the rows of its own. Its pieces are joined again first,
        // so that a field is only cut where one of the new rows ends. A piece counts one bit
        // short.
        rows = cut_again(model, bits_per_row);
    }

    let mut labels: Vec<Vec<Label>> = Vec::new();
    if down {
        for row in &rows {
            let mut row_labels = Vec::new();
            for block in row {
                let room = ((block.end - block.start + 1.0) * bit_width
                    - padding_x.min(bit_width - 1.0)
                    - 6.0)
                    .max(1.0);
                row_labels.push(literal_label(
                    decode_entities(&block.label).trim(),
                    measurer,
                    label_size,
                    room,
                ));
            }
            labels.push(row_labels);
        }
    }

    // The bit numbers sit above each row and need room of their own.
    let numbers = if show_bits { bit_size + 4.0 } else { 0.0 };
    let row_heights: Vec<f64> = rows
        .iter()
        .enumerate()
        .map(|(index, _row)| {
            let mut height = row_height;
            if down {
                for label in &labels[index] {
                    height = height.max(label.height + 10.0);
                }
            }
            height
        })
        .collect();
    let mut tops: Vec<f64> = Vec::new();
    let mut rows_height = 0.0;
    for height in &row_heights {
        tops.push(rows_height);
        rows_height += numbers + height + padding_y;
    }
    if !rows.is_empty() {
        rows_height -= padding_y;
    }

    let gap = padding_x.min(bit_width - 1.0);
    let width = if rows.is_empty() {
        0.0
    } else {
        bits_per_row * bit_width - gap
    };
    let title = layout_label(
        model.title.as_deref(),
        false,
        measurer,
        size,
        if down { title_room(options) } else { 4000.0 },
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + if rows_height > 0.0 { 12.0 } else { 0.0 }
    } else {
        0.0
    };

    // The pieces of a field cut across rows share its label, so each width is fitted once.
    let mut fits: HashMap<(String, u64), Option<Fitted>> = HashMap::new();
    let mut fields = String::new();
    let mut bits = String::new();
    let left = pad + (title.width - width).max(0.0) / 2.0;
    for (index, row) in rows.iter().enumerate() {
        let y = pad + tops[index] + numbers;
        let row_height = row_heights[index];
        for (place, block) in row.iter().enumerate() {
            let count = block.end - block.start + 1.0;
            let first = block.start % bits_per_row;
            let x = left
                + (if descending {
                    bits_per_row - first - count
                } else {
                    first
                }) * bit_width;
            let w = count * bit_width - gap;
            fields.push_str(&format!(
                "<g class=\"pele-node\" data-id=\"{}-{}\">",
                num(block.start),
                num(block.end)
            ));
            fields.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\" \
                 stroke=\"var(--_b)\"{ROUNDED}/>",
                num(x),
                num(y),
                num(w),
                num(row_height)
            ));
            if down {
                fields.push_str(&label_svg(
                    &labels[index][place],
                    x + w / 2.0,
                    y + row_height / 2.0,
                    "",
                ));
            } else {
                let key = (block.label.clone(), (w - 6.0).to_bits());
                let text = fits
                    .entry(key)
                    .or_insert_with(|| {
                        fit_text(
                            decode_entities(&block.label).trim(),
                            w - 6.0,
                            label_size,
                            round(size * 0.5),
                            measurer,
                        )
                    })
                    .clone();
                if let Some(text) = text.filter(|text| text.size <= row_height) {
                    let font = if text.size == label_size {
                        String::new()
                    } else {
                        format!(" font-size=\"{}\"", num(text.size))
                    };
                    fields.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\"{font}>{}</text>",
                        num(x + w / 2.0),
                        num(y + row_height / 2.0 + text.size * 0.35),
                        esc(&text.text)
                    ));
                }
            }
            fields.push_str("</g>");
            if !show_bits {
                continue;
            }
            let leading = if descending { block.end } else { block.start };
            let trailing = if descending { block.start } else { block.end };
            if count == 1.0 {
                bits.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
                    num(x + w / 2.0),
                    num(y - 4.0),
                    num(leading)
                ));
            } else {
                bits.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\">{}</text>",
                    num(x),
                    num(y - 4.0),
                    num(leading)
                ));
                bits.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"end\">{}</text>",
                    num(x + w),
                    num(y - 4.0),
                    num(trailing)
                ));
            }
        }
    }

    let total_width = (width.max(title.width) + 2.0 * pad).ceil();
    let total_height = (rows_height + title_height + 2.0 * pad).ceil();
    let content = format!(
        "{}{}{}",
        if fields.is_empty() {
            String::new()
        } else {
            format!(
                "<g class=\"pele-packet-fields\" font-size=\"{}\" text-anchor=\"middle\">{}</g>",
                num(label_size),
                fields
            )
        },
        if bits.is_empty() {
            String::new()
        } else {
            format!(
                "<g class=\"pele-packet-bits\" font-size=\"{}\" fill=\"var(--_m)\">{}</g>",
                num(bit_size),
                bits
            )
        },
        // Mermaid puts a packet's title under it.
        label_svg(
            &title,
            total_width / 2.0,
            total_height - pad - title.height / 2.0,
            " class=\"pele-title\" font-weight=\"var(--_tw)\"",
        )
    );

    let acc = Accessible {
        title: model.acc_title.clone(),
        descr: model.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            crate::DiagramKind::Packet,
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

/// Cuts the fields of the model again, at the ends of the rows of a packet that is drawn down.
/// The pieces of a field the model cut across rows are joined again first, so that a field is
/// only cut where one of the new rows ends.
fn cut_again(model: &PacketModel, bits_per_row: f64) -> Vec<Vec<PacketBlock>> {
    let piece = |block: &PacketBlock| block.bits == block.end - block.start;
    let mut whole: Vec<PacketBlock> = Vec::new();
    for row in &model.rows {
        for block in row {
            let joins = match whole.last() {
                Some(previous) => {
                    piece(previous)
                        && piece(block)
                        && previous.label == block.label
                        && block.start == previous.end + 1.0
                        && block.start % model.bits_per_row == 0.0
                }
                None => false,
            };
            if joins {
                let previous = whole.last_mut().expect("the previous piece is there");
                previous.end = block.end;
                previous.bits = previous.end - previous.start;
            } else {
                whole.push(block.clone());
            }
        }
    }

    let mut rows: Vec<Vec<PacketBlock>> = Vec::new();
    for block in whole {
        let mut start = block.start;
        while start <= block.end {
            let end = block
                .end
                .min((start / bits_per_row).floor() * bits_per_row + bits_per_row - 1.0);
            let index = (start / bits_per_row).floor() as usize;
            while rows.len() <= index {
                rows.push(Vec::new());
            }
            rows[index].push(PacketBlock {
                start,
                end,
                ..block.clone()
            });
            start = end + 1.0;
        }
    }
    rows
}
