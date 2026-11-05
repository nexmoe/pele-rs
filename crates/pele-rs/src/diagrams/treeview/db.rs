//! The model of a tree view: the nodes of the tree, as Mermaid's database holds them.
//!
//! Mermaid's database takes the nodes in the order they were read, with the depth each was
//! indented to, and hangs them under the node above them: `packages/mermaid/src/diagrams/
//! treeView/db.ts` in Mermaid 12.1.0.

use super::parser::Ast;

/// What a node stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeType {
    File,
    Directory,
}

/// A node of the tree.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Node {
    /// The number Mermaid gives the node. The root is zero, the first node is one.
    pub id: usize,
    /// How deep the node sits. The root is at minus one.
    pub level: i64,
    pub name: String,
    pub node_type: NodeType,
    /// The icon the node names, as it was written, or `none` for no icon at all.
    pub icon: Option<String>,
    /// The classes the node names.
    pub css_class: Option<String>,
    pub description: Option<String>,
    /// The node this one hangs under.
    pub parent: Option<usize>,
    /// The nodes that hang under this one.
    pub children: Vec<usize>,
}

/// The tree a source describes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// The nodes of the tree. The first one is the root, which holds the top-level nodes.
    pub nodes: Vec<Node>,
    /// The nodes the tree is built on. The last one is the node a new node hangs under.
    stack: Vec<usize>,
}

/// Builds the tree of a source. The title of the front matter is the one of the diagram when
/// the source does not name one.
pub(crate) fn build(ast: &Ast, title: Option<&str>) -> Model {
    let mut model = Model {
        // Mermaid keeps a title that is empty for the one of the front matter.
        title: non_empty(ast.title.as_deref()).or_else(|| title.map(str::to_string)),
        acc_title: non_empty(ast.acc_title.as_deref()),
        acc_descr: non_empty(ast.acc_descr.as_deref()),
        nodes: vec![Node {
            id: 0,
            level: -1,
            name: "/".to_string(),
            node_type: NodeType::Directory,
            icon: None,
            css_class: None,
            description: None,
            parent: None,
            children: Vec::new(),
        }],
        stack: vec![0],
    };
    for node in &ast.nodes {
        let level = node.indent.unwrap_or(0) as i64;
        // A name that ends in a slash stands for a folder.
        let (name, node_type) = match node.name.strip_suffix('/') {
            Some(name) => (name.to_string(), NodeType::Directory),
            None => (node.name.clone(), NodeType::File),
        };
        model.add_node(
            level,
            name,
            node_type,
            non_empty(node.class_annotation.as_deref()),
            node.icon_annotation.clone().map(|icon| {
                if icon.is_empty() {
                    "none".to_string()
                } else {
                    icon
                }
            }),
            non_empty(node.desc_annotation.as_deref()),
        );
    }
    model
}

/// The text of a value, when it is not empty.
fn non_empty(text: Option<&str>) -> Option<String> {
    text.filter(|text| !text.is_empty()).map(str::to_string)
}

impl Model {
    /// Hangs a node under the node above it. A node that is not deeper than the one above it
    /// starts a new branch.
    fn add_node(
        &mut self,
        level: i64,
        name: String,
        node_type: NodeType,
        css_class: Option<String>,
        icon: Option<String>,
        description: Option<String>,
    ) {
        while level <= self.nodes[*self.stack.last().expect("the root is on the stack")].level {
            self.stack.pop();
        }
        let parent = *self.stack.last().expect("the root is on the stack");
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            level,
            name,
            node_type,
            icon,
            css_class,
            description,
            parent: Some(parent),
            children: Vec::new(),
        });
        self.nodes[parent].children.push(id);
        self.stack.push(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagrams::treeview::parser;

    fn model(src: &str) -> Model {
        let chars: Vec<char> = src.chars().collect();
        let ast = parser::parse(&chars).expect("the source reads");
        build(&ast, None)
    }

    #[test]
    fn hangs_the_nodes_under_the_one_above() {
        let model = model("treeView-beta\na\n    b\n    c\nd\n");
        assert_eq!(model.nodes.len(), 5);
        // The root holds `a` and `d`.
        assert_eq!(model.nodes[0].children, vec![1, 4]);
        // `a` holds `b` and `c`.
        assert_eq!(model.nodes[1].children, vec![2, 3]);
        assert_eq!(model.nodes[2].parent, Some(1));
        assert_eq!(model.nodes[4].parent, Some(0));
    }

    #[test]
    fn reads_a_folder_from_its_trailing_slash() {
        let folder = model("treeView-beta\nsrc/\n");
        assert_eq!(folder.nodes[1].node_type, NodeType::Directory);
        assert_eq!(folder.nodes[1].name, "src");
        let file = model("treeView-beta\nsrc\n");
        assert_eq!(file.nodes[1].node_type, NodeType::File);
    }

    #[test]
    fn reads_the_annotations_of_a_node() {
        let model = model("treeView-beta\n    a :::c icon(pack:n) ## d\n");
        assert_eq!(model.nodes[1].css_class.as_deref(), Some("c"));
        assert_eq!(model.nodes[1].icon.as_deref(), Some("pack:n"));
        assert_eq!(model.nodes[1].description.as_deref(), Some("d"));
    }

    #[test]
    fn reads_an_icon_that_hides_the_icon() {
        let model = model("treeView-beta\n    a icon()\n");
        assert_eq!(model.nodes[1].icon.as_deref(), Some("none"));
    }

    #[test]
    fn numbers_the_nodes_as_mermaid_does() {
        let model = model("treeView-beta\na\n    b\n");
        assert_eq!(model.nodes[0].id, 0);
        assert_eq!(model.nodes[1].id, 1);
        assert_eq!(model.nodes[2].id, 2);
    }
}
