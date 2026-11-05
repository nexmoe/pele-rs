//! Block diagrams: blocks in the grid of a composite, and the links between them.

mod arrows;
mod db;
mod layout;
mod lexer;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct Block;

impl Diagram for Block {
    fn math(&self) -> bool {
        true
    }

    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let parsed = parser::parse_block(&input.source, input.options.limits.max_depth)?;
        let model = db::build_block(&parsed, input.title)?;
        Ok(Box::new(move |input: &Input<'_>| {
            render::render_block(&model, input)
        }))
    }
}
