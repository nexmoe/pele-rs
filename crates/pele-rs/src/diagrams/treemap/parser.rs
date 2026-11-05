//! The tokens and the parser of a treemap, over the grammar Mermaid gives with Langium.
//! Ported from pele's `diagrams/treemap/parser.ts`, which follows Mermaid's
//! `packages/parser/src/language/treemap/treemap.langium`.
//!
//! The grammar is
//!
//! ```text
//! entry Treemap: TREEMAP_KEYWORD (TitleAndAccessibilities | TreemapRows+=TreemapRow)*;
//! TreemapRow: indent=INDENTATION? (item=Item | ClassDef);
//! Item: Leaf | Section;
//! Section: name=STRING2 (STYLE_SEPARATOR classSelector=ID2)?;
//! Leaf: name=STRING2 INDENTATION? (SEPARATOR | COMMA) INDENTATION? value=MyNumber
//!        (STYLE_SEPARATOR classSelector=ID2)?;
//! ```
//!
//! and the terminals are tried in the order the grammar declares them, the first match winning,
//! as in Chevrotain.

use crate::diagrams::common::tokens::{
    ACC_DESCR, ACC_TITLE, END, Found, Pattern, Reader, TITLE, TokenType, acc_descr_value,
    acc_title_value, literal_token, match_token, title_value, tokenize,
};
use crate::error::Error;
use crate::util::js_parse_float;

// The kinds of the tokens, in the order they are tried, as in the grammar.
const ACC_DESCR_KIND: usize = 0;
const ACC_TITLE_KIND: usize = 1;
const TITLE_KIND: usize = 2;
const KEYWORD: usize = 3;
const CLASS_DEF: usize = 4;
const STYLE_SEPARATOR: usize = 5;
const SEPARATOR: usize = 6;
const COMMA: usize = 7;
const INDENTATION: usize = 8;
const ID2: usize = 12;
const NUMBER2: usize = 13;
const STRING2: usize = 14;

static TOKENS: [TokenType; 15] = [
    ACC_DESCR,
    ACC_TITLE,
    TITLE,
    match_token("TREEMAP_KEYWORD", keyword),
    match_token("CLASS_DEF", class_def),
    literal_token(":::"),
    literal_token(":"),
    literal_token(","),
    match_token("INDENTATION", indentation),
    hidden("WS", Pattern::Match(whitespace)),
    hidden("ML_COMMENT", Pattern::Match(comment)),
    hidden("NL", Pattern::Match(newline)),
    match_token("ID2", id2),
    match_token("NUMBER2", number2),
    match_token("STRING2", string2),
];

/// A token type no parser sees, as the grammar declares `WS`, `ML_COMMENT` and `NL` hidden.
const fn hidden(name: &'static str, pattern: Pattern) -> TokenType {
    TokenType {
        name,
        pattern,
        hidden: true,
        longer: &[],
    }
}

/// `'treemap-beta' | 'treemap'`: the keyword that opens a treemap.
fn keyword(src: &str, at: usize) -> Found {
    if src[at..].starts_with("treemap-beta") {
        Found::End(at + "treemap-beta".len())
    } else if src[at..].starts_with("treemap") {
        Found::End(at + "treemap".len())
    } else {
        Found::No
    }
}

/// `classDef\s+([a-zA-Z_][a-zA-Z0-9_]+)(?:\s+([^;\r\n]*))?(?:;)?`: a class definition, whose
/// name has at least two characters.
fn class_def(src: &str, at: usize) -> Found {
    let Some(rest) = src[at..].strip_prefix("classDef") else {
        return Found::No;
    };
    let mut i = src.len() - rest.len();
    let Some(after_space) = skip_spaces(src, i) else {
        return Found::No;
    };
    i = after_space;
    let Some(&first) = src.as_bytes().get(i) else {
        return Found::No;
    };
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return Found::No;
    }
    i += 1;
    let name_start = i;
    while matches!(src.as_bytes().get(i), Some(b) if b.is_ascii_alphanumeric() || *b == b'_') {
        i += 1;
    }
    if i == name_start {
        return Found::No;
    }
    // The styles, which run to the end of the line or to a `;`.
    if let Some(after_space) = skip_spaces(src, i) {
        i = after_space;
        while !matches!(
            src.as_bytes().get(i),
            None | Some(b';') | Some(b'\r') | Some(b'\n')
        ) {
            i += 1;
        }
    }
    if src.as_bytes().get(i) == Some(&b';') {
        i += 1;
    }
    Found::End(i)
}

