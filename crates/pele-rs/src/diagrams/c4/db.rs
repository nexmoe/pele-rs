//! The model of a C4 diagram: the shapes, the boundaries and the relations between them.
//!
//! The model follows Mermaid's `c4Db.ts` as pele's `db.ts` ports it. One place differs:
//! Mermaid keeps a named argument written in the label slot as the text of the label, where
//! pele applies it to the field it names; this follows pele, whose shape the golden file
//! records.

use std::collections::HashMap;

use crate::util::collapse_indent;

/// A map that keeps the order its keys were added in, as a JavaScript `Map` does.
#[derive(Debug, Clone, Default)]
pub(crate) struct Map {
    pub items: Vec<(String, String)>,
}

impl Map {
    /// Sets a value; a key already there keeps its place.
    fn set(&mut self, key: &str, value: String) {
        if let Some(entry) = self.items.iter_mut().find(|(name, _)| name == key) {
            entry.1 = value;
            return;
        }
        self.items.push((key.to_string(), value));
    }
}

/// A `$key="value"` argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Named {
    pub key: String,
    pub value: String,
}

/// A positional argument: a text or a named one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Attr {
    Text(String),
    Named(Named),
}

impl Attr {
    /// The text of the argument, when it is a plain one.
    pub(crate) fn text(&self) -> Option<&str> {
        match self {
            Attr::Text(t) => Some(t),
            Attr::Named(_) => None,
        }
    }
}

/// The fields a named argument of a shape may set.
const ELEMENT_FIELDS: &[&str] = &[
    "label",
    "type",
    "descr",
    "techn",
    "sprite",
    "tags",
    "link",
    "bgColor",
    "fontColor",
    "borderColor",
    "shadowing",
    "shape",
    "legendText",
    "legendSprite",
];

/// The fields a named argument of a relation may set.
const REL_FIELDS: &[&str] = &[
    "label",
    "descr",
    "techn",
    "sprite",
    "tags",
    "link",
    "textColor",
    "lineColor",
];

/// A shape or a boundary.
#[derive(Debug, Clone, Default)]
pub(crate) struct Element {
    pub alias: String,
    pub label: String,
    /// A named argument written where the alias belongs.
    pub alias_attr: Option<Named>,
    /// A named argument written where the label belongs.
    pub label_attr: Option<Named>,
    /// The boundary this element sits in.
    pub parent: Option<usize>,
    pub type_: Option<String>,
    pub descr: Option<String>,
    pub techn: Option<String>,
    pub sprite: Option<String>,
    pub tags: Option<String>,
    pub link: Option<String>,
    pub bg_color: Option<String>,
    pub font_color: Option<String>,
    pub border_color: Option<String>,
    pub shadowing: Option<String>,
    pub shape: Option<String>,
    pub legend_text: Option<String>,
    pub legend_sprite: Option<String>,
    /// Named arguments that set no known field.
    pub extra: Map,
}

impl Element {
    fn set_field(&mut self, key: &str, value: &str) {
        match key {
            "label" => self.label = value.to_string(),
            "type" => self.type_ = Some(value.to_string()),
            "descr" => self.descr = Some(value.to_string()),
            "techn" => self.techn = Some(value.to_string()),
            "sprite" => self.sprite = Some(value.to_string()),
            "tags" => self.tags = Some(value.to_string()),
            "link" => self.link = Some(value.to_string()),
            "bgColor" => self.bg_color = Some(value.to_string()),
            "fontColor" => self.font_color = Some(value.to_string()),
            "borderColor" => self.border_color = Some(value.to_string()),
            "shadowing" => self.shadowing = Some(value.to_string()),
            "shape" => self.shape = Some(value.to_string()),
            "legendText" => self.legend_text = Some(value.to_string()),
            "legendSprite" => self.legend_sprite = Some(value.to_string()),
            _ => {}
        }
    }

