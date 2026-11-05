//! The graph an agentflow diagram draws: the agents, the flows that hold them, and the edges
//! between them. Ported from pele's `diagrams/agentflow/graph.ts`.

use super::db::{AgentflowDb, LabelType};
use super::shapes::{is_allowed_shape, is_removed_shape, resolve_shape_alias};

/// The palette slot of each kind of agent, so that editing the diagram around one does not
/// recolor it.
const KIND_SLOT: &[(&str, usize)] = &[
    ("tool", 0),
    ("task", 1),
    ("decision", 2),
    ("input", 3),
    ("refdoc", 4),
    ("connector", 5),
    ("action", 6),
];

/// The number of slots the kinds take.
const KIND_COUNT: usize = 7;

/// The number of series colours of the palette.
pub(crate) const PALETTE: usize = 8;

/// An agent, or a flow, as it is drawn.
#[derive(Debug, Clone)]
pub(crate) struct GraphNode {
    pub id: String,
    pub label: Option<String>,
    pub label_type: LabelType,
    /// The shape, as the drawing names it.
    pub shape: String,
    /// Whether the node is a flow that holds other nodes.
    pub is_group: bool,
    /// The flow that holds this one.
    pub parent_id: Option<String>,
    pub css_styles: Vec<String>,
    pub css_compiled_styles: Vec<String>,
    pub css_classes: String,
    pub dir: Option<String>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub tooltip: Option<String>,
    /// The palette slot of a flow.
    pub color_index: Option<usize>,
    /// What the agent is, which picks its palette slot.
    pub kind: Option<String>,
}

/// An edge, as it is drawn.
#[derive(Debug, Clone)]
pub(crate) struct GraphEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    pub label: String,
    pub label_type: LabelType,
    /// The line the edge is drawn with.
    pub thickness: Option<String>,
    pub minlen: Option<i64>,
    pub arrow_start: String,
    pub arrow_end: String,
    pub css_compiled_styles: Vec<String>,
    pub style: Vec<String>,
    pub classes: String,
    pub curve: Option<String>,
}

/// The graph of a diagram.
pub(crate) struct AgentGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// The shape a node is drawn with, as Mermaid's `getTypeFromVertex` reads it.
fn type_from_vertex(shape: Option<&str>) -> String {
    let resolved = shape.map(resolve_shape_alias).unwrap_or_default();
    match resolved.as_str() {
        "square" | "" => "squareRect".to_string(),
        "round" => "roundedRect".to_string(),
        "ellipse" => "ellipse".to_string(),
        other => other.to_string(),
    }
}

