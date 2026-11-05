//! Ishikawa diagrams: the causes of an effect, drawn as a fishbone.

mod db;
mod lexer;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

/// The ishikawa diagram type.
pub(crate) struct Ishikawa;

impl Diagram for Ishikawa {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let ast = parser::parse_ishikawa(&input.source)?;
        let model = db::build_ishikawa(
            &ast,
            input.title.map(str::to_string),
            input.options.limits.max_depth,
        )?;
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_ishikawa(&model, input))
        }))
    }
}
