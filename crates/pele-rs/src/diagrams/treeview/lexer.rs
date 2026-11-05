//! The tokens of Mermaid's tree view grammar.
//!
//! Mermaid reads a tree view with a Langium grammar, `packages/parser/src/language/treeView/
//! treeView.langium` in Mermaid 12.1.0. Its lexer tries the terminals in the order the grammar
//! declares them and keeps the first that matches, so an annotation is read as a whole before
//! the indentation in front of it. The terminals that are hidden in the grammar — the spaces, the
//! comments and the line breaks — carry nothing the tree view needs, so they are dropped here.

use crate::util::is_space;

/// The keyword the diagram starts with.
const KEYWORD: &str = "treeView-beta";

/// The terminals of the grammar, in the order the lexer tries them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Keyword,
    AccDescr,
    AccTitle,
    Title,
    ClassAnnotation,
    IconAnnotation,
    DescAnnotation,
    Indentation,
    QuotedName,
    /// A terminal the grammar hides: the spaces, a comment or a line break.
    Hidden,
    BareName,
}

/// A token of the source.
#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub kind: Kind,
    /// The text the terminal matched.
    pub text: String,
    /// Where the token starts, counted in characters.
    pub start: usize,
}

/// Reads the source into tokens. Fails with the character the lexer could not read.
pub(crate) fn lex(src: &[char]) -> Result<Vec<Token>, usize> {
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < src.len() {
        let Some((kind, end)) = next(src, i) else {
            return Err(i);
        };
        if kind != Kind::Hidden {
            tokens.push(Token {
                kind,
                text: src[i..end].iter().collect(),
                start: i,
            });
        }
        i = end;
    }
    Ok(tokens)
}

/// The terminal that matches at `i`, and where it ends.
fn next(src: &[char], i: usize) -> Option<(Kind, usize)> {
    if let Some(end) = match_keyword(src, i) {
        // The keyword is a name that may be longer: "treeView-beta src" is the name of a node
        // and not the keyword of a diagram.
        if let Some(longer) = match_bare_name(src, i) {
            if longer > end {
                return Some((Kind::BareName, longer));
            }
        }
        return Some((Kind::Keyword, end));
    }
    if let Some(end) = match_acc_descr(src, i) {
        return Some((Kind::AccDescr, end));
    }
    if let Some(end) = match_acc_title(src, i) {
        return Some((Kind::AccTitle, end));
    }
    if let Some(end) = match_title(src, i) {
        return Some((Kind::Title, end));
    }
    if let Some(end) = match_class_annotation(src, i) {
        return Some((Kind::ClassAnnotation, end));
    }
    if let Some(end) = match_icon_annotation(src, i) {
        return Some((Kind::IconAnnotation, end));
    }
    if let Some(end) = match_desc_annotation(src, i) {
        return Some((Kind::DescAnnotation, end));
    }
    if let Some(end) = match_indentation(src, i) {
        return Some((Kind::Indentation, end));
    }
    if let Some(end) = match_quoted_name(src, i) {
        return Some((Kind::QuotedName, end));
    }
    // The spaces are hidden, but the indentation is read before them, so nothing is left of
    // them. The comments and the line breaks are hidden as well.
    if starts_with(src, i, "%%") {
        let mut end = i + 2;
        while end < src.len() && src[end] != '\n' {
            end += 1;
        }
        return Some((Kind::Hidden, end));
    }
    if src[i] == '\n' {
        return Some((Kind::Hidden, i + 1));
    }
    if src[i] == '\r' && src.get(i + 1) == Some(&'\n') {
        return Some((Kind::Hidden, i + 2));
    }
    if let Some(end) = match_bare_name(src, i) {
        return Some((Kind::BareName, end));
    }
    None
}

/// Whether the text at `i` starts with `word`.
fn starts_with(src: &[char], i: usize, word: &str) -> bool {
    word.chars()
        .enumerate()
        .all(|(at, c)| src.get(i + at) == Some(&c))
}

/// The spaces and tabs at `i`.
fn spaces(src: &[char], i: usize) -> usize {
    let mut at = i;
    while at < src.len() && (src[at] == ' ' || src[at] == '\t') {
        at += 1;
    }
    at
}

/// Whether a character may be in the name of a class.
fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// Whether a character may start the name of a class.
fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

