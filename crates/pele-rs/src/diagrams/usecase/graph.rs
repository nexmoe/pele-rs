//! What a use case diagram is drawn as: every actor, use case, note, JSON table and boundary as
//! a node with its class styles, and every relationship and note connector as an edge with its
//! markers. Ports `getData` from Mermaid's use case database.

use super::db::{ActorType, Animation, BoundaryType, LabelType, RelationshipType, UsecaseModel};
use super::json::{JsonRow, flatten_rows};

/// The shape a node is drawn as, in Mermaid's names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphShape {
    UsecaseActor,
    UsecaseActorHollow,
    UsecaseActorAwesome,
    UsecaseActorIcon,
    UsecaseEllipse,
    UsecaseBusiness,
    Rect,
    Note,
    UsecaseJsonTable,
    UsecaseSystemBoundary,
}

impl GraphShape {
    #[allow(dead_code, reason = "the golden tests read the name")]
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            GraphShape::UsecaseActor => "usecaseActor",
            GraphShape::UsecaseActorHollow => "usecaseActorHollow",
            GraphShape::UsecaseActorAwesome => "usecaseActorAwesome",
            GraphShape::UsecaseActorIcon => "usecaseActorIcon",
            GraphShape::UsecaseEllipse => "usecaseEllipse",
            GraphShape::UsecaseBusiness => "usecaseBusiness",
            GraphShape::Rect => "rect",
            GraphShape::Note => "note",
            GraphShape::UsecaseJsonTable => "usecaseJsonTable",
            GraphShape::UsecaseSystemBoundary => "usecaseSystemBoundary",
        }
    }
}

/// What an edge means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EdgeKind {
    Association,
    Include,
    Extend,
    Generalization,
    Note,
}

impl EdgeKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            EdgeKind::Association => "association",
            EdgeKind::Include => "include",
            EdgeKind::Extend => "extend",
            EdgeKind::Generalization => "generalization",
            EdgeKind::Note => "note",
        }
    }
}

/// How an edge is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pattern {
    Solid,
    Dotted,
}

impl Pattern {
    #[allow(dead_code, reason = "the golden tests read the name")]
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Pattern::Solid => "solid",
            Pattern::Dotted => "dotted",
        }
    }
}

/// A node of the drawing.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UsecaseGraphNode {
    pub id: String,
    pub label: String,
    pub label_type: LabelType,
    pub shape: GraphShape,
    pub is_group: bool,
    pub padding: f64,
    pub css_classes: String,
    pub css_styles: Vec<String>,
    pub css_compiled_styles: Vec<String>,
    pub actor_type: Option<ActorType>,
    pub business: Option<bool>,
    pub icon: Option<String>,
    pub stereotype: Option<String>,
    pub parent_id: Option<String>,
    pub note_target: Option<String>,
    pub json_rows: Option<Vec<JsonRow>>,
    pub boundary_type: Option<BoundaryType>,
}

/// An edge of the drawing.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UsecaseGraphEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    pub relationship_type: EdgeKind,
    pub pattern: Pattern,
    pub arrow_type_start: String,
    pub arrow_type_end: String,
    pub label: Option<String>,
    pub label_type: Option<LabelType>,
    pub classes: String,
    pub style: Vec<String>,
    pub css_compiled_styles: Vec<String>,
    pub animate: bool,
    pub animation: Option<Animation>,
    pub minlen: i32,
    pub explicit_id: bool,
    /// The dotted line from a note to what it is about.
    pub internal: bool,
}

/// The drawing of a use case diagram.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct UsecaseGraph {
    pub nodes: Vec<UsecaseGraphNode>,
    pub edges: Vec<UsecaseGraphEdge>,
}

/// Start and end markers for each arrow type, in the order of Mermaid's `Arrow` enum.
const MARKERS: [&[&str; 2]; 7] = [
    &["none", "arrow_point"],
    &["arrow_point", "none"],
    &["none", "none"],
    &["none", "arrow_circle"],
    &["none", "arrow_cross"],
    &["arrow_circle", "none"],
    &["arrow_cross", "none"],
];

