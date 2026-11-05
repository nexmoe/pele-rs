//! The model of a Wardley map.
//!
//! Mermaid fills its database the way `populateDb` does: the size first, then the stages of the
//! evolution axis, then the anchors, the components, the notes, the pipelines, the links, the
//! evolutions, the annotations and the forces. The database holds the nodes in the order they
//! were added, and a link names a node by its id, or by the label of a node when no node has
//! that id.

use crate::diagrams::common::ordered::Ordered;
use crate::diagrams::wardley::parser::{Ast, LabelOffset, Stage};
use crate::error::{Error, ErrorCode};

/// Which way a link points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flow {
    Forward,
    Backward,
    Bidirectional,
}

/// A node of the map.
#[derive(Debug, Clone)]
pub(crate) struct WardleyNode {
    pub id: String,
    pub label: String,
    /// How far along the evolution axis the node is, in percent.
    pub x: f64,
    /// How high up the visibility axis the node is, in percent.
    pub y: f64,
    /// What the node is drawn as: an `anchor`, a `component`, a `pipeline-parent` or a
    /// `pipeline-component`.
    pub class_name: &'static str,
    pub label_offset_x: Option<f64>,
    pub label_offset_y: Option<f64>,
    /// Whether the component is part of a pipeline. Mermaid leaves it unset on a node that is
    /// not part of one.
    pub in_pipeline: Option<bool>,
    /// Whether the node is the parent of a pipeline.
    pub is_pipeline_parent: Option<bool>,
    /// Whether the component holds its position. Mermaid leaves it unset on a node that is not
    /// a component.
    pub inertia: Option<bool>,
    pub source_strategy: Option<String>,
}

/// A link between two nodes.
#[derive(Debug, Clone)]
pub(crate) struct WardleyLink {
    pub source: String,
    pub target: String,
    pub dashed: bool,
    pub label: Option<String>,
    pub flow: Option<Flow>,
}

/// Where a component is heading.
#[derive(Debug, Clone)]
pub(crate) struct WardleyTrend {
    pub node_id: String,
    pub target_x: f64,
    pub target_y: f64,
}

/// A set of components under a parent.
#[derive(Debug, Clone)]
pub(crate) struct WardleyPipeline {
    pub node_id: String,
    pub component_ids: Vec<String>,
}

/// An annotation of the map.
#[derive(Debug, Clone)]
pub(crate) struct WardleyAnnotation {
    pub number: f64,
    pub coordinates: Vec<(f64, f64)>,
    pub text: Option<String>,
}

/// A note on the map.
#[derive(Debug, Clone)]
pub(crate) struct WardleyNote {
    pub text: String,
    pub x: f64,
    pub y: f64,
}

/// An accelerator or a deaccelerator.
#[derive(Debug, Clone)]
pub(crate) struct WardleyForce {
    pub name: String,
    pub x: f64,
    pub y: f64,
}

/// The axes of the map.
#[derive(Debug, Clone, Default)]
pub(crate) struct Axes {
    /// The names of the stages of the evolution axis.
    pub stages: Option<Vec<String>>,
    /// Where the stages of the evolution axis end, when the diagram names them.
    pub stage_boundaries: Option<Vec<f64>>,
    /// The name of the axis of the evolution. Mermaid never sets it.
    pub x_label: Option<String>,
    /// The name of the axis of the visibility. Mermaid never sets it.
    pub y_label: Option<String>,
}

/// The size of the canvas.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WardleySize {
    pub width: f64,
    pub height: f64,
}

/// The diagram, as Mermaid's database holds it.
#[derive(Debug, Clone, Default)]
pub(crate) struct WardleyModel {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// The nodes, in the order they were added.
    pub nodes: Ordered<WardleyNode>,
    pub links: Vec<WardleyLink>,
    pub trends: Vec<WardleyTrend>,
    pub pipelines: Vec<WardleyPipeline>,
    pub annotations: Vec<WardleyAnnotation>,
    pub notes: Vec<WardleyNote>,
    pub accelerators: Vec<WardleyForce>,
    pub deaccelerators: Vec<WardleyForce>,
    pub annotations_box: Option<(f64, f64)>,
    pub axes: Axes,
    pub size: Option<WardleySize>,
}

