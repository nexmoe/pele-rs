//! What a state diagram describes, built as Mermaid's `StateDB` (version 2) builds it. Ported
//! from pele's `diagrams/state/db.ts` and `types.ts`.

pub(crate) use crate::diagrams::common::ordered::Ordered;

use super::graph::{StateGraph, build_state_graph};
use crate::error::Error;

/// A state's description: one text, or a title and lines (`state "a" as b:c`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Desc {
    Text(String),
    List(Vec<String>),
}

impl Desc {
    /// Whether JavaScript would read the value as true.
    pub(crate) fn truthy(&self) -> bool {
        match self {
            Desc::Text(t) => !t.is_empty(),
            Desc::List(_) => true,
        }
    }

    fn items(&self) -> Vec<String> {
        match self {
            Desc::Text(t) => vec![t.clone()],
            Desc::List(l) => l.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StateNote {
    pub position: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StateStmt {
    pub id: String,
    pub kind: Option<String>,
    pub description: Option<Desc>,
    pub doc: Option<Vec<Stmt>>,
    pub note: Option<StateNote>,
    /// Set on `[*]`: whether it starts (rather than ends) its document.
    pub start: Option<bool>,
    pub classes: Option<Vec<String>>,
}

impl StateStmt {
    pub(crate) fn new(id: String, kind: &str) -> Self {
        Self {
            id,
            kind: Some(kind.to_string()),
            description: None,
            doc: None,
            note: None,
            start: None,
            classes: None,
        }
    }
}

/// A statement of a document. Mermaid's grammar also leaves the text of statements that have
/// no effect (`hide empty description`, `scale`, a bare `state name`, a floating note); those
/// are `Text`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Stmt {
    Text(String),
    State(StateStmt),
    Relation {
        state1: Box<StateStmt>,
        state2: Box<StateStmt>,
        description: Option<String>,
    },
    ClassDef {
        id: String,
        classes: String,
    },
    Style {
        id: String,
        style_class: String,
    },
    ApplyClass {
        id: String,
        style_class: String,
    },
    Dir(String),
    Click {
        id: String,
        url: String,
        tooltip: String,
    },
}

/// A state as the model keeps it.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct StateRec {
    pub id: String,
    pub descriptions: Vec<String>,
    pub kind: String,
    pub has_doc: bool,
    pub note: Option<StateNote>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StateRelation {
    pub id1: String,
    pub id2: String,
    pub title: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct StateClassDef {
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StateLink {
    pub url: String,
    pub tooltip: String,
}

#[derive(Debug, Default)]
pub(crate) struct StateDb {
    pub root_doc: Vec<Stmt>,
    pub states: Ordered<StateRec>,
    pub relations: Vec<StateRelation>,
    pub classes: Ordered<StateClassDef>,
    pub links: Ordered<StateLink>,
    pub graph: StateGraph,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    divider_count: usize,
}

/// `str.trim()`.
fn trim(s: &str) -> &str {
    crate::util::trim(s)
}

impl StateDb {
    /// The direction the root document is laid out in: its last `direction` statement.
    pub(crate) fn direction(&self) -> &str {
        direction_of(&self.root_doc)
    }

    pub(crate) fn divider_id(&mut self) -> String {
        self.divider_count += 1;
        format!("divider-id-{}", self.divider_count)
    }

    pub(crate) fn set_root_doc(&mut self, doc: Vec<Stmt>) -> Result<(), Error> {
        self.root_doc = doc;
        let mut root = std::mem::take(&mut self.root_doc);
        self.translate_doc("root", &mut root);
        self.root_doc = root;
        self.extract()
    }

    /// Mermaid's docTranslator for one document: gives each `[*]` an id scoped to its parent,
    /// and turns a document split by `--` into one region per part.
    fn translate_doc(&mut self, parent_id: &str, doc: &mut Vec<Stmt>) {
        let has_divider = doc.iter().any(|item| {
            matches!(item, Stmt::State(s) if s.kind.as_deref() == Some("divider") && s.doc.is_none())
        });
        if has_divider {
            let mut regions: Vec<Stmt> = Vec::new();
            let mut current: Vec<Stmt> = Vec::new();
            for item in std::mem::take(doc) {
                match item {
                    Stmt::State(mut s)
                        if s.kind.as_deref() == Some("divider") && s.doc.is_none() =>
                    {
                        s.doc = Some(std::mem::take(&mut current));
                        regions.push(Stmt::State(s));
                    }
                    other => current.push(other),
                }
            }
            // Mermaid names the region after the last divider with a random id; here it
            // continues the divider numbering. Mermaid also leaves a document that ends with a
            // divider ungrouped and then fails to draw it; here the regions before that
            // divider are kept.
            if !current.is_empty() {
                let mut last = StateStmt::new(self.divider_id(), "divider");
                last.doc = Some(current);
                regions.push(Stmt::State(last));
            }
            *doc = regions;
        }
        for item in doc.iter_mut() {
            match item {
                Stmt::Relation { state1, state2, .. } => {
                    Self::translate_id(parent_id, state1, true);
                    Self::translate_id(parent_id, state2, false);
                }
                Stmt::State(s) => {
                    Self::translate_id(parent_id, s, true);
                    if let Some(mut inner) = s.doc.take() {
                        let id = s.id.clone();
                        self.translate_doc(&id, &mut inner);
                        s.doc = Some(inner);
                    }
                }
                _ => {}
            }
        }
    }

    fn translate_id(parent_id: &str, s: &mut StateStmt, first: bool) {
        if s.id == "[*]" {
            s.id = format!("{parent_id}{}", if first { "_start" } else { "_end" });
            s.start = Some(first);
        } else {
            s.id = trim(&s.id).to_string();
        }
    }

    /// Only the root document's statements become states, relations and classes here; the
    /// statements inside composites are read when the graph is built.
    fn extract(&mut self) -> Result<(), Error> {
        self.states = Ordered::default();
        self.relations.clear();
        self.classes = Ordered::default();
        let doc = std::mem::take(&mut self.root_doc);
        for item in &doc {
            match item {
                Stmt::State(s) => self.add_state(
                    trim(&s.id),
                    s.kind.as_deref(),
                    s.doc.is_some(),
                    s.description.as_ref(),
                    s.note.as_ref(),
                    None,
                ),
                Stmt::Relation {
                    state1,
                    state2,
                    description,
                } => {
                    for s in [state1, state2] {
                        self.add_state(
                            &s.id,
                            s.kind.as_deref(),
                            s.doc.is_some(),
                            s.description.as_ref(),
                            s.note.as_ref(),
                            s.classes.as_deref(),
                        );
                    }
                    self.relations.push(StateRelation {
                        id1: trim(&state1.id).to_string(),
                        id2: trim(&state2.id).to_string(),
                        title: description.clone().unwrap_or_default(),
                    });
                }
                Stmt::ClassDef { id, classes } => self.add_style_class(trim(id), classes),
                Stmt::Style { id, style_class } => {
                    let styles: Vec<String> = style_class
                        .split(',')
                        .map(|s| trim(&s.replace(';', "")).to_string())
                        .collect();
                    for key in self.named(trim(id)) {
                        if let Some(state) = self.states.get_mut(&key) {
                            state.styles = styles.clone();
                        }
                    }
                }
                Stmt::ApplyClass { id, style_class } => self.set_css_class(trim(id), style_class),
                Stmt::Click { id, url, tooltip } => self.links.set(
                    id,
                    StateLink {
                        url: url.clone(),
                        tooltip: tooltip.clone(),
                    },
                ),
                Stmt::Text(_) | Stmt::Dir(_) => {}
            }
        }
        self.root_doc = doc;
        self.graph = build_state_graph(&self.root_doc, &self.states, &self.classes)?;
        Ok(())
    }

    fn add_state(
        &mut self,
        id: &str,
        kind: Option<&str>,
        has_doc: bool,
        descr: Option<&Desc>,
        note: Option<&StateNote>,
        classes: Option<&[String]>,
    ) {
        let id = trim(id).to_string();
        // `type = 'default'` stands in only for a missing type.
        let kind = kind.unwrap_or("default");
        match self.states.get_mut(&id) {
            None => self.states.set(
                &id,
                StateRec {
                    id: id.clone(),
                    kind: kind.to_string(),
                    has_doc,
                    note: note.cloned(),
                    ..StateRec::default()
                },
            ),
            Some(state) => {
                state.has_doc |= has_doc;
                if state.kind.is_empty() {
                    state.kind = kind.to_string();
                }
            }
        }
        if let Some(descr) = descr.filter(|d| d.truthy()) {
            for text in descr.items() {
                self.add_description(&id, trim(&text));
            }
        }
        if let Some(note) = note {
            if let Some(state) = self.states.get_mut(&id) {
                state.note = Some(note.clone());
            }
        }
        for name in classes.unwrap_or_default() {
            self.set_css_class(&id, trim(name));
        }
    }

    fn add_description(&mut self, id: &str, descr: &str) {
        if let Some(state) = self.states.get_mut(id) {
            let text = match descr.strip_prefix(':') {
                Some(rest) => trim(rest).to_string(),
                None => descr.to_string(),
            };
            state.descriptions.push(text);
        }
    }

    fn add_style_class(&mut self, id: &str, attributes: &str) {
        if !self.classes.contains(id) {
            self.classes.set(id, StateClassDef::default());
        }
        if attributes.is_empty() {
            return;
        }
        let Some(found) = self.classes.get_mut(id) else {
            return;
        };
        for attrib in attributes.split(',') {
            let fixed = trim(&attrib.replacen(';', "", 1)).to_string();
            if attrib.contains("color") {
                found.text_styles.push(
                    fixed
                        .replacen("fill", "bgFill", 1)
                        .replacen("color", "fill", 1),
                );
            }
            found.styles.push(fixed);
        }
    }

    fn set_css_class(&mut self, ids: &str, class_name: &str) {
        for key in self.named(ids) {
            if let Some(state) = self.states.get_mut(&key) {
                state.classes.push(class_name.to_string());
            }
        }
    }

    /// The keys of the states a comma-separated list of ids names, created where they do not
    /// exist yet. As in Mermaid, an id is looked up as written and trimmed only when it has to
    /// be created.
    fn named(&mut self, ids: &str) -> Vec<String> {
        let mut out = Vec::new();
        for id in ids.split(',') {
            if self.states.contains(id) {
                out.push(id.to_string());
                continue;
            }
            self.add_state(id, None, false, None, None, None);
            let key = trim(id);
            if self.states.contains(key) {
                out.push(key.to_string());
            }
        }
        out
    }

    pub(crate) fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(crate::util::trim_start(text).to_string());
    }

    pub(crate) fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(crate::util::collapse_indent(text));
    }
}

/// The last `direction` of a document, or TB.
pub(crate) fn direction_of(doc: &[Stmt]) -> &str {
    doc.iter()
        .rev()
        .find_map(|item| match item {
            Stmt::Dir(d) => Some(d.as_str()),
            _ => None,
        })
        .unwrap_or("TB")
}
