//! Radar charts: a set of axes around a centre, with the values of every curve drawn as a
//! polygon, as Mermaid's `radar-beta` reads them.

mod db;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct Radar;

impl Diagram for Radar {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let ast = parser::parse_radar(&input.source)?;
        let model = db::build_radar(&ast, input.title)?;
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_radar(&model, input))
        }))
    }
}