/// The end of a terminal that takes the rest of the line. The pattern stops at the first `%%`
/// of the line, which a comment starts.
fn line_end(src: &[char], i: usize) -> usize {
    let mut end = i;
    while end < src.len() && src[end] != '\n' && src[end] != '\r' {
        if src[end] == '%' && src.get(end + 1) == Some(&'%') {
            return end;
        }
        end += 1;
    }
    end
}

/// `treeView-beta`, which nothing but whitespace or a comment may follow.
fn match_keyword(src: &[char], i: usize) -> Option<usize> {
    if !starts_with(src, i, KEYWORD) {
        return None;
    }
    let after = i + KEYWORD.chars().count();
    let follows = match src.get(after) {
        None => true,
        Some(&c) => is_space(c) || starts_with(src, after, "%%"),
    };
    follows.then_some(after)
}

/// `[\t ]*accDescr(?:[\t ]*:…|\s*{…})`.
fn match_acc_descr(src: &[char], i: usize) -> Option<usize> {
    let at = spaces(src, i);
    if !starts_with(src, at, "accDescr") {
        return None;
    }
    let after = at + "accDescr".chars().count();
    let colon = spaces(src, after);
    if src.get(colon) == Some(&':') {
        return Some(line_end(src, colon + 1));
    }
    // The other form takes its text between braces, which may sit on the next line.
    let mut brace = after;
    while brace < src.len() && is_space(src[brace]) {
        brace += 1;
    }
    if src.get(brace) != Some(&'{') {
        return None;
    }
    let mut end = brace + 1;
    while end < src.len() && src[end] != '}' {
        end += 1;
    }
    (end < src.len()).then_some(end + 1)
}

/// `[\t ]*accTitle[\t ]*:…`.
fn match_acc_title(src: &[char], i: usize) -> Option<usize> {
    let at = spaces(src, i);
    if !starts_with(src, at, "accTitle") {
        return None;
    }
    let colon = spaces(src, at + "accTitle".chars().count());
    if src.get(colon) != Some(&':') {
        return None;
    }
    Some(line_end(src, colon + 1))
}

/// `[\t ]*title(?:…|)`, whose text is the rest of the line.
fn match_title(src: &[char], i: usize) -> Option<usize> {
    let at = spaces(src, i);
    if !starts_with(src, at, "title") {
        return None;
    }
    let after = at + "title".chars().count();
    match src.get(after) {
        Some(&c) if c == ' ' || c == '\t' => Some(line_end(src, after + 1)),
        _ => Some(after),
    }
}

/// `[ \t]+:::[ \t]*[A-Za-z_][\w-]*`.
fn match_class_annotation(src: &[char], i: usize) -> Option<usize> {
    let at = spaces(src, i);
    if at == i || !starts_with(src, at, ":::") {
        return None;
    }
    let name = spaces(src, at + 3);
    if !src.get(name).copied().is_some_and(is_name_start) {
        return None;
    }
    let mut end = name + 1;
    while end < src.len() && is_name_char(src[end]) {
        end += 1;
    }
    Some(end)
}

/// `[ \t]+icon\([\w-]*(?::[\w-]+)?\)`.
fn match_icon_annotation(src: &[char], i: usize) -> Option<usize> {
    let at = spaces(src, i);
    if at == i || !starts_with(src, at, "icon(") {
        return None;
    }
    let mut end = at + "icon(".chars().count();
    while end < src.len() && is_name_char(src[end]) {
        end += 1;
    }
    // The pack of an icon, which the name of the icon may carry.
    if src.get(end) == Some(&':') {
        let mut pack = end + 1;
        while pack < src.len() && is_name_char(src[pack]) {
            pack += 1;
        }
        if pack > end + 1 {
            end = pack;
        }
    }
    (src.get(end) == Some(&')')).then_some(end + 1)
}

/// `[ \t]+##[^\n\r]*`.
fn match_desc_annotation(src: &[char], i: usize) -> Option<usize> {
    let at = spaces(src, i);
    if at == i || !starts_with(src, at, "##") {
        return None;
    }
    let mut end = at + 2;
    while end < src.len() && src[end] != '\n' && src[end] != '\r' {
        end += 1;
    }
    Some(end)
}

/// `[ \t]{1,}`.
fn match_indentation(src: &[char], i: usize) -> Option<usize> {
    let at = spaces(src, i);
    (at != i).then_some(at)
}

