//! Cynefin diagrams: the five domains of the Cynefin framework, the items in them, and the
//! transitions between them.

mod boundaries;
mod db;
mod lexer;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

/// The cynefin diagram type.
pub(crate) struct Cynefin;

impl Diagram for Cynefin {
    /// Mermaid's grammar has a terminal for the comments, so the parser is given the text with
    /// them, and the positions it reports are the ones of the source.
    fn keep_comments(&self) -> bool {
        true
    }

    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let parsed = parser::parse_cynefin(&input.source)?;
        let model = db::build_cynefin(&parsed, input.title);
        Ok(Box::new(move |input: &Input<'_>| {
            render::render_cynefin(&model, input)
        }))
    }
}