/// A value up to 1 is a fraction; a larger one is already a percentage.
fn to_percent(value: f64, context: &str) -> Result<f64, Error> {
    let normalized = if value <= 1.0 { value * 100.0 } else { value };
    if !(0.0..=100.0).contains(&normalized) {
        return Err(Error::new(
            ErrorCode::Semantic,
            format!(
                "{context} must be between 0-1 (decimal) or 0-100 (percentage). Received: {value}"
            ),
        ));
    }
    Ok(normalized)
}

/// The place of a node on the map: the evolution is the width, the visibility the height.
fn to_coordinates(visibility: f64, evolution: f64, context: &str) -> Result<(f64, f64), Error> {
    Ok((
        to_percent(evolution, &format!("{context} evolution"))?,
        to_percent(visibility, &format!("{context} visibility"))?,
    ))
}

/// The flow a port of a link asks for.
fn flow_from_port(port: Option<&str>) -> Option<Flow> {
    match port {
        Some("+<>") => Some(Flow::Bidirectional),
        Some("+<") => Some(Flow::Backward),
        Some("+>") => Some(Flow::Forward),
        _ => None,
    }
}

/// The flow and the label an arrow of a link asks for.
fn flow_from_arrow(arrow: Option<&str>) -> (Option<Flow>, Option<String>) {
    let Some(arrow) = arrow.filter(|arrow| arrow.starts_with('+')) else {
        return (None, None);
    };
    let label = arrow
        .strip_prefix("+'")
        .and_then(|rest| rest.find('\'').map(|end| rest[..end].to_string()));
    let flow = if arrow.contains("<>") {
        Some(Flow::Bidirectional)
    } else if arrow.contains('<') {
        Some(Flow::Backward)
    } else if arrow.contains('>') {
        Some(Flow::Forward)
    } else {
        None
    };
    (flow, label)
}

/// The offset of the label of a node from the node.
fn offsets(label: Option<&LabelOffset>) -> (Option<f64>, Option<f64>) {
    label.map_or((None, None), |label| {
        (
            Some(if label.neg_x {
                -label.offset_x
            } else {
                label.offset_x
            }),
            Some(if label.neg_y {
                -label.offset_y
            } else {
                label.offset_y
            }),
        )
    })
}

/// A node as it is given to the database.
struct NodeIn {
    id: String,
    label: String,
    x: f64,
    y: f64,
    class_name: &'static str,
    label_offset_x: Option<f64>,
    label_offset_y: Option<f64>,
    inertia: Option<bool>,
    source_strategy: Option<String>,
}

/// The database Mermaid fills.
#[derive(Default)]
struct Db {
    nodes: Ordered<WardleyNode>,
    links: Vec<WardleyLink>,
    trends: Ordered<WardleyTrend>,
    pipelines: Ordered<WardleyPipeline>,
    annotations: Vec<WardleyAnnotation>,
    notes: Vec<WardleyNote>,
    accelerators: Vec<WardleyForce>,
    deaccelerators: Vec<WardleyForce>,
    annotations_box: Option<(f64, f64)>,
    axes: Axes,
    size: Option<WardleySize>,
}

impl Db {
    /// Adds a node, or replaces the one with the same id, keeping the place it had.
    fn add_node(&mut self, node: NodeIn) {
        let merged = match self.nodes.get(&node.id) {
            None => WardleyNode {
                id: node.id.clone(),
                label: node.label.clone(),
                x: node.x,
                y: node.y,
                class_name: node.class_name,
                label_offset_x: node.label_offset_x,
                label_offset_y: node.label_offset_y,
                in_pipeline: None,
                is_pipeline_parent: None,
                inertia: node.inertia,
                source_strategy: node.source_strategy,
            },
            Some(existing) => WardleyNode {
                id: node.id.clone(),
                label: node.label.clone(),
                x: node.x,
                y: node.y,
                class_name: node.class_name,
                label_offset_x: node.label_offset_x.or(existing.label_offset_x),
                label_offset_y: node.label_offset_y.or(existing.label_offset_y),
                in_pipeline: existing.in_pipeline,
                is_pipeline_parent: existing.is_pipeline_parent,
                inertia: node.inertia,
                source_strategy: node.source_strategy,
            },
        };
        self.nodes.set(&node.id, merged);
    }

