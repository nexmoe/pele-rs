//! What a state diagram draws: Mermaid's dataFetcher, which walks the documents and resolves
//! them into nodes and edges. Ported from pele's `diagrams/state/graph.ts`.

use std::collections::HashMap;

use super::db::{Desc, Ordered, StateClassDef, StateRec, StateStmt, Stmt, direction_of};
use crate::error::{Error, ErrorCode};

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct StateNode {
    pub id: String,
    /// rect, rectWithTitle, stateStart, stateEnd, choice, fork, join, divider,
    /// roundedWithTitle, note or noteGroup.
    pub shape: Option<String>,
    pub label: Option<String>,
    pub description: Vec<String>,
    pub css_classes: String,
    pub css_compiled_styles: Vec<String>,
    pub css_styles: Vec<String>,
    pub kind: Option<String>,
    pub is_group: bool,
    pub parent_id: Option<String>,
    /// The direction inside a composite: its last `direction` statement, or TB.
    pub dir: Option<String>,
    pub position: Option<String>,
    pub note_for: Option<String>,
    pub color_index: Option<usize>,
}

/// Labels are markdown throughout, as in Mermaid.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StateEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    pub label: String,
    pub classes: &'static str,
    pub dashed: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct StateGraph {
    pub nodes: Vec<StateNode>,
    pub edges: Vec<StateEdge>,
}

/// A description as the dataFetcher holds it, a text or a list.
#[derive(Clone)]
enum Draft {
    Text(String),
    List(Vec<String>),
}

impl Draft {
    /// `.length`: UTF-16 units of a text, items of a list.
    fn len(&self) -> usize {
        match self {
            Draft::Text(t) => t.encode_utf16().count(),
            Draft::List(l) => l.len(),
        }
    }
}

struct DraftState {
    shape: Option<String>,
    description: Draft,
    css_classes: String,
    css_styles: Vec<String>,
    kind: Option<String>,
    dir: Option<String>,
    color_index: Option<usize>,
}

/// Which keys a node object carries in pele, which `Object.assign` copies onto an existing
/// node of the same id.
#[derive(Clone, Copy, PartialEq)]
enum From {
    State,
    NoteGroup,
    Note,
}

struct Builder<'a> {
    states: &'a Ordered<StateRec>,
    classes: &'a Ordered<StateClassDef>,
    nodes: Vec<StateNode>,
    edges: Vec<StateEdge>,
    by_id: HashMap<String, usize>,
    drafts: HashMap<String, DraftState>,
    slots: HashMap<String, Option<usize>>,
    counter: usize,
    next_slot: usize,
}