/// Builds the graph of a diagram. The shape and the containment problems it finds are recorded
/// on the model as diagnostics.
pub(crate) fn build_agent_graph(db: &mut AgentflowDb) -> AgentGraph {
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();

    // Everything inside a collapsed flow is hidden and stands for the flow itself.
    let mut hidden: Vec<String> = Vec::new();
    let mut owner: Vec<(String, String)> = Vec::new();
    let mut walked: Vec<String> = Vec::new();
    for sg in &db.subgraphs {
        if !is_collapsed(sg) || hidden.contains(&sg.id) {
            continue;
        }
        let mut pending = vec![sg.id.clone()];
        while let Some(current) = pending.pop() {
            let Some(members) = db.subgraph(&current).map(|sg| sg.nodes.clone()) else {
                continue;
            };
            for child in members {
                if child == sg.id || hidden.contains(&child) {
                    continue;
                }
                hidden.push(child.clone());
                owner.push((child.clone(), sg.id.clone()));
                if !walked.contains(&child) {
                    pending.push(child);
                }
            }
        }
        walked.push(sg.id.clone());
    }
    let hidden_of = |id: &str| hidden.iter().any(|h| h == id);

    // Two flows can each name the other. The nesting that would close the loop is dropped.
    let mut parent_of: Vec<(String, String)> = Vec::new();
    for at in (0..db.subgraphs.len()).rev() {
        let sg = db.subgraphs[at].clone();
        if hidden_of(&sg.id) {
            continue;
        }
        for id in &sg.nodes {
            if closes_loop(id, &sg.id, &db.subgraphs, &parent_of) {
                db.emit(
                    "CONTAINMENT_VIOLATION",
                    format!(
                        "Container \"{}\" cannot contain \"{}\" because \"{}\" already contains it. \
                         The nesting that would close the loop is dropped.",
                        sg.id, id, id
                    ),
                    Some(id.clone()),
                );
                continue;
            }
            parent_of.push((id.clone(), sg.id.clone()));
        }
    }
    let parent_id_of = |id: &str| {
        parent_of
            .iter()
            .find(|(child, _)| child == id)
            .map(|(_, parent)| parent.clone())
    };

    // The flows, from the outermost to the innermost.
    for sg in db.subgraphs.iter().rev() {
        if hidden_of(&sg.id) {
            continue;
        }
        let collapsed = is_collapsed(sg);
        let node = GraphNode {
            id: sg.id.clone(),
            label: sg.title.clone(),
            label_type: sg.label_type,
            shape: if collapsed {
                "collapsedGroup".to_string()
            } else {
                "flowGroup".to_string()
            },
            is_group: !collapsed,
            parent_id: parent_id_of(&sg.id),
            css_styles: Vec::new(),
            css_compiled_styles: compiled_styles(db, &sg.classes),
            css_classes: sg.classes.join(" "),
            dir: sg.dir.clone(),
            link: None,
            link_target: None,
            tooltip: None,
            color_index: None,
            kind: None,
        };
        nodes.push(node);
    }

    // The agents.
    for agent in &db.nodes {
        if hidden_of(&agent.id) {
            continue;
        }
        if db.subgraph(&agent.id).is_some() {
            // A flow that was given a metadata block has a placeholder agent of its own.
            continue;
        }
        let mut classes = vec!["default".to_string()];
        classes.extend(agent.classes.iter().cloned());
        let shape = type_from_vertex(agent.shape.as_deref());
        nodes.push(GraphNode {
            id: agent.id.clone(),
            label: agent.text.clone(),
            label_type: agent.label_type,
            shape,
            is_group: false,
            parent_id: parent_id_of(&agent.id),
            css_styles: agent.styles.clone(),
            css_compiled_styles: compiled_styles(db, &classes),
            css_classes: classes.join(" "),
            dir: agent.dir.clone(),
            link: agent.link.clone(),
            link_target: agent.link_target.clone(),
            tooltip: db.tooltips.get(&agent.id).cloned(),
            color_index: None,
            kind: None,
        });
    }

    for (at, edge) in db.edges.iter().enumerate() {
        let start =
            collapsed_into(&hidden, &owner, &edge.start).unwrap_or_else(|| edge.start.clone());
        let end = collapsed_into(&hidden, &owner, &edge.end).unwrap_or_else(|| edge.end.clone());
        // An edge between two things inside one collapsed flow has nothing left to connect.
        if start == end && edge.start != edge.end {
            continue;
        }
        let mut style = db.default_edge_style.clone();
        style.extend(edge.style.iter().cloned());
        let arrow_end = match edge.kind {
            Some(super::db::EdgeKind::ArrowCross) => "arrow_cross",
            Some(super::db::EdgeKind::ArrowOpen) => "none",
            _ => "arrow_point",
        };
        edges.push(GraphEdge {
            id: edge
                .id
                .clone()
                .unwrap_or_else(|| format!("L_{start}_{end}_{at}")),
            start,
            end,
            label: edge.text.clone(),
            label_type: edge.label_type,
            thickness: edge.stroke.map(|s| match s {
                super::db::Stroke::Dotted => "dotted".to_string(),
                _ => "normal".to_string(),
            }),
            minlen: edge.length.filter(|_| edge.length != Some(1)),
            arrow_start: "none".to_string(),
            arrow_end: arrow_end.to_string(),
            css_compiled_styles: compiled_styles(db, &edge.classes),
            style,
            classes: "edge-thickness-normal edge-pattern-solid flowchart-link".to_string(),
            curve: edge.interpolate.clone(),
        });
    }

    normalise_shapes(db, &mut nodes);
    assign_color_slots(db, &mut nodes);
    AgentGraph { nodes, edges }
}

/// Whether a flow is drawn collapsed.
fn is_collapsed(sg: &super::db::Subgraph) -> bool {
    sg.metadata
        .as_ref()
        .and_then(|meta| meta.get("view"))
        .map(|view| view.js_string() == "collapsed")
        .unwrap_or(false)
}

/// The flow a hidden agent stands for.
fn collapsed_into(hidden: &[String], owner: &[(String, String)], id: &str) -> Option<String> {
    if !hidden.iter().any(|h| h == id) {
        return None;
    }
    let mut path = vec![id.to_string()];
    let mut top = owner
        .iter()
        .find(|(child, _)| child == id)
        .map(|(_, parent)| parent.clone())?;
    loop {
        let known = owner.iter().find(|(child, _)| *child == top);
        match known {
            Some((_, up)) => {
                path.push(top.clone());
                top = up.clone();
            }
            None => break,
        }
    }
    path.retain(|p| p != id);
    path.last().cloned()
}

