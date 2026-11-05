//! The tokens and the parser of a packet diagram, over the grammar Mermaid gives with Langium.
//! Ported from pele's `diagrams/packet/parser.ts`, with Mermaid's own grammar as the rule:
//! `packages/parser/src/language/packet/packet.langium` of Mermaid 12.1.0.

use crate::diagrams::packet::tokens::{
    END, Kind, Reader, acc_descr_value, acc_title_value, string_value, title_value, tokenize,
};
use crate::error::Error;
use crate::util::js_to_number;

/// A field of the packet, as the grammar reads it. A field names the bit it starts at, the bit
/// it ends at, or the number of bits it takes; what it does not name is filled in later.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PacketBlockAst {
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub bits: Option<f64>,
    pub label: String,
}

/// What the parser reads out of the text.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PacketAst {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub blocks: Vec<PacketBlockAst>,
}

/// The end of a line: a line break, or the end of the text.
fn end_of_line(r: &mut Reader<'_>) -> Result<(), Error> {
    if r.kind() == END {
        return Ok(());
    }
    if r.kind() != Kind::Newline {
        return Err(r.fail("a line break or the end of input"));
    }
    while r.kind() == Kind::Newline {
        r.i += 1;
    }
    Ok(())
}

/// Reads a packet diagram out of the text.
pub(crate) fn parse_packet(src: &str) -> Result<PacketAst, Error> {
    let tokens = tokenize(src)?;
    let mut r = Reader::new(tokens);
    let mut ast = PacketAst::default();

    while r.kind() == Kind::Newline {
        r.i += 1;
    }
    if !r.accept(Kind::PacketBeta) {
        r.expect(Kind::Packet)?;
    }

    while r.kind() != END {
        match r.kind() {
            Kind::Newline => r.i += 1,
            Kind::AccDescr => {
                ast.acc_descr = Some(acc_descr_value(r.take()));
                end_of_line(&mut r)?;
            }
            Kind::AccTitle => {
                ast.acc_title = Some(acc_title_value(r.take()));
                end_of_line(&mut r)?;
            }
            Kind::Title => {
                ast.title = Some(title_value(r.take()));
                end_of_line(&mut r)?;
            }
            Kind::Int | Kind::Plus => {
                let mut block = PacketBlockAst::default();
                if r.accept(Kind::Plus) {
                    block.bits = Some(js_to_number(r.expect(Kind::Int)?));
                } else {
                    block.start = Some(js_to_number(r.take()));
                    if r.accept(Kind::Dash) {
                        block.end = Some(js_to_number(r.expect(Kind::Int)?));
                    }
                }
                r.expect(Kind::Colon)?;
                block.label = string_value(r.expect(Kind::Str)?);
                ast.blocks.push(block);
                end_of_line(&mut r)?;
            }
            _ => return Err(r.fail("a field, a title, or a line break")),
        }
    }
    Ok(ast)
}
