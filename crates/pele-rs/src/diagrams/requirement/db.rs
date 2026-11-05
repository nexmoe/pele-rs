//! The model of a requirement diagram. Ported from pele's `diagrams/requirement/db.ts`.

use crate::diagrams::common::ordered::Ordered;
use crate::error::{Error, ErrorCode};
use crate::util::{collapse_indent, trim_start};

/// Applying a class copies its declarations onto each node, so a short text can ask for
/// hundreds of millions of copies. Past this many the diagram is refused.
const MAX_STYLE_COPIES: usize = 1_000_000;

#[derive(Debug, Clone)]
pub(crate) struct Requirement {
    pub name: String,
    pub kind: &'static str,
    pub requirement_id: String,
    pub text: String,
    pub risk: String,
    pub verify_method: String,
    pub css_styles: Vec<String>,
    pub classes: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Element {
    pub name: String,
    pub kind: String,
    pub doc_ref: String,
    pub css_styles: Vec<String>,
    pub classes: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Relation {
    pub kind: &'static str,
    pub src: String,
    pub dst: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StyleClass {
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct RequirementDb {
    pub direction: &'static str,
    pub requirements: Ordered<Requirement>,
    pub elements: Ordered<Element>,
    pub relations: Vec<Relation>,
    pub classes: Ordered<StyleClass>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    // The grammar reports a body's fields before the requirement or element they belong to.
    pub new_id: String,
    pub new_text: String,
    pub new_risk: String,
    pub new_verify_method: String,
    pub new_element_type: String,
    pub new_doc_ref: String,
    style_copies: usize,
}

fn limit() -> Error {
    Error::new(
        ErrorCode::Limit,
        "Requirement diagram applies too many style declarations.",
    )
}

impl RequirementDb {
    pub(crate) fn new() -> RequirementDb {
        RequirementDb {
            direction: "TB",
            ..RequirementDb::default()
        }
    }

    fn count(&mut self, n: usize) -> Result<(), Error> {
        self.style_copies += n;
        if self.style_copies > MAX_STYLE_COPIES {
            return Err(limit());
        }
        Ok(())
    }

    pub(crate) fn add_requirement(&mut self, name: &str, kind: &'static str) {
        if !self.requirements.contains(name) {
            let requirement = Requirement {
                name: name.to_string(),
                kind,
                requirement_id: std::mem::take(&mut self.new_id),
                text: std::mem::take(&mut self.new_text),
                risk: std::mem::take(&mut self.new_risk),
                verify_method: std::mem::take(&mut self.new_verify_method),
                css_styles: Vec::new(),
                classes: vec!["default".to_string()],
            };
            self.requirements.set(name, requirement);
        }
        self.new_id.clear();
        self.new_text.clear();
        self.new_risk.clear();
        self.new_verify_method.clear();
    }

    pub(crate) fn add_element(&mut self, name: &str) {
        if !self.elements.contains(name) {
            let element = Element {
                name: name.to_string(),
                kind: std::mem::take(&mut self.new_element_type),
                doc_ref: std::mem::take(&mut self.new_doc_ref),
                css_styles: Vec::new(),
                classes: vec!["default".to_string()],
            };
            self.elements.set(name, element);
        }
        self.new_element_type.clear();
        self.new_doc_ref.clear();
    }

    pub(crate) fn add_relationship(&mut self, kind: &'static str, src: &str, dst: &str) {
        self.relations.push(Relation {
            kind,
            src: src.to_string(),
            dst: dst.to_string(),
        });
    }

    /// The styles and classes of the requirement or element with this name.
    fn node(&mut self, id: &str) -> Option<(&mut Vec<String>, &mut Vec<String>)> {
        if self.requirements.contains(id) {
            let r = self.requirements.get_mut(id).expect("requirement");
            return Some((&mut r.css_styles, &mut r.classes));
        }
        self.elements
            .get_mut(id)
            .map(|e| (&mut e.css_styles, &mut e.classes))
    }

    pub(crate) fn set_css_style(&mut self, ids: &[String], styles: &[String]) -> Result<(), Error> {
        for id in ids {
            let parts: Vec<String> = styles
                .iter()
                .flat_map(|s| s.split(',').map(str::to_string))
                .collect();
            // Mermaid stops at the first unknown id, leaving the rest of the list unstyled.
            if self.node(id).is_none() {
                return Ok(());
            }
            self.count(parts.len())?;
            let (css, _) = self.node(id).expect("node");
            css.extend(parts);
        }
        Ok(())
    }

    pub(crate) fn set_class(&mut self, ids: &[String], names: &[String]) -> Result<(), Error> {
        for id in ids {
            if self.node(id).is_none() {
                continue;
            }
            for name in names {
                let styles = self.classes.get(name).map(|c| c.styles.clone());
                if let Some(styles) = &styles {
                    self.count(styles.len())?;
                }
                let (css, classes) = self.node(id).expect("node");
                classes.push(name.clone());
                if let Some(styles) = styles {
                    css.extend(styles);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn define_class(&mut self, ids: &[String], style: &[String]) -> Result<(), Error> {
        for id in ids {
            if !self.classes.contains(id) {
                self.classes.set(id, StyleClass::default());
            }
            let class = self.classes.get_mut(id).expect("class");
            for s in style {
                if s.contains("color") {
                    class.text_styles.push(s.replacen("fill", "bgFill", 1));
                }
                class.styles.push(s.clone());
            }
            let split: Vec<String> = style
                .iter()
                .flat_map(|s| s.split(',').map(str::to_string))
                .collect();
            let mut copies = 0;
            for (_, r) in &self.requirements.items {
                if r.classes.contains(id) {
                    copies += split.len();
                }
            }
            for (_, e) in &self.elements.items {
                if e.classes.contains(id) {
                    copies += split.len();
                }
            }
            self.count(copies)?;
            for (_, r) in &mut self.requirements.items {
                if r.classes.contains(id) {
                    r.css_styles.extend(split.iter().cloned());
                }
            }
            for (_, e) in &mut self.elements.items {
                if e.classes.contains(id) {
                    e.css_styles.extend(split.iter().cloned());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(trim_start(text).to_string());
    }

    pub(crate) fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(collapse_indent(text));
    }
}
