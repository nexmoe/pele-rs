//! The model of a mind map. Ported from pele's `diagrams/mindmap/db.ts`.

use crate::diagrams::common::outline::OutlineDb;
use crate::error::{Error, ErrorCode};

/// Mermaid's branch colors wrap after this many first-level branches.
const SECTIONS: usize = 11;

#[derive(Debug, Clone)]
pub(crate) struct Node {
    pub node_id: String,
    /// Indentation, less the root's.
    pub level: isize,
    pub descr: String,
    pub kind: u8,
    /// Indices into the model's nodes.
    pub children: Vec<usize>,
    /// Index of the first-level branch the node belongs to; none for the root.
    pub section: Option<usize>,
    /// The color of the node when `mindmap.colorBy` is `depth`: its distance from the root, less
    /// one, wrapping as the branches do. None for the root.
    pub depth_section: Option<usize>,
    pub class: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct MindmapDb {
    pub title: Option<String>,
    /// In source order, which is depth-first. The first node is the root.
    pub nodes: Vec<Node>,
    base_level: isize,
    /// The chain of nodes that can still take children, with strictly increasing levels.
    open: Vec<usize>,
}

impl OutlineDb for MindmapDb {
    fn add_node(
        &mut self,
        level: usize,
        id: &str,
        descr: &str,
        kind: u8,
        _shape_data: Option<&str>,
    ) -> Result<(), Error> {
        let is_root = self.nodes.is_empty();
        let level = level as isize;
        if is_root {
            self.base_level = level;
        }
        let level = level - self.base_level;

        // Mermaid's parent is the last node with a smaller indentation, wherever it is in the
        // tree.
        let mut depth = self.open.len();
        while depth > 0 && self.nodes[self.open[depth - 1]].level >= level {
            depth -= 1;
        }
        let parent = depth.checked_sub(1).map(|d| self.open[d]);
        if parent.is_none() && !is_root {
            return Err(Error::new(
                ErrorCode::Semantic,
                format!("There can be only one root. No parent could be found for (\"{descr}\")"),
            ));
        }
        let index = self.nodes.len();
        let section = parent.map(|p| {
            let parent = &self.nodes[p];
            if p == 0 {
                parent.children.len() % SECTIONS
            } else {
                parent.section.unwrap_or(0)
            }
        });
        if let Some(p) = parent {
            self.nodes[p].children.push(index);
        }
        self.nodes.push(Node {
            node_id: id.to_string(),
            level,
            descr: descr.to_string(),
            kind,
            children: Vec::new(),
            section,
            // `depth` is the number of ancestors the node has, so the root's children are 1.
            depth_section: parent.map(|_| (depth - 1) % SECTIONS),
            class: None,
            icon: None,
        });
        self.open.truncate(depth);
        self.open.push(index);
        Ok(())
    }

    /// Mermaid throws a TypeError when a decoration comes before any node. Here it is ignored.
    fn decorate_node(&mut self, icon: Option<&str>, class: Option<&str>) {
        let Some(node) = self.nodes.last_mut() else {
            return;
        };
        if let Some(icon) = icon.filter(|i| !i.is_empty()) {
            node.icon = Some(icon.to_string());
        }
        if let Some(class) = class.filter(|c| !c.is_empty()) {
            node.class = Some(class.to_string());
        }
    }
}
