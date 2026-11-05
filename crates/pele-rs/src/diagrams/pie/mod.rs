//! Pie charts: `pie`, an optional `showData`, a title and accessibility headers, then one
//! `"label" : value` line per slice.

#[cfg(test)]
mod compat;
mod model;
mod parser;
mod render;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct Pie;

impl Diagram for Pie {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let model = model::build_pie(parser::parse_pie(&input.source)?, input.title)?;
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_pie(&model, input))
        }))
    }
}
