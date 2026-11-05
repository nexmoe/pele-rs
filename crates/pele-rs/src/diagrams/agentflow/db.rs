//! The model of an agentflow diagram: the agents, the flows that hold them, the connectors
//! and the edges between them. Ported from Mermaid's `diagrams/agentflow/agentflowDb.ts`.

use std::collections::{HashMap, HashSet};

use super::shapes::{is_valid_shape, resolve_shape_alias};

/// How a label was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum LabelType {
    #[default]
    Text,
    Str,
    Markdown,
}

impl LabelType {
    /// The name Mermaid uses for it.
    #[allow(dead_code, reason = "read by the compat summary")]
    pub(crate) fn name(self) -> &'static str {
        match self {
            LabelType::Text => "text",
            LabelType::Str => "string",
            LabelType::Markdown => "markdown",
        }
    }
}

/// A label and how it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlowText {
    pub text: String,
    pub kind: LabelType,
}

/// The arrow at the end of an edge, as Mermaid's `destructLink` reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EdgeKind {
    ArrowOpen,
    ArrowPoint,
    ArrowCross,
    Invalid,
}

impl EdgeKind {
    /// The name Mermaid uses for it.
    #[allow(dead_code, reason = "read by the compat summary")]
    pub(crate) fn name(self) -> &'static str {
        match self {
            EdgeKind::ArrowOpen => "arrow_open",
            EdgeKind::ArrowPoint => "arrow_point",
            EdgeKind::ArrowCross => "arrow_cross",
            EdgeKind::Invalid => "INVALID",
        }
    }
}

/// The line an edge is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stroke {
    Normal,
    Dotted,
    Invalid,
}

impl Stroke {
    /// The name Mermaid uses for it.
    #[allow(dead_code, reason = "read by the compat summary")]
    pub(crate) fn name(self) -> &'static str {
        match self {
            Stroke::Normal => "normal",
            Stroke::Dotted => "dotted",
            Stroke::Invalid => "INVALID",
        }
    }
}

/// What an edge means, as the agentflow syntax defines it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EdgeSemantic {
    Sequence,
    Failure,
    Reference,
}

impl EdgeSemantic {
    /// The name Mermaid uses for it.
    #[allow(dead_code, reason = "read by the compat summary")]
    pub(crate) fn name(self) -> &'static str {
        match self {
            EdgeSemantic::Sequence => "sequence",
            EdgeSemantic::Failure => "failure",
            EdgeSemantic::Reference => "reference",
        }
    }
}

/// An agent, or a connector.
#[derive(Debug, Clone, Default)]
pub(crate) struct Node {
    pub id: String,
    /// The label of the agent. An agent written without one has its id as its label.
    pub text: Option<String>,
    /// The shape as it was written, before the aliases are resolved.
    pub shape: Option<String>,
    pub label_type: LabelType,
    pub styles: Vec<String>,
    pub classes: Vec<String>,
    pub props: Vec<(String, String)>,
    pub dir: Option<String>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub is_connector: bool,
    /// The metadata of an `@{ … }` block, as the YAML it holds.
    pub metadata: Option<Metadata>,
    /// The callback a `click` statement gave the agent.
    pub callback: Option<String>,
}

/// The metadata of an `@{ … }` block.
pub(crate) type Metadata = crate::preprocess::Config;

/// An edge between two agents.
#[derive(Debug, Clone)]
pub(crate) struct Edge {
    pub start: String,
    pub end: String,
    pub kind: Option<EdgeKind>,
    pub stroke: Option<Stroke>,
    /// The length of the arrow, as Mermaid reads it off the link text.
    pub length: Option<i64>,
    pub semantic: Option<EdgeSemantic>,
    pub text: String,
    pub label_type: LabelType,
    pub id: Option<String>,
    pub is_user_defined_id: bool,
    pub interpolate: Option<String>,
    pub style: Vec<String>,
    pub classes: Vec<String>,
    pub metadata: Option<Metadata>,
}

/// A flow: the container that holds a set of agents.
#[derive(Debug, Clone)]
pub(crate) struct Subgraph {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: Option<String>,
    pub label_type: LabelType,
    pub classes: Vec<String>,
    pub dir: Option<String>,
    /// The kind of container. Agentflow has one: `flow`.
    pub kind: String,
    pub metadata: Option<Metadata>,
}

