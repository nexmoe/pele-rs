//! Swimlane diagrams: a flowchart whose top-level subgraphs are the lanes.
//!
//! Mermaid draws the swimlane with the flowchart's own parser and model, so this reads them
//! too: the syntax is the flowchart's, and only the layout and the drawing of the lanes are of
//! its own.

mod lanes;
mod render;
mod shapes;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::diagrams::flowchart::db::FlowDb;
use crate::diagrams::flowchart::parser::parse_flowchart;
use crate::error::Error;
use crate::preprocess::Value;

pub(crate) struct Swimlane;

impl Diagram for Swimlane {
    fn math(&self) -> bool {
        true
    }

    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let inherit_dir = match input.config.get("flowchart") {
            Some(Value::Map(flow)) => matches!(flow.get("inheritDir"), Some(Value::Bool(true))),
            _ => false,
        };
        let limits = &input.options.limits;
        let mut db = FlowDb::new(limits.max_edges, inherit_dir);
        if let Some(title) = input.title.filter(|t| !t.is_empty()) {
            db.title = Some(title.to_string());
        }
        parse_flowchart(&input.source, &mut db, limits.max_depth)?;
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_swimlane(&db, input))
        }))
    }
}
