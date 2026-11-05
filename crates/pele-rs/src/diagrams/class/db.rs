//! The class diagram model: Mermaid's `classDb`, as pele's `db.ts` and `members.ts` keep it.

use std::collections::{BTreeSet, HashMap};

use crate::diagrams::common::generics::parse_generic_types;
use crate::diagrams::common::ordered::Ordered;
use crate::error::{Error, ErrorCode};
use crate::text::entities::decode_entities;
use crate::url::sanitize_url;
use crate::util::{collapse_indent, is_line_end, is_space, trim, trim_start};

pub(crate) const LOLLIPOP: u8 = 4;

/// Mermaid looks up ids that start with a digit under this prefix, so they never match a class.
const DOM_ID_PREFIX: &str = "classId-";

/// A class member: an attribute, or a method with its parameters and return type.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClassMember {
    pub id: String,
    pub method: bool,
    pub visibility: Option<char>,
    /// `$` (static), `*` (abstract) or none.
    pub classifier: Option<char>,
    pub parameters: String,
    pub return_type: String,
}

fn is_visibility(c: char) -> bool {
    matches!(c, '#' | '+' | '~' | '-')
}

fn text(chars: &[char]) -> String {
    chars.iter().collect()
}

impl ClassMember {
    pub(crate) fn new(input: &str, method: bool) -> ClassMember {
        let mut m = ClassMember {
            id: String::new(),
            method,
            visibility: None,
            classifier: None,
            parameters: String::new(),
            return_type: String::new(),
        };
        let chars: Vec<char> = input.chars().collect();
        // Mermaid throws a TypeError for a method with no opening parenthesis, such as `run)`.
        // It is kept here as an attribute, so it is shown as written.
        if !method || !m.parse_method(&chars) {
            m.parse_attribute(&chars);
        }
        m.id = if m.id.starts_with(' ') {
            format!(" {}", trim(&m.id))
        } else {
            trim(&m.id).to_string()
        };
        m
    }

    /// The same split as Mermaid's `/([#+~-])?(.+)\((.*)\)([\s$*])?(.*)([$*])?/`, found in one
    /// pass: the first line with an opening parenthesis after its first character and a
    /// closing one later.
    fn parse_method(&mut self, input: &[char]) -> bool {
        let n = input.len();
        let mut start = 0;
        while start < n {
            let mut end = start;
            while end < n && !is_line_end(input[end]) {
                end += 1;
            }
            let found = input[start..end]
                .iter()
                .rposition(|&c| c == ')')
                .map(|close| start + close)
                .filter(|&close| close > start)
                .and_then(|close| {
                    input[start + 1..close]
                        .iter()
                        .rposition(|&c| c == '(')
                        .map(|open| (start + 1 + open, close))
                });
            if let Some((open, close)) = found {
                let first = input[start];
                let visible = is_visibility(first) && open > start + 1;
                if visible {
                    self.visibility = Some(first);
                }
                self.id = text(&input[if visible { start + 1 } else { start }..open]);
                self.parameters = trim(&text(&input[open + 1..close])).to_string();
                let mut rest = close + 1;
                let mut classifier = None;
                if rest < n {
                    let next = input[rest];
                    if next == '$' || next == '*' || is_space(next) {
                        classifier = (!is_space(next)).then_some(next);
                        rest += 1;
                    }
                }
                let mut stop = rest;
                while stop < n && !is_line_end(input[stop]) {
                    stop += 1;
                }
                self.return_type = trim(&text(&input[rest.min(stop)..stop])).to_string();
                if classifier.is_none() {
                    if let Some(last) = self
                        .return_type
                        .chars()
                        .last()
                        .filter(|&c| c == '$' || c == '*')
                    {
                        classifier = Some(last);
                        self.return_type.pop();
                    }
                }
                self.classifier = classifier;
                return true;
            }
            start = end + 1;
        }
        self.method = false;
        false
    }

