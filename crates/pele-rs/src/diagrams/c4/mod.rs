//! C4 diagrams: the shapes, the boundaries that hold them, and the relations between them.

mod db;
mod layout;
mod lexer;
mod parser;
mod render;
mod route;
mod tokens;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct C4;

impl Diagram for C4 {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let actions = parser::parse_c4(&input.source)?;
        let mut model = db::build(actions);
        if model.title.is_none() {
            model.title = input.title.map(str::to_string);
        }
        Ok(Box::new(move |input: &Input<'_>| {
            render::render_c4(&model, input)
        }))
    }
}
