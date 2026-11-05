//! Sankey charts: bands of flow between the nodes of a chart, written as CSV.

mod db;
mod layout;
mod lexer;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct Sankey;

impl Diagram for Sankey {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let mut db = db::SankeyDb::default();
        db.title = input.title.map(str::to_string);
        parser::parse_sankey(&parser::prepare_text_for_parsing(&input.source), &mut db)?;
        Ok(Box::new(move |input: &Input<'_>| {
            render::render_sankey(&db, input)
        }))
    }
}