/// The whitespace `\s` matches, which includes a line break.
fn skip_spaces(src: &str, at: usize) -> Option<usize> {
    let mut i = at;
    while matches!(
        src.as_bytes().get(i),
        Some(b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
    ) {
        i += 1;
    }
    (i > at).then_some(i)
}

/// `[ \t]{1,}`
fn indentation(src: &str, at: usize) -> Found {
    let mut i = at;
    while matches!(src.as_bytes().get(i), Some(b' ' | b'\t')) {
        i += 1;
    }
    if i > at { Found::End(i) } else { Found::No }
}

/// `[a-zA-Z_][a-zA-Z0-9_]*`
fn id2(src: &str, at: usize) -> Found {
    let Some(&first) = src.as_bytes().get(at) else {
        return Found::No;
    };
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return Found::No;
    }
    let mut i = at + 1;
    while matches!(src.as_bytes().get(i), Some(b) if b.is_ascii_alphanumeric() || *b == b'_') {
        i += 1;
    }
    Found::End(i)
}

/// `[0-9_\.\,]+`
fn number2(src: &str, at: usize) -> Found {
    let mut i = at;
    while matches!(
        src.as_bytes().get(i),
        Some(b'0'..=b'9' | b'_' | b'.' | b',')
    ) {
        i += 1;
    }
    if i > at { Found::End(i) } else { Found::No }
}

/// `"[^"]*"|'[^']*'`: a quoted name, which may run over a line break.
fn string2(src: &str, at: usize) -> Found {
    let quote = match src.as_bytes().get(at) {
        Some(&b'"') => b'"',
        Some(&b'\'') => b'\'',
        _ => return Found::No,
    };
    let mut i = at + 1;
    while let Some(&byte) = src.as_bytes().get(i) {
        if byte == quote {
            return Found::End(i + 1);
        }
        i += 1;
    }
    Found::No
}

/// `[ \t]+`
fn whitespace(src: &str, at: usize) -> Found {
    let mut i = at;
    while matches!(src.as_bytes().get(i), Some(b' ' | b'\t')) {
        i += 1;
    }
    if i > at { Found::End(i) } else { Found::No }
}

/// `\%\%[^\n]*`
fn comment(src: &str, at: usize) -> Found {
    let Some(rest) = src[at..].strip_prefix("%%") else {
        return Found::No;
    };
    let mut i = src.len() - rest.len();
    while !matches!(src.as_bytes().get(i), None | Some(b'\n')) {
        i += 1;
    }
    Found::End(i)
}

/// `\r?\n`
fn newline(src: &str, at: usize) -> Found {
    match src.as_bytes().get(at..) {
        Some([b'\r', b'\n', ..]) => Found::End(at + 2),
        Some([b'\n', ..]) => Found::End(at + 1),
        _ => Found::No,
    }
}

/// The name of a class, and the styles it gives, as Mermaid's value converter reads them out of
/// a `classDef`: `classDef\s+([A-Z_a-z]\w+)(?:\s+([^\n\r;]*))?;?`.
fn class_def_value(text: &str) -> (String, Option<String>) {
    let Some(rest) = text.strip_prefix("classDef") else {
        return (String::new(), None);
    };
    let Some(after_space) = skip_spaces(text, text.len() - rest.len()) else {
        return (String::new(), None);
    };
    let bytes = text.as_bytes();
    let Some(&first) = bytes.get(after_space) else {
        return (String::new(), None);
    };
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return (String::new(), None);
    }
    let mut i = after_space + 1;
    while matches!(bytes.get(i), Some(b) if b.is_ascii_alphanumeric() || *b == b'_') {
        i += 1;
    }
    let name = text[after_space..i].to_string();
    let mut style = None;
    if let Some(after_space) = skip_spaces(text, i) {
        let mut end = after_space;
        while !matches!(
            bytes.get(end),
            None | Some(b';') | Some(b'\r') | Some(b'\n')
        ) {
            end += 1;
        }
        let styles = &text[after_space..end];
        if !styles.is_empty() {
            style = Some(styles.to_string());
        }
    }
    (name, style)
}

