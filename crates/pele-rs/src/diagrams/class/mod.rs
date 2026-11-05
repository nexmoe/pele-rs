//! Class diagrams: classes with members, generic types and annotations, relations with
//! cardinalities, namespaces, notes, lollipop interfaces, `classDef`, `cssClass`, `style`,
//! `link`, `click` and `callback`.

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

pub(crate) struct Class;

impl Diagram for Class {
    fn math(&self) -> bool {
        true
    }

    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let mut db = db::ClassDb::new(input.options.limits.max_depth);
        if let Some(title) = input.title.filter(|t| !t.is_empty()) {
            db.title = Some(title.to_string());
        }
        parser::parse_class(&input.source, &mut db)?;
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_class(&db, input))
        }))
    }
}
