//! Venn diagrams: sets of labelled circles that overlap, as Mermaid's `venn-beta` reads them.

mod db;
mod layout;
mod lexer;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

/// The venn diagram type.
pub(crate) struct Venn;

impl Diagram for Venn {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let mut model = db::VennDb::default();
        parser::parse_venn(&input.source, &mut model)?;
        let model = model.build(input.title);
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_venn(&model, input))
        }))
    }
}
