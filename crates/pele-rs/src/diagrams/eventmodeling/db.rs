//! The model of an event modeling diagram: the frames in their lanes, the relations between
//! them, and the notes and specifications that hang off them. Ported from pele's
//! `diagrams/eventmodeling/model.ts`, which is a port of Mermaid's `db.ts`.
//!
//! One thing differs from Mermaid's `db.ts`, and pele has it right: Mermaid finds the lane of
//! a frame by its namespace whatever the kind of the frame is, so a namespace that is used by a
//! command and by an event shares one lane there, and the event is drawn in the lane of the
//! commands. Mermaid's documentation says the opposite ("Each combination of the Namespace and
//! Entity type forms new Swimlane"), so every namespace has a lane of its own in each group,
//! which is what pele-rs does.

use std::collections::HashSet;

use crate::error::{Error, ErrorCode, locate};
use crate::util::{is_space, trim, trim_end};

use super::parser::{EntityKind, EventModelAst, FrameKind};

/// The group of lanes a frame belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LaneGroup {
    Trigger,
    State,
    Stream,
}

impl LaneGroup {
    /// The name the summary uses.
    pub(crate) fn name(self) -> &'static str {
        match self {
            LaneGroup::Trigger => "trigger",
            LaneGroup::State => "state",
            LaneGroup::Stream => "stream",
        }
    }

    fn index(self) -> usize {
        match self {
            LaneGroup::Trigger => 0,
            LaneGroup::State => 1,
            LaneGroup::Stream => 2,
        }
    }
}

/// The lane a frame is drawn in.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Lane {
    /// Mermaid's ordering key: 0 for UI/Automation, 100 for Command/Read Model, 200 for
    /// Events, and the next free number after those for each namespace.
    pub index: usize,
    pub label: String,
    pub group: LaneGroup,
    pub namespace: Option<String>,
}

/// A frame, as it is drawn.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FrameBox {
    /// The number of the frame, as it is written.
    pub id: String,
    pub kind: EntityKind,
    /// The entity the frame is about, with its namespace.
    pub entity: String,
    /// The name of the entity, without its namespace.
    pub name: String,
    pub namespace: Option<String>,
    pub reset: bool,
    /// The lane the frame is drawn in.
    pub lane: usize,
    /// The data the frame carries, as it is written.
    pub data: Option<String>,
    pub data_type: Option<String>,
}

/// A relation between two frames.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Relation {
    pub source: String,
    pub target: String,
}

/// A note, written under the frame it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Note {
    pub frame: String,
    pub text: String,
    pub data_type: Option<String>,
}

/// A given/when/then specification, written under the frame it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Specification {
    pub frame: String,
    pub given: Vec<String>,
    pub when: Vec<String>,
    pub then: Vec<String>,
}

/// What is drawn.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct EventModel {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub boxes: Vec<FrameBox>,
    /// The lanes, from top to bottom.
    pub lanes: Vec<Lane>,
    pub relations: Vec<Relation>,
    pub notes: Vec<Note>,
    pub specifications: Vec<Specification>,
    pub entities: Vec<String>,
}

/// The first lane of a group, its label, and the label its namespaces get.
const GROUPS: [(usize, &str, &str); 3] = [
    (0, "UI/Automation", "UI/A: "),
    (100, "Command/Read Model", "C/RM: "),
    (200, "Events", "Stream: "),
];

fn group_of(kind: EntityKind) -> LaneGroup {
    match kind {
        EntityKind::Ui | EntityKind::Processor => LaneGroup::Trigger,
        EntityKind::Event => LaneGroup::Stream,
        EntityKind::Command | EntityKind::ReadModel => LaneGroup::State,
    }
}

/// The lane of a frame: one per group, and one more per namespace within a group.
fn lane_for(
    lanes: &mut Vec<(String, Lane)>,
    next: &mut [Option<usize>; 3],
    kind: EntityKind,
    namespace: Option<&str>,
) -> usize {
    let group = group_of(kind);
    let key = match namespace {
        Some(namespace) => format!("{} {namespace}", group.name()),
        None => group.name().to_string(),
    };
    if let Some((_, lane)) = lanes.iter().find(|(other, _)| other == &key) {
        return lane.index;
    }
    let (first, label, prefix) = GROUPS[group.index()];
    let index = match namespace {
        Some(_) => {
            let index = next[group.index()].unwrap_or(first + 1);
            next[group.index()] = Some(index + 1);
            index
        }
        None => first,
    };
    let lane = Lane {
        index,
        label: match namespace {
            Some(namespace) => format!("{prefix}{namespace}"),
            None => label.to_string(),
        },
        group,
        namespace: namespace.map(str::to_string),
    };
    lanes.push((key, lane.clone()));
    lane.index
}

