//! State diagrams: states with descriptions, composite states with their own direction,
//! concurrency regions, start and end, choice, fork and join, notes, `classDef`, `class`,
//! `style` and `click`.

mod db;
mod graph;
mod lexer;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;

pub(crate) struct State;

impl Diagram for State {
    fn math(&self) -> bool {
        true
    }

    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let mut db = db::StateDb::default();
        if let Some(title) = input.title.filter(|t| !t.is_empty()) {
            db.title = Some(title.to_string());
        }
        parser::parse_state(&input.source, &mut db, input.options.limits.max_depth)?;
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_state(&db, input))
        }))
    }
}