/// Whether holding `child` in `parent` would close a loop.
fn closes_loop(
    child: &str,
    parent: &str,
    subgraphs: &[super::db::Subgraph],
    parent_of: &[(String, String)],
) -> bool {
    let Some(child_at) = subgraphs.iter().position(|sg| sg.id == child) else {
        return false;
    };
    let Some(parent_at) = subgraphs.iter().position(|sg| sg.id == parent) else {
        return false;
    };
    if child_at < parent_at {
        return false;
    }
    let mut current = Some(parent.to_string());
    let mut hops = 0;
    while let Some(id) = current {
        if id == child {
            return true;
        }
        current = parent_of
            .iter()
            .find(|(held, _)| *held == id)
            .map(|(_, parent)| parent.clone());
        hops += 1;
        if hops > subgraphs.len() {
            break;
        }
    }
    false
}

/// The styles of the classes an element is in.
fn compiled_styles(db: &AgentflowDb, classes: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for name in classes {
        let Some(def) = db.class(name) else {
            continue;
        };
        for style in &def.styles {
            out.push(style.trim().to_string());
        }
        for style in &def.text_styles {
            out.push(style.trim().to_string());
        }
    }
    out
}

/// Reduces the shapes of the agents to the ones agentflow has, reporting the ones it does not.
fn normalise_shapes(db: &mut AgentflowDb, nodes: &mut [GraphNode]) {
    for node in nodes.iter_mut() {
        if node.is_group {
            continue;
        }
        let shape = node.shape.as_str();
        if shape.is_empty() || shape == "squareRect" || shape == "rect" {
            node.shape = super::shapes::DEFAULT_SHAPE.to_string();
            continue;
        }
        if is_removed_shape(shape) {
            db.emit(
                "SHAPE_REMOVED",
                format!(
                    "shape \"{shape}\" was removed in v0.8.1, using \"{}\"",
                    super::shapes::DEFAULT_SHAPE
                ),
                Some(node.id.clone()),
            );
            node.shape = super::shapes::DEFAULT_SHAPE.to_string();
            continue;
        }
        if !is_allowed_shape(shape) {
            db.emit(
                "SHAPE_UNSUPPORTED",
                format!(
                    "shape \"{shape}\" is not supported, using \"{}\"",
                    super::shapes::DEFAULT_SHAPE
                ),
                Some(node.id.clone()),
            );
            node.shape = super::shapes::DEFAULT_SHAPE.to_string();
        }
    }
}

/// Gives every agent the palette slot of its kind, and every flow the slot of its place in the
/// nesting.
fn assign_color_slots(db: &AgentflowDb, nodes: &mut [GraphNode]) {
    // The flows are numbered in the order they were written: a walk of the nesting, parents
    // first.
    let mut order: Vec<String> = Vec::new();
    let mut roots: Vec<&str> = Vec::new();
    for sg in &db.subgraphs {
        let parent = nodes
            .iter()
            .find(|n| n.id == sg.id)
            .and_then(|n| n.parent_id.clone());
        match parent {
            Some(parent) if parent != sg.id => {
                if let Some(at) = order.iter().position(|id| *id == parent) {
                    order.insert(at, sg.id.clone());
                } else {
                    order.push(sg.id.clone());
                }
            }
            _ => roots.push(&sg.id),
        }
    }
    let mut counter = 0;
    for root in roots {
        let mut pending = vec![root.to_string()];
        while let Some(id) = pending.pop() {
            if order.contains(&id) {
                continue;
            }
            order.push(id.clone());
            for sg in db.subgraphs.iter().rev() {
                let parent = nodes
                    .iter()
                    .find(|n| n.id == sg.id)
                    .and_then(|n| n.parent_id.clone());
                if parent.as_deref() == Some(id.as_str()) {
                    pending.push(sg.id.clone());
                }
            }
        }
        counter += 1;
    }
    let container_slots = PALETTE.saturating_sub(KIND_COUNT).max(1);
    for node in nodes.iter_mut() {
        if node.is_group || node.shape == "collapsedGroup" {
            let at = order
                .iter()
                .position(|id| *id == node.id)
                .unwrap_or(counter);
            node.color_index = Some((KIND_COUNT + at % container_slots) % PALETTE);
            continue;
        }
        let kind = db
            .node(&node.id)
            .map(|agent| db.vertex_kind(agent))
            .unwrap_or("task");
        if KIND_SLOT.iter().any(|(name, _)| *name == kind) {
            node.kind = Some(kind.to_string());
        }
    }
}