/// The styles of a class.
#[derive(Debug, Clone, Default)]
pub(crate) struct ClassDef {
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

/// A problem with the model that the parser reports.
#[derive(Debug, Clone)]
pub(crate) enum ModelError {
    /// The metadata names a shape that Mermaid does not have.
    Shape(String),
}

/// A problem Mermaid reports about a diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Diagnostic {
    pub id: &'static str,
    pub message: String,
    pub node_id: Option<String>,
}

/// The model of a parsed agentflow diagram.
#[derive(Clone)]
pub(crate) struct AgentflowDb {
    /// The agents and connectors, in the order they were written.
    pub nodes: Vec<Node>,
    /// Where each agent is in `nodes`.
    node_at: HashMap<String, usize>,
    pub edges: Vec<Edge>,
    pub subgraphs: Vec<Subgraph>,
    /// Where each flow is in `subgraphs`.
    subgraph_at: HashMap<String, usize>,
    /// The connectors, by id.
    pub connectors: HashMap<String, usize>,
    pub classes: Vec<(String, ClassDef)>,
    pub tooltips: HashMap<String, String>,
    pub default_edge_style: Vec<String>,
    pub default_interpolate: Option<String>,
    pub direction: Option<String>,
    /// The title of the diagram, from the front matter.
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
    /// The ids of the agents a flow already holds, so that only one flow holds each.
    members: HashSet<String>,
    /// The ids of the agents that are shared by every flow.
    global_nodes: HashSet<String>,
    /// The number of flows that were read, which names the ones without a name.
    sub_count: usize,
    max_edges: usize,
    /// Whether a flow without a direction of its own takes the direction of the diagram.
    inherit_dir: bool,
}

impl AgentflowDb {
    /// A model with room for `max_edges` edges.
    pub(crate) fn new(max_edges: usize, inherit_dir: bool) -> Self {
        AgentflowDb {
            nodes: Vec::new(),
            node_at: HashMap::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
            subgraph_at: HashMap::new(),
            connectors: HashMap::new(),
            classes: Vec::new(),
            tooltips: HashMap::new(),
            default_edge_style: Vec::new(),
            default_interpolate: None,
            direction: None,
            title: None,
            acc_title: None,
            acc_descr: None,
            diagnostics: Vec::new(),
            members: HashSet::new(),
            global_nodes: HashSet::new(),
            sub_count: 0,
            max_edges,
            inherit_dir,
        }
    }

    /// The agent with that id.
    pub(crate) fn node(&self, id: &str) -> Option<&Node> {
        self.node_at.get(id).map(|&at| &self.nodes[at])
    }

    /// The agent with that id, for writing to it.
    pub(crate) fn node_mut(&mut self, id: &str) -> Option<&mut Node> {
        let at = *self.node_at.get(id)?;
        Some(&mut self.nodes[at])
    }

    /// The flow with that id.
    pub(crate) fn subgraph(&self, id: &str) -> Option<&Subgraph> {
        self.subgraph_at.get(id).map(|&at| &self.subgraphs[at])
    }

    /// The flow with that id, for writing to it.
    pub(crate) fn subgraph_mut(&mut self, id: &str) -> Option<&mut Subgraph> {
        let at = *self.subgraph_at.get(id)?;
        Some(&mut self.subgraphs[at])
    }

