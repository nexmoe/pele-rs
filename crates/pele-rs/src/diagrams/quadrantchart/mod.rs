//! Quadrant charts: four boxes with the axes around them, as Mermaid's `quadrantChart` reads
//! it.

mod db;
mod lexer;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct QuadrantChart;

impl Diagram for QuadrantChart {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let mut db = db::QuadrantDb {
            title: input.title.map(str::to_string),
            ..Default::default()
        };
        parser::parse_quadrant(&input.source, &mut db)?;
        Ok(Box::new(move |input: &Input<'_>| {
            render::render_quadrant(&db, input)
        }))
    }
}
