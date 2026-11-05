//! Timelines: periods with their events, grouped into sections, running across or down.

mod db;
mod lexer;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct Timeline;

impl Diagram for Timeline {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let mut db = db::TimelineDb::new();
        db.title = input.title.map(str::to_string);
        parser::parse_timeline(&input.source, &mut db)?;
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_timeline(&db, input))
        }))
    }
}
