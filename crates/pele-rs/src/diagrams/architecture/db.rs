//! The model of an architecture diagram.
//!
//! Mermaid fills its database the way `populateDb` does: every group, then the services, then
//! the junctions, then the edges, then the alignments. The database checks what it is given, so
//! a service whose group is not there, a name that is in use twice, or an edge between two
//! services of the same group that says it leaves the group are all refused.

use std::collections::HashMap;

use crate::diagrams::architecture::parser::{Alignment, Ast, Edge};
use crate::diagrams::common::ordered::Ordered;
use crate::error::{Error, ErrorCode};

/// What a node of the diagram is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeKind {
    Service,
    Junction,
}

/// A service or a junction.
#[derive(Debug, Clone)]
pub(crate) struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub icon: Option<String>,
    pub icon_text: Option<String>,
    pub title: Option<String>,
    pub parent: Option<String>,
}

/// A group of services.
#[derive(Debug, Clone)]
pub(crate) struct GroupModel {
    pub id: String,
    pub icon: Option<String>,
    pub title: Option<String>,
    pub parent: Option<String>,
}

/// The diagram, as Mermaid's database holds it.
#[derive(Debug, Clone, Default)]
pub(crate) struct ArchitectureModel {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// The services, then the junctions, in the order they were added.
    pub nodes: Ordered<Node>,
    pub groups: Ordered<GroupModel>,
    pub edges: Vec<Edge>,
    pub layout_hints: Vec<Alignment>,
}

/// What a name is in use by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Registered {
    Node,
    Group,
}

impl Registered {
    fn as_str(self) -> &'static str {
        match self {
            Registered::Node => "node",
            Registered::Group => "group",
        }
    }
}

/// Fills the model, and refuses what Mermaid's database refuses.
///
/// The title of the front matter is the title of the diagram, unless the diagram has one of its
/// own.
pub(crate) fn build_architecture(
    ast: &Ast,
    title: Option<&str>,
) -> Result<ArchitectureModel, Error> {
    let mut model = ArchitectureModel {
        title: match &ast.title {
            Some(title) if !title.is_empty() => Some(title.clone()),
            _ => title.map(str::to_string),
        },
        acc_title: ast
            .acc_title
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(str::to_string),
        acc_descr: ast
            .acc_descr
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(str::to_string),
        ..ArchitectureModel::default()
    };
    let mut registered: HashMap<String, Registered> = HashMap::new();

    for group in &ast.groups {
        register(&mut registered, "group", &group.id, group.parent.as_deref())?;
        model.groups.set(
            &group.id,
            GroupModel {
                id: group.id.clone(),
                icon: group.icon.clone(),
                title: group.title.clone(),
                parent: group.parent.clone(),
            },
        );
    }
    for service in &ast.services {
        register(
            &mut registered,
            "service",
            &service.id,
            service.parent.as_deref(),
        )?;
        model.nodes.set(
            &service.id,
            Node {
                id: service.id.clone(),
                kind: NodeKind::Service,
                icon: service.icon.clone(),
                icon_text: service.icon_text.clone(),
                title: service.title.clone(),
                parent: service.parent.clone(),
            },
        );
    }
    for junction in &ast.junctions {
        register(
            &mut registered,
            "junction",
            &junction.id,
            junction.parent.as_deref(),
        )?;
        model.nodes.set(
            &junction.id,
            Node {
                id: junction.id.clone(),
                kind: NodeKind::Junction,
                icon: None,
                icon_text: None,
                title: None,
                parent: junction.parent.clone(),
            },
        );
    }
    for edge in &ast.edges {
        add_edge(&mut model, &registered, edge)?;
    }
    for alignment in &ast.alignments {
        add_layout_hint(&mut model, &registered, alignment)?;
    }
    Ok(model)
}

