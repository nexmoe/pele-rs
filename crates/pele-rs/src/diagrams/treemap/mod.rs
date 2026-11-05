//! Treemaps: a tree of values, drawn as nested boxes whose areas stand for the values, as
//! Mermaid's `treemap` reads it.

mod db;
mod format;
mod layout;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct Treemap;

impl Diagram for Treemap {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let ast = parser::parse_treemap(&input.source)?;
        let mut db = db::build_db(&ast);
        // A title in the diagram is the one of the front matter only when it has none of its own.
        if db.title.is_none() {
            db.title = input.title.map(str::to_string);
        }
        Ok(Box::new(move |input: &Input<'_>| {
            render::render_treemap(&db, input)
        }))
    }
}
