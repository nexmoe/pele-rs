//! The syntax tree of a tree view.
//!
//! Mermaid parses a tree view with a Langium grammar and turns the tokens into values with a
//! value converter: `packages/parser/src/language/treeView/valueConverter.ts` in Mermaid 12.1.0.
//! The conversions are ported here, so that a name, an annotation and an indentation mean what
//! they mean in Mermaid.

use crate::util::trim;

use super::lexer::{self, Kind};

/// A node of the tree, as the grammar read it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Node {
    /// How far the node is indented, in characters. A node without an indentation is at the
    /// top of the tree.
    pub indent: Option<usize>,
    pub name: String,
    pub class_annotation: Option<String>,
    pub icon_annotation: Option<String>,
    pub desc_annotation: Option<String>,
}

/// The tree a source describes.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Ast {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub nodes: Vec<Node>,
}

/// Reads the source into a tree. Fails with the character the grammar could not read.
pub(crate) fn parse(src: &[char]) -> Result<Ast, usize> {
    let tokens = lexer::lex(src)?;
    let mut at = match tokens.first() {
        Some(token) if token.kind == Kind::Keyword => 1,
        Some(token) => return Err(token.start),
        None => return Err(0),
    };

    let mut ast = Ast::default();
    // The title and the accessibility lines come before the nodes, and the last of each kind
    // is the one that is kept.
    while let Some(token) = tokens.get(at) {
        match token.kind {
            Kind::AccDescr => {
                ast.acc_descr = Some(acc_descr(&token.text));
                at += 1;
            }
            Kind::AccTitle => {
                ast.acc_title = Some(acc_title(&token.text));
                at += 1;
            }
            Kind::Title => {
                ast.title = Some(title(&token.text));
                at += 1;
            }
            _ => break,
        }
    }

    while let Some(token) = tokens.get(at) {
        if !matches!(
            token.kind,
            Kind::Indentation | Kind::QuotedName | Kind::BareName
        ) {
            return Err(token.start);
        }
        let indent = if token.kind == Kind::Indentation {
            let count = token.text.chars().count();
            at += 1;
            Some((count, token.start))
        } else {
            None
        };
        let Some(name) = tokens.get(at) else {
            // The node has an indentation but no name.
            return Err(indent.map_or(0, |(_, start)| start));
        };
        let text = match name.kind {
            Kind::QuotedName => quoted_name(&name.text),
            Kind::BareName => bare_name(&name.text),
            _ => return Err(name.start),
        };
        at += 1;

        let mut class_annotation = None;
        let mut icon_annotation = None;
        let mut desc_annotation = None;
        while let Some(token) = tokens.get(at) {
            match token.kind {
                Kind::ClassAnnotation => {
                    class_annotation = Some(class_of(&token.text));
                    at += 1;
                }
                Kind::IconAnnotation => {
                    icon_annotation = Some(icon_of(&token.text));
                    at += 1;
                }
                Kind::DescAnnotation => {
                    desc_annotation = Some(desc_of(&token.text));
                    at += 1;
                }
                _ => break,
            }
        }
        ast.nodes.push(Node {
            indent: indent.map(|(count, _)| count),
            name: text,
            class_annotation,
            icon_annotation,
            desc_annotation,
        });
    }

    if let Some(token) = tokens.get(at) {
        return Err(token.start);
    }
    Ok(ast)
}

/// The name of a quoted node, without its quotes.
fn quoted_name(text: &str) -> String {
    let mut chars = text.chars();
    chars.next();
    chars.next_back();
    chars.as_str().to_string()
}

/// The name of a node that is not quoted. Only the spaces at its end are dropped.
fn bare_name(text: &str) -> String {
    text.trim_end_matches([' ', '\t']).to_string()
}

/// The class a node names: `:::name` gives `name`.
fn class_of(text: &str) -> String {
    let trimmed = trim(text);
    let rest: String = trimmed.chars().skip(3).collect();
    trim(&rest).to_string()
}

/// The icon a node names: `icon(pack:name)` gives `pack:name`, and `icon()` gives nothing.
fn icon_of(text: &str) -> String {
    let trimmed = trim(text);
    let chars: Vec<char> = trimmed.chars().collect();
    let end = chars.len().saturating_sub(1);
    chars.iter().skip(5).take(end.saturating_sub(5)).collect()
}

/// The description a node carries: `##text` gives `text`.
fn desc_of(text: &str) -> String {
    let trimmed = trim(text);
    let rest: String = trimmed.chars().skip(2).collect();
    trim(&rest).to_string()
}

