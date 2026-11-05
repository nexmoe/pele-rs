//! The model of a venn diagram: its sets, its overlaps, its texts and its styles.
//! Ported from Mermaid's `diagrams/venn/vennDB.ts`, which pele's `diagrams/venn/db.ts` follows.

use crate::error::{Error, ErrorCode};

/// A set, or an overlap of sets, with the area it takes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VennSubset {
    /// The ids of the sets, sorted.
    pub sets: Vec<String>,
    pub size: f64,
    pub label: Option<String>,
}

/// A line of text shown inside the region of the given sets.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VennText {
    /// The ids of the sets, sorted.
    pub sets: Vec<String>,
    pub id: String,
    pub label: Option<String>,
}

/// The declarations of a style, for a set or an overlap.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VennStyle {
    /// The ids of the sets, sorted.
    pub targets: Vec<String>,
    /// The properties, in the order they were declared. A property declared twice keeps its
    /// place and takes the last value.
    pub styles: Vec<(String, String)>,
}

/// The model of a venn diagram.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct VennModel {
    pub title: Option<String>,
    pub subsets: Vec<VennSubset>,
    pub text_nodes: Vec<VennText>,
    pub styles: Vec<VennStyle>,
}

/// Mermaid takes one pair of quotes off an id, a label or a style value.
pub(crate) fn normalize_text(text: &str) -> String {
    let trimmed = crate::util::trim(text);
    let count = trimmed.chars().count();
    if count >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        return trimmed.chars().skip(1).take(count - 2).collect();
    }
    trimmed.to_string()
}

/// A string as `JSON.stringify` writes it.
pub(crate) fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The ids of a list of sets, with the quotes taken off.
fn normalize_ids(ids: &[String]) -> Vec<String> {
    ids.iter().map(|id| normalize_text(id)).collect()
}

/// The ids of a list of sets, with the quotes taken off and in the order JavaScript sorts
/// them: as sequences of UTF-16 code units.
fn sorted_ids(ids: &[String]) -> Vec<String> {
    let mut out = normalize_ids(ids);
    out.sort_by(|a, b| js_cmp(a, b));
    out
}

/// The order JavaScript's `Array.prototype.sort` puts two strings in.
pub(crate) fn js_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let mut left = a.encode_utf16();
    let mut right = b.encode_utf16();
    loop {
        match (left.next(), right.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x == y => continue,
            (Some(x), Some(y)) => return x.cmp(&y),
        }
    }
}

/// The model of a venn diagram, as the parser fills it.
#[derive(Debug, Clone, Default)]
pub(crate) struct VennDb {
    pub title: Option<String>,
    pub subsets: Vec<VennSubset>,
    pub text_nodes: Vec<VennText>,
    pub styles: Vec<VennStyle>,
    /// The sets that have been declared, by their id.
    known: Vec<String>,
    /// The sets of the last set or union, which an indented `text` line belongs to.
    current: Option<Vec<String>>,
}

impl VennDb {
    /// The title of the diagram.
    pub(crate) fn set_diagram_title(&mut self, title: &str) {
        self.title = Some(title.to_string());
    }

    /// A set without a size gets 10, and an overlap of n sets 10 / n².
    pub(crate) fn add_subset_data(
        &mut self,
        ids: Vec<String>,
        label: Option<String>,
        size: Option<f64>,
    ) {
        let sets = sorted_ids(&ids);
        let size = size.unwrap_or(10.0 / (ids.len() as f64).powi(2));
        self.current = Some(sets.clone());
        if sets.len() == 1 && !self.known.contains(&sets[0]) {
            self.known.push(sets[0].clone());
        }
        self.subsets.push(VennSubset {
            sets,
            size,
            label: label.filter(|l| !l.is_empty()).map(|l| normalize_text(&l)),
        });
    }

    /// A line of text in the region of the given sets.
    pub(crate) fn add_text_data(&mut self, ids: Vec<String>, id: String, label: Option<String>) {
        self.text_nodes.push(VennText {
            sets: sorted_ids(&ids),
            id: normalize_text(&id),
            label: label.filter(|l| !l.is_empty()).map(|l| normalize_text(&l)),
        });
    }

    /// The declarations of a style, for a set or an overlap.
    pub(crate) fn add_style_data(&mut self, ids: Vec<String>, data: Vec<(String, String)>) {
        let targets = sorted_ids(&ids);
        let mut styles: Vec<(String, String)> = Vec::new();
        for (key, value) in data {
            let value = normalize_text(&value);
            match styles.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 = value,
                None => styles.push((key, value)),
            }
        }
        self.styles.push(VennStyle { targets, styles });
    }

    /// The sets of a union must have been declared before.
    pub(crate) fn validate_union_identifiers(&self, ids: &[String]) -> Result<(), Error> {
        let unknown: Vec<String> = normalize_ids(ids)
            .into_iter()
            .filter(|id| !self.known.contains(id))
            .collect();
        if !unknown.is_empty() {
            return Err(Error::new(
                ErrorCode::Semantic,
                format!("unknown set identifier: {}", unknown.join(", ")),
            ));
        }
        Ok(())
    }

    /// The sets of the last set or union, which an indented `text` line belongs to.
    pub(crate) fn current_sets(&self) -> Option<&[String]> {
        self.current.as_deref()
    }

    /// The model of the diagram, with the title of the front matter as a fallback.
    pub(crate) fn build(self, title: Option<&str>) -> VennModel {
        VennModel {
            title: self.title.or_else(|| title.map(str::to_string)),
            subsets: self.subsets,
            text_nodes: self.text_nodes,
            styles: self.styles,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_quotes_off() {
        assert_eq!(normalize_text("\"A\""), "A");
        assert_eq!(normalize_text("  \"A\"  "), "A");
        assert_eq!(normalize_text("A"), "A");
        assert_eq!(normalize_text("\"\""), "");
    }

    #[test]
    fn sorts_the_ids_as_javascript_does() {
        let ids = vec!["b".to_string(), "A".to_string(), "a".to_string()];
        assert_eq!(sorted_ids(&ids), ["A", "a", "b"]);
    }

    #[test]
    fn gives_a_set_ten_and_an_overlap_less() {
        let mut db = VennDb::default();
        db.add_subset_data(vec!["A".to_string()], None, None);
        db.add_subset_data(vec!["A".to_string(), "B".to_string()], None, None);
        assert_eq!(db.subsets[0].size, 10.0);
        assert_eq!(db.subsets[1].size, 2.5);
    }

    #[test]
    fn keeps_the_last_value_of_a_property() {
        let mut db = VennDb::default();
        db.add_style_data(
            vec!["A".to_string()],
            vec![
                ("fill".to_string(), "red".to_string()),
                ("stroke".to_string(), "blue".to_string()),
                ("fill".to_string(), "green".to_string()),
            ],
        );
        assert_eq!(
            db.styles[0].styles,
            vec![
                ("fill".to_string(), "green".to_string()),
                ("stroke".to_string(), "blue".to_string())
            ]
        );
    }
}