    /// The node with that id.
    fn node(&self, id: &str) -> Option<&WardleyNode> {
        self.nodes.get(id)
    }

    /// The id of the node a name stands for: the id itself, or the id of the node whose label
    /// is that name.
    fn resolve_node_id(&self, name: &str) -> String {
        if self.nodes.get(name).is_some() {
            return name.to_string();
        }
        self.nodes
            .items
            .iter()
            .find(|(_, node)| node.label == name)
            .map_or_else(|| name.to_string(), |(id, _)| id.clone())
    }

    /// Starts a pipeline under a node.
    fn start_pipeline(&mut self, node_id: &str) {
        self.pipelines.set(
            node_id,
            WardleyPipeline {
                node_id: node_id.to_string(),
                component_ids: Vec::new(),
            },
        );
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.is_pipeline_parent = Some(true);
        }
    }

    /// Adds a component to a pipeline.
    fn add_pipeline_component(&mut self, pipeline_node_id: &str, component_id: &str) {
        if let Some(pipeline) = self.pipelines.get_mut(pipeline_node_id) {
            pipeline.component_ids.push(component_id.to_string());
        }
        if let Some(node) = self.nodes.get_mut(component_id) {
            node.in_pipeline = Some(true);
        }
    }
}

/// Fills the model from the syntax tree.
///
/// The title of the front matter is the title of the diagram, unless the diagram has one of its
/// own.
pub(crate) fn build_wardley(ast: &Ast, title: Option<&str>) -> Result<WardleyModel, Error> {
    let mut db = Db::default();

    if let Some(size) = ast.size {
        db.size = Some(WardleySize {
            width: size.width,
            height: size.height,
        });
    }

    if let Some(stages) = &ast.stages {
        db.axes = Axes {
            stages: Some(stage_names(stages)),
            stage_boundaries: Some(
                stages
                    .iter()
                    .filter_map(|stage| stage.boundary)
                    .collect::<Vec<_>>(),
            ),
            ..Axes::default()
        };
    }

    for anchor in &ast.anchors {
        let (x, y) = to_coordinates(
            anchor.visibility,
            anchor.evolution,
            &format!("Anchor \"{}\"", anchor.name),
        )?;
        db.add_node(NodeIn {
            id: anchor.name.clone(),
            label: anchor.name.clone(),
            x,
            y,
            class_name: "anchor",
            label_offset_x: None,
            label_offset_y: None,
            inertia: None,
            source_strategy: None,
        });
    }

    for component in &ast.components {
        let (x, y) = to_coordinates(
            component.visibility,
            component.evolution,
            &format!("Component \"{}\"", component.name),
        )?;
        let (label_offset_x, label_offset_y) = offsets(component.label.as_ref());
        db.add_node(NodeIn {
            id: component.name.clone(),
            label: component.name.clone(),
            x,
            y,
            class_name: "component",
            label_offset_x,
            label_offset_y,
            inertia: Some(component.inertia),
            source_strategy: component.strategy.clone(),
        });
    }

    for note in &ast.notes {
        let (x, y) = to_coordinates(
            note.visibility,
            note.evolution,
            &format!("Note \"{}\"", note.text),
        )?;
        db.notes.push(WardleyNote {
            text: note.text.clone(),
            x,
            y,
        });
    }

    for pipeline in &ast.pipelines {
        let Some(parent) = db.node(&pipeline.parent) else {
            return Err(Error::new(
                ErrorCode::Semantic,
                format!(
                    "Pipeline \"{}\" must reference an existing component with coordinates.",
                    pipeline.parent
                ),
            ));
        };
        let parent_y = parent.y;
        db.start_pipeline(&pipeline.parent);
        for component in &pipeline.components {
            let id = format!("{}_{}", pipeline.parent, component.name);
            let (label_offset_x, label_offset_y) = offsets(component.label.as_ref());
            let x = to_percent(
                component.evolution,
                &format!("Pipeline component \"{}\" evolution", component.name),
            )?;
            db.add_node(NodeIn {
                id: id.clone(),
                label: component.name.clone(),
                x,
                y: parent_y,
                class_name: "pipeline-component",
                label_offset_x,
                label_offset_y,
                inertia: None,
                source_strategy: None,
            });
            db.add_pipeline_component(&pipeline.parent, &id);
        }
    }

    for link in &ast.links {
        let dashed = link
            .arrow
            .as_deref()
            .is_some_and(|arrow| arrow.contains("-.->") || arrow.contains(".-."));
        let (arrow_flow, flow_label) = flow_from_arrow(link.arrow.as_deref());
        let flow = flow_from_port(link.from_port.as_deref())
            .or_else(|| flow_from_port(link.to_port.as_deref()))
            .or(arrow_flow);
        db.links.push(WardleyLink {
            source: db.resolve_node_id(&link.from),
            target: db.resolve_node_id(&link.to),
            dashed,
            label: flow_label.or_else(|| link.link_label.clone()),
            flow,
        });
    }

    for evolve in &ast.evolves {
        let Some(node) = db.node(&evolve.component) else {
            continue;
        };
        let target = to_percent(
            evolve.target,
            &format!("Evolve target for \"{}\"", evolve.component),
        )?;
        let target_y = node.y;
        db.trends.set(
            &evolve.component,
            WardleyTrend {
                node_id: evolve.component.clone(),
                target_x: target,
                target_y,
            },
        );
    }

    if let Some(annotations) = ast.annotations.first() {
        let (x, y) = to_coordinates(annotations.x, annotations.y, "Annotations box")?;
        db.annotations_box = Some((x, y));
    }

    for annotation in &ast.annotation {
        let (x, y) = to_coordinates(
            annotation.x,
            annotation.y,
            &format!("Annotation {}", annotation.number),
        )?;
        db.annotations.push(WardleyAnnotation {
            number: annotation.number,
            coordinates: vec![(x, y)],
            text: Some(annotation.text.clone()),
        });
    }

    for accelerator in &ast.accelerators {
        let (x, y) = to_coordinates(
            accelerator.x,
            accelerator.y,
            &format!("Accelerator \"{}\"", accelerator.name),
        )?;
        db.accelerators.push(WardleyForce {
            name: accelerator.name.clone(),
            x,
            y,
        });
    }

    for deaccelerator in &ast.deaccelerators {
        let (x, y) = to_coordinates(
            deaccelerator.x,
            deaccelerator.y,
            &format!("Deaccelerator \"{}\"", deaccelerator.name),
        )?;
        db.deaccelerators.push(WardleyForce {
            name: deaccelerator.name.clone(),
            x,
            y,
        });
    }

    Ok(WardleyModel {
        title: ast
            .title
            .as_deref()
            .filter(|title| !title.is_empty())
            .map(str::to_string)
            .or_else(|| title.map(str::to_string)),
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
        nodes: db.nodes,
        links: db.links,
        trends: db
            .trends
            .items
            .into_iter()
            .map(|(_, trend)| trend)
            .collect(),
        pipelines: db
            .pipelines
            .items
            .into_iter()
            .map(|(_, pipeline)| pipeline)
            .collect(),
        annotations: db.annotations,
        notes: db.notes,
        accelerators: db.accelerators,
        deaccelerators: db.deaccelerators,
        annotations_box: db.annotations_box,
        axes: db.axes,
        size: db.size,
    })
}

/// The names of the stages of the evolution axis. A stage with a second name is written with
/// both of them.
fn stage_names(stages: &[Stage]) -> Vec<String> {
    stages
        .iter()
        .map(|stage| match &stage.second_name {
            Some(second) => format!("{} / {}", stage.name.trim(), second.trim()),
            None => stage.name.trim().to_string(),
        })
        .collect()
}
