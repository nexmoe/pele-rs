//! The nodes and edges a requirement diagram lays out. Ported from pele's
//! `diagrams/requirement/graph.ts`.

use super::db::{Element, Requirement, RequirementDb};

pub(crate) struct GraphNode<'a> {
    pub id: &'a str,
    pub requirement: Option<&'a Requirement>,
    pub element: Option<&'a Element>,
    pub css_styles: &'a [String],
    pub css_classes: String,
}

pub(crate) struct GraphEdge<'a> {
    pub id: String,
    pub start: Option<&'a str>,
    pub end: Option<&'a str>,
    pub kind: &'static str,
    pub contains: bool,
}

/// Requirements first, then elements, as Mermaid orders them.
pub(crate) fn build_requirement_graph(
    db: &RequirementDb,
) -> (Vec<GraphNode<'_>>, Vec<GraphEdge<'_>>) {
    let mut nodes = Vec::new();
    for r in db.requirements.values() {
        nodes.push(GraphNode {
            id: &r.name,
            requirement: Some(r),
            element: None,
            css_styles: &r.css_styles,
            css_classes: r.classes.join(" "),
        });
    }
    for e in db.elements.values() {
        nodes.push(GraphNode {
            id: &e.name,
            requirement: None,
            element: Some(e),
            css_styles: &e.css_styles,
            css_classes: e.classes.join(" "),
        });
    }
    let has = |id: &str| db.requirements.contains(id) || db.elements.contains(id);
    let edges = db
        .relations
        .iter()
        .enumerate()
        .map(|(index, r)| GraphEdge {
            id: format!("{}-{}-{index}", r.src, r.dst),
            start: has(&r.src).then_some(r.src.as_str()),
            end: has(&r.dst).then_some(r.dst.as_str()),
            kind: r.kind,
            contains: r.kind == "contains",
        })
        .collect();
    (nodes, edges)
}