/// The text of an `accDescr` line, as Mermaid's value converter reads it.
fn acc_descr(text: &str) -> String {
    match colon_value(text, "accDescr") {
        Some(value) => collapse(value.trim()),
        None => match braced(text) {
            Some(value) => collapse_multiline(&value),
            None => text.to_string(),
        },
    }
}

/// The text of an `accTitle` line, as Mermaid's value converter reads it.
fn acc_title(text: &str) -> String {
    match colon_value(text, "accTitle") {
        Some(value) => collapse(value.trim()),
        None => text.to_string(),
    }
}

/// The text of a `title` line, as Mermaid's value converter reads it.
fn title(text: &str) -> String {
    match text.find("title") {
        Some(at) => collapse(text[at + "title".len()..].trim()),
        None => text.to_string(),
    }
}

/// The text that follows the colon of a line that names itself. The text ends with the line.
fn colon_value(text: &str, name: &str) -> Option<String> {
    let at = text.find(name)?;
    let rest = &text[at + name.len()..];
    let after = rest.trim_start_matches([' ', '\t']);
    let value = after.strip_prefix(':')?;
    Some(value.split(['\n', '\r']).next().unwrap_or("").to_string())
}

/// The text between the braces of an `accDescr` line.
fn braced(text: &str) -> Option<String> {
    let at = text.find("accDescr")? + "accDescr".len();
    let rest = &text[at..];
    let open = rest.find('{')?;
    let close = rest[open + 1..].find('}')? + open + 1;
    Some(rest[open + 1..close].to_string())
}

/// The spaces and tabs of a run of them, as one space.
fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut spaces = 0;
    for c in text.chars() {
        if c == ' ' || c == '\t' {
            spaces += 1;
            continue;
        }
        if spaces > 0 {
            out.push(' ');
            spaces = 0;
        }
        out.push(c);
    }
    if spaces > 0 {
        out.push(' ');
    }
    out
}

/// The text of a description that spans lines: the spaces at the start and the end of every
/// line are dropped, and a blank line is one line break.
fn collapse_multiline(text: &str) -> String {
    let mut out = String::new();
    for line in text.split('\n') {
        let line = collapse(line.trim());
        if line.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(src: &str) -> Ast {
        let chars: Vec<char> = src.chars().collect();
        parse(&chars).expect("the source reads")
    }

    #[test]
    fn reads_a_tree() {
        let ast = read("treeView-beta\nsrc/\n    index.js\npackage.json\n");
        assert_eq!(ast.nodes.len(), 3);
        assert_eq!(ast.nodes[0].indent, None);
        assert_eq!(ast.nodes[0].name, "src/");
        assert_eq!(ast.nodes[1].indent, Some(4));
        assert_eq!(ast.nodes[1].name, "index.js");
        assert_eq!(ast.nodes[2].indent, None);
    }

    #[test]
    fn reads_the_annotations() {
        let ast = read("treeView-beta\n    a :::c icon(pack:n) ## d\n");
        let node = &ast.nodes[0];
        assert_eq!(node.class_annotation.as_deref(), Some("c"));
        assert_eq!(node.icon_annotation.as_deref(), Some("pack:n"));
        assert_eq!(node.desc_annotation.as_deref(), Some("d"));
    }

    #[test]
    fn reads_an_icon_without_a_name() {
        let ast = read("treeView-beta\n    a icon()\n");
        assert_eq!(ast.nodes[0].icon_annotation.as_deref(), Some(""));
    }

    #[test]
    fn reads_a_quoted_name() {
        let ast = read("treeView-beta\n    \"a name\"\n");
        assert_eq!(ast.nodes[0].name, "a name");
    }

    #[test]
    fn reads_the_title_and_the_accessibility_lines() {
        let ast = read("treeView-beta\ntitle A title\naccTitle: A\naccDescr: D\n    a\n");
        assert_eq!(ast.title.as_deref(), Some("A title"));
        assert_eq!(ast.acc_title.as_deref(), Some("A"));
        assert_eq!(ast.acc_descr.as_deref(), Some("D"));
    }

    #[test]
    fn reads_a_description_between_braces() {
        let ast = read("treeView-beta\naccDescr {A\n  B}\n    a\n");
        assert_eq!(ast.acc_descr.as_deref(), Some("A\nB"));
    }

    #[test]
    fn refuses_a_source_without_the_keyword() {
        let chars: Vec<char> = "hello\n".chars().collect();
        assert!(parse(&chars).is_err());
    }
}