impl Builder<'_> {
    fn insert(&mut self, mut node: StateNode, from: From) {
        // The two literal ids are what DOMPurify leaves of `<<fork>>` and `<<choice>>` in
        // Mermaid; kept for parity.
        if node.id.is_empty() || node.id == "</join></fork>" || node.id == "</choice>" {
            return;
        }
        if !node.css_classes.is_empty() {
            let mut compiled: Option<Vec<String>> = None;
            for name in node.css_classes.split(' ') {
                let Some(def) = self.classes.get(name) else {
                    continue;
                };
                compiled
                    .get_or_insert_with(|| node.css_compiled_styles.clone())
                    .extend(def.styles.iter().cloned());
            }
            if let Some(compiled) = compiled {
                node.css_compiled_styles = compiled;
            }
        }
        match self.by_id.get(&node.id) {
            Some(&i) => {
                let existing = &mut self.nodes[i];
                let parent_id = node
                    .parent_id
                    .clone()
                    .or_else(|| existing.parent_id.clone());
                let position = existing.position.take();
                let note_for = existing.note_for.take();
                let color_index = existing.color_index;
                *existing = node;
                // Keys the new object does not have keep their old values.
                if from == From::State {
                    existing.position = position;
                    existing.note_for = note_for;
                } else {
                    existing.color_index = color_index;
                    if from == From::NoteGroup {
                        existing.note_for = note_for;
                    }
                }
                if parent_id.is_some() {
                    existing.parent_id = parent_id;
                }
            }
            None => {
                self.by_id.insert(node.id.clone(), self.nodes.len());
                self.nodes.push(node);
            }
        }
    }

    fn visit(&mut self, parent: Option<&str>, item: &StateStmt, alt: bool) {
        let id = item.id.as_str();
        if id == "root" {
            return;
        }
        let known = self.states.get(id);
        let class_str = known.map(|k| k.classes.join(" ")).unwrap_or_default();
        let styles = known.map(|k| k.styles.clone()).unwrap_or_default();

        if !self.drafts.contains_key(id) {
            let mut shape = Some("rect".to_string());
            match item.start {
                Some(true) => shape = Some("stateStart".to_string()),
                Some(false) => shape = Some("stateEnd".to_string()),
                None => {}
            }
            if item.kind.as_deref() != Some("default") {
                shape = item.kind.clone();
            }
            self.drafts.insert(
                id.to_string(),
                DraftState {
                    shape,
                    description: Draft::Text(id.to_string()),
                    css_classes: format!("{class_str} statediagram-state"),
                    css_styles: styles.clone(),
                    kind: None,
                    dir: None,
                    color_index: None,
                },
            );
        }
        let parent_slot = parent.and_then(|p| self.slots.get(p).copied());
        let draft = self.drafts.get_mut(id).expect("draft");

        if let Some(descr) = item.description.as_ref().filter(|d| d.truthy()) {
            let items = match descr {
                Desc::Text(t) => vec![t.clone()],
                Desc::List(l) => l.clone(),
            };
            match &draft.description {
                Draft::List(list) => {
                    draft.shape = Some("rectWithTitle".to_string());
                    let mut list = list.clone();
                    list.extend(items);
                    draft.description = Draft::List(list);
                }
                Draft::Text(text) if !text.is_empty() => {
                    draft.shape = Some("rectWithTitle".to_string());
                    let mut list = if text == id {
                        Vec::new()
                    } else {
                        vec![text.clone()]
                    };
                    list.extend(items);
                    draft.description = Draft::List(list);
                }
                Draft::Text(_) => {
                    draft.shape = Some("rect".to_string());
                    draft.description = match descr {
                        Desc::Text(t) => Draft::Text(t.clone()),
                        Desc::List(l) => Draft::List(l.clone()),
                    };
                }
            }
        }
        if draft.description.len() == 1 && draft.shape.as_deref() == Some("rectWithTitle") {
            draft.shape = Some(
                if draft.kind.as_deref() == Some("group") {
                    "roundedWithTitle"
                } else {
                    "rect"
                }
                .to_string(),
            );
        }

        if draft.kind.is_none() {
            if let Some(doc) = &item.doc {
                draft.kind = Some("group".to_string());
                draft.dir = Some(direction_of(doc).to_string());
                draft.shape = Some(
                    if item.kind.as_deref() == Some("divider") {
                        "divider"
                    } else {
                        "roundedWithTitle"
                    }
                    .to_string(),
                );
                // A container takes the next palette slot unless it is a region, which shares
                // its composite's.
                if draft.shape.as_deref() == Some("divider") && parent_slot.is_some() {
                    draft.color_index = parent_slot.flatten();
                } else {
                    let slot = self.next_slot;
                    self.next_slot += 1;
                    draft.color_index =
                        if !crate::util::trim(&class_str).is_empty() || !styles.is_empty() {
                            None
                        } else {
                            Some(slot)
                        };
                }
                self.slots.insert(id.to_string(), draft.color_index);
                draft.css_classes = format!(
                    "{} statediagram-cluster {}",
                    draft.css_classes,
                    if alt { "statediagram-cluster-alt" } else { "" }
                );
            }
        }

        let (label, description) = match &draft.description {
            Draft::List(l) => (l.first().cloned(), l.iter().skip(1).cloned().collect()),
            Draft::Text(t) => (Some(t.clone()), Vec::new()),
        };
        let mut node = StateNode {
            id: id.to_string(),
            shape: draft.shape.clone(),
            label,
            description,
            css_classes: draft.css_classes.clone(),
            css_compiled_styles: Vec::new(),
            css_styles: draft.css_styles.clone(),
            kind: draft.kind.clone(),
            is_group: draft.kind.as_deref() == Some("group"),
            parent_id: None,
            dir: draft.dir.clone(),
            position: None,
            note_for: None,
            color_index: draft.color_index,
        };
        let draft_classes = draft.css_classes.clone();
        if node.shape.as_deref() == Some("divider") {
            node.label = Some(String::new());
        }
        if let Some(parent) = parent.filter(|&p| p != "root") {
            node.parent_id = Some(parent.to_string());
        }

        let Some(note) = &item.note else {
            self.insert(node, From::State);
            return;
        };
        // Mermaid puts each note in a group of its own, named after the state. Pele keeps that
        // group in the model and lays the note out beside its state instead.
        let aside =
            |aside_id: String, shape: &str, css_classes: String, is_group: bool| StateNode {
                id: aside_id,
                shape: Some(shape.to_string()),
                label: Some(note.text.clone()),
                css_classes,
                kind: Some(if is_group { "group" } else { "node" }.to_string()),
                is_group,
                position: note.position.clone(),
                ..StateNode::default()
            };
        let mut note_node = aside(
            format!("{id}----note-{}", self.counter),
            "note",
            "statediagram-note".to_string(),
            false,
        );
        self.counter += 1;
        let group_id = format!("{id}----parent");
        note_node.parent_id = Some(group_id.clone());
        note_node.note_for = Some(id.to_string());
        let note_id = note_node.id.clone();
        self.insert(
            aside(group_id, "noteGroup", draft_classes, true),
            From::NoteGroup,
        );
        self.insert(note_node, From::Note);
        self.insert(node, From::State);
        let left = note.position.as_deref() == Some("left of");
        let (from, to) = if left {
            (note_id, id.to_string())
        } else {
            (id.to_string(), note_id)
        };
        self.edges.push(StateEdge {
            id: format!("{from}-{to}"),
            start: from,
            end: to,
            label: String::new(),
            classes: "transition note-edge",
            dashed: true,
        });
    }

    fn walk(&mut self, parent: Option<&StateStmt>, doc: &[Stmt], alt: bool) {
        let parent_id = parent.map(|p| p.id.as_str());
        for item in doc {
            match item {
                Stmt::State(s) => {
                    self.visit(parent_id, s, alt);
                    if let Some(inner) = &s.doc {
                        self.walk(Some(s), inner, !alt);
                    }
                }
                Stmt::Relation {
                    state1,
                    state2,
                    description,
                } => {
                    self.visit(parent_id, state1, alt);
                    self.visit(parent_id, state2, alt);
                    self.edges.push(StateEdge {
                        id: format!("edge{}", self.counter),
                        start: state1.id.clone(),
                        end: state2.id.clone(),
                        label: description.clone().unwrap_or_default(),
                        classes: "transition",
                        dashed: false,
                    });
                    self.counter += 1;
                }
                _ => {}
            }
        }
    }
}

pub(crate) fn build_state_graph(
    root_doc: &[Stmt],
    states: &Ordered<StateRec>,
    classes: &Ordered<StateClassDef>,
) -> Result<StateGraph, Error> {
    let mut b = Builder {
        states,
        classes,
        nodes: Vec::new(),
        edges: Vec::new(),
        by_id: HashMap::new(),
        drafts: HashMap::new(),
        slots: HashMap::new(),
        counter: 0,
        next_slot: 0,
    };
    b.walk(None, root_doc, true);
    if let Some(node) = b
        .nodes
        .iter()
        .find(|n| n.is_group && !n.description.is_empty())
    {
        return Err(Error::new(
            ErrorCode::Semantic,
            format!(
                "Group nodes can only have label. Remove the additional description for node [{}]",
                node.id
            ),
        ));
    }
    Ok(StateGraph {
        nodes: b.nodes,
        edges: b.edges,
    })
}
