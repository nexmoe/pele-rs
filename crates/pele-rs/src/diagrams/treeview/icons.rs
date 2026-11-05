//! The icons of a tree view.
//!
//! Mermaid draws the icon a node names in front of its label, with an icon font it fetches at
//! runtime: `packages/mermaid/src/diagrams/treeView/icons.ts` in Mermaid 12.1.0. pele-rs has no
//! icon font and draws no images, so it writes the name of the icon where the icon would be.

use crate::preprocess::Value;

use super::db::{Node, NodeType};

/// The pack Mermaid keeps its own icons in.
pub(crate) const BUILTIN_PACK: &str = "mermaid-treeview";

/// The icons a node may show.
#[derive(Debug, Clone, Default)]
pub(crate) struct Icons {
    /// Whether the nodes that name no icon show one all the same.
    pub show_icons: bool,
    /// The pack an icon name without a pack of its own belongs to.
    pub default_icon_pack: String,
    /// The icon of a file of a certain name.
    pub filename_icons: Vec<(String, String)>,
    /// The icon of a file of a certain extension.
    pub extension_icons: Vec<(String, String)>,
}

impl Icons {
    /// Reads the settings of the diagram.
    pub(crate) fn from_config(config: &crate::preprocess::Config) -> Icons {
        let mut icons = Icons::default();
        let Some(Value::Map(settings)) = config.get("treeView") else {
            return icons;
        };
        if let Some(Value::Bool(show)) = settings.get("showIcons") {
            icons.show_icons = *show;
        }
        if let Some(Value::String(pack)) = settings.get("defaultIconPack") {
            icons.default_icon_pack = pack.clone();
        }
        icons.filename_icons = pairs(settings.get("filenameIcons"));
        icons.extension_icons = pairs(settings.get("extensionIcons"));
        icons
    }

    /// The icon a node shows, as it is written. `None` when the node shows none.
    pub(crate) fn of(&self, node: &Node) -> Option<String> {
        if node.icon.as_deref() == Some("none") {
            return None;
        }
        if let Some(icon) = node.icon.as_deref().filter(|icon| !icon.is_empty()) {
            return Some(self.qualify(icon));
        }
        if !self.show_icons {
            return None;
        }
        if node.node_type == NodeType::File {
            if let Some(detected) = self.detect(&node.name) {
                if detected == "none" {
                    return None;
                }
                return Some(self.qualify(&detected));
            }
        }
        Some(format!(
            "{BUILTIN_PACK}:{}",
            match node.node_type {
                NodeType::Directory => "folder",
                NodeType::File => "file",
            }
        ))
    }

    /// The icon of a file, from the maps of the configuration. The name of the file wins over
    /// its extension, and an extension matches in lower case with or without its dot.
    fn detect(&self, name: &str) -> Option<String> {
        if let Some((_, icon)) = self.filename_icons.iter().find(|(file, _)| file == name) {
            return Some(icon.clone());
        }
        let dot = name.rfind('.')?;
        if dot == 0 {
            return None;
        }
        let extension = name[dot..].to_lowercase();
        let found = self
            .extension_icons
            .iter()
            .find(|(key, _)| *key == extension)
            .or_else(|| {
                self.extension_icons
                    .iter()
                    .find(|(key, _)| *key == extension[1..])
            });
        found.map(|(_, icon)| icon.clone())
    }

    /// The name of an icon with the pack it belongs to. The icons of Mermaid's own pack win
    /// over the pack of the configuration.
    fn qualify(&self, icon: &str) -> String {
        if icon.contains(':') {
            return icon.to_string();
        }
        if is_builtin(icon) || self.default_icon_pack.is_empty() {
            return format!("{BUILTIN_PACK}:{icon}");
        }
        format!("{}:{}", self.default_icon_pack, icon)
    }
}

/// The entries of a map of icons.
fn pairs(value: Option<&Value>) -> Vec<(String, String)> {
    let Some(Value::Map(map)) = value else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(key, value)| match value {
            Value::String(icon) => Some((key.clone(), icon.clone())),
            _ => None,
        })
        .collect()
}

/// Whether an icon is one of Mermaid's own.
fn is_builtin(icon: &str) -> bool {
    matches!(icon, "file" | "folder")
}

/// The name an icon is drawn under: the name of Mermaid's own icons without their pack, and
/// the whole name of every other icon.
pub(crate) fn label(icon: &str) -> String {
    match icon.strip_prefix(&format!("{BUILTIN_PACK}:")) {
        Some(name) => name.to_string(),
        None => icon.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagrams::treeview::{db, parser};

    fn node(src: &str) -> db::Node {
        let chars: Vec<char> = src.chars().collect();
        let ast = parser::parse(&chars).expect("the source reads");
        db::build(&ast, None).nodes[1].clone()
    }

    #[test]
    fn shows_no_icon_by_default() {
        let icons = Icons::default();
        assert_eq!(icons.of(&node("treeView-beta\na\n")), None);
        assert_eq!(
            icons.of(&node("treeView-beta\na icon(logos:react)\n")),
            Some("logos:react".to_string())
        );
        assert_eq!(icons.of(&node("treeView-beta\na icon()\n")), None);
    }

    #[test]
    fn shows_the_icons_of_mermaid_when_they_are_asked_for() {
        let icons = Icons {
            show_icons: true,
            ..Icons::default()
        };
        assert_eq!(
            icons.of(&node("treeView-beta\na\n")),
            Some("mermaid-treeview:file".to_string())
        );
        assert_eq!(
            icons.of(&node("treeView-beta\nd/\n")),
            Some("mermaid-treeview:folder".to_string())
        );
    }

    #[test]
    fn reads_the_icon_of_a_file_from_the_configuration() {
        let mut config = crate::preprocess::Config::new();
        let mut settings = crate::preprocess::Config::new();
        settings.insert("showIcons".to_string(), Value::Bool(true));
        settings.insert(
            "extensionIcons".to_string(),
            Value::Map(crate::preprocess::Config::from_iter([(
                ".ts".to_string(),
                Value::String("material:typescript".to_string()),
            )])),
        );
        config.insert("treeView".to_string(), Value::Map(settings));
        let icons = Icons::from_config(&config);
        assert_eq!(
            icons.of(&node("treeView-beta\na.ts\n")),
            Some("material:typescript".to_string())
        );
    }

    #[test]
    fn writes_the_name_of_an_icon() {
        assert_eq!(label("mermaid-treeview:folder"), "folder");
        assert_eq!(label("logos:react"), "logos:react");
    }
}