/// The text between the outer braces of a data block, without the indentation its lines share.
pub(crate) fn block_text(raw: &str) -> String {
    let inner = match (raw.find('{'), raw.rfind('}')) {
        (Some(open), Some(close)) if close > open => &raw[open + 1..close],
        (None, _) if raw.len() >= 2 => &raw[1..raw.len() - 1],
        _ => "",
    };
    let text = inner.replace('\r', "");
    let mut lines: Vec<&str> = text.split('\n').collect();
    while lines.first().is_some_and(|line| trim(line).is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| trim(line).is_empty()) {
        lines.pop();
    }
    let indent = lines
        .iter()
        .filter(|line| !trim(line).is_empty())
        .map(|line| line.chars().count() - line.trim_start_matches(is_space).chars().count())
        .min()
        .unwrap_or(usize::MAX);
    lines
        .iter()
        .map(|line| {
            let rest: String = line.chars().skip(indent).collect();
            trim_end(&rest).to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Builds the model of an event modeling diagram.
pub(crate) fn build_event_model(
    ast: &EventModelAst,
    title: Option<&str>,
    src: &str,
) -> Result<EventModel, Error> {
    // A frame number is used once.
    let mut ids: HashSet<&str> = HashSet::new();
    for frame in &ast.frames {
        if !ids.insert(frame.name.as_str()) {
            let line = locate(src, frame.offset).line;
            return Err(Error::new(
                ErrorCode::Semantic,
                format!(
                    "Duplicate event modeling frame ID \"{}\" on line {line}",
                    frame.name
                ),
            )
            .at(src, frame.offset));
        }
    }

    // The first data block of a name is the one a frame that refers to it reads.
    let mut data: Vec<(&str, &str)> = Vec::new();
    for entity in &ast.data_entities {
        if !data.iter().any(|(name, _)| *name == entity.name.as_str()) {
            data.push((entity.name.as_str(), entity.data.as_str()));
        }
    }

    let mut lanes: Vec<(String, Lane)> = Vec::new();
    let mut next: [Option<usize>; 3] = [None; 3];
    let mut boxes: Vec<FrameBox> = Vec::new();
    let mut relations: Vec<Relation> = Vec::new();
    for (index, frame) in ast.frames.iter().enumerate() {
        let parts: Vec<&str> = frame.entity.split('.').collect();
        // Only a two-part name has a namespace.
        let namespace = if parts.len() == 2 {
            Some(parts[0].to_string())
        } else {
            None
        };
        let mut text = frame.data_inline.as_deref().map(block_text);
        if let Some(reference) = frame.data_reference.as_deref() {
            if let Some((_, block)) = data.iter().find(|(name, _)| *name == reference) {
                text = Some(block_text(block));
            }
        }
        let lane = lane_for(
            &mut lanes,
            &mut next,
            frame.entity_kind,
            namespace.as_deref(),
        );
        let item = FrameBox {
            id: frame.name.clone(),
            kind: frame.entity_kind,
            entity: frame.entity.clone(),
            name: if parts.len() == 2 {
                parts[1].to_string()
            } else {
                frame.entity.clone()
            },
            namespace,
            reset: frame.kind == FrameKind::Reset,
            lane,
            data: text,
            data_type: frame.data_type.clone(),
        };
        boxes.push(item.clone());
        if !frame.sources.is_empty() {
            // Sources connect in frame order, and only frames that already have a box.
            for source in &boxes {
                if frame.sources.contains(&source.id) {
                    relations.push(Relation {
                        source: source.id.clone(),
                        target: item.id.clone(),
                    });
                }
            }
        } else if index > 0 && !item.reset {
            // Otherwise the nearest earlier frame in another lane feeds this one.
            for earlier in (0..index).rev() {
                if boxes[earlier].lane != item.lane {
                    relations.push(Relation {
                        source: boxes[earlier].id.clone(),
                        target: item.id.clone(),
                    });
                    break;
                }
            }
        }
    }

    let mut lanes: Vec<Lane> = lanes.into_iter().map(|(_, lane)| lane).collect();
    lanes.sort_by_key(|lane| lane.index);

    Ok(EventModel {
        title: ast
            .title
            .as_deref()
            .map(crate::diagrams::common::tokens::title_value)
            .filter(|s| !s.is_empty())
            .or_else(|| title.map(str::to_string)),
        acc_title: ast
            .acc_title
            .as_deref()
            .map(crate::diagrams::common::tokens::acc_title_value)
            .map(|s| s.trim_start_matches(crate::util::is_space).to_string()),
        acc_descr: ast
            .acc_descr
            .as_deref()
            .map(crate::diagrams::common::tokens::acc_descr_value)
            .map(|s| {
                s.split('\n')
                    .map(str::trim_start)
                    .collect::<Vec<_>>()
                    .join("\n")
            }),
        boxes,
        lanes,
        relations,
        notes: ast
            .notes
            .iter()
            .map(|note| Note {
                frame: note.frame.clone(),
                text: block_text(&note.data),
                data_type: note.data_type.clone(),
            })
            .collect(),
        specifications: ast
            .specifications
            .iter()
            .map(|specification| Specification {
                frame: specification.frame.clone(),
                given: specification.given.clone(),
                when: specification.when.clone(),
                then: specification.then.clone(),
            })
            .collect(),
        entities: ast.model_entities.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagrams::eventmodeling::parser::parse_event_modeling;

    fn model(src: &str) -> EventModel {
        let ast = parse_event_modeling(src).unwrap();
        build_event_model(&ast, None, src).unwrap()
    }

    #[test]
    fn puts_the_frames_in_their_lanes() {
        let model = model("eventmodeling\ntf 01 ui CartUI\ntf 02 cmd AddItem\ntf 03 evt Item\n");
        assert_eq!(model.lanes.len(), 3);
        assert_eq!(model.lanes[0].label, "UI/Automation");
        assert_eq!(model.lanes[1].label, "Command/Read Model");
        assert_eq!(model.lanes[2].label, "Events");
        assert_eq!(model.boxes[0].lane, 0);
        assert_eq!(model.boxes[1].lane, 100);
        assert_eq!(model.boxes[2].lane, 200);
    }

    #[test]
    fn gives_every_namespace_a_lane_of_its_own() {
        let model =
            model("eventmodeling\ntf 01 cmd Inventory.AddItem\ntf 02 evt Inventory.ItemAdded\n");
        assert_eq!(model.lanes.len(), 2);
        assert_eq!(model.lanes[0].label, "C/RM: Inventory");
        assert_eq!(model.lanes[1].label, "Stream: Inventory");
        assert_eq!(model.boxes[0].namespace.as_deref(), Some("Inventory"));
        assert_eq!(model.boxes[1].name, "ItemAdded");
    }

    #[test]
    fn infers_the_relations() {
        let model = model("eventmodeling\ntf 01 ui CartUI\ntf 02 cmd AddItem\ntf 03 evt Item\n");
        // The command follows the user interface, and the event follows the command.
        assert_eq!(model.relations.len(), 2);
        assert_eq!(model.relations[0].source, "01");
        assert_eq!(model.relations[0].target, "02");
        assert_eq!(model.relations[1].source, "02");
        assert_eq!(model.relations[1].target, "03");
    }

    #[test]
    fn a_reset_frame_ends_the_inferred_relations() {
        let model = model(
            "eventmodeling\ntf 01 evt ItemAdded\ntf 02 ui OtherUI\nrf 03 rmo CartItems ->> 01\n",
        );
        // The user interface follows the event, and the read model is fed by the event it
        // names.
        assert_eq!(model.relations.len(), 2);
        assert_eq!(model.relations[0].source, "01");
        assert_eq!(model.relations[0].target, "02");
        assert_eq!(model.relations[1].source, "01");
        assert_eq!(model.relations[1].target, "03");
    }

    #[test]
    fn refuses_a_number_that_is_used_twice() {
        let ast = parse_event_modeling("eventmodeling\ntf 01 evt A\nrf 01 evt B\n").unwrap();
        let err =
            build_event_model(&ast, None, "eventmodeling\ntf 01 evt A\nrf 01 evt B\n").unwrap_err();
        assert_eq!(err.code, ErrorCode::Semantic);
        assert!(
            err.message
                .contains("Duplicate event modeling frame ID \"01\""),
            "{}",
            err.message
        );
    }

    #[test]
    fn reads_the_data_of_a_frame() {
        let with_block = model("eventmodeling\ndata D {\n  a: 1\n}\ntf 01 cmd AddItem [[D]]\n");
        assert_eq!(with_block.boxes[0].data.as_deref(), Some("a: 1"));
        let inline = model("eventmodeling\ntf 01 cmd AddItem { a: 1 }\n");
        assert_eq!(inline.boxes[0].data.as_deref(), Some("a: 1"));
    }
}