/// A row of a treemap: an item with the indentation that says where it sits, or a class
/// definition.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Row {
    Item {
        indent: usize,
        item: Item,
    },
    ClassDef {
        indent: usize,
        class_name: String,
        style_text: Option<String>,
    },
}

/// An item of a treemap: a section, which holds other items, or a leaf, which has a value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Item {
    Section {
        name: String,
        class_selector: Option<String>,
    },
    Leaf {
        name: String,
        value: f64,
        class_selector: Option<String>,
    },
}

impl Item {
    /// The name of the item.
    pub(crate) fn name(&self) -> &str {
        match self {
            Item::Section { name, .. } | Item::Leaf { name, .. } => name,
        }
    }

    /// The class the item names, if it names one.
    pub(crate) fn class_selector(&self) -> Option<&str> {
        match self {
            Item::Section { class_selector, .. } | Item::Leaf { class_selector, .. } => {
                class_selector.as_deref()
            }
        }
    }

    /// The value of the item, which only a leaf has.
    pub(crate) fn value(&self) -> Option<f64> {
        match self {
            Item::Leaf { value, .. } => Some(*value),
            Item::Section { .. } => None,
        }
    }

    /// Whether the item is a leaf.
    pub(crate) fn is_leaf(&self) -> bool {
        matches!(self, Item::Leaf { .. })
    }
}

/// What the parser reads out of the text.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct TreemapAst {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub rows: Vec<Row>,
}

/// Reads a treemap out of the text.
pub(crate) fn parse_treemap(src: &str) -> Result<TreemapAst, Error> {
    let tokens = tokenize(src, &TOKENS)?;
    // The kinds of the tokens, so that the parser can look at the token after an indentation
    // without consuming it.
    let kinds = tokens.kinds.clone();
    let mut r = Reader::new(tokens, &TOKENS);
    let mut ast = TreemapAst::default();

    if !r.accept(KEYWORD) {
        return Err(r.fail("token of type 'TREEMAP_KEYWORD'"));
    }

    while r.kind() != END {
        match r.kind() {
            ACC_DESCR_KIND => ast.acc_descr = Some(acc_descr_value(r.take())),
            ACC_TITLE_KIND => ast.acc_title = Some(acc_title_value(r.take())),
            TITLE_KIND => ast.title = Some(title_value(r.take())),
            _ => {
                let indent = if r.kind() == INDENTATION {
                    r.take().chars().count()
                } else {
                    0
                };
                let row = match r.kind() {
                    CLASS_DEF => {
                        let (class_name, style_text) = class_def_value(r.take());
                        Row::ClassDef {
                            indent,
                            class_name,
                            style_text,
                        }
                    }
                    STRING2 => Row::Item {
                        indent,
                        item: item(&mut r, &kinds)?,
                    },
                    _ => return Err(r.fail("a name or a class definition")),
                };
                ast.rows.push(row);
            }
        }
    }
    Ok(ast)
}

/// Reads an item: a leaf, which has a value, or a section, which has a class.
fn item(r: &mut Reader<'_>, kinds: &[usize]) -> Result<Item, Error> {
    let name = string_value(r.expect(STRING2)?);
    // The grammar reads a leaf before a section, and the leaf's indentation sits between the
    // name and the separator. An indentation that is not followed by a separator belongs to the
    // row below, so the name is a section and the indentation starts the next row.
    let leaf = match r.kind() {
        SEPARATOR | COMMA => true,
        INDENTATION => {
            let next = kinds.get(r.i + 1).copied().unwrap_or(END);
            next == SEPARATOR || next == COMMA
        }
        _ => false,
    };
    if leaf {
        r.accept(INDENTATION);
        if !r.accept(SEPARATOR) && !r.accept(COMMA) {
            return Err(r.fail("token of type 'SEPARATOR'"));
        }
        r.accept(INDENTATION);
        let text = r.expect(NUMBER2)?;
        let value = js_parse_float(&text.replace(',', ""));
        let class_selector = class_selector(r)?;
        return Ok(Item::Leaf {
            name,
            value,
            class_selector,
        });
    }
    // A section, whose class is optional.
    let class_selector = class_selector(r)?;
    Ok(Item::Section {
        name,
        class_selector,
    })
}

