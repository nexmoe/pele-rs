//! Git graphs: the history of a branch, with the commits on it and the branches it grows.

mod db;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct GitGraph;

impl Diagram for GitGraph {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let ast = parser::parse_git(&input.source)?;
        let model = db::build_git(&ast, input.config, input.title)?;
        Ok(Box::new(move |input: &Input<'_>| {
            render::render_git(&model, input)
        }))
    }
}
