//! Wardley maps: a grid of axes with the components of a strategy placed on it.

mod db;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

/// The wardley diagram type.
pub(crate) struct Wardley;

impl Diagram for Wardley {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let ast = parser::parse_wardley(&input.source)?;
        let model = db::build_wardley(&ast, input.title)?;
        Ok(Box::new(move |input: &Input<'_>| {
            render::render_wardley(&model, input)
        }))
    }
}
