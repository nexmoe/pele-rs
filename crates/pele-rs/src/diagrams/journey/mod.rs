//! User journeys: tasks scored from 1 to 5, the people taking part, and sections.

mod db;
mod lexer;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct Journey;

impl Diagram for Journey {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let mut db = db::JourneyDb::default();
        db.title = input.title.map(str::to_string);
        parser::parse_journey(&input.source, &mut db)?;
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_journey(&db, input))
        }))
    }
}
