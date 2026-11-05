//! Entity relationship diagrams: entities with attribute tables, relationships with crow's foot
//! cardinalities, aliases, subgraphs, `classDef`, `class` and `style`.

mod db;
mod graph;
mod lexer;
mod markers;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct Er;

impl Diagram for Er {
    fn math(&self) -> bool {
        true
    }

    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let mut db = db::ErDb::new();
        if let Some(title) = input.title.filter(|t| !t.is_empty()) {
            db.title = Some(title.to_string());
        }
        parser::parse_er(&input.source, &mut db, input.options.limits.max_depth)?;
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_er(&db, input))
        }))
    }
}
