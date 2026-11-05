//! Architecture diagrams: groups of services, drawn as boxes with the edges between them.

mod db;
mod icons;
mod layout;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct Architecture;

impl Diagram for Architecture {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let ast = parser::parse_architecture(&input.source)?;
        let model = db::build_architecture(&ast, input.title)?;
        Ok(Box::new(move |input: &Input<'_>| {
            render::render_architecture(&model, input)
        }))
    }
}
