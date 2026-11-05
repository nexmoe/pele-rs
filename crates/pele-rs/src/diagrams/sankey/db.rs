//! The model of a sankey chart. Ported from pele's `diagrams/sankey/db.ts`.

use crate::diagrams::common::ordered::Ordered;

/// A node of the chart: a name. Its place among the chart's nodes is where it stands in
/// [`SankeyDb::nodes`].
#[derive(Debug, Clone)]
pub(crate) struct SankeyNode {
    pub id: String,
}

/// A band of flow from one node to another.
#[derive(Debug, Clone)]
pub(crate) struct SankeyLink {
    /// The index of the node the band leaves.
    pub source: usize,
    /// The index of the node the band enters.
    pub target: usize,
    pub value: f64,
}

#[derive(Debug, Default)]
pub(crate) struct SankeyDb {
    /// The nodes, in the order they were first named.
    pub nodes: Vec<SankeyNode>,
    pub links: Vec<SankeyLink>,
    pub title: Option<String>,
    /// The index of each node, by name.
    lookup: Ordered<usize>,
}

impl SankeyDb {
    /// The node with that name, or a new one.
    pub(crate) fn find_or_create_node(&mut self, id: &str) -> usize {
        if let Some(&index) = self.lookup.get(id) {
            return index;
        }
        let index = self.nodes.len();
        self.nodes.push(SankeyNode { id: id.to_string() });
        self.lookup.set(id, index);
        index
    }

    pub(crate) fn add_link(&mut self, source: usize, target: usize, value: f64) {
        self.links.push(SankeyLink {
            source,
            target,
            value,
        });
    }
}