    /// The styles of the class with that name.
    pub(crate) fn class(&self, name: &str) -> Option<&ClassDef> {
        self.classes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, def)| def)
    }

    /// The v0.8.1 kind of an agent: what it is, which picks its colour.
    pub(crate) fn vertex_kind(&self, node: &Node) -> &'static str {
        if is_tool_definition(node) {
            return "tool";
        }
        let shape = node
            .shape
            .as_deref()
            .map(resolve_shape_alias)
            .unwrap_or_default();
        match shape.as_str() {
            "hexagon" | "hex" => "action",
            "lean-right" | "lean_right" => "input",
            "lin-doc" | "lined-document" => "refdoc",
            "diamond" => "decision",
            _ => "task",
        }
    }

    /// Adds an agent, or widens the one that is already there. The metadata of an `@{ … }`
    /// block may go to an agent, a flow, a connector or an edge, which is what Mermaid's
    /// `addVertex` decides.
    #[allow(clippy::too_many_arguments, reason = "the grammar passes them all")]
    pub(crate) fn add_vertex(
        &mut self,
        id: &str,
        text: Option<&FlowText>,
        shape: Option<&str>,
        style: Option<&[String]>,
        classes: Option<&[String]>,
        dir: Option<&str>,
        props: Option<Vec<(String, String)>>,
        metadata: Option<Metadata>,
    ) -> Result<(), ModelError> {
        if id.trim().is_empty() {
            return Ok(());
        }
        // The shape of the metadata is resolved before anything else, but the name the author
        // wrote is kept: the rule that a shape name is lowercase judges what was written, not
        // the name the alias stands for.
        let mut authored_shape: Option<String> = None;
        let metadata = metadata.map(|mut meta| {
            if let Some(crate::preprocess::Value::String(shape)) = meta.get("shape") {
                authored_shape = Some(shape.clone());
                let resolved = resolve_shape_alias(shape);
                meta.insert(
                    "shape".to_string(),
                    crate::preprocess::Value::String(resolved),
                );
            }
            meta
        });

        // The metadata of a flow goes to the flow.
        if let Some(sg) = self.subgraph_mut(id) {
            if let Some(meta) = metadata {
                sg.metadata = Some(merge(sg.metadata.take(), meta));
            }
            return Ok(());
        }
        // The metadata of a connector goes to the connector.
        if self.connectors.contains_key(id) {
            if let (Some(node), Some(meta)) = (self.node_mut(id), metadata.clone()) {
                node.metadata = Some(merge(node.metadata.take(), meta));
            }
            return Ok(());
        }
        // The id 'connectors' is reserved for the group of synthesised connectors, which
        // v0.8.1 removed but whose id stays reserved.
        if id == "connectors" {
            return Ok(());
        }
        // The metadata of an edge goes to the edge, and not to a new agent.
        if let Some(edge) = self.edges.iter_mut().find(|e| e.id.as_deref() == Some(id)) {
            if let Some(meta) = metadata {
                if let Some(crate::preprocess::Value::String(curve)) = meta.get("curve") {
                    edge.interpolate = Some(curve.clone());
                }
                edge.metadata = Some(merge(edge.metadata.take(), meta));
            }
            return Ok(());
        }

        let at = match self.node_at.get(id) {
            Some(&at) => at,
            None => {
                let at = self.nodes.len();
                self.nodes.push(Node {
                    id: id.to_string(),
                    ..Node::default()
                });
                self.node_at.insert(id.to_string(), at);
                at
            }
        };
        let node = &mut self.nodes[at];
        match text {
            Some(t) => {
                node.text = Some(strip_quotes(t.text.trim()));
                node.label_type = t.kind;
            }
            None => {
                if node.text.is_none() {
                    node.text = Some(id.to_string());
                }
            }
        }
        if let Some(shape) = shape {
            node.shape = Some(shape.to_string());
        }
        if let Some(style) = style {
            node.styles.extend(style.iter().cloned());
        }
        if let Some(classes) = classes {
            node.classes.extend(classes.iter().cloned());
        }
        if let Some(dir) = dir {
            node.dir = Some(dir.to_string());
        }
        if let Some(props) = props {
            for (key, value) in props {
                match node.props.iter_mut().find(|(k, _)| *k == key) {
                    Some(slot) => slot.1 = value,
                    None => node.props.push((key, value)),
                }
            }
        }
        let Some(meta) = metadata else {
            return Ok(());
        };
        node.metadata = Some(merge(node.metadata.take(), meta.clone()));
        // A shape that is not there at all is no shape, and Mermaid leaves the shape of the
        // agent as it is.
        let shape = match meta.get("shape") {
            Some(crate::preprocess::Value::Null) | None => None,
            Some(shape) => Some(shape),
        };
        if let Some(shape) = shape {
            let authored = match authored_shape {
                Some(authored) => authored,
                None => match shape {
                    crate::preprocess::Value::String(s) => s.clone(),
                    other => {
                        return Err(ModelError::Shape(format!(
                            "No such shape: {}.",
                            other.js_string()
                        )));
                    }
                },
            };
            if authored != authored.to_lowercase() || authored.contains('_') {
                return Err(ModelError::Shape(format!(
                    "No such shape: {authored}. Shape names should be lowercase."
                )));
            }
            let resolved = shape.js_string();
            if !is_valid_shape(&resolved) {
                return Err(ModelError::Shape(format!("No such shape: {resolved}.")));
            }
            node.shape = Some(resolved);
        }
        if let Some(label) = meta.get("label") {
            node.text = Some(strip_quotes(&label.js_string()));
        }
        if let Some(kind) = meta.get("labelType") {
            node.label_type = match kind.js_string().as_str() {
                "markdown" => LabelType::Markdown,
                "string" => LabelType::Str,
                _ => LabelType::Text,
            };
        }
        Ok(())
    }

    /// Adds a flow, with the agents it holds.
    pub(crate) fn add_subgraph(
        &mut self,
        id: Option<&str>,
        nodes: &[String],
        title: Option<&FlowText>,
        dir: Option<&str>,
    ) -> String {
        let mut id = id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string);
        let title_text = title
            .map(|t| strip_quotes(t.text.trim()))
            .unwrap_or_default();
        if id.as_deref() == Some(title_text.as_str()) && title_text.contains(char::is_whitespace) {
            id = None;
        }
        // The agents of a flow are the ones it names, without the ones it names twice.
        let mut seen = HashSet::new();
        let mut held: Vec<String> = Vec::new();
        for node in nodes {
            let node = node.trim();
            if node.is_empty() || !seen.insert(node.to_string()) {
                continue;
            }
            held.push(node.to_string());
        }
        let dir = dir
            .map(str::to_string)
            .or_else(|| self.inherit_dir.then(|| self.direction.clone()).flatten());
        let id = id.unwrap_or_else(|| format!("subGraph{}", self.sub_count));
        self.sub_count += 1;
        // A flow never holds itself, an agent every flow shares, or an agent another flow
        // already holds.
        let members = &self.members;
        let held: Vec<String> = held
            .into_iter()
            .filter(|node| *node != id)
            .filter(|node| !self.global_nodes.contains(node))
            .filter(|node| !members.contains(node))
            .collect();
        for node in &held {
            self.members.insert(node.clone());
        }
        if let Some(&at) = self.subgraph_at.get(&id) {
            // A flow that is written twice holds the agents of both. The first one keeps its
            // place, the last one wins for the title and the direction.
            let existing = &mut self.subgraphs[at];
            for node in held {
                if !existing.nodes.contains(&node) {
                    existing.nodes.push(node);
                }
            }
            if !title_text.is_empty() {
                existing.title = Some(title_text);
            }
            if let Some(dir) = dir {
                existing.dir = Some(dir);
            }
            return id;
        }
        let at = self.subgraphs.len();
        self.subgraphs.push(Subgraph {
            id: id.clone(),
            nodes: held,
            title: Some(title_text),
            label_type: title.map_or(LabelType::Text, |t| t.kind),
            classes: Vec::new(),
            dir,
            kind: "flow".to_string(),
            metadata: None,
        });
        self.subgraph_at.insert(id.clone(), at);
        id
    }

    /// Makes a set of agents shared by every flow.
    pub(crate) fn add_global(&mut self, nodes: &[String]) {
        let mut released = false;
        for node in nodes {
            let id = node.trim();
            if id.is_empty() {
                continue;
            }
            self.global_nodes.insert(id.to_string());
            if self.members.remove(id) {
                released = true;
            }
        }
        if !released {
            return;
        }
        // The flows that were read before this block claimed the agents; release them.
        for sg in &mut self.subgraphs {
            sg.nodes.retain(|node| !self.global_nodes.contains(node));
        }
    }

    /// Adds a connector: an agent that other agents point at.
    pub(crate) fn add_connector(&mut self, id: &str, title: Option<&FlowText>) -> String {
        let id = id.trim();
        if id.is_empty() {
            return String::new();
        }
        let title = title
            .map(|t| strip_quotes(t.text.trim()))
            .unwrap_or_default();
        let label = if title.is_empty() {
            id.to_string()
        } else {
            title
        };
        let at = match self.node_at.get(id) {
            Some(&at) => at,
            None => {
                let at = self.nodes.len();
                self.nodes.push(Node {
                    id: id.to_string(),
                    ..Node::default()
                });
                self.node_at.insert(id.to_string(), at);
                at
            }
        };
        let node = &mut self.nodes[at];
        node.is_connector = true;
        if !label.is_empty() {
            node.text = Some(label);
        }
        self.connectors.insert(id.to_string(), at);
        id.to_string()
    }

    /// Adds the edges between two sets of agents.
    pub(crate) fn add_link(&mut self, start: &[String], end: &[String], link: &LinkInfo) {
        let id = link.id.as_deref().map(|id| id.replacen('@', "", 1));
        let last_start = start.last().cloned().unwrap_or_default();
        let first_end = end.first().cloned().unwrap_or_default();
        for s in start {
            for e in end {
                // Only the edge from the last agent to the first one keeps the id the
                // author gave it.
                let id = if *s == last_start && *e == first_end {
                    id.clone()
                } else {
                    None
                };
                self.add_single_link(s, e, link, id);
            }
        }
    }

    /// Adds one edge.
    fn add_single_link(&mut self, start: &str, end: &str, link: &LinkInfo, id: Option<String>) {
        let mut edge = Edge {
            start: start.to_string(),
            end: end.to_string(),
            kind: None,
            stroke: None,
            length: None,
            semantic: None,
            text: String::new(),
            label_type: LabelType::Text,
            id: None,
            is_user_defined_id: false,
            interpolate: self.default_interpolate.clone(),
            style: Vec::new(),
            classes: Vec::new(),
            metadata: None,
        };
        if let Some(text) = &link.text {
            edge.text = strip_quotes(text.text.trim());
            edge.label_type = text.kind;
        }
        edge.kind = Some(link.kind);
        edge.stroke = Some(link.stroke);
        edge.length = Some(link.length.min(10));
        edge.semantic = link.semantic;
        match id {
            Some(id) if !self.edges.iter().any(|e| e.id.as_deref() == Some(&id)) => {
                edge.id = Some(id);
                edge.is_user_defined_id = true;
            }
            _ => {
                let existing = self
                    .edges
                    .iter()
                    .filter(|e| e.start == edge.start && e.end == edge.end)
                    .count();
                edge.id = Some(if existing == 0 {
                    format!("L_{}_{}_0", edge.start, edge.end)
                } else {
                    format!("L_{}_{}_{}", edge.start, edge.end, existing + 1)
                });
            }
        }
        if self.edges.len() >= self.max_edges {
            return;
        }
        self.edges.push(edge);
    }

    /// Gives agents a link.
    pub(crate) fn set_link(&mut self, ids: &str, url: &str, target: Option<&str>) {
        for id in ids.split(',') {
            let id = id.trim();
            if id.is_empty() {
                continue;
            }
            let Some(node) = self.node_mut(id) else {
                continue;
            };
            node.link = Some(url.to_string());
            node.link_target = target.map(str::to_string);
        }
    }

    /// Gives agents a tooltip.
    pub(crate) fn set_tooltip(&mut self, ids: &str, tooltip: &str) {
        for id in ids.split(',') {
            let id = id.trim();
            if id.is_empty() {
                continue;
            }
            self.tooltips.insert(id.to_string(), tooltip.to_string());
        }
    }

    /// Gives agents a callback.
    pub(crate) fn set_click_event(&mut self, ids: &str, name: &str, args: Option<&str>) {
        for id in ids.split(',') {
            let id = id.trim();
            if id.is_empty() {
                continue;
            }
            let callback = match args {
                Some(args) => format!("{name}({args})"),
                None => name.to_string(),
            };
            if let Some(node) = self.node_mut(id) {
                node.callback = Some(callback);
            }
        }
    }

    /// Puts agents in a class.
    pub(crate) fn set_class(&mut self, ids: &str, class: &str) {
        for id in ids.split(',') {
            let id = id.trim();
            if id.is_empty() {
                continue;
            }
            let Some(node) = self.node_mut(id) else {
                continue;
            };
            if !node.classes.iter().any(|c| c == class) {
                node.classes.push(class.to_string());
            }
        }
    }

    /// Adds the styles of a class.
    pub(crate) fn add_class(&mut self, ids: &str, style: &[String]) {
        // Mermaid joins the styles with a comma, then splits them on semicolons, so that a
        // style may hold a semicolon of its own.
        let joined = style.join(",");
        let parts = split_styles(&joined);
        for id in ids.split(',') {
            let id = id.trim();
            if id.is_empty() {
                continue;
            }
            if let Some(def) = self.classes.iter_mut().find(|(n, _)| n == id) {
                def.1.styles.extend(parts.iter().cloned());
            } else {
                self.classes.push((
                    id.to_string(),
                    ClassDef {
                        styles: parts.to_vec(),
                        text_styles: Vec::new(),
                    },
                ));
            }
        }
    }

    /// Gives edges the styles of a `linkStyle` statement.
    pub(crate) fn update_link(&mut self, positions: &[String], style: &[String]) {
        for position in positions {
            if position == "default" {
                self.default_edge_style = style.to_vec();
                continue;
            }
            let Ok(at) = position.parse::<usize>() else {
                continue;
            };
            let Some(edge) = self.edges.get_mut(at) else {
                continue;
            };
            edge.style = style.to_vec();
            if !edge.style.iter().any(|s| s.starts_with("fill")) {
                edge.style.push("fill:none".to_string());
            }
        }
    }

    /// Gives edges the curve of a `linkStyle` statement.
    pub(crate) fn update_link_interpolate(&mut self, positions: &[String], interpolate: &str) {
        for position in positions {
            if position == "default" {
                self.default_interpolate = Some(interpolate.to_string());
                continue;
            }
            let Ok(at) = position.parse::<usize>() else {
                continue;
            };
            if let Some(edge) = self.edges.get_mut(at) {
                edge.interpolate = Some(interpolate.to_string());
            }
        }
    }

    /// Sets the direction of the diagram, the way Mermaid reads it.
    pub(crate) fn set_direction(&mut self, dir: &str) {
        let mut dir = dir.trim().to_string();
        if dir.contains('<') {
            dir = "RL".to_string();
        }
        if dir.contains('^') {
            dir = "BT".to_string();
        }
        if dir.contains('>') {
            dir = "LR".to_string();
        }
        if dir.contains('v') {
            dir = "TB".to_string();
        }
        if dir == "TD" {
            dir = "TB".to_string();
        }
        self.direction = Some(dir);
    }

    /// Gives an agent, a flow, a connector or an edge the metadata of an `@{ … }` block.
    pub(crate) fn add_metadata(
        &mut self,
        id: &str,
        metadata: Option<Metadata>,
    ) -> Result<(), ModelError> {
        self.add_vertex(id, None, None, None, None, None, None, metadata)
    }

    /// Records a problem with the diagram.
    pub(crate) fn emit(&mut self, id: &'static str, message: String, node_id: Option<String>) {
        if let Some(node_id) = &node_id {
            if self
                .diagnostics
                .iter()
                .any(|d| d.id == id && d.node_id.as_deref() == Some(node_id.as_str()))
            {
                return;
            }
        }
        self.diagnostics.push(Diagnostic {
            id,
            message,
            node_id,
        });
    }
}

