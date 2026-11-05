//! The model of an ER diagram. Ported from pele's `diagrams/er/db.ts`.

use std::collections::{HashMap, HashSet};

use crate::diagrams::common::ordered::Ordered;
use crate::util::{collapse_indent, trim, trim_start};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cardinality {
    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
    OnlyOne,
    MdParent,
}

#[cfg(test)]
impl Cardinality {
    /// Mermaid's name for it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Cardinality::ZeroOrOne => "ZERO_OR_ONE",
            Cardinality::ZeroOrMore => "ZERO_OR_MORE",
            Cardinality::OneOrMore => "ONE_OR_MORE",
            Cardinality::OnlyOne => "ONLY_ONE",
            Cardinality::MdParent => "MD_PARENT",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ErAttribute {
    pub kind: String,
    pub name: String,
    pub keys: Vec<String>,
    pub comment: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ErEntity {
    pub id: String,
    pub label: String,
    pub alias: String,
    pub attributes: Vec<ErAttribute>,
    pub css_classes: String,
    pub css_styles: Vec<String>,
}

/// `card_b` sits at `entity_a`'s end of the line and `card_a` at `entity_b`'s, as in Mermaid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RelSpec {
    pub card_a: Cardinality,
    pub identifying: bool,
    pub card_b: Cardinality,
}

/// `entity_a` and `entity_b` hold an entity's id, or a subgraph's id when the name was a
/// subgraph at the time.
#[derive(Debug, Clone)]
pub(crate) struct ErRelationship {
    pub entity_a: String,
    pub role_a: String,
    pub entity_b: String,
    pub rel_spec: RelSpec,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ErClassDef {
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ErSubgraph {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    pub css_styles: Vec<String>,
    pub dir: Option<&'static str>,
}

/// What Mermaid's grammar hands to `addSubGraph` for one statement inside a subgraph.
#[derive(Debug, Clone)]
pub(crate) enum DocItem {
    Name(String),
    Dir(&'static str),
}

#[derive(Debug)]
pub(crate) struct ErDb {
    pub direction: &'static str,
    pub entities: Ordered<ErEntity>,
    pub relationships: Vec<ErRelationship>,
    pub classes: Ordered<ErClassDef>,
    pub subgraphs: Vec<ErSubgraph>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    subgraph_lookup: HashMap<String, usize>,
    claimed: HashSet<String>,
}

impl ErDb {
    pub(crate) fn new() -> ErDb {
        ErDb {
            direction: "TB",
            entities: Ordered::default(),
            relationships: Vec::new(),
            classes: Ordered::default(),
            subgraphs: Vec::new(),
            title: None,
            acc_title: None,
            acc_descr: None,
            subgraph_lookup: HashMap::new(),
            claimed: HashSet::new(),
        }
    }

    /// Adds an entity, or gives one already there the alias it lacks. Returns its id.
    pub(crate) fn add_entity(&mut self, name: &str, alias: &str) -> String {
        if let Some(entity) = self.entities.get_mut(name) {
            if entity.alias.is_empty() && !alias.is_empty() {
                entity.alias = alias.to_string();
            }
            return entity.id.clone();
        }
        let id = format!("entity-{name}-{}", self.entities.len());
        self.entities.set(
            name,
            ErEntity {
                id: id.clone(),
                label: name.to_string(),
                alias: alias.to_string(),
                attributes: Vec::new(),
                css_classes: "default".to_string(),
                css_styles: Vec::new(),
            },
        );
        id
    }

    /// The grammar builds the list back to front, so the last attribute written comes first.
    pub(crate) fn add_attributes(&mut self, name: &str, attributes: Vec<ErAttribute>) {
        self.add_entity(name, "");
        let entity = self.entities.get_mut(name).expect("entity");
        entity.attributes.extend(attributes.into_iter().rev());
    }

    pub(crate) fn add_relationship(&mut self, a: &str, role: &str, b: &str, spec: RelSpec) {
        let entity_a = if self.subgraph_lookup.contains_key(a) {
            a.to_string()
        } else {
            self.add_entity(a, "")
        };
        let entity_b = if self.subgraph_lookup.contains_key(b) {
            b.to_string()
        } else {
            self.add_entity(b, "")
        };
        self.relationships.push(ErRelationship {
            entity_a,
            role_a: role.to_string(),
            entity_b,
            rel_spec: spec,
        });
    }

    pub(crate) fn add_css_styles(&mut self, ids: &[String], styles: &[String]) {
        for id in ids {
            if let Some(entity) = self.entities.get_mut(id) {
                entity.css_styles.extend(styles.iter().cloned());
            }
            if let Some(&k) = self.subgraph_lookup.get(id) {
                self.subgraphs[k].css_styles.extend(styles.iter().cloned());
            }
        }
    }

    pub(crate) fn add_class(&mut self, ids: &[String], styles: &[String]) {
        for id in ids {
            if !self.classes.contains(id) {
                self.classes.set(id, ErClassDef::default());
            }
            let def = self.classes.get_mut(id).expect("class");
            for style in styles {
                if style.contains("color") {
                    def.text_styles.push(style.replacen("fill", "bgFill", 1));
                }
                def.styles.push(style.clone());
            }
        }
    }

    pub(crate) fn set_class(&mut self, ids: &[String], names: &[String]) {
        for id in ids {
            if let Some(entity) = self.entities.get_mut(id) {
                for name in names {
                    entity.css_classes.push(' ');
                    entity.css_classes.push_str(name);
                }
            }
            if let Some(&k) = self.subgraph_lookup.get(id) {
                self.subgraphs[k].classes.extend(names.iter().cloned());
            }
        }
    }

    pub(crate) fn add_subgraph(&mut self, id: &str, list: &[DocItem], title: &str) -> String {
        let id = trim(id).to_string();
        let mut seen: HashSet<&str> = HashSet::new();
        let mut nodes: Vec<String> = Vec::new();
        let mut dir = None;
        for item in list {
            let item = match item {
                DocItem::Dir(d) => {
                    dir = Some(*d);
                    continue;
                }
                DocItem::Name(name) => name,
            };
            let trimmed = trim(item);
            if trimmed.is_empty() || !seen.insert(trimmed) {
                continue;
            }
            // A node belongs to the first subgraph that lists it.
            if !self.claimed.contains(item) {
                nodes.push(item.clone());
            }
        }
        for node in &nodes {
            self.claimed.insert(node.clone());
        }
        // A later subgraph with the same id takes over the lookup, as a JavaScript Map set does.
        self.subgraph_lookup
            .insert(id.clone(), self.subgraphs.len());
        self.subgraphs.push(ErSubgraph {
            id: id.clone(),
            nodes,
            title: trim(title).to_string(),
            classes: Vec::new(),
            css_styles: Vec::new(),
            dir,
        });
        id
    }

    pub(crate) fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(trim_start(text).to_string());
    }

    pub(crate) fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(collapse_indent(text));
    }
}