    /// A named argument reaches the field it names, structural ones included.
    fn named(&mut self, attr: &Named) {
        if ELEMENT_FIELDS.contains(&attr.key.as_str()) {
            self.set_field(&attr.key, &attr.value);
        } else {
            self.extra.set(&attr.key, attr.value.clone());
        }
    }

    /// A positional argument fills the field of its slot; a named one fills the field it names.
    fn put(&mut self, field: &str, attr: Option<&Attr>) {
        let Some(attr) = attr else {
            return;
        };
        match attr {
            Attr::Text(t) => self.set_field(field, t),
            Attr::Named(n) => self.named(n),
        }
    }

    /// A text field that an argument that is not there leaves empty, unless it already holds
    /// something.
    fn text_field(&mut self, field: &str, attr: Option<&Attr>) {
        match attr {
            None => {
                if self.field(field).is_none() {
                    self.set_field(field, "");
                }
            }
            Some(attr) => self.put(field, Some(attr)),
        }
    }

    /// The text of a field, when it holds one.
    fn field(&self, key: &str) -> Option<&str> {
        match key {
            "type" => self.type_.as_deref(),
            "descr" => self.descr.as_deref(),
            "techn" => self.techn.as_deref(),
            "sprite" => self.sprite.as_deref(),
            "tags" => self.tags.as_deref(),
            "link" => self.link.as_deref(),
            _ => None,
        }
    }

    /// The label of the element. A named argument in this slot is applied to the field it names.
    fn set_label(&mut self, attr: Option<&Attr>) {
        self.label_attr = None;
        match attr {
            None => self.label = String::new(),
            Some(Attr::Text(t)) => self.label = t.clone(),
            Some(Attr::Named(n)) => {
                self.label = String::new();
                self.label_attr = Some(n.clone());
                self.named(n);
            }
        }
    }
}

/// A shape of the diagram.
#[derive(Debug, Clone, Default)]
pub(crate) struct Shape {
    pub element: Element,
    /// Mermaid's `typeC4Shape`: `person`, `external_system_db`, `container_queue` and so on.
    pub kind: String,
}

/// A boundary of the diagram: a box that holds shapes and other boundaries.
#[derive(Debug, Clone, Default)]
pub(crate) struct Boundary {
    pub element: Element,
    /// Set on deployment nodes: `node`, `nodeL` or `nodeR`.
    pub node_type: Option<String>,
}

/// A relation between two shapes.
#[derive(Debug, Clone, Default)]
pub(crate) struct Rel {
    pub kind: String,
    pub from: String,
    pub to: String,
    pub label: String,
    pub from_attr: Option<Named>,
    pub to_attr: Option<Named>,
    pub label_attr: Option<Named>,
    pub techn: Option<String>,
    pub descr: Option<String>,
    pub sprite: Option<String>,
    pub tags: Option<String>,
    pub link: Option<String>,
    pub text_color: Option<String>,
    pub line_color: Option<String>,
    pub offset_x: Option<f64>,
    pub offset_y: Option<f64>,
    /// Named arguments that set no known field.
    pub extra: Map,
}

impl Rel {
    fn set_field(&mut self, key: &str, value: &str) {
        match key {
            "label" => self.label = value.to_string(),
            "descr" => self.descr = Some(value.to_string()),
            "techn" => self.techn = Some(value.to_string()),
            "sprite" => self.sprite = Some(value.to_string()),
            "tags" => self.tags = Some(value.to_string()),
            "link" => self.link = Some(value.to_string()),
            "textColor" => self.text_color = Some(value.to_string()),
            "lineColor" => self.line_color = Some(value.to_string()),
            _ => {}
        }
    }

    fn named(&mut self, attr: &Named) {
        if attr.key == "offsetX" || attr.key == "offsetY" {
            let value = parse_int(&attr.value);
            if attr.key == "offsetX" {
                self.offset_x = Some(value);
            } else {
                self.offset_y = Some(value);
            }
        } else if REL_FIELDS.contains(&attr.key.as_str()) {
            self.set_field(&attr.key, &attr.value);
        } else {
            self.extra.set(&attr.key, attr.value.clone());
        }
    }