/// What a name may not be used for twice, and where it may be placed.
fn register(
    registered: &mut HashMap<String, Registered>,
    kind: &str,
    id: &str,
    parent: Option<&str>,
) -> Result<(), Error> {
    if let Some(other) = registered.get(id) {
        return Err(Error::new(
            ErrorCode::Semantic,
            format!(
                "The {kind} id [{id}] is already in use by another {}",
                other.as_str()
            ),
        ));
    }
    if let Some(parent) = parent {
        if parent == id {
            return Err(Error::new(
                ErrorCode::Semantic,
                format!("The {kind} [{id}] cannot be placed within itself"),
            ));
        }
        let Some(other) = registered.get(parent) else {
            return Err(Error::new(
                ErrorCode::Semantic,
                format!(
                    "The {kind} [{id}]'s parent does not exist. Please make sure the parent is \
                     created before this {kind}"
                ),
            ));
        };
        if *other == Registered::Node {
            return Err(Error::new(
                ErrorCode::Semantic,
                format!("The {kind} [{id}]'s parent is not a group"),
            ));
        }
    }
    registered.insert(
        id.to_string(),
        if kind == "group" {
            Registered::Group
        } else {
            Registered::Node
        },
    );
    Ok(())
}

/// Adds an edge, and refuses what Mermaid's database refuses.
fn add_edge(
    model: &mut ArchitectureModel,
    registered: &HashMap<String, Registered>,
    edge: &Edge,
) -> Result<(), Error> {
    for (side, id) in [("left", &edge.lhs_id), ("right", &edge.rhs_id)] {
        if !registered.contains_key(id) {
            return Err(Error::new(
                ErrorCode::Semantic,
                format!(
                    "The {side}-hand id [{id}] does not yet exist. Please create the service/group \
                     before declaring an edge to it."
                ),
            ));
        }
    }
    // Mermaid lets a group through the check above and then fails when it reads the group of the
    // end, which is not there.
    for (side, id) in [("left", &edge.lhs_id), ("right", &edge.rhs_id)] {
        if model.nodes.get(id).is_none() {
            return Err(Error::new(
                ErrorCode::Semantic,
                format!(
                    "The {side}-hand id [{id}] is a group. Add {{group}} to a service inside it \
                     instead."
                ),
            ));
        }
    }
    let lhs_parent = model.nodes.get(&edge.lhs_id).and_then(|n| n.parent.clone());
    let rhs_parent = model.nodes.get(&edge.rhs_id).and_then(|n| n.parent.clone());
    if lhs_parent.is_some() && lhs_parent == rhs_parent {
        for (side, id, crosses) in [
            ("left", &edge.lhs_id, edge.lhs_group),
            ("right", &edge.rhs_id, edge.rhs_group),
        ] {
            if crosses {
                return Err(Error::new(
                    ErrorCode::Semantic,
                    format!(
                        "The {side}-hand id [{id}] is modified to traverse the group boundary, but \
                         the edge does not pass through two groups."
                    ),
                ));
            }
        }
    }
    model.edges.push(edge.clone());
    Ok(())
}

/// Adds an alignment, and refuses what Mermaid's database refuses.
fn add_layout_hint(
    model: &mut ArchitectureModel,
    registered: &HashMap<String, Registered>,
    hint: &Alignment,
) -> Result<(), Error> {
    if hint.members.len() < 2 {
        return Err(Error::new(
            ErrorCode::Semantic,
            format!(
                "An align directive requires at least two members; got {}",
                hint.members.len()
            ),
        ));
    }
    let mut seen: Vec<&str> = Vec::new();
    for id in &hint.members {
        match registered.get(id) {
            Some(Registered::Node) => {}
            _ => {
                return Err(Error::new(
                    ErrorCode::Semantic,
                    format!(
                        "align {} references [{id}], which is not a service or junction",
                        hint.direction.as_str()
                    ),
                ));
            }
        }
        if seen.contains(&id.as_str()) {
            return Err(Error::new(
                ErrorCode::Semantic,
                format!(
                    "align {} lists [{id}] more than once",
                    hint.direction.as_str()
                ),
            ));
        }
        seen.push(id);
    }
    model.layout_hints.push(hint.clone());
    Ok(())
}