/// Reads the class an item names, if it names one.
fn class_selector(r: &mut Reader<'_>) -> Result<Option<String>, Error> {
    if !r.accept(STYLE_SEPARATOR) {
        return Ok(None);
    }
    Ok(Some(r.expect(ID2)?.to_string()))
}

/// The name of an item: the text between the quotes, as Mermaid's value converter reads it.
fn string_value(text: &str) -> String {
    text[1..text.len() - 1].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(src: &str) -> Vec<Row> {
        parse_treemap(src).expect("parses").rows
    }

    #[test]
    fn reads_the_rows_of_a_treemap() {
        assert_eq!(
            rows("treemap\n\"A\"\n  \"B\": 10\n"),
            vec![
                Row::Item {
                    indent: 0,
                    item: Item::Section {
                        name: "A".to_string(),
                        class_selector: None
                    }
                },
                Row::Item {
                    indent: 2,
                    item: Item::Leaf {
                        name: "B".to_string(),
                        value: 10.0,
                        class_selector: None
                    }
                },
            ]
        );
    }

    #[test]
    fn reads_the_keyword_of_either_spelling() {
        assert!(parse_treemap("treemap-beta\n\"A\"\n").is_ok());
        assert!(parse_treemap("treemap\n\"A\"\n").is_ok());
    }

    #[test]
    fn reads_the_styles_of_a_class() {
        assert_eq!(
            rows("treemap\nclassDef myClass fill:red,stroke:#333;"),
            vec![Row::ClassDef {
                indent: 0,
                class_name: "myClass".to_string(),
                style_text: Some("fill:red,stroke:#333".to_string()),
            }]
        );
    }

    #[test]
    fn reads_a_class_without_styles() {
        assert_eq!(
            rows("treemap\nclassDef myClass;"),
            vec![Row::ClassDef {
                indent: 0,
                class_name: "myClass".to_string(),
                style_text: None,
            }]
        );
    }

    #[test]
    fn reads_a_class_of_one_letter_as_something_else() {
        // The grammar wants at least two characters in the name of a class.
        assert!(parse_treemap("treemap\nclassDef a fill:red").is_err());
    }

    #[test]
    fn reads_a_value_with_a_separator_or_a_comma() {
        let leaf = |src: &str| match &rows(src)[0] {
            Row::Item {
                item: Item::Leaf { value, .. },
                ..
            } => *value,
            other => panic!("a leaf, not {other:?}"),
        };
        assert_eq!(leaf("treemap\n\"A\": 10"), 10.0);
        assert_eq!(leaf("treemap\n\"A\", 10"), 10.0);
        assert_eq!(leaf("treemap\n\"A\" , 10"), 10.0);
        assert_eq!(leaf("treemap\n\"A\": 1,000"), 1000.0);
        assert_eq!(leaf("treemap\n\"A\": 1.5"), 1.5);
    }

    #[test]
    fn reads_a_section_with_a_class() {
        assert_eq!(
            rows("treemap\n\"A\":::cls"),
            vec![Row::Item {
                indent: 0,
                item: Item::Section {
                    name: "A".to_string(),
                    class_selector: Some("cls".to_string())
                }
            }]
        );
    }

    #[test]
    fn rejects_what_mermaid_rejects() {
        // A space before the class of a section makes it a leaf, which has no class.
        assert!(parse_treemap("treemap\n\"A\" :::cls").is_err());
        // A class of one letter is not a class.
        assert!(parse_treemap("treemap\nclassDef a fill:red").is_err());
        // A value that is not a number.
        assert!(parse_treemap("treemap\n\"A\": x").is_err());
        // A keyword that is not the keyword.
        assert!(parse_treemap("treemx\n\"A\"").is_err());
    }

    #[test]
    fn reads_the_title_and_the_accessibility_text() {
        let ast =
            parse_treemap("treemap\ntitle My Chart\naccTitle: A\naccDescr: B\n\"A\": 1").unwrap();
        assert_eq!(ast.title.as_deref(), Some("My Chart"));
        assert_eq!(ast.acc_title.as_deref(), Some("A"));
        assert_eq!(ast.acc_descr.as_deref(), Some("B"));
    }
}