fn class_list(names: &[&str]) -> String {
    names
        .iter()
        .filter(|name| !name.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The markers of a relationship.
fn markers(relationship_type: RelationshipType, arrow_type: u8) -> (&'static str, &'static str) {
    if relationship_type == RelationshipType::Generalization {
        return ("none", "extension");
    }
    if relationship_type != RelationshipType::Association {
        return (MARKERS[0][0], MARKERS[0][1]);
    }
    let markers = MARKERS
        .get(arrow_type as usize)
        .copied()
        .unwrap_or(MARKERS[2]);
    (markers[0], markers[1])
}

/// Resolves a parsed use case diagram into what is drawn.
pub(crate) fn build_usecase_graph(model: &UsecaseModel) -> UsecaseGraph {
    let mut nodes: Vec<UsecaseGraphNode> = Vec::new();
    let mut edges: Vec<UsecaseGraphEdge> = Vec::new();

    // A later class wins a property but keeps the place the property first took.
    let compiled = |names: &[String]| -> Vec<String> {
        let mut out: Vec<(String, String)> = Vec::new();
        let mut all = vec!["default".to_string()];
        all.extend(names.iter().cloned());
        for name in all {
            let Some(definition) = model.class_defs.get(&name) else {
                continue;
            };
            for raw in &definition.styles {
                let style = raw.trim().to_string();
                let property = match style.find(':') {
                    None => style.clone(),
                    Some(at) => style[..at].trim().to_string(),
                };
                if property.is_empty() {
                    continue;
                }
                match out.iter_mut().find(|(key, _)| *key == property) {
                    Some(entry) => entry.1 = style,
                    None => out.push((property, style)),
                }
            }
        }
        out.into_iter().map(|(_, style)| style).collect()
    };

    for actor in model.actors.values() {
        let shape = match actor.actor_type {
            ActorType::Normal => GraphShape::UsecaseActor,
            ActorType::Hollow => GraphShape::UsecaseActorHollow,
            ActorType::Awesome => GraphShape::UsecaseActorAwesome,
            ActorType::Icon => GraphShape::UsecaseActorIcon,
        };
        let mut classes = vec!["default", "usecase-actor"];
        classes.push(match actor.actor_type {
            ActorType::Normal => "usecase-actor-normal",
            ActorType::Hollow => "usecase-actor-hollow",
            ActorType::Awesome => "usecase-actor-awesome",
            ActorType::Icon => "usecase-actor-icon",
        });
        if actor.business {
            classes.push("usecase-business");
        }
        classes.extend(actor.classes.iter().map(String::as_str));
        nodes.push(UsecaseGraphNode {
            id: actor.id.clone(),
            label: actor.label.clone(),
            label_type: actor.label_type,
            shape,
            is_group: false,
            padding: 10.0,
            css_classes: class_list(&classes),
            css_styles: actor.styles.clone(),
            css_compiled_styles: compiled(&actor.classes),
            actor_type: Some(actor.actor_type),
            business: Some(actor.business),
            icon: actor.icon.clone(),
            stereotype: actor.stereotype.clone(),
            parent_id: actor.parent_id.clone(),
            note_target: None,
            json_rows: None,
            boundary_type: None,
        });
    }
    for use_case in model.use_cases.values() {
        let ellipse = use_case.shape == super::db::UseCaseShape::Ellipse;
        let mut classes = vec!["default", "usecase-element"];
        classes.push(match use_case.shape {
            super::db::UseCaseShape::Ellipse => "usecase-ellipse",
            super::db::UseCaseShape::Rect => "usecase-rect",
        });
        if use_case.business {
            classes.push("usecase-business");
        }
        classes.extend(use_case.classes.iter().map(String::as_str));
        nodes.push(UsecaseGraphNode {
            id: use_case.id.clone(),
            label: use_case.label.clone(),
            label_type: use_case.label_type,
            shape: if !ellipse {
                GraphShape::Rect
            } else if use_case.business {
                GraphShape::UsecaseBusiness
            } else {
                GraphShape::UsecaseEllipse
            },
            is_group: false,
            padding: if ellipse { 20.0 } else { 10.0 },
            css_classes: class_list(&classes),
            css_styles: use_case.styles.clone(),
            css_compiled_styles: compiled(&use_case.classes),
            actor_type: None,
            business: Some(use_case.business),
            icon: None,
            stereotype: use_case.stereotype.clone(),
            parent_id: use_case.parent_id.clone(),
            note_target: None,
            json_rows: None,
            boundary_type: None,
        });
    }
    for note in model.notes.values() {
        nodes.push(UsecaseGraphNode {
            id: note.id.clone(),
            label: note.label.clone(),
            label_type: note.label_type,
            shape: GraphShape::Note,
            is_group: false,
            padding: 10.0,
            css_classes: class_list(&["default", "usecase-note"]),
            css_styles: Vec::new(),
            css_compiled_styles: compiled(&[]),
            actor_type: None,
            business: None,
            icon: None,
            stereotype: None,
            parent_id: None,
            note_target: Some(note.target.clone()),
            json_rows: None,
            boundary_type: None,
        });
    }
    for json in model.json_nodes.values() {
        let mut classes = vec!["default", "usecase-json-table"];
        classes.extend(json.classes.iter().map(String::as_str));
        nodes.push(UsecaseGraphNode {
            id: json.id.clone(),
            label: json.id.clone(),
            label_type: LabelType::Text,
            shape: GraphShape::UsecaseJsonTable,
            is_group: false,
            padding: 10.0,
            css_classes: class_list(&classes),
            css_styles: json.styles.clone(),
            css_compiled_styles: compiled(&json.classes),
            actor_type: None,
            business: None,
            icon: None,
            stereotype: None,
            parent_id: None,
            note_target: None,
            json_rows: Some(flatten_rows(&json.value)),
            boundary_type: None,
        });
    }
    for boundary in model.system_boundaries.values() {
        let mut classes = vec!["default", "system-boundary"];
        classes.push(match boundary.boundary_type {
            BoundaryType::Rect => "system-boundary-rect",
            BoundaryType::Package => "system-boundary-package",
        });
        classes.extend(boundary.classes.iter().map(String::as_str));
        nodes.push(UsecaseGraphNode {
            id: boundary.id.clone(),
            label: boundary.label.clone(),
            label_type: boundary.label_type,
            shape: GraphShape::UsecaseSystemBoundary,
            is_group: true,
            padding: 20.0,
            css_classes: class_list(&classes),
            css_styles: boundary.styles.clone(),
            css_compiled_styles: compiled(&boundary.classes),
            actor_type: None,
            business: None,
            icon: None,
            stereotype: None,
            parent_id: None,
            note_target: None,
            json_rows: None,
            boundary_type: Some(boundary.boundary_type),
        });
    }

    for relationship in &model.relationships {
        let semantic = matches!(
            relationship.relationship_type,
            RelationshipType::Include | RelationshipType::Extend
        );
        let (arrow_type_start, arrow_type_end) =
            markers(relationship.relationship_type, relationship.arrow_type);
        let animated = relationship.animate || relationship.animation.is_some();
        let mut classes = vec!["default", "relationship"];
        classes.push(match relationship.relationship_type {
            RelationshipType::Association => "relationship-association",
            RelationshipType::Include => "relationship-include",
            RelationshipType::Extend => "relationship-extend",
            RelationshipType::Generalization => "relationship-generalization",
        });
        classes.extend(relationship.classes.iter().map(String::as_str));
        if animated {
            classes.push(match relationship.animation {
                Some(Animation::Slow) => "edge-animation-slow",
                _ => "edge-animation-fast",
            });
        }
        let mut edge = UsecaseGraphEdge {
            id: relationship.id.clone(),
            start: relationship.source.clone(),
            end: relationship.target.clone(),
            relationship_type: match relationship.relationship_type {
                RelationshipType::Association => EdgeKind::Association,
                RelationshipType::Include => EdgeKind::Include,
                RelationshipType::Extend => EdgeKind::Extend,
                RelationshipType::Generalization => EdgeKind::Generalization,
            },
            pattern: if semantic {
                Pattern::Dotted
            } else {
                Pattern::Solid
            },
            arrow_type_start: arrow_type_start.to_string(),
            arrow_type_end: arrow_type_end.to_string(),
            label: None,
            label_type: None,
            classes: class_list(&classes),
            style: relationship.styles.clone(),
            css_compiled_styles: compiled(&relationship.classes),
            animate: relationship.animate,
            animation: relationship.animation,
            minlen: relationship.minlen,
            explicit_id: relationship.explicit_id,
            internal: false,
        };
        // Include and extend are labelled with their kind, whatever label the relationship
        // carries.
        if semantic {
            edge.label = Some(relationship.relationship_type.as_str().to_string());
            edge.label_type = Some(LabelType::Text);
        } else if relationship.relationship_type == RelationshipType::Association {
            if let Some(label) = &relationship.label {
                edge.label = Some(label.clone());
            }
            if let Some(label_type) = relationship.label_type {
                edge.label_type = Some(label_type);
            }
        }
        edges.push(edge);
    }
    for note in model.notes.values() {
        edges.push(UsecaseGraphEdge {
            id: format!("{}-edge", note.id),
            start: note.id.clone(),
            end: note.target.clone(),
            relationship_type: EdgeKind::Note,
            pattern: Pattern::Dotted,
            arrow_type_start: "none".to_string(),
            arrow_type_end: "none".to_string(),
            label: None,
            label_type: None,
            classes: class_list(&["default", "relationship", "relationship-note"]),
            style: Vec::new(),
            css_compiled_styles: compiled(&[]),
            animate: false,
            animation: None,
            minlen: 1,
            explicit_id: false,
            internal: true,
        });
    }
    UsecaseGraph { nodes, edges }
}