    fn put(&mut self, field: &str, attr: Option<&Attr>) {
        let Some(attr) = attr else {
            return;
        };
        match attr {
            Attr::Text(t) => self.set_field(field, t),
            Attr::Named(n) => self.named(n),
        }
    }

    fn text_field(&mut self, field: &str, attr: Option<&Attr>) {
        match attr {
            None => {
                if self.field(field).is_none() {
                    self.set_field(field, "");
                }
            }
            Some(attr) => self.put(field, Some(attr)),
        }
    }

    /// The text of a field, when it holds one.
    fn field(&self, key: &str) -> Option<&str> {
        match key {
            "descr" => self.descr.as_deref(),
            "techn" => self.techn.as_deref(),
            _ => None,
        }
    }

    fn set_label(&mut self, attr: Option<&Attr>) {
        self.label_attr = None;
        match attr {
            None => self.label = String::new(),
            Some(Attr::Text(t)) => self.label = t.clone(),
            Some(Attr::Named(n)) => {
                self.label = String::new();
                self.label_attr = Some(n.clone());
                self.named(n);
            }
        }
    }
}

/// What a statement of the grammar does to the model.
#[derive(Debug, Clone)]
pub(crate) enum Action {
    SetC4Type(String),
    SetTitle(String),
    SetAccDescription(String),
    PersonOrSystem { kind: String, attrs: Vec<Attr> },
    Container { kind: String, attrs: Vec<Attr> },
    Boundary { container: bool, attrs: Vec<Attr> },
    DeploymentNode { node_type: String, attrs: Vec<Attr> },
    Rel { kind: String, attrs: Vec<Attr> },
    UpdateElStyle(Vec<Attr>),
    UpdateRelStyle(Vec<Attr>),
    UpdateLayoutConfig(Vec<Attr>),
    PopBoundary,
}

/// Builds the model from the statements of the grammar.
pub(crate) fn build(actions: Vec<Action>) -> Model {
    let mut model = Model::default();
    for action in actions {
        match action {
            Action::SetC4Type(kind) => model.set_c4_type(kind),
            Action::SetTitle(title) => model.set_title(title),
            Action::SetAccDescription(descr) => model.set_acc_description(descr),
            Action::PersonOrSystem { kind, attrs } => model.add_person_or_system(&kind, &attrs),
            Action::Container { kind, attrs } => model.add_container(&kind, &attrs),
            Action::Boundary { container, attrs } => {
                if container {
                    model.add_container_boundary(&attrs);
                } else {
                    model.add_person_or_system_boundary(&attrs);
                }
            }
            Action::DeploymentNode { node_type, attrs } => {
                model.add_deployment_node(&node_type, &attrs);
            }
            Action::Rel { kind, attrs } => model.add_rel(&kind, &attrs),
            Action::UpdateElStyle(attrs) => model.update_el_style(&attrs),
            Action::UpdateRelStyle(attrs) => model.update_rel_style(&attrs),
            Action::UpdateLayoutConfig(attrs) => model.update_layout_config(&attrs),
            Action::PopBoundary => model.pop_boundary_parse_stack(),
        }
    }
    model
}

/// A shape or a boundary, as a relation names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Ref {
    Shape(usize),
    Boundary(usize),
}