    fn parse_attribute(&mut self, input: &[char]) {
        if let Some(&first) = input.first().filter(|&&c| is_visibility(c)) {
            self.visibility = Some(first);
        }
        if let Some(&last) = input.last().filter(|&&c| c == '$' || c == '*') {
            self.classifier = Some(last);
        }
        let from = usize::from(self.visibility.is_some());
        let to = input.len() - usize::from(self.classifier.is_some());
        self.id = text(&input[from.min(to)..to.max(from)]);
    }

    /// The text a member is shown as.
    pub(crate) fn display_text(&self) -> String {
        let mut out = String::new();
        if let Some(v) = self.visibility {
            out.push(v);
        }
        out += &parse_generic_types(&self.id);
        if self.method {
            out += &format!("({})", parse_generic_types(trim(&self.parameters)));
            if !self.return_type.is_empty() {
                out += " : ";
                out += &parse_generic_types(&self.return_type);
            }
        }
        trim(&out).to_string()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ClassNode {
    pub id: String,
    /// The generic type, from `Name~T~`.
    pub generic: String,
    pub label: String,
    pub css_classes: String,
    pub methods: Vec<ClassMember>,
    pub members: Vec<ClassMember>,
    pub annotations: Vec<String>,
    pub styles: Vec<String>,
    pub parent: Option<String>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub have_callback: bool,
    pub tooltip: Option<String>,
}

/// One end of a relation: Mermaid's relationType number, or none.
pub(crate) type RelationEnd = Option<u8>;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClassRelation {
    pub id1: String,
    pub id2: String,
    pub title1: String,
    pub title2: String,
    pub title: Option<String>,
    pub type1: RelationEnd,
    pub type2: RelationEnd,
    pub dotted: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClassNote {
    pub id: String,
    pub class: Option<String>,
    pub text: String,
    pub index: usize,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClassInterface {
    pub id: String,
    pub label: String,
    pub class_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Namespace {
    pub id: String,
    pub label: String,
    pub classes: Vec<String>,
    pub notes: Vec<String>,
    pub children: Vec<String>,
    pub parent: Option<String>,
    /// False for the ancestors created on the way to a dotted name such as `A.B.C`.
    pub explicit: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct StyleClass {
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct ClassDb {
    pub direction: String,
    pub classes: Ordered<ClassNode>,
    pub relations: Vec<ClassRelation>,
    pub notes: Ordered<ClassNote>,
    pub namespaces: Ordered<Namespace>,
    pub interfaces: Vec<ClassInterface>,
    pub style_classes: Ordered<StyleClass>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    max_depth: usize,
    namespace_stack: Vec<String>,
    /// The classes tagged with each CSS class, by their place in `classes`.
    with_css_class: HashMap<String, BTreeSet<usize>>,
}

/// Splits `Name~T~` into the class name and its generic type.
fn split_name(id: &str) -> (&str, &str) {
    match id.find('~').filter(|&at| at > 0) {
        None => (id, ""),
        Some(at) => {
            let rest = &id[at + 1..];
            (&id[..at], rest.find('~').map_or(rest, |end| &rest[..end]))
        }
    }
}

fn lookup_id(id: &str) -> String {
    if id.starts_with(|c: char| c.is_ascii_digit()) {
        split_name(&format!("{DOM_ID_PREFIX}{id}")).0.to_string()
    } else {
        split_name(id).0.to_string()
    }
}

/// `label.startsWith(':') ? label.substring(1) : label`, trimmed.
pub(crate) fn cleanup_label(label: &str) -> String {
    trim(label.strip_prefix(':').unwrap_or(label)).to_string()
}

impl ClassDb {
    pub(crate) fn new(max_depth: usize) -> ClassDb {
        ClassDb {
            direction: "TB".to_string(),
            classes: Ordered::default(),
            relations: Vec::new(),
            notes: Ordered::default(),
            namespaces: Ordered::default(),
            interfaces: Vec::new(),
            style_classes: Ordered::default(),
            title: None,
            acc_title: None,
            acc_descr: None,
            max_depth,
            namespace_stack: Vec::new(),
            with_css_class: HashMap::new(),
        }
    }

    pub(crate) fn add_class(&mut self, id: &str) {
        let (name, generic) = split_name(id);
        if self.classes.contains(name) {
            return;
        }
        self.classes.set(
            name,
            ClassNode {
                id: name.to_string(),
                generic: generic.to_string(),
                label: name.to_string(),
                css_classes: "default".to_string(),
                ..ClassNode::default()
            },
        );
        self.tag(self.classes.len() - 1, "default");
    }

    fn tag(&mut self, node: usize, css_class: &str) {
        self.with_css_class
            .entry(css_class.to_string())
            .or_default()
            .insert(node);
    }

    /// The class for a name that may carry a generic type, created if needed. Mermaid throws a
    /// TypeError where this creates, as for an annotation on a class not yet declared.
    fn ensure(&mut self, id: &str) -> &mut ClassNode {
        self.add_class(id);
        self.classes
            .get_mut(split_name(id).0)
            .expect("class just added")
    }

    pub(crate) fn set_class_label(&mut self, id: &str, label: String) {
        self.ensure(id).label = label;
    }

    pub(crate) fn add_relation(&mut self, mut relation: ClassRelation) {
        if relation.type1 == Some(LOLLIPOP) && relation.type2.is_none() {
            self.add_class(&relation.id2);
            relation.id1 = self.add_interface(&relation.id1, &relation.id2);
        } else if relation.type2 == Some(LOLLIPOP) && relation.type1.is_none() {
            self.add_class(&relation.id1);
            relation.id2 = self.add_interface(&relation.id2, &relation.id1);
        } else {
            self.add_class(&relation.id1);
            self.add_class(&relation.id2);
        }
        relation.id1 = split_name(&relation.id1).0.to_string();
        relation.id2 = split_name(&relation.id2).0.to_string();
        relation.title1 = trim(&relation.title1).to_string();
        relation.title2 = trim(&relation.title2).to_string();
        self.relations.push(relation);
    }

    fn add_interface(&mut self, label: &str, class_id: &str) -> String {
        let id = format!("interface{}", self.interfaces.len());
        self.interfaces.push(ClassInterface {
            id: id.clone(),
            label: label.to_string(),
            class_id: class_id.to_string(),
        });
        id
    }

    pub(crate) fn add_annotation(&mut self, class_name: &str, annotation: String) {
        self.ensure(class_name).annotations.push(annotation);
    }

    pub(crate) fn add_member(&mut self, class_name: &str, member: &str) {
        let node = self.ensure(class_name);
        let text = trim(member);
        if text.starts_with("<<") && text.ends_with(">>") && text.len() >= 4 {
            node.annotations.push(text[2..text.len() - 2].to_string());
        } else if text.find(')').is_some_and(|at| at > 0) {
            let parsed = ClassMember::new(text, true);
            if parsed.method {
                node.methods.push(parsed);
            } else {
                node.members.push(parsed);
            }
        } else if !text.is_empty() {
            node.members.push(ClassMember::new(text, false));
        }
    }

    pub(crate) fn add_note(&mut self, text: String, class: Option<String>) -> String {
        let index = self.notes.len();
        let id = format!("note{index}");
        self.notes.set(
            &id,
            ClassNote {
                id: id.clone(),
                class,
                text,
                index,
                parent: None,
            },
        );
        id
    }

    pub(crate) fn set_css_class(&mut self, ids: &str, class_name: &str) {
        for id in ids.split(',') {
            let Some(at) = self.classes.position(&lookup_id(id)) else {
                continue;
            };
            let node = &mut self.classes.items[at].1;
            node.css_classes.push(' ');
            node.css_classes.push_str(class_name);
            for word in class_name.split(' ') {
                self.tag(at, word);
            }
        }
    }

    pub(crate) fn define_class(&mut self, ids: &[String], style: &[String]) {
        for id in ids {
            if !self.style_classes.contains(id) {
                self.style_classes.set(id, StyleClass::default());
            }
            let def = self.style_classes.get_mut(id).expect("style class");
            for s in style {
                if s.contains("color") {
                    def.text_styles.push(s.replacen("fill", "bgFill", 1));
                }
                def.styles.push(s.clone());
            }
            // Mermaid tests `cssClasses.includes(id)`, which also matches part of a longer name
            // (a classDef named `def` styles every class, through `default`). Whole names are
            // compared here.
            let Some(tagged) = self.with_css_class.get(id) else {
                continue;
            };
            for &node in tagged {
                let styles = &mut self.classes.items[node].1.styles;
                for s in style {
                    styles.extend(s.split(',').map(str::to_string));
                }
            }
        }
    }

    pub(crate) fn set_tooltip(&mut self, ids: &str, tooltip: Option<&str>) {
        let Some(tooltip) = tooltip else {
            return;
        };
        for id in ids.split(',') {
            if let Some(node) = self.classes.get_mut(split_name(id).0) {
                node.tooltip = Some(tooltip.to_string());
            }
        }
    }

    pub(crate) fn set_link(&mut self, ids: &str, link: &str, target: Option<&str>) {
        // Entity codes are resolved first, so that what is checked is what a browser would
        // follow.
        let decoded = decode_entities(link);
        let url = trim(&decoded);
        for id in ids.split(',') {
            if let Some(node) = self.classes.get_mut(&lookup_id(id)) {
                node.link = (!url.is_empty()).then(|| sanitize_url(url));
                node.link_target = Some(target.unwrap_or("_blank").to_string());
            }
        }
        self.set_css_class(ids, "clickable");
    }

    /// Callbacks are never executed; the class is only marked clickable, as in Mermaid's
    /// strict mode.
    pub(crate) fn set_click_event(&mut self, ids: &str) {
        for id in ids.split(',') {
            if let Some(node) = self.classes.get_mut(split_name(id).0) {
                node.have_callback = true;
            }
        }
        self.set_css_class(ids, "clickable");
    }

    pub(crate) fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(trim_start(text).to_string());
    }

    pub(crate) fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(collapse_indent(text));
    }

    pub(crate) fn add_namespace(
        &mut self,
        id: &str,
        label: Option<String>,
    ) -> Result<String, Error> {
        let prefix = self
            .namespace_stack
            .last()
            .filter(|p| !p.is_empty())
            .cloned();
        let qualified = match &prefix {
            Some(p) => format!("{p}.{id}"),
            None => id.to_string(),
        };
        // Each part of a dotted name is a namespace inside the one before.
        let depth = qualified.matches('.').count() + 1;
        if depth > self.max_depth {
            return Err(Error::new(
                ErrorCode::Limit,
                format!(
                    "Namespaces are nested deeper than the limit of {}.",
                    self.max_depth
                ),
            ));
        }
        self.namespace_stack.push(qualified.clone());

        if let Some(existing) = self.namespaces.get_mut(&qualified) {
            existing.explicit = true;
            if let Some(label) = label.filter(|l| !l.is_empty()) {
                existing.label = label;
            }
            return Ok(qualified);
        }

        // The enclosing namespace and its ancestors exist already, so only the parts of `id`
        // are walked.
        let mut parent_id = prefix.clone();
        let mut from = prefix.as_ref().map_or(0, |p| p.len() + 1);
        loop {
            let dot = qualified[from..].find('.').map(|d| from + d);
            let leaf = dot.is_none();
            let current = &qualified[..dot.unwrap_or(qualified.len())];
            match self.namespaces.get_mut(current) {
                None => {
                    let part = &qualified[from..dot.unwrap_or(qualified.len())];
                    let label = match &label {
                        Some(l) if leaf && !l.is_empty() => l.clone(),
                        _ => part.to_string(),
                    };
                    self.namespaces.set(
                        current,
                        Namespace {
                            id: current.to_string(),
                            label,
                            classes: Vec::new(),
                            notes: Vec::new(),
                            children: Vec::new(),
                            parent: parent_id.clone(),
                            explicit: leaf,
                        },
                    );
                }
                Some(node) if leaf => node.explicit = true,
                Some(_) => {}
            }
            if let Some(pid) = parent_id.as_deref().filter(|p| !p.is_empty()) {
                if let Some(parent) = self.namespaces.get_mut(pid) {
                    if !parent.children.iter().any(|c| c == current) {
                        parent.children.push(current.to_string());
                    }
                    let node = self.namespaces.get_mut(current).expect("namespace");
                    if node.parent.is_none() {
                        node.parent = Some(pid.to_string());
                    }
                }
            }
            match dot {
                None => return Ok(qualified),
                Some(dot) => {
                    parent_id = Some(current.to_string());
                    from = dot + 1;
                }
            }
        }
    }

    pub(crate) fn pop_namespace(&mut self) {
        self.namespace_stack.pop();
    }

    pub(crate) fn add_classes_to_namespace(
        &mut self,
        id: &str,
        class_names: &[String],
        note_names: &[String],
    ) {
        if !self.namespaces.contains(id) {
            return;
        }
        for name in class_names {
            let class_name = split_name(name).0;
            let Some(node) = self.classes.get_mut(class_name) else {
                continue;
            };
            node.parent = Some(id.to_string());
            let ns = self.namespaces.get_mut(id).expect("namespace");
            if !ns.classes.iter().any(|c| c == class_name) {
                ns.classes.push(class_name.to_string());
            }
        }
        for name in note_names {
            let Some(note) = self.notes.get_mut(name) else {
                continue;
            };
            note.parent = Some(id.to_string());
            let ns = self.namespaces.get_mut(id).expect("namespace");
            if !ns.notes.iter().any(|c| c == name) {
                ns.notes.push(name.clone());
            }
        }
    }

    pub(crate) fn set_css_style(&mut self, id: &str, styles: &[String]) {
        if let Some(node) = self.classes.get_mut(id) {
            for s in styles {
                node.styles.extend(s.split(',').map(str::to_string));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(text: &str, method: bool) -> ClassMember {
        ClassMember::new(text, method)
    }

    #[test]
    fn reads_members() {
        let m = member("+getName(int id) List~String~$", true);
        assert!(m.method);
        assert_eq!(m.visibility, Some('+'));
        assert_eq!(m.id, "getName");
        assert_eq!(m.parameters, "int id");
        assert_eq!(m.return_type, "List~String~");
        assert_eq!(m.classifier, Some('$'));
        assert_eq!(m.display_text(), "+getName(int id) : List<String>");
        let a = member("-int count*", false);
        assert_eq!((a.visibility, a.classifier), (Some('-'), Some('*')));
        assert_eq!(a.display_text(), "-int count");
        assert!(!member("run)", true).method);
    }

    #[test]
    fn nests_dotted_namespaces() {
        let mut db = ClassDb::new(64);
        assert_eq!(db.add_namespace("A.B", None).unwrap(), "A.B");
        db.pop_namespace();
        let a = db.namespaces.get("A").unwrap();
        assert!(!a.explicit);
        assert_eq!(a.children, ["A.B"]);
        assert_eq!(
            db.namespaces.get("A.B").unwrap().parent.as_deref(),
            Some("A")
        );
        let deep = vec!["x"; 100].join(".");
        assert_eq!(
            db.add_namespace(&deep, None).unwrap_err().code,
            ErrorCode::Limit
        );
    }

    #[test]
    fn splits_generic_names() {
        assert_eq!(split_name("List~int~"), ("List", "int"));
        assert_eq!(split_name("~x"), ("~x", ""));
        assert_eq!(lookup_id("1a"), "classId-1a");
    }
}
