//! Mind maps: a tree of ideas, written as an indented outline.

mod db;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::diagrams::common::outline::parse_outline;
use crate::error::Error;

pub(crate) struct Mindmap;

impl Diagram for Mindmap {
    fn math(&self) -> bool {
        true
    }

    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let mut db = db::MindmapDb::default();
        db.title = input.title.map(str::to_string);
        parse_outline(&input.source, &mut db, false)?;
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_mindmap(&db, input))
        }))
    }
}