/// The model of a C4 diagram.
#[derive(Debug, Clone)]
pub(crate) struct Model {
    /// The header keyword: C4Context, C4Container, C4Component, C4Dynamic or C4Deployment.
    pub c4_type: Option<String>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub shapes: Vec<Shape>,
    /// The first boundary is the unnamed one that holds everything at the top level.
    pub boundaries: Vec<Boundary>,
    pub rels: Vec<Rel>,
    pub shape_in_row: usize,
    pub boundary_in_row: usize,
    shape_index: HashMap<String, usize>,
    // The top-level boundary is not listed, so a boundary a diagram names `global` is its own box.
    boundary_index: HashMap<String, usize>,
    rel_index: HashMap<(String, String), usize>,
    current: usize,
    stack: Vec<usize>,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            c4_type: None,
            title: None,
            acc_title: None,
            acc_descr: None,
            shapes: Vec::new(),
            boundaries: vec![Boundary {
                element: Element {
                    alias: "global".to_string(),
                    label: "global".to_string(),
                    type_: Some("global".to_string()),
                    ..Element::default()
                },
                node_type: None,
            }],
            rels: Vec::new(),
            shape_in_row: 4,
            boundary_in_row: 2,
            shape_index: HashMap::new(),
            boundary_index: HashMap::new(),
            rel_index: HashMap::new(),
            current: 0,
            stack: Vec::new(),
        }
    }
}

impl Model {
    pub(crate) fn set_c4_type(&mut self, kind: String) {
        self.c4_type = Some(kind);
    }

