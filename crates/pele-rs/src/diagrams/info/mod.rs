//! Information diagrams: the version of the library, under the title of the diagram.

mod db;
mod parser;
mod render;
mod tokens;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

/// The info diagram type.
pub(crate) struct Info;

impl Diagram for Info {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let ast = parser::parse_info(&input.source)?;
        let model = db::build_info(&ast, input.title);
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_info(&model, input))
        }))
    }
}
