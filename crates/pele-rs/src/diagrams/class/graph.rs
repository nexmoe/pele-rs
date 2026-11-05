//! What a class diagram draws: the namespaces, classes, notes and interfaces, and the edges
//! between them. Ported from pele's `diagrams/class/graph.ts`.

use std::collections::HashMap;

use super::db::{ClassDb, LOLLIPOP, RelationEnd};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    Namespace,
    Class,
    Note,
    Interface,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GraphNode {
    pub id: String,
    pub label: String,
    pub shape: Shape,
    pub parent_id: Option<String>,
    /// The namespace node this one is drawn in.
    pub parent: i64,
    /// The class's place in the model.
    pub class: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndMarker {
    Aggregation,
    Extension,
    Composition,
    Dependency,
    Lollipop,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pattern {
    Solid,
    Dashed,
    Dotted,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GraphEdge {
    pub id: String,
    /// Places in the node list. Ids alone are ambiguous: a class may be named `note0` or
    /// `interface0`.
    pub source: i64,
    pub target: i64,
    pub label: Option<String>,
    pub start: EndMarker,
    pub end: EndMarker,
    pub start_label: String,
    pub end_label: String,
    pub pattern: Pattern,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ClassGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

fn marker(end: RelationEnd) -> EndMarker {
    match end {
        Some(0) => EndMarker::Aggregation,
        Some(1) => EndMarker::Extension,
        Some(2) => EndMarker::Composition,
        Some(3) => EndMarker::Dependency,
        Some(4) => EndMarker::Lollipop,
        _ => EndMarker::None,
    }
}

/// Resolves a parsed class diagram into the nodes and edges that are drawn. With
/// `hierarchical` off, only namespaces the source declares are kept and each is labeled with
/// its full name.
pub(crate) fn build_class_graph(db: &ClassDb, hierarchical: bool) -> ClassGraph {
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut namespace_at: HashMap<&str, usize> = HashMap::new();
    let mut class_at: HashMap<&str, usize> = HashMap::new();
    let mut interface_at: HashMap<&str, usize> = HashMap::new();

    let mut declared: HashMap<String, Option<String>> = HashMap::new();
    let mut declared_ancestor = |id: Option<&String>| -> Option<String> {
        let id = id?;
        if hierarchical {
            return Some(id.clone());
        }
        let mut chain: Vec<&str> = Vec::new();
        let mut current = Some(id.as_str());
        let mut found: Option<String> = None;
        while let Some(at) = current.filter(|c| !c.is_empty()) {
            if let Some(known) = declared.get(at) {
                found = known.clone();
                break;
            }
            let Some(ns) = db.namespaces.get(at) else {
                break;
            };
            if ns.explicit {
                found = Some(at.to_string());
                break;
            }
            chain.push(at);
            current = ns.parent.as_deref();
        }
        for visited in chain {
            declared.insert(visited.to_string(), found.clone());
        }
        found
    };

    for ns in db.namespaces.values() {
        if !hierarchical && !ns.explicit {
            continue;
        }
        namespace_at.insert(ns.id.as_str(), nodes.len());
        nodes.push(GraphNode {
            id: ns.id.clone(),
            label: if hierarchical {
                ns.label.clone()
            } else {
                ns.id.clone()
            },
            shape: Shape::Namespace,
            parent_id: if hierarchical {
                ns.parent.clone()
            } else {
                None
            },
            parent: -1,
            class: None,
        });
    }
    let group = |namespace_at: &HashMap<&str, usize>, id: Option<&str>| -> i64 {
        id.and_then(|id| namespace_at.get(id))
            .map_or(-1, |&i| i as i64)
    };
    for node in &mut nodes {
        node.parent = group(&namespace_at, node.parent_id.as_deref());
    }

    for (k, (_, class)) in db.classes.items.iter().enumerate() {
        let parent_id = declared_ancestor(class.parent.as_ref());
        class_at.insert(class.id.as_str(), nodes.len());
        let parent = group(&namespace_at, parent_id.as_deref());
        nodes.push(GraphNode {
            id: class.id.clone(),
            label: class.label.clone(),
            shape: Shape::Class,
            parent_id,
            parent,
            class: Some(k),
        });
    }

    for note in db.notes.values() {
        let parent_id = declared_ancestor(note.parent.as_ref());
        let target = note.class.as_deref().and_then(|c| class_at.get(c).copied());
        if let Some(target) = target {
            edges.push(GraphEdge {
                id: format!("edgeNote{}", note.index),
                source: nodes.len() as i64,
                target: target as i64,
                label: None,
                start: EndMarker::None,
                end: EndMarker::None,
                start_label: String::new(),
                end_label: String::new(),
                pattern: Pattern::Dotted,
            });
        }
        let parent = group(&namespace_at, parent_id.as_deref());
        nodes.push(GraphNode {
            id: note.id.clone(),
            label: note.text.clone(),
            shape: Shape::Note,
            parent_id,
            parent,
            class: None,
        });
    }

    // Mermaid leaves every lollipop interface outside the namespaces. Here it stays beside its
    // class.
    for item in &db.interfaces {
        let parent = class_at
            .get(item.class_id.as_str())
            .map_or(-1, |&owner| nodes[owner].parent);
        interface_at.insert(item.id.as_str(), nodes.len());
        nodes.push(GraphNode {
            id: item.id.clone(),
            label: item.label.clone(),
            shape: Shape::Interface,
            parent_id: None,
            parent,
            class: None,
        });
    }

    for (count, relation) in db.relations.iter().enumerate() {
        let (type1, type2) = (relation.type1, relation.type2);
        // addRelation swaps in an interface for the lollipop end of a relation with no other
        // marker.
        let start_interface = type1 == Some(LOLLIPOP) && type2.is_none();
        let end_interface = !start_interface && type2 == Some(LOLLIPOP) && type1.is_none();
        let find = |interface: bool, id: &str| {
            if interface { &interface_at } else { &class_at }
                .get(id)
                .map_or(-1, |&i| i as i64)
        };
        edges.push(GraphEdge {
            id: format!("id_{}_{}_{}", relation.id1, relation.id2, count + 1),
            source: find(start_interface, &relation.id1),
            target: find(end_interface, &relation.id2),
            label: relation.title.clone(),
            start: marker(type1),
            end: marker(type2),
            start_label: if relation.title1 == "none" {
                String::new()
            } else {
                relation.title1.clone()
            },
            end_label: if relation.title2 == "none" {
                String::new()
            } else {
                relation.title2.clone()
            },
            pattern: if relation.dotted {
                Pattern::Dashed
            } else {
                Pattern::Solid
            },
        });
    }

    ClassGraph { nodes, edges }
}
