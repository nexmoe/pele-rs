//! Use case diagrams: the actors, the use cases, the boundaries and the notes, and the
//! relationships between them.

mod db;
mod graph;
mod json;
mod lexer;
mod parser;
mod render;
mod tokens;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct Usecase;

impl Diagram for Usecase {
    fn math(&self) -> bool {
        true
    }

    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let model =
            parser::parse_usecase(&input.source, input.title, input.options.limits.max_depth)?;
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_usecase(&model, input))
        }))
    }
}
