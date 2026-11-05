//! What an ER diagram draws: subgraphs as groups, entities as nodes with their class styles, and
//! relationships as edges with a marker type at each end. Ported from pele's
//! `diagrams/er/graph.ts`.

use std::collections::{HashMap, HashSet};

use super::db::{Cardinality, ErAttribute, ErDb};

#[derive(Debug, Clone)]
pub(crate) struct ErGraphNode<'a> {
    pub id: String,
    pub label: &'a str,
    pub alias: &'a str,
    pub attributes: &'a [ErAttribute],
    pub is_group: bool,
    pub parent_id: Option<&'a str>,
    pub css_styles: &'a [String],
    pub css_compiled_styles: Vec<String>,
    pub css_classes: String,
    pub dir: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub(crate) struct ErGraphEdge<'a> {
    pub id: String,
    pub start: &'a str,
    pub end: &'a str,
    pub label: &'a str,
    pub arrow_start: Cardinality,
    pub arrow_end: Cardinality,
    pub dashed: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ErGraph<'a> {
    pub nodes: Vec<ErGraphNode<'a>>,
    pub edges: Vec<ErGraphEdge<'a>>,
}

pub(crate) fn build_er_graph(db: &ErDb) -> ErGraph<'_> {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    let compiled = |names: &mut dyn Iterator<Item = &str>| -> Vec<String> {
        let mut out = Vec::new();
        for name in names {
            let Some(def) = db.classes.get(name) else {
                continue;
            };
            out.extend(def.styles.iter().map(|s| crate::util::trim(s).to_string()));
            out.extend(
                def.text_styles
                    .iter()
                    .map(|s| crate::util::trim(s).to_string()),
            );
        }
        out
    };

    let mut parent_of: HashMap<&str, &str> = HashMap::new();
    let mut subgraph_ids: HashSet<&str> = HashSet::new();
    for subgraph in db.subgraphs.iter().rev() {
        subgraph_ids.insert(&subgraph.id);
        for id in &subgraph.nodes {
            parent_of.insert(id, &subgraph.id);
        }
    }

    for subgraph in db.subgraphs.iter().rev() {
        nodes.push(ErGraphNode {
            id: subgraph.id.clone(),
            label: &subgraph.title,
            alias: "",
            attributes: &[],
            is_group: true,
            parent_id: parent_of.get(subgraph.id.as_str()).copied(),
            css_styles: &subgraph.css_styles,
            css_compiled_styles: compiled(&mut subgraph.classes.iter().map(String::as_str)),
            css_classes: subgraph.classes.join(" "),
            dir: subgraph.dir,
        });
    }

    // A name that is also a subgraph id stands for the subgraph. Mermaid drops such an entity
    // and leaves relationships written before the subgraph pointing at nothing; here they follow
    // it.
    let mut moved: HashMap<&str, &str> = HashMap::new();
    for (name, entity) in &db.entities.items {
        if subgraph_ids.contains(name.as_str()) {
            moved.insert(&entity.id, name);
            continue;
        }
        nodes.push(ErGraphNode {
            id: entity.id.clone(),
            label: &entity.label,
            alias: &entity.alias,
            attributes: &entity.attributes,
            is_group: false,
            parent_id: parent_of.get(name.as_str()).copied(),
            css_styles: &entity.css_styles,
            css_compiled_styles: compiled(&mut entity.css_classes.split(' ')),
            css_classes: entity.css_classes.clone(),
            dir: None,
        });
    }

    for (index, rel) in db.relationships.iter().enumerate() {
        edges.push(ErGraphEdge {
            id: format!("id_{}_{}_{index}", rel.entity_a, rel.entity_b),
            start: moved
                .get(rel.entity_a.as_str())
                .copied()
                .unwrap_or(&rel.entity_a),
            end: moved
                .get(rel.entity_b.as_str())
                .copied()
                .unwrap_or(&rel.entity_b),
            label: &rel.role_a,
            arrow_start: rel.rel_spec.card_b,
            arrow_end: rel.rel_spec.card_a,
            dashed: !rel.rel_spec.identifying,
        });
    }

    ErGraph { nodes, edges }
}
