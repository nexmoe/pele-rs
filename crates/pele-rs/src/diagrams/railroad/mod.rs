//! Railroad diagrams: the rules of a grammar, drawn as the track that joins their parts.
//!
//! Mermaid has four grammars for this diagram — its own notation, EBNF, ABNF and PEG — and
//! they all end in the same model, so they all share this drawing.

mod db;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

/// The railroad diagram type.
pub(crate) struct Railroad;

impl Diagram for Railroad {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let model =
            parser::parse_railroad(&input.source, input.options.limits.max_depth, input.title)?;
        Ok(Box::new(move |input: &Input<'_>| {
            render::render_railroad(&model, input)
        }))
    }
}
