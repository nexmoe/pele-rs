//! The model of a treemap: the tree of items and the classes they name.
//! Ported from pele's `diagrams/treemap/model.ts`, which follows Mermaid's
//! `diagrams/treemap/db.ts` and `diagrams/treemap/utils.ts`.

use crate::diagrams::common::ordered::Ordered;

use super::parser::{Row, TreemapAst};

/// Whether an item is a section, which holds other items, or a leaf, which has a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum NodeKind {
    #[default]
    Section,
    Leaf,
}

impl NodeKind {
    /// The name Mermaid gives the kind.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            NodeKind::Section => "Section",
            NodeKind::Leaf => "Leaf",
        }
    }
}

/// An item of the tree.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Node {
    pub name: String,
    pub kind: NodeKind,
    /// The value of a leaf.
    pub value: Option<f64>,
    /// The class the item names.
    pub class_selector: Option<String>,
    /// The styles of that class, as Mermaid compiles them.
    pub styles: Vec<String>,
    /// The items under a section.
    pub children: Vec<usize>,
}

/// The tree of items, held flat so that the layout can walk it.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Tree {
    pub nodes: Vec<Node>,
    /// The items that have no section above them.
    pub roots: Vec<usize>,
}

impl Tree {
    /// The value of an item: its own, or the sum of the values under it. A value that is not a
    /// number counts as nothing, as Mermaid's sum reads it.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "the compat summary totals the items")
    )]
    pub(crate) fn total(&self, index: usize) -> f64 {
        let node = &self.nodes[index];
        let mut total = match node.value {
            Some(value) if !value.is_nan() => value,
            _ => 0.0,
        };
        for &child in &node.children {
            total += self.total(child);
        }
        total
    }
}

/// The model of a treemap.
#[derive(Debug, Clone, Default)]
pub(crate) struct TreemapDb {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub tree: Tree,
    /// The styles of each class, in the order the classes were named.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "the compat summary lists the classes")
    )]
    pub classes: Ordered<Vec<String>>,
}

/// An item with the level it sits at, as Mermaid's model holds it before the tree is built.
struct Entry {
    level: usize,
    name: String,
    kind: NodeKind,
    value: Option<f64>,
    class_selector: Option<String>,
    styles: Vec<String>,
}

/// Reads the model out of what the parser read.
pub(crate) fn build_db(ast: &TreemapAst) -> TreemapDb {
    let classes = build_classes(ast);

    let mut entries: Vec<Entry> = Vec::new();
    for row in &ast.rows {
        let Row::Item { indent, item } = row else {
            continue;
        };
        let styles = item
            .class_selector()
            .and_then(|selector| classes.get(selector))
            .cloned()
            .unwrap_or_default();
        entries.push(Entry {
            level: *indent,
            name: item.name().to_string(),
            kind: if item.is_leaf() {
                NodeKind::Leaf
            } else {
                NodeKind::Section
            },
            value: item.value(),
            class_selector: item.class_selector().map(str::to_string),
            styles,
        });
    }

    TreemapDb {
        title: ast.title.clone().filter(|t| !t.is_empty()),
        acc_title: ast.acc_title.clone().filter(|t| !t.is_empty()),
        acc_descr: ast.acc_descr.clone().filter(|t| !t.is_empty()),
        tree: build_hierarchy(entries),
        classes,
    }
}

/// The styles of each class. A class named twice keeps the styles of both.
fn build_classes(ast: &TreemapAst) -> Ordered<Vec<String>> {
    let mut classes: Ordered<Vec<String>> = Ordered::default();
    for row in &ast.rows {
        let Row::ClassDef {
            class_name,
            style_text,
            ..
        } = row
        else {
            continue;
        };
        // An escaped comma stays in the value; the others separate declarations.
        let text = style_text.as_deref().unwrap_or("");
        let text = text
            .replace("\\,", "§§§")
            .replace(',', ";")
            .replace("§§§", ",");
        let styles: Vec<String> = text.split(';').map(str::to_string).collect();
        match classes.get_mut(class_name) {
            Some(existing) => existing.extend(styles),
            None => {
                classes.set(class_name, styles);
            }
        }
    }
    classes
}

/// Lays the items out as a tree: an item belongs to the section above it that is indented less.
fn build_hierarchy(entries: Vec<Entry>) -> Tree {
    let mut nodes: Vec<Node> = Vec::new();
    let mut roots: Vec<usize> = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();

    for entry in entries {
        let level = entry.level;
        let index = nodes.len();
        let section = entry.kind == NodeKind::Section;
        nodes.push(Node {
            name: entry.name,
            kind: entry.kind,
            value: entry.value,
            class_selector: entry.class_selector,
            styles: entry.styles,
            children: Vec::new(),
        });
        while stack.last().is_some_and(|&(l, _)| l >= level) {
            stack.pop();
        }
        match stack.last() {
            Some(&(_, parent)) => nodes[parent].children.push(index),
            None => roots.push(index),
        }
        if section {
            stack.push((level, index));
        }
    }

    Tree { nodes, roots }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagrams::treemap::parser::parse_treemap;

    fn db(src: &str) -> TreemapDb {
        build_db(&parse_treemap(src).expect("parses"))
    }

    #[test]
    fn tells_a_section_from_a_leaf() {
        assert_eq!(NodeKind::Section.as_str(), "Section");
        assert_eq!(NodeKind::Leaf.as_str(), "Leaf");
    }

    #[test]
    fn reads_the_tree_of_a_treemap() {
        let db = db("treemap\n\"A\"\n  \"B\": 10\n  \"C\"\n    \"D\": 5\n");
        assert_eq!(db.tree.roots.len(), 1);
        let a = db.tree.roots[0];
        assert_eq!(db.tree.nodes[a].name, "A");
        assert_eq!(db.tree.nodes[a].children.len(), 2);
        let b = db.tree.nodes[a].children[0];
        assert_eq!(db.tree.nodes[b].name, "B");
        assert_eq!(db.tree.nodes[b].value, Some(10.0));
        let c = db.tree.nodes[a].children[1];
        assert_eq!(db.tree.nodes[c].children.len(), 1);
    }

    #[test]
    fn sums_the_values_under_a_section() {
        let db = db("treemap\n\"A\"\n  \"B\": 10\n  \"C\": 5\n");
        let a = db.tree.roots[0];
        assert_eq!(db.tree.total(a), 15.0);
    }

    #[test]
    fn reads_the_styles_of_a_class() {
        let db = db("treemap\nclassDef cls fill:red,stroke:#333;\n\"A\":::cls\n");
        assert_eq!(
            db.classes.get("cls"),
            Some(&vec!["fill:red".to_string(), "stroke:#333".to_string()])
        );
        let a = db.tree.roots[0];
        assert_eq!(db.tree.nodes[a].styles.len(), 2);
    }

    #[test]
    fn keeps_the_styles_of_a_class_named_twice() {
        let db = db("treemap\nclassDef cls fill:red\nclassDef cls stroke:#333\n\"A\":::cls\n");
        assert_eq!(db.classes.get("cls").map(Vec::len), Some(2));
    }

    #[test]
    fn reads_no_styles_for_an_unknown_class() {
        let db = db("treemap\n\"A\":::cls\n");
        let a = db.tree.roots[0];
        assert!(db.tree.nodes[a].styles.is_empty());
    }
}
