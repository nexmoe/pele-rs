//! Sequence diagrams: participants and actors, messages of every arrow kind, activations,
//! notes, `loop` / `alt` / `opt` / `par` / `critical` / `break` / `rect` blocks, boxes,
//! `autonumber`, and participants that are created and destroyed.

mod actors;
mod colors;
mod db;
mod lexer;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;
use crate::preprocess::Value;

pub(crate) struct Sequence;

impl Diagram for Sequence {
    fn math(&self) -> bool {
        true
    }

    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let wrap = match input.config.get("wrap") {
            Some(Value::Bool(b)) => Some(*b),
            _ => None,
        };
        let sequence_wrap = match input.config.get("sequence") {
            Some(Value::Map(seq)) => matches!(seq.get("wrap"), Some(Value::Bool(true))),
            _ => false,
        };
        let mut db = db::SeqDb::new(wrap, sequence_wrap);
        if let Some(title) = input.title.filter(|t| !t.is_empty()) {
            db.title = Some(title.to_string());
        }
        parser::parse_sequence(&input.source, &mut db, input.options.limits.max_depth)?;
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_sequence(&db, input))
        }))
    }
}