    pub(crate) fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }

    pub(crate) fn set_acc_description(&mut self, descr: String) {
        self.acc_descr = Some(collapse_indent(&descr));
    }

    /// The shape or the boundary an alias names.
    pub(crate) fn get_c4_shape(&self, alias: &str) -> Option<Ref> {
        if let Some(&at) = self.shape_index.get(alias) {
            return Some(Ref::Shape(at));
        }
        self.boundary_index.get(alias).map(|&at| Ref::Boundary(at))
    }

    fn shape_element(&mut self, alias: Option<&Attr>, title: Option<&Attr>) -> usize {
        let found = match alias {
            Some(Attr::Text(a)) => self.shape_index.get(a).copied(),
            _ => None,
        };
        let at = match found {
            Some(at) => at,
            None => {
                let mut element = Element::default();
                let at = self.shapes.len();
                match alias {
                    Some(Attr::Text(a)) => {
                        element.alias = a.clone();
                        self.shape_index.insert(a.clone(), at);
                    }
                    Some(Attr::Named(n)) => {
                        element.alias_attr = Some(n.clone());
                        element.named(n);
                    }
                    None => {}
                }
                self.shapes.push(Shape {
                    element,
                    kind: String::new(),
                });
                at
            }
        };
        self.shapes[at].element.set_label(title);
        at
    }

    fn boundary_element(&mut self, alias: Option<&Attr>, title: Option<&Attr>) -> usize {
        let found = match alias {
            Some(Attr::Text(a)) => self.boundary_index.get(a).copied(),
            _ => None,
        };
        let at = match found {
            Some(at) => at,
            None => {
                let mut element = Element::default();
                let at = self.boundaries.len();
                match alias {
                    Some(Attr::Text(a)) => {
                        element.alias = a.clone();
                        self.boundary_index.insert(a.clone(), at);
                    }
                    Some(Attr::Named(n)) => {
                        element.alias_attr = Some(n.clone());
                        element.named(n);
                    }
                    None => {}
                }
                self.boundaries.push(Boundary {
                    element,
                    node_type: None,
                });
                at
            }
        };
        self.boundaries[at].element.set_label(title);
        at
    }

    /// A person or a system: alias, label, description, sprite, tags, link.
    pub(crate) fn add_person_or_system(&mut self, kind: &str, attrs: &[Attr]) {
        let at = self.shape_element(attrs.first(), attrs.get(1));
        self.shapes[at].element.text_field("descr", attrs.get(2));
        self.finish_shape(at, kind, attrs, 3);
    }

    /// A container or a component: alias, label, technology, description, sprite, tags, link.
    pub(crate) fn add_container(&mut self, kind: &str, attrs: &[Attr]) {
        let at = self.shape_element(attrs.first(), attrs.get(1));
        self.shapes[at].element.text_field("techn", attrs.get(2));
        self.shapes[at].element.text_field("descr", attrs.get(3));
        self.finish_shape(at, kind, attrs, 4);
    }

    fn finish_shape(&mut self, at: usize, kind: &str, attrs: &[Attr], from: usize) {
        let current = self.current;
        let element = &mut self.shapes[at].element;
        element.put("sprite", attrs.get(from));
        element.put("tags", attrs.get(from + 1));
        element.put("link", attrs.get(from + 2));
        element.parent = Some(current);
        self.shapes[at].kind = kind.to_string();
    }

    /// A boundary: alias, label, type, tags, link.
    fn boundary(&mut self, attrs: &[Attr], fallback: &str) -> usize {
        let at = self.boundary_element(attrs.first(), attrs.get(1));
        match attrs.get(2) {
            None => self.boundaries[at].element.type_ = Some(fallback.to_string()),
            Some(attr) => self.boundaries[at].element.put("type", Some(attr)),
        }
        at
    }

    fn open(&mut self, at: usize) {
        let current = self.current;
        self.boundaries[at].element.parent = Some(current);
        self.stack.push(current);
        self.current = at;
    }

    pub(crate) fn add_person_or_system_boundary(&mut self, attrs: &[Attr]) {
        let at = self.boundary(attrs, "system");
        self.boundaries[at].element.put("tags", attrs.get(3));
        self.boundaries[at].element.put("link", attrs.get(4));
        self.open(at);
    }

    pub(crate) fn add_container_boundary(&mut self, attrs: &[Attr]) {
        let at = self.boundary(attrs, "container");
        self.boundaries[at].element.put("tags", attrs.get(3));
        self.boundaries[at].element.put("link", attrs.get(4));
        self.open(at);
    }

    /// A deployment node: alias, label, type, description, sprite, tags, link.
    pub(crate) fn add_deployment_node(&mut self, node_type: &str, attrs: &[Attr]) {
        let at = self.boundary(attrs, "node");
        self.boundaries[at]
            .element
            .text_field("descr", attrs.get(3));
        self.boundaries[at].element.put("sprite", attrs.get(4));
        self.boundaries[at].element.put("tags", attrs.get(5));
        self.boundaries[at].element.put("link", attrs.get(6));
        self.boundaries[at].node_type = Some(node_type.to_string());
        self.open(at);
    }

    pub(crate) fn pop_boundary_parse_stack(&mut self) {
        self.current = self.stack.pop().unwrap_or(0);
    }

    /// A relation: from, to, label, technology, description, sprite, tags, link.
    pub(crate) fn add_rel(&mut self, kind: &str, attrs: &[Attr]) {
        let (Some(from), Some(to), Some(label)) = (attrs.first(), attrs.get(1), attrs.get(2))
        else {
            return;
        };
        let known = matches!(from, Attr::Text(_)) && matches!(to, Attr::Text(_));
        let found = match (from.text(), to.text()) {
            (Some(f), Some(t)) if known => {
                self.rel_index.get(&(f.to_string(), t.to_string())).copied()
            }
            _ => None,
        };
        let at = match found {
            Some(at) => at,
            None => {
                let mut rel = Rel {
                    kind: kind.to_string(),
                    ..Rel::default()
                };
                let at = self.rels.len();
                match from {
                    Attr::Text(t) => {
                        rel.from = t.clone();
                        if let Some(to) = to.text() {
                            self.rel_index.insert((t.clone(), to.to_string()), at);
                        }
                    }
                    Attr::Named(n) => rel.from_attr = Some(n.clone()),
                }
                match to {
                    Attr::Text(t) => rel.to = t.clone(),
                    Attr::Named(n) => rel.to_attr = Some(n.clone()),
                }
                self.rels.push(rel);
                at
            }
        };
        self.rels[at].kind = kind.to_string();
        self.rels[at].set_label(Some(label));
        self.rels[at].text_field("techn", attrs.get(3));
        self.rels[at].text_field("descr", attrs.get(4));
        self.rels[at].put("sprite", attrs.get(5));
        self.rels[at].put("tags", attrs.get(6));
        self.rels[at].put("link", attrs.get(7));
    }

    /// The style of a shape: name, background, font, border, shadowing, shape, sprite,
    /// technology, legend text, legend sprite.
    pub(crate) fn update_el_style(&mut self, attrs: &[Attr]) {
        let Some(name) = attrs.first().and_then(Attr::text) else {
            return;
        };
        let found = self.get_c4_shape(name);
        let element = match found {
            Some(Ref::Shape(at)) => &mut self.shapes[at].element,
            Some(Ref::Boundary(at)) => &mut self.boundaries[at].element,
            None => return,
        };
        element.put("bgColor", attrs.get(1));
        element.put("fontColor", attrs.get(2));
        element.put("borderColor", attrs.get(3));
        element.put("shadowing", attrs.get(4));
        element.put("shape", attrs.get(5));
        element.put("sprite", attrs.get(6));
        element.put("techn", attrs.get(7));
        element.put("legendText", attrs.get(8));
        element.put("legendSprite", attrs.get(9));
    }

    /// The style of a relation: from, to, text color, line color, offset x, offset y.
    pub(crate) fn update_rel_style(&mut self, attrs: &[Attr]) {
        let (Some(from), Some(to)) = (
            attrs.first().and_then(Attr::text),
            attrs.get(1).and_then(Attr::text),
        ) else {
            return;
        };
        let Some(&at) = self.rel_index.get(&(from.to_string(), to.to_string())) else {
            return;
        };
        self.rels[at].put("textColor", attrs.get(2));
        self.rels[at].put("lineColor", attrs.get(3));
        match attrs.get(4) {
            Some(Attr::Text(x)) => self.rels[at].offset_x = Some(parse_int(x)),
            Some(Attr::Named(n)) => self.rels[at].named(n),
            None => {}
        }
        match attrs.get(5) {
            Some(Attr::Text(y)) => self.rels[at].offset_y = Some(parse_int(y)),
            Some(Attr::Named(n)) => self.rels[at].named(n),
            None => {}
        }
    }

    /// The layout of the diagram: shapes in a row, boundaries in a row. As in Mermaid, the slot
    /// decides which one a value sets, not its name.
    pub(crate) fn update_layout_config(&mut self, attrs: &[Attr]) {
        let count = |attr: Option<&Attr>| -> f64 {
            match attr {
                None => f64::NAN,
                Some(Attr::Text(t)) => parse_int(t),
                Some(Attr::Named(n)) => parse_int(&n.value),
            }
        };
        let shapes = count(attrs.first());
        let boundaries = count(attrs.get(1));
        if shapes >= 1.0 {
            self.shape_in_row = shapes as usize;
        }
        if boundaries >= 1.0 {
            self.boundary_in_row = boundaries as usize;
        }
    }
}

/// JavaScript's `parseInt`: the longest prefix that is a whole number, and whatever follows it
/// ignored.
fn parse_int(text: &str) -> f64 {
    let s = crate::util::trim(text);
    let bytes = s.as_bytes();
    let mut i = 0;
    let negative = match bytes.first() {
        Some(b'-') => {
            i = 1;
            true
        }
        Some(b'+') => {
            i = 1;
            false
        }
        _ => false,
    };
    let radix = if s[i..].starts_with("0x") || s[i..].starts_with("0X") {
        i += 2;
        16
    } else {
        10
    };
    let from = i;
    while i < bytes.len() {
        let d = (bytes[i] as char).to_digit(radix);
        match d {
            Some(_) => i += 1,
            None => break,
        }
    }
    if i == from {
        return f64::NAN;
    }
    let value = i64::from_str_radix(&s[from..i], radix).unwrap_or(0) as f64;
    if negative { -value } else { value }
}