/// Merges two sets of metadata, the later one winning.
fn merge(into: Option<Metadata>, from: Metadata) -> Metadata {
    let mut into = into.unwrap_or_default();
    for (key, value) in from {
        into.insert(key, value);
    }
    into
}

/// Strips the quotes a label is written with.
fn strip_quotes(text: &str) -> String {
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        text[1..text.len() - 1].to_string()
    } else {
        text.to_string()
    }
}

/// Splits a style list on semicolons, the way Mermaid does.
fn split_styles(joined: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = joined.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&',') {
            current.push(',');
            chars.next();
        } else if c == ';' {
            out.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(c);
        }
    }
    out.push(current.trim().to_string());
    out
}

/// Whether an agent is a tool, which v0.8.1 reads off the shape it was written with.
fn is_tool_definition(node: &Node) -> bool {
    const SUBROUTINE_ALIASES: &[&str] = &[
        "subroutine",
        "subprocess",
        "subproc",
        "framed-rectangle",
        "tool",
    ];
    node.shape
        .as_deref()
        .is_some_and(|shape| SUBROUTINE_ALIASES.contains(&shape))
}

/// What a link says: the arrow it ends in, the line it is drawn with and its label.
#[derive(Debug, Clone)]
pub(crate) struct LinkInfo {
    pub kind: EdgeKind,
    pub stroke: Stroke,
    pub length: i64,
    pub semantic: Option<EdgeSemantic>,
    pub text: Option<FlowText>,
    pub id: Option<String>,
}

