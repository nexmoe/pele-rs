//! Packet diagrams: a block of bits, drawn as a row of fields, each with the bits it takes.

mod db;
mod parser;
mod render;
mod text;
mod tokens;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct Packet;

impl Diagram for Packet {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let ast = parser::parse_packet(&input.source)?;
        let model = db::build_packet(&ast, input.config, input.title)?;
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_packet(&model, input.config, input))
        }))
    }
}