/// `"[^"]*"` or `'[^']*'`.
fn match_quoted_name(src: &[char], i: usize) -> Option<usize> {
    let quote = match src.get(i) {
        Some(&'"') => '"',
        Some(&'\'') => '\'',
        _ => return None,
    };
    let mut end = i + 1;
    while end < src.len() && src[end] != quote {
        end += 1;
    }
    (end < src.len()).then_some(end + 1)
}

/// `(?!:::|icon\(|##)[^ \t\n\r"'](?:…)*`, the name of a node that is not quoted.
fn match_bare_name(src: &[char], i: usize) -> Option<usize> {
    let first = *src.get(i)?;
    if matches!(first, ' ' | '\t' | '\n' | '\r' | '"' | '\'') {
        return None;
    }
    if starts_with(src, i, ":::") || starts_with(src, i, "icon(") || starts_with(src, i, "##") {
        return None;
    }
    let mut end = i + 1;
    while end < src.len() && src[end] != '\n' && src[end] != '\r' {
        if annotation_ahead(src, end) {
            break;
        }
        end += 1;
    }
    Some(end)
}

/// Whether an annotation starts at `i`, which ends the name before it.
fn annotation_ahead(src: &[char], i: usize) -> bool {
    let at = spaces(src, i);
    if at == i {
        return false;
    }
    if starts_with(src, at, ":::") {
        let name = spaces(src, at + 3);
        return src.get(name).copied().is_some_and(is_name_start);
    }
    starts_with(src, at, "icon(") || starts_with(src, at, "##")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<(Kind, String)> {
        let chars: Vec<char> = src.chars().collect();
        lex(&chars)
            .expect("the source reads")
            .into_iter()
            .map(|t| (t.kind, t.text))
            .collect()
    }

    #[test]
    fn reads_a_name_and_its_annotations() {
        assert_eq!(
            kinds("treeView-beta\n    file.txt :::c icon(x) ## d\n"),
            vec![
                (Kind::Keyword, "treeView-beta".to_string()),
                (Kind::Indentation, "    ".to_string()),
                (Kind::BareName, "file.txt".to_string()),
                (Kind::ClassAnnotation, " :::c".to_string()),
                (Kind::IconAnnotation, " icon(x)".to_string()),
                (Kind::DescAnnotation, " ## d".to_string()),
            ]
        );
    }

    #[test]
    fn reads_a_title_before_the_nodes() {
        assert_eq!(
            kinds("treeView-beta\ntitle A title\n    a\n"),
            vec![
                (Kind::Keyword, "treeView-beta".to_string()),
                (Kind::Title, "title A title".to_string()),
                (Kind::Indentation, "    ".to_string()),
                (Kind::BareName, "a".to_string()),
            ]
        );
    }

    #[test]
    fn reads_the_accessibility_lines() {
        assert_eq!(
            kinds("treeView-beta\naccTitle: A\naccDescr {B}\n"),
            vec![
                (Kind::Keyword, "treeView-beta".to_string()),
                (Kind::AccTitle, "accTitle: A".to_string()),
                (Kind::AccDescr, "accDescr {B}".to_string()),
            ]
        );
    }

    #[test]
    fn reads_a_quoted_name() {
        assert_eq!(
            kinds("treeView-beta\n    \"a name\"\n"),
            vec![
                (Kind::Keyword, "treeView-beta".to_string()),
                (Kind::Indentation, "    ".to_string()),
                (Kind::QuotedName, "\"a name\"".to_string()),
            ]
        );
    }

    #[test]
    fn stops_a_name_before_an_annotation() {
        assert_eq!(
            kinds("treeView-beta\n    a b :::c\n"),
            vec![
                (Kind::Keyword, "treeView-beta".to_string()),
                (Kind::Indentation, "    ".to_string()),
                (Kind::BareName, "a b".to_string()),
                (Kind::ClassAnnotation, " :::c".to_string()),
            ]
        );
    }

    #[test]
    fn reads_the_keyword_only_when_nothing_follows_it() {
        assert_eq!(
            kinds("treeView-beta-x\n"),
            vec![(Kind::BareName, "treeView-beta-x".to_string())]
        );
    }

    #[test]
    fn reads_a_longer_name_instead_of_the_keyword() {
        // The keyword is a name that may be longer, so "treeView-beta src" is a node.
        assert_eq!(
            kinds("treeView-beta src\n"),
            vec![(Kind::BareName, "treeView-beta src".to_string())]
        );
        assert_eq!(
            kinds("treeView-beta\n"),
            vec![(Kind::Keyword, "treeView-beta".to_string())]
        );
    }
}
