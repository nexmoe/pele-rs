//! Event modeling diagrams: the timeline of a system, with the frames that trigger it, the
//! commands and queries they carry, and the events they cause.

mod db;
mod lexer;
mod parser;
mod render;

#[cfg(test)]
mod compat;
#[cfg(test)]
mod summary;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

/// The event modeling diagram type.
pub(crate) struct EventModeling;

impl Diagram for EventModeling {
    /// Mermaid's grammar has a terminal for the comments, so the parser is given the text with
    /// them, and the positions it reports are the ones of the source.
    fn keep_comments(&self) -> bool {
        true
    }

    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let ast = parser::parse_event_modeling(&input.source)?;
        let model = db::build_event_model(&ast, input.title, &input.source)?;
        Ok(Box::new(move |input: &Input<'_>| {
            render::render_event_model(&model, input)
        }))
    }
}