/// Reads a link the way Mermaid's `destructLink` does.
pub(crate) fn destruct_link(end: &str, start: Option<&str>) -> LinkInfo {
    let (kind, stroke, length) = destruct_end_link(end);
    let Some(start) = start else {
        return LinkInfo {
            kind,
            stroke,
            length,
            semantic: edge_semantic(kind, stroke),
            text: None,
            id: None,
        };
    };
    let (_, start_stroke) = destruct_start_link(start);
    if start_stroke != stroke {
        return LinkInfo {
            kind: EdgeKind::Invalid,
            stroke: Stroke::Invalid,
            length: 0,
            semantic: None,
            text: None,
            id: None,
        };
    }
    LinkInfo {
        kind,
        stroke: start_stroke,
        length,
        semantic: edge_semantic(kind, start_stroke),
        text: None,
        id: None,
    }
}

/// Reads the arrow at the end of a link.
fn destruct_end_link(text: &str) -> (EdgeKind, Stroke, i64) {
    let str = text.trim();
    let mut line = &str[..str.len().saturating_sub(1)];
    let mut kind = EdgeKind::ArrowOpen;
    match str.chars().last() {
        Some('x') => kind = EdgeKind::ArrowCross,
        Some('>') => kind = EdgeKind::ArrowPoint,
        Some('-') | Some('.') => {
            // The `-.-` form: the operator is not directional, so no arrowhead is drawn.
            kind = EdgeKind::ArrowOpen;
            line = str;
        }
        _ => {}
    }
    let mut stroke = Stroke::Normal;
    let mut length = line.chars().count() as i64 - 1;
    let dots = line.chars().filter(|&c| c == '.').count() as i64;
    if dots > 0 {
        stroke = Stroke::Dotted;
        length = dots;
    }
    (kind, stroke, length)
}

