//! The model of a kanban board. Ported from pele's `diagrams/kanban/db.ts`.

use crate::diagrams::common::outline::OutlineDb;
use crate::error::{Error, ErrorCode};
use crate::preprocess::{Value, parse_yaml};

#[derive(Debug, Clone, Default)]
pub(crate) struct Item {
    pub id: String,
    #[cfg_attr(not(test), allow(dead_code, reason = "read by the compat test"))]
    pub level: usize,
    pub label: String,
    pub icon: Option<String>,
    pub assigned: Option<String>,
    pub ticket: Option<String>,
    pub priority: Option<String>,
    pub css_classes: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Section {
    pub node: Item,
    pub items: Vec<Item>,
}

#[derive(Debug, Default)]
pub(crate) struct KanbanDb {
    pub title: Option<String>,
    pub sections: Vec<Section>,
    /// Whether the node added last is a section, rather than the last item of the last section.
    last_is_section: bool,
    section_level: usize,
    misplaced: Option<String>,
}

impl KanbanDb {
    fn last(&mut self) -> Option<&mut Item> {
        let section = self.sections.last_mut()?;
        if self.last_is_section {
            Some(&mut section.node)
        } else {
            section.items.last_mut()
        }
    }
}

impl OutlineDb for KanbanDb {
    fn add_node(
        &mut self,
        level: usize,
        id: &str,
        descr: &str,
        _kind: u8,
        shape_data: Option<&str>,
    ) -> Result<(), Error> {
        let mut node = Item {
            id: id.to_string(),
            level,
            label: descr.to_string(),
            ..Item::default()
        };
        if let Some(data) = shape_data {
            let yaml = if data.contains('\n') {
                format!("{data}\n")
            } else {
                format!("{{\n{data}\n}}")
            };
            let doc = parse_yaml(&yaml).map_err(|message| {
                Error::new(
                    ErrorCode::Syntax,
                    format!("Metadata of \"{id}\" is not valid YAML. {message}"),
                )
            })?;
            // Mermaid throws a TypeError on metadata that is empty or not a mapping. Here it is
            // ignored.
            if let Value::Map(doc) = doc {
                let get = |key: &str| doc.get(key).filter(|v| v.truthy()).map(Value::js_string);
                let shape = get("shape").unwrap_or_default();
                if shape != shape.to_lowercase() || shape.contains('_') {
                    return Err(Error::new(
                        ErrorCode::Semantic,
                        format!("No such shape: {shape}. Shape names should be lowercase."),
                    ));
                }
                if let Some(label) = get("label") {
                    node.label = label;
                }
                node.icon = get("icon");
                node.assigned = get("assigned");
                node.ticket = get("ticket");
                node.priority = get("priority");
            }
        }

        // A node indented less than the first column becomes a card, and Mermaid rejects
        // whatever follows it.
        if let Some(label) = &self.misplaced {
            return Err(Error::new(
                ErrorCode::Semantic,
                format!("Items without section detected, found section (\"{label}\")"),
            ));
        }
        if self.sections.is_empty() {
            self.section_level = level;
        }
        if level == self.section_level {
            self.sections.push(Section {
                node,
                items: Vec::new(),
            });
            self.last_is_section = true;
        } else {
            if level < self.section_level {
                self.misplaced = Some(node.label.clone());
            }
            self.sections
                .last_mut()
                .expect("the first node is a section")
                .items
                .push(node);
            self.last_is_section = false;
        }
        Ok(())
    }

    /// Mermaid throws a TypeError when a decoration comes before any node. Here it is ignored.
    fn decorate_node(&mut self, icon: Option<&str>, class: Option<&str>) {
        let Some(node) = self.last() else {
            return;
        };
        if let Some(icon) = icon.filter(|i| !i.is_empty()) {
            node.icon = Some(icon.to_string());
        }
        if let Some(class) = class.filter(|c| !c.is_empty()) {
            node.css_classes = Some(class.to_string());
        }
    }
}
