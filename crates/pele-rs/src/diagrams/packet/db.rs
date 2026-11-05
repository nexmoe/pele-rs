//! The model of a packet diagram: the rows of fields the packet is drawn in. Ported from
//! pele's `diagrams/packet/model.ts`, which is a port of Mermaid's `diagrams/packet/parser.ts`.
//!
//! Mermaid's own spec for the type, `packet.spec.ts`, expects a field that is split across rows
//! to be drawn on a row of its own, but the code of Mermaid 12.1.0 draws it beside the field
//! before it, in the row both share. The code is what Mermaid draws, so this follows the code.

use crate::diagrams::packet::parser::PacketAst;
use crate::error::{Error, ErrorCode};
use crate::preprocess::{Config, Value};
use crate::util::{js_number, trim_start};

/// The most rows a packet may have.
const MAX_ROWS: usize = 10_000;

/// A field of the packet.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PacketBlock {
    /// The bit the field starts at.
    pub start: f64,
    /// The bit the field ends at.
    pub end: f64,
    /// The number of bits, as Mermaid counts it: one less than the width of a piece of a field
    /// that is split across rows.
    pub bits: f64,
    pub label: String,
}

/// The model of a packet diagram.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PacketModel {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// The number of bits a row holds.
    pub bits_per_row: f64,
    /// The rows of fields the packet is drawn in.
    pub rows: Vec<Vec<PacketBlock>>,
}

/// A number of the config of the packet diagram, as Mermaid reads it: a number that is not
/// smaller than `min`, and not larger than `max`.
pub(crate) fn packet_option(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    match config.get("packet") {
        Some(Value::Map(map)) => match map.get(key) {
            Some(Value::Number(value)) if *value >= min => value.min(max),
            _ => fallback,
        },
        _ => fallback,
    }
}

/// A failure of the packet, as Mermaid reports it.
fn fail(message: String) -> Error {
    Error::new(ErrorCode::Semantic, message)
}

/// Builds the model of a packet diagram out of what the parser read.
pub(crate) fn build_packet(
    ast: &PacketAst,
    config: &Config,
    title: Option<&str>,
) -> Result<PacketModel, Error> {
    let bits_per_row = packet_option(config, "bitsPerRow", 32.0, 1.0, 4096.0).floor();
    let mut rows: Vec<Vec<PacketBlock>> = Vec::new();
    let mut last_bit = -1.0;
    let mut word: Vec<PacketBlock> = Vec::new();
    let mut row = 1.0;

    for block in &ast.blocks {
        let mut start = block.start;
        let mut end = block.end;
        let mut bits = block.bits;
        if let (Some(start), Some(end)) = (start, end) {
            if end < start {
                return Err(fail(format!(
                    "Packet block {} - {} is invalid. End must be greater than start.",
                    js_number(start),
                    js_number(end)
                )));
            }
        }
        if start.is_none() {
            start = Some(last_bit + 1.0);
        }
        let mut start = start.expect("the start is filled in above");
        if start != last_bit + 1.0 {
            return Err(fail(format!(
                "Packet block {} - {} is not contiguous. It should start from {}.",
                js_number(start),
                js_number(end.unwrap_or(start)),
                js_number(last_bit + 1.0)
            )));
        }
        if bits == Some(0.0) {
            return Err(fail(format!(
                "Packet block {} is invalid. Cannot have a zero bit field.",
                js_number(start)
            )));
        }
        if end.is_none() {
            end = Some(start + bits.unwrap_or(1.0) - 1.0);
        }
        let end = end.expect("the end is filled in above");
        if bits.is_none() {
            bits = Some(end - start + 1.0);
        }
        let mut bits = bits.expect("the bits are filled in above");
        last_bit = end;

        // The field is cut at the end of every row it reaches over.
        while (word.len() as f64) <= bits_per_row + 1.0 && rows.len() < MAX_ROWS {
            let row_end = row * bits_per_row - 1.0;
            if end <= row_end {
                word.push(PacketBlock {
                    start,
                    end,
                    bits,
                    label: block.label.clone(),
                });
                if end == row_end {
                    rows.push(std::mem::take(&mut word));
                    row += 1.0;
                }
                break;
            }
            word.push(PacketBlock {
                start,
                end: row_end,
                bits: row_end - start,
                label: block.label.clone(),
            });
            rows.push(std::mem::take(&mut word));
            row += 1.0;
            start = row_end + 1.0;
            bits = end - start;
        }
    }
    if !word.is_empty() {
        rows.push(word);
    }

    Ok(PacketModel {
        title: ast
            .title
            .as_deref()
            .filter(|title| !title.is_empty())
            .map(str::to_string)
            .or_else(|| title.map(str::to_string)),
        acc_title: ast
            .acc_title
            .as_deref()
            .filter(|acc_title| !acc_title.is_empty())
            .map(|acc_title| trim_start(acc_title).to_string()),
        acc_descr: ast
            .acc_descr
            .as_deref()
            .filter(|acc_descr| !acc_descr.is_empty())
            .map(join_lines),
        bits_per_row,
        rows,
    })
}

/// `text.replace(/\n\s+/g, '\n')`: a line break and the whitespace after it become one line
/// break.
fn join_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c != '\n' {
            continue;
        }
        while chars
            .peek()
            .is_some_and(|&next| crate::util::is_space(next))
        {
            chars.next();
        }
    }
    out
}