/// Reads the half of a split link that comes before the label.
fn destruct_start_link(text: &str) -> (EdgeKind, Stroke) {
    let str = text.trim();
    let stroke = if str.contains('.') {
        Stroke::Dotted
    } else {
        Stroke::Normal
    };
    (EdgeKind::ArrowOpen, stroke)
}

/// What an edge means, as the agentflow syntax defines it.
fn edge_semantic(kind: EdgeKind, stroke: Stroke) -> Option<EdgeSemantic> {
    match (kind, stroke) {
        (EdgeKind::ArrowPoint, Stroke::Normal) => Some(EdgeSemantic::Sequence),
        (EdgeKind::ArrowCross, Stroke::Normal) => Some(EdgeSemantic::Failure),
        (EdgeKind::ArrowOpen, Stroke::Dotted) => Some(EdgeSemantic::Reference),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_kinds_of_link() {
        let seq = destruct_link(" --> ", None);
        assert_eq!(seq.kind, EdgeKind::ArrowPoint);
        assert_eq!(seq.stroke, Stroke::Normal);
        assert_eq!(seq.length, 1);
        assert_eq!(seq.semantic, Some(EdgeSemantic::Sequence));

        let fail = destruct_link(" --x ", None);
        assert_eq!(fail.kind, EdgeKind::ArrowCross);
        assert_eq!(fail.semantic, Some(EdgeSemantic::Failure));

        let reference = destruct_link(" -.- ", None);
        assert_eq!(reference.kind, EdgeKind::ArrowOpen);
        assert_eq!(reference.stroke, Stroke::Dotted);
        assert_eq!(reference.semantic, Some(EdgeSemantic::Reference));
    }

    #[test]
    fn a_split_link_takes_its_arrow_from_the_end() {
        let link = destruct_link(" --> ", Some(" -- "));
        assert_eq!(link.kind, EdgeKind::ArrowPoint);
        assert_eq!(link.stroke, Stroke::Normal);
        assert_eq!(link.semantic, Some(EdgeSemantic::Sequence));
    }
}
