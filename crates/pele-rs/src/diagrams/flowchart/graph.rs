//! Resolves a parsed flowchart into the nodes and edges that are drawn: group membership,
//! collapsed subgraphs, class styles and edge defaults. Ported from pele's `graph.ts`.

use std::collections::{HashMap, HashSet};

use super::db::{FlowDb, FlowNode, LabelType};
use crate::preprocess::Value;

#[derive(Debug, Clone)]
pub(crate) struct GraphNode {
    pub id: String,
    pub label: Option<String>,
    pub label_type: LabelType,
    pub shape: String,
    pub is_group: bool,
    pub parent_id: Option<String>,
    pub css_styles: Vec<String>,
    pub css_compiled_styles: Vec<String>,
    pub css_classes: String,
    pub dir: Option<String>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub tooltip: Option<String>,
    pub icon: Option<String>,
    pub pos: Option<String>,
    pub img: Option<String>,
    pub asset_width: Option<f64>,
    pub asset_height: Option<f64>,
    pub constraint: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct GraphEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    pub label: String,
    pub label_type: LabelType,
    pub thickness: Option<String>,
    pub minlen: Option<i32>,
    pub arrow_start: String,
    pub arrow_end: String,
    pub css_compiled_styles: Vec<String>,
    pub style: Vec<String>,
    pub classes: String,
    pub curve: Option<String>,
}

pub(crate) struct FlowGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

fn shape_of(vertex: &FlowNode) -> String {
    if vertex.img.is_some() {
        return "imageSquare".to_string();
    }
    if vertex.icon.is_some() {
        return match vertex.form.as_deref() {
            Some("circle") => "iconCircle",
            Some("square") => "iconSquare",
            Some("rounded") => "iconRounded",
            _ => "icon",
        }
        .to_string();
    }
    match vertex.kind.as_deref() {
        None | Some("square") => "squareRect".to_string(),
        Some("round") => "roundedRect".to_string(),
        Some(kind) => kind.to_string(),
    }
}

fn arrows(kind: Option<&str>) -> (String, String) {
    match kind {
        Some(k @ ("arrow_point" | "arrow_circle" | "arrow_cross")) => {
            ("none".to_string(), k.to_string())
        }
        Some(k @ ("double_arrow_point" | "double_arrow_circle" | "double_arrow_cross")) => {
            let single = k.replacen("double_", "", 1);
            (single.clone(), single)
        }
        _ => ("none".to_string(), "arrow_point".to_string()),
    }
}

fn collapsed(db: &FlowDb, id: &str) -> bool {
    db.subgraph(id)
        .and_then(|sg| sg.metadata.as_ref())
        .and_then(|m| m.get("view"))
        .is_some_and(|v| matches!(v, Value::String(s) if s == "collapsed"))
}

