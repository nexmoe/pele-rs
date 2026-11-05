//! Flowcharts: `graph` / `flowchart`, nodes in every shape, edges, subgraphs, classes, styles
//! and click links.

#[cfg(test)]
mod compat;
// The swimlane diagram is a flowchart whose subgraphs are lanes, so it reads the flowchart's
// own model and parser rather than a second copy of them.
pub(crate) mod db;
pub(crate) mod graph;
mod lexer;
pub(crate) mod parser;
mod render;
mod shapes;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::Error;
use crate::preprocess::Value;

pub(crate) struct Flowchart;

impl Diagram for Flowchart {
    fn math(&self) -> bool {
        true
    }

    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let inherit_dir = match input.config.get("flowchart") {
            Some(Value::Map(flow)) => matches!(flow.get("inheritDir"), Some(Value::Bool(true))),
            _ => false,
        };
        let limits = &input.options.limits;
        let mut db = db::FlowDb::new(limits.max_edges, inherit_dir);
        if let Some(title) = input.title.filter(|t| !t.is_empty()) {
            db.title = Some(title.to_string());
        }
        parser::parse_flowchart(&input.source, &mut db, limits.max_depth)?;
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_flowchart(&db, input))
        }))
    }
}
