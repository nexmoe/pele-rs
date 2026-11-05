//! What a flowchart describes, built up statement by statement as Mermaid's `FlowDB` does.
//! Ported from pele's `diagrams/flowchart/db.ts`.

use std::collections::{HashMap, HashSet};

use super::shapes::is_valid_shape;
use crate::error::{Error, ErrorCode};
use crate::preprocess::{Config, Value, parse_yaml};
use crate::url::safe_url;
use crate::util::{is_space, trim, trim_start};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LabelType {
    Text,
    String,
    Markdown,
}

fn label_type(value: Option<&Value>) -> LabelType {
    match value {
        Some(Value::String(s)) if s == "text" => LabelType::Text,
        Some(Value::String(s)) if s == "string" => LabelType::String,
        _ => LabelType::Markdown,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FlowText {
    pub text: String,
    pub kind: LabelType,
}

#[derive(Debug, Clone)]
pub(crate) struct FlowNode {
    pub id: String,
    pub text: Option<String>,
    pub label_type: LabelType,
    pub kind: Option<String>,
    pub styles: Vec<String>,
    pub classes: Vec<String>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub icon: Option<String>,
    pub form: Option<String>,
    pub pos: Option<String>,
    pub img: Option<String>,
    pub constraint: Option<String>,
    pub asset_width: Option<f64>,
    pub asset_height: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct FlowEdge {
    pub start: String,
    pub end: String,
    pub kind: Option<String>,
    pub text: String,
    pub label_type: LabelType,
    pub stroke: Option<String>,
    pub length: Option<i32>,
    pub style: Option<Vec<String>>,
    pub classes: Vec<String>,
    pub id: String,
    pub interpolate: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FlowClassDef {
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct FlowSubgraph {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    pub dir: Option<String>,
    pub label_type: LabelType,
    pub metadata: Option<Config>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LinkInfo {
    pub kind: String,
    pub stroke: String,
    pub length: Option<i32>,
    pub text: Option<FlowText>,
    pub id: Option<String>,
}

/// One entry of a subgraph's statement list.
#[derive(Debug, Clone)]
pub(crate) enum DocItem {
    Id(String),
    Ids(Vec<String>),
    Dir(String),
}

fn unquote(text: &str) -> &str {
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        &text[1..text.len() - 1]
    } else {
        text
    }
}

fn destruct_start_link(raw: &str) -> (String, String) {
    let mut s = trim(raw);
    let mut kind = "arrow_open";
    match s.chars().next() {
        Some('<') => {
            kind = "arrow_point";
            s = &s[1..];
        }
        Some('x') => {
            kind = "arrow_cross";
            s = &s[1..];
        }
        Some('o') => {
            kind = "arrow_circle";
            s = &s[1..];
        }
        _ => {}
    }
    let mut stroke = "normal";
    if s.contains('=') {
        stroke = "thick";
    }
    if s.contains('.') {
        stroke = "dotted";
    }
    (kind.to_string(), stroke.to_string())
}

fn destruct_end_link(raw: &str) -> LinkInfo {
    let s = trim(raw);
    let mut line = &s[..s.len().saturating_sub(1)];
    let mut kind = "arrow_open".to_string();
    match s.chars().last() {
        Some('x') => {
            kind = "arrow_cross".to_string();
            if s.starts_with('x') {
                kind = format!("double_{kind}");
                line = line.get(1..).unwrap_or("");
            }
        }
        Some('>') => {
            kind = "arrow_point".to_string();
            if s.starts_with('<') {
                kind = format!("double_{kind}");
                line = line.get(1..).unwrap_or("");
            }
        }
        Some('o') => {
            kind = "arrow_circle".to_string();
            if s.starts_with('o') {
                kind = format!("double_{kind}");
                line = line.get(1..).unwrap_or("");
            }
        }
        _ => {}
    }
    let mut stroke = "normal";
    let mut length = line.len() as i32 - 1;
    if line.starts_with('=') {
        stroke = "thick";
    }
    if line.starts_with('~') {
        stroke = "invisible";
    }
    let dots = line.matches('.').count() as i32;
    if dots > 0 {
        stroke = "dotted";
        length = dots;
    }
    LinkInfo {
        kind,
        stroke: stroke.to_string(),
        length: Some(length),
        text: None,
        id: None,
    }
}

/// Text from a YAML value, as JavaScript's `String(value)` writes it.
fn yaml_string(value: &Value) -> String {
    value.js_string()
}

fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => *n != 0.0 && !n.is_nan(),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// JavaScript's `Number(value)` for a YAML value.
fn yaml_number(value: &Value) -> f64 {
    match value {
        Value::Number(n) => *n,
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Null => 0.0,
        Value::String(s) => {
            let t = trim(s);
            if t.is_empty() {
                0.0
            } else {
                t.parse().unwrap_or(f64::NAN)
            }
        }
        _ => f64::NAN,
    }
}

pub(crate) struct FlowDb {
    pub direction: Option<String>,
    /// Nodes in the order they were first named.
    pub nodes: Vec<FlowNode>,
    node_index: HashMap<String, usize>,
    pub edges: Vec<FlowEdge>,
    pub subgraphs: Vec<FlowSubgraph>,
    pub classes: HashMap<String, FlowClassDef>,
    pub tooltips: HashMap<String, String>,
    pub default_edge_style: Option<Vec<String>>,
    pub default_interpolate: Option<String>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    subgraph_lookup: HashMap<String, usize>,
    members: HashSet<String>,
    edge_by_id: HashMap<String, usize>,
    pair_count: HashMap<(String, String), usize>,
    sub_count: usize,
    max_edges: usize,
    inherit_dir: bool,
}

fn semantic(message: String) -> Error {
    Error::new(ErrorCode::Semantic, message)
}

impl FlowDb {
    pub(crate) fn new(max_edges: usize, inherit_dir: bool) -> Self {
        Self {
            direction: None,
            nodes: Vec::new(),
            node_index: HashMap::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
            classes: HashMap::new(),
            tooltips: HashMap::new(),
            default_edge_style: None,
            default_interpolate: None,
            title: None,
            acc_title: None,
            acc_descr: None,
            subgraph_lookup: HashMap::new(),
            members: HashSet::new(),
            edge_by_id: HashMap::new(),
            pair_count: HashMap::new(),
            sub_count: 0,
            max_edges,
            inherit_dir,
        }
    }

    pub(crate) fn subgraph(&self, id: &str) -> Option<&FlowSubgraph> {
        self.subgraph_lookup.get(id).map(|&i| &self.subgraphs[i])
    }

    /// Names a node, or adds to what is known of it. `metadata` is the YAML text of a `@{…}`.
    pub(crate) fn add_vertex(
        &mut self,
        id: &str,
        text: Option<&FlowText>,
        kind: Option<&str>,
        style: Option<Vec<String>>,
        metadata: Option<&str>,
    ) -> Result<(), Error> {
        if trim(id).is_empty() {
            return Ok(());
        }
        let mut doc: Option<Config> = None;
        if let Some(metadata) = metadata {
            let yaml = if metadata.contains('\n') {
                format!("{metadata}\n")
            } else {
                format!("{{\n{metadata}\n}}")
            };
            match parse_yaml(&yaml) {
                Ok(Value::Map(map)) => doc = Some(map),
                Ok(_) => {}
                Err(message) => {
                    return Err(Error::new(
                        ErrorCode::Syntax,
                        format!("Metadata of \"{id}\" is not valid YAML. {message}"),
                    ));
                }
            }
        }

        if let (Some(&si), Some(doc)) = (self.subgraph_lookup.get(id), doc.as_ref()) {
            let meta = self.subgraphs[si].metadata.get_or_insert_with(Config::new);
            for (k, v) in doc {
                meta.insert(k.clone(), v.clone());
            }
            return Ok(());
        }

        if let Some(&ei) = self.edge_by_id.get(id) {
            if let Some(curve) = doc.as_ref().and_then(|d| d.get("curve")) {
                self.edges[ei].interpolate = Some(yaml_string(curve));
            }
            return Ok(());
        }

        let index = match self.node_index.get(id) {
            Some(&i) => i,
            None => {
                self.node_index.insert(id.to_string(), self.nodes.len());
                self.nodes.push(FlowNode {
                    id: id.to_string(),
                    text: None,
                    label_type: LabelType::Text,
                    kind: None,
                    styles: Vec::new(),
                    classes: Vec::new(),
                    link: None,
                    link_target: None,
                    icon: None,
                    form: None,
                    pos: None,
                    img: None,
                    constraint: None,
                    asset_width: None,
                    asset_height: None,
                });
                self.nodes.len() - 1
            }
        };
        let vertex = &mut self.nodes[index];
        if let Some(text) = text {
            vertex.label_type = text.kind;
            vertex.text = Some(unquote(trim(&text.text)).to_string());
        } else if vertex.text.is_none() {
            vertex.text = Some(id.to_string());
        }
        if let Some(kind) = kind {
            vertex.kind = Some(kind.to_string());
        }
        if let Some(style) = style {
            vertex.styles.extend(style);
        }

        let Some(doc) = doc else { return Ok(()) };
        if let Some(shape) = doc.get("shape").filter(|v| truthy(Some(v))) {
            let shape = yaml_string(shape);
            if shape != shape.to_lowercase() || shape.contains('_') {
                return Err(semantic(format!(
                    "No such shape: {shape}. Shape names should be lowercase."
                )));
            }
            if !is_valid_shape(&shape) {
                return Err(semantic(format!("No such shape: {shape}.")));
            }
            vertex.kind = Some(shape);
        }
        let label = match doc.get("label") {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        };
        if truthy(doc.get("label")) {
            vertex.text = doc.get("label").map(yaml_string);
            vertex.label_type = label_type(doc.get("labelType"));
        }
        let blank_label = label.as_deref().is_none_or(|l| trim(l).is_empty());
        if let Some(icon) = doc.get("icon").filter(|v| truthy(Some(v))) {
            vertex.icon = Some(yaml_string(icon));
            if blank_label && vertex.text.as_deref() == Some(id) {
                vertex.text = Some(String::new());
            }
        }
        if let Some(v) = doc.get("form").filter(|v| truthy(Some(v))) {
            vertex.form = Some(yaml_string(v));
        }
        if let Some(v) = doc.get("pos").filter(|v| truthy(Some(v))) {
            vertex.pos = Some(yaml_string(v));
        }
        if let Some(img) = doc.get("img").filter(|v| truthy(Some(v))) {
            vertex.img = Some(yaml_string(img));
            if blank_label && vertex.text.as_deref() == Some(id) {
                vertex.text = Some(String::new());
            }
        }
        if let Some(v) = doc.get("constraint").filter(|v| truthy(Some(v))) {
            vertex.constraint = Some(yaml_string(v));
        }
        if let Some(v) = doc.get("w").filter(|v| truthy(Some(v))) {
            vertex.asset_width = Some(yaml_number(v));
        }
        if let Some(v) = doc.get("h").filter(|v| truthy(Some(v))) {
            vertex.asset_height = Some(yaml_number(v));
        }
        Ok(())
    }

    fn add_single_link(
        &mut self,
        start: &str,
        end: &str,
        info: &LinkInfo,
        id: Option<&str>,
    ) -> Result<(), Error> {
        let mut edge = FlowEdge {
            start: start.to_string(),
            end: end.to_string(),
            kind: Some(info.kind.clone()),
            text: String::new(),
            label_type: LabelType::Text,
            stroke: Some(info.stroke.clone()),
            length: info.length.map(|l| l.min(10)),
            style: None,
            classes: Vec::new(),
            id: String::new(),
            interpolate: self.default_interpolate.clone(),
        };
        if let Some(text) = &info.text {
            edge.text = unquote(trim(&text.text)).to_string();
            edge.label_type = match text.kind {
                LabelType::Markdown => LabelType::Markdown,
                LabelType::String => LabelType::String,
                LabelType::Text => LabelType::Text,
            };
        }
        let pair = (start.to_string(), end.to_string());
        let count = self.pair_count.get(&pair).copied().unwrap_or(0);
        edge.id = match id.filter(|id| !id.is_empty() && !self.edge_by_id.contains_key(*id)) {
            Some(id) => id.to_string(),
            None => format!("L_{start}_{end}_{}", if count == 0 { 0 } else { count + 1 }),
        };
        if self.edges.len() >= self.max_edges {
            return Err(Error::new(
                ErrorCode::Limit,
                format!(
                    "Edge limit exceeded. {} edges found, but the limit is {}.",
                    self.edges.len(),
                    self.max_edges
                ),
            ));
        }
        let eid = edge.id.clone();
        self.edges.push(edge);
        self.pair_count.insert(pair, count + 1);
        self.edge_by_id.entry(eid).or_insert(self.edges.len() - 1);
        Ok(())
    }

    pub(crate) fn add_link(
        &mut self,
        starts: &[String],
        ends: &[String],
        info: &LinkInfo,
    ) -> Result<(), Error> {
        let id = info.id.as_deref().map(|id| id.replacen('@', "", 1));
        let last_start = starts.last();
        let first_end = ends.first();
        for start in starts {
            for end in ends {
                let own = Some(start) == last_start && Some(end) == first_end;
                self.add_single_link(start, end, info, if own { id.as_deref() } else { None })?;
            }
        }
        Ok(())
    }

    fn edge_at(&mut self, pos: &str) -> Result<&mut FlowEdge, Error> {
        let count = self.edges.len();
        match pos.parse::<usize>().ok().filter(|&i| i < count) {
            Some(i) => Ok(&mut self.edges[i]),
            None => Err(semantic(format!(
                "The index {pos} for linkStyle is out of bounds. Valid indices for linkStyle are \
                 between 0 and {}. (Help: Ensure that the index is within the range of existing \
                 edges.)",
                count as i64 - 1
            ))),
        }
    }

    pub(crate) fn update_link_interpolate(
        &mut self,
        positions: &[String],
        interpolate: &str,
    ) -> Result<(), Error> {
        for pos in positions {
            if pos == "default" {
                self.default_interpolate = Some(interpolate.to_string());
            } else {
                self.edge_at(pos)?.interpolate = Some(interpolate.to_string());
            }
        }
        Ok(())
    }

    pub(crate) fn update_link(
        &mut self,
        positions: &[String],
        style: &[String],
    ) -> Result<(), Error> {
        for pos in positions {
            if pos == "default" {
                self.default_edge_style = Some(style.to_vec());
                continue;
            }
            let edge = self.edge_at(pos)?;
            let mut style = style.to_vec();
            if !style.is_empty() && !style.iter().any(|s| s.starts_with("fill")) {
                style.push("fill:none".to_string());
            }
            edge.style = Some(style);
        }
        Ok(())
    }

    pub(crate) fn add_class(&mut self, ids: &str, styles: &[String]) {
        let joined = styles
            .join(",")
            .replace("\\,", "§§§")
            .replace(',', ";")
            .replace("§§§", ",");
        let style: Vec<&str> = joined.split(';').collect();
        for id in ids.split(',') {
            let class = self.classes.entry(id.to_string()).or_default();
            for s in &style {
                if s.contains("color") {
                    class.text_styles.push(s.replacen("fill", "bgFill", 1));
                }
                class.styles.push((*s).to_string());
            }
        }
    }

    pub(crate) fn set_direction(&mut self, dir: &str) {
        let mut d = trim(dir).to_string();
        if d.contains('<') {
            d = "RL".to_string();
        }
        if d.contains('^') {
            d = "BT".to_string();
        }
        if d.contains('>') {
            d = "LR".to_string();
        }
        if d.contains('v') {
            d = "TB".to_string();
        }
        if d == "TD" {
            d = "TB".to_string();
        }
        self.direction = Some(d);
    }

    pub(crate) fn set_class(&mut self, ids: &str, class: &str) {
        for id in ids.split(',') {
            if let Some(&i) = self.node_index.get(id) {
                self.nodes[i].classes.push(class.to_string());
            }
            if let Some(&i) = self.edge_by_id.get(id) {
                self.edges[i].classes.push(class.to_string());
            }
            if let Some(&i) = self.subgraph_lookup.get(id) {
                self.subgraphs[i].classes.push(class.to_string());
            }
        }
    }

    pub(crate) fn set_tooltip(&mut self, ids: &str, tooltip: &str) {
        for id in ids.split(',') {
            self.tooltips.insert(id.to_string(), tooltip.to_string());
        }
    }

    pub(crate) fn set_link(&mut self, ids: &str, link: &str, target: Option<&str>) {
        let url = trim(link);
        for id in ids.split(',') {
            if let Some(&i) = self.node_index.get(id) {
                let v = &mut self.nodes[i];
                v.link = (!url.is_empty()).then(|| safe_url(url));
                v.link_target = target.map(str::to_string);
            }
        }
        self.set_class(ids, "clickable");
    }

    /// Callbacks are never run; the node is only marked clickable, as in Mermaid's strict mode.
    pub(crate) fn set_click_event(&mut self, ids: &str) {
        self.set_class(ids, "clickable");
    }

    pub(crate) fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(trim_start(text).to_string());
    }

    pub(crate) fn set_acc_description(&mut self, text: &str) {
        // `.replace(/\n\s+/g, '\n')`
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            out.push(c);
            if c == '\n' {
                while chars.next_if(|&c| is_space(c)).is_some() {}
            }
        }
        self.acc_descr = Some(out);
    }

    pub(crate) fn add_subgraph(
        &mut self,
        id_text: Option<&FlowText>,
        list: &[DocItem],
        title_text: Option<&FlowText>,
        same: bool,
    ) -> String {
        let mut id = id_text.map(|t| trim(&t.text).to_string());
        let title = title_text.map_or(String::new(), |t| t.text.clone());
        if id_text.is_some() && same && title.chars().any(is_space) {
            id = None;
        }
        let mut seen = HashSet::new();
        let mut node_list = Vec::new();
        let mut dir = None;
        for entry in list {
            match entry {
                DocItem::Id(item) => {
                    if !trim(item).is_empty() && seen.insert(item.clone()) {
                        node_list.push(item.clone());
                    }
                }
                DocItem::Ids(items) => {
                    for item in items {
                        if !trim(item).is_empty() && seen.insert(item.clone()) {
                            node_list.push(item.clone());
                        }
                    }
                }
                DocItem::Dir(value) => dir = Some(value.clone()),
            }
        }
        if dir.is_none() && self.inherit_dir {
            dir = self.direction.clone();
        }
        let id = id.unwrap_or_else(|| format!("subGraph{}", self.sub_count));
        self.sub_count += 1;
        // A node belongs to the first subgraph that lists it, and a subgraph never contains
        // itself.
        let nodes: Vec<String> = node_list
            .into_iter()
            .filter(|n| !self.members.contains(n) && *n != id)
            .collect();
        for n in &nodes {
            self.members.insert(n.clone());
        }
        match self.subgraph_lookup.get(&id) {
            Some(&i) => self.subgraphs[i].nodes.extend(nodes),
            None => {
                self.subgraph_lookup
                    .insert(id.clone(), self.subgraphs.len());
                self.subgraphs.push(FlowSubgraph {
                    id: id.clone(),
                    nodes,
                    title: trim(&title).to_string(),
                    classes: Vec::new(),
                    dir,
                    label_type: match title_text.map(|t| t.kind) {
                        Some(LabelType::Text) => LabelType::Text,
                        Some(LabelType::String) => LabelType::String,
                        _ => LabelType::Markdown,
                    },
                    metadata: None,
                });
            }
        }
        id
    }

    pub(crate) fn destruct_link(&self, end: &str, start: Option<&str>) -> LinkInfo {
        let info = destruct_end_link(end);
        let Some(start) = start.filter(|s| !s.is_empty()) else {
            return info;
        };
        let (mut kind, stroke) = destruct_start_link(start);
        if stroke != info.stroke {
            return LinkInfo {
                kind: "INVALID".to_string(),
                stroke: "INVALID".to_string(),
                ..LinkInfo::default()
            };
        }
        if kind == "arrow_open" {
            kind = info.kind.clone();
        } else {
            if kind != info.kind {
                return LinkInfo {
                    kind: "INVALID".to_string(),
                    stroke: "INVALID".to_string(),
                    ..LinkInfo::default()
                };
            }
            kind = format!("double_{kind}");
        }
        if kind == "double_arrow" {
            kind = "double_arrow_point".to_string();
        }
        LinkInfo {
            kind,
            stroke,
            length: info.length,
            text: None,
            id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructs_links() {
        let db = FlowDb::new(100, false);
        let l = db.destruct_link(" --> ", None);
        assert_eq!(
            (l.kind.as_str(), l.stroke.as_str(), l.length),
            ("arrow_point", "normal", Some(1))
        );
        let l = db.destruct_link("<-->", None);
        assert_eq!(l.kind, "double_arrow_point");
        let l = db.destruct_link("-.->", None);
        assert_eq!((l.stroke.as_str(), l.length), ("dotted", Some(1)));
        let l = db.destruct_link("==>", Some("=="));
        assert_eq!(
            (l.kind.as_str(), l.stroke.as_str()),
            ("arrow_point", "thick")
        );
        let l = db.destruct_link("---", None);
        assert_eq!(l.kind, "arrow_open");
        let l = db.destruct_link("~~~", None);
        assert_eq!(l.stroke, "invisible");
    }
}