pub(crate) fn build_flow_graph(db: &FlowDb, default_curve: Option<&str>) -> FlowGraph {
    let subgraphs = &db.subgraphs;
    let compiled = |classes: &[String]| -> Vec<String> {
        let mut out = Vec::new();
        for name in classes {
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

    let mut subgraph_parent: HashMap<&str, &str> = HashMap::new();
    for sg in subgraphs {
        for child in &sg.nodes {
            if db.subgraph(child).is_some() {
                subgraph_parent.insert(child, &sg.id);
            }
        }
    }

    let any_collapsed = subgraphs.iter().any(|sg| collapsed(db, &sg.id));
    // The outermost collapsed subgraph around each subgraph.
    let outermost = |id: &str| -> Option<String> {
        let mut result = None;
        let mut current = Some(id);
        let mut seen = HashSet::new();
        while let Some(c) = current {
            if !seen.insert(c) {
                break;
            }
            if collapsed(db, c) {
                result = Some(c.to_string());
            }
            current = subgraph_parent.get(c).copied();
        }
        result
    };

    let mut hidden: HashSet<String> = HashSet::new();
    let mut collapsed_into: HashMap<String, String> = HashMap::new();
    if any_collapsed {
        for sg in subgraphs {
            let Some(ancestor) = outermost(&sg.id) else {
                continue;
            };
            if sg.id != ancestor {
                hidden.insert(sg.id.clone());
                collapsed_into.insert(sg.id.clone(), ancestor.clone());
            }
            for child in &sg.nodes {
                if *child == ancestor {
                    continue;
                }
                hidden.insert(child.clone());
                collapsed_into.insert(child.clone(), ancestor.clone());
            }
        }
    }

    let mut parent_of: HashMap<&str, &str> = HashMap::new();
    let mut groups: HashSet<&str> = HashSet::new();
    for sg in subgraphs.iter().rev() {
        if hidden.contains(&sg.id) {
            continue;
        }
        if !sg.nodes.is_empty() {
            groups.insert(&sg.id);
        }
        for id in &sg.nodes {
            parent_of.insert(id, &sg.id);
        }
    }

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut by_id: HashMap<String, usize> = HashMap::new();
    for sg in subgraphs.iter().rev() {
        if hidden.contains(&sg.id) {
            continue;
        }
        let is_collapsed = collapsed(db, &sg.id);
        by_id.insert(sg.id.clone(), nodes.len());
        nodes.push(GraphNode {
            id: sg.id.clone(),
            label: Some(sg.title.clone()),
            label_type: sg.label_type,
            shape: if is_collapsed {
                "collapsedGroup"
            } else {
                "rect"
            }
            .to_string(),
            is_group: !is_collapsed,
            parent_id: parent_of.get(sg.id.as_str()).map(|s| s.to_string()),
            css_styles: Vec::new(),
            css_compiled_styles: compiled(&sg.classes),
            css_classes: sg.classes.join(" "),
            dir: sg.dir.clone(),
            link: None,
            link_target: None,
            tooltip: None,
            icon: None,
            pos: None,
            img: None,
            asset_width: None,
            asset_height: None,
            constraint: None,
        });
    }

    for vertex in &db.nodes {
        if hidden.contains(&vertex.id) {
            continue;
        }
        if let Some(&i) = by_id.get(&vertex.id) {
            let existing = &mut nodes[i];
            existing.css_styles = vertex.styles.clone();
            existing.css_compiled_styles = compiled(&vertex.classes);
            existing.css_classes = vertex.classes.join(" ");
            continue;
        }
        let is_group = groups.contains(vertex.id.as_str());
        let mut classes = vec!["default".to_string(), "node".to_string()];
        classes.extend(vertex.classes.iter().cloned());
        nodes.push(GraphNode {
            id: vertex.id.clone(),
            label: vertex.text.clone(),
            label_type: vertex.label_type,
            shape: if is_group {
                "rect".to_string()
            } else {
                shape_of(vertex)
            },
            is_group,
            parent_id: parent_of.get(vertex.id.as_str()).map(|s| s.to_string()),
            css_styles: vertex.styles.clone(),
            css_compiled_styles: compiled(&classes),
            css_classes: format!("default {}", vertex.classes.join(" ")),
            dir: None,
            link: vertex.link.clone(),
            link_target: vertex.link_target.clone(),
            tooltip: db.tooltips.get(&vertex.id).cloned(),
            icon: vertex.icon.clone(),
            pos: vertex.pos.clone(),
            img: vertex.img.clone(),
            asset_width: vertex.asset_width,
            asset_height: vertex.asset_height,
            constraint: vertex.constraint.clone(),
        });
    }

    let mut edges = Vec::new();
    for (index, raw) in db.edges.iter().enumerate() {
        let start = collapsed_into
            .get(&raw.start)
            .cloned()
            .unwrap_or_else(|| raw.start.clone());
        let end = collapsed_into
            .get(&raw.end)
            .cloned()
            .unwrap_or_else(|| raw.end.clone());
        if start == end
            && (collapsed_into.contains_key(&raw.start) || collapsed_into.contains_key(&raw.end))
        {
            continue;
        }
        let (arrow_start, arrow_end) = arrows(raw.kind.as_deref());
        let mut style = db.default_edge_style.clone().unwrap_or_default();
        if let Some(own) = &raw.style {
            style.extend(own.iter().cloned());
        }
        let invisible = raw.stroke.as_deref() == Some("invisible");
        let plain = invisible || raw.kind.as_deref() == Some("arrow_open");
        edges.push(GraphEdge {
            id: if raw.id.is_empty() {
                format!("L_{start}_{end}_{index}")
            } else {
                raw.id.clone()
            },
            start,
            end,
            label: raw.text.clone(),
            label_type: raw.label_type,
            thickness: raw.stroke.clone(),
            minlen: raw.length,
            arrow_start: if plain {
                "none".to_string()
            } else {
                arrow_start
            },
            arrow_end: if plain { "none".to_string() } else { arrow_end },
            css_compiled_styles: compiled(&raw.classes),
            style,
            classes: if invisible {
                String::new()
            } else {
                "edge-thickness-normal edge-pattern-solid flowchart-link".to_string()
            },
            curve: raw
                .interpolate
                .clone()
                .filter(|c| !c.is_empty())
                .or_else(|| db.default_interpolate.clone().filter(|c| !c.is_empty()))
                .or_else(|| default_curve.map(str::to_string)),
        });
    }
    FlowGraph { nodes, edges }
}
