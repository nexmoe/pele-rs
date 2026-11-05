//! The architecture diagram parser.
//!
//! Mermaid reads this diagram with a Langium grammar, not with a jison one: the words of the
//! diagram are keywords of the grammar and the names are a terminal of their own. The tokenizer
//! below follows the token types Mermaid's lexer builds, in the order it builds them, and the
//! parser follows the grammar of `architecture.langium`.
//!
//! The grammar of Mermaid 12.1.0 (commit `21f72f07`):
//!
//! ```text
//! entry Architecture: NEWLINE* 'architecture-beta' (NEWLINE | TitleAndAccessibilities | Statement)*;
//! TitleAndAccessibilities: ((accDescr=ACC_DESCR | accTitle=ACC_TITLE | title=TITLE) EOL)+;
//! Group: 'group' id=ID icon=ARCH_ICON? title=ARCH_TITLE? ('in' in=ID)? EOL;
//! Service: 'service' id=ID (iconText=STRING | icon=ARCH_ICON)? title=ARCH_TITLE? ('in' in=ID)? EOL;
//! Junction: 'junction' id=ID ('in' in=ID)? EOL;
//! Edge: lhsId=ID lhsGroup?=ARROW_GROUP? Arrow rhsId=ID rhsGroup?=ARROW_GROUP? EOL;
//! Alignment: 'align' direction=('row' | 'column') members+=ID (members+=ID)+ EOL;
//! Arrow: LeftPort lhsInto?=ARROW_INTO? ('--' | '-' title=ARCH_TITLE '-') rhsInto?=ARROW_INTO? RightPort;
//! LeftPort: ':' lhsDir=ARROW_DIRECTION;
//! RightPort: rhsDir=ARROW_DIRECTION ':';
//! EOL returns string: NEWLINE+ | EOF;
//! ```

use crate::diagrams::common::tokens::{
    ACC_DESCR, ACC_TITLE, DIRECTIVE, END, Found, NEWLINE, Reader, SINGLE_LINE_COMMENT, STRING,
    TITLE, TokenType, WHITESPACE, YAML, acc_descr_value, acc_title_value, literal_token,
    match_token, string_value, title_value, tokenize, word_token,
};
use crate::error::Error;
use crate::util::{is_word, trim};

/// The kinds of token, as indexes into [`TYPES`].
const HEADER: usize = 0;
const JUNCTION: usize = 1;
const SERVICE: usize = 2;
const COLUMN: usize = 3;
const GROUP: usize = 4;
const ALIGN: usize = 5;
const ROW: usize = 6;
const DASHES: usize = 7;
const IN: usize = 8;
const COLON: usize = 9;
const DASH: usize = 10;
const ARROW_DIRECTION: usize = 11;
const ARROW_GROUP: usize = 12;
const ARROW_INTO: usize = 13;
const ACC_DESCR_TOKEN: usize = 14;
const ACC_TITLE_TOKEN: usize = 15;
const TITLE_TOKEN: usize = 16;
const STRING_TOKEN: usize = 17;
const ID: usize = 18;
const NEWLINE_TOKEN: usize = 19;
/// The places of the token types that the parser does not read itself.
#[allow(dead_code, reason = "the places of the token types")]
const WHITESPACE_TOKEN: usize = 20;
const YAML_TOKEN: usize = 21;
#[allow(dead_code, reason = "the places of the token types")]
const DIRECTIVE_TOKEN: usize = 22;
#[allow(dead_code, reason = "the places of the token types")]
const COMMENT_TOKEN: usize = 23;
const ARCH_ICON: usize = 24;
const ARCH_TITLE: usize = 25;

/// The token types, in the order Mermaid's lexer has them: the keywords first, the longest
/// first, and then the terminals. A keyword is matched as plain text, but a name that may be
/// longer than it wins: `service` is the word of the grammar, and `serviceX` is a name. The
/// words of the grammar are therefore names no more, which is what Mermaid does.
const TYPES: &[TokenType] = &[
    word_token("architecture-beta", &[ID]),
    word_token("junction", &[ID]),
    word_token("service", &[ID]),
    word_token("column", &[ID]),
    word_token("group", &[ID]),
    word_token("align", &[ID]),
    word_token("row", &[ID]),
    word_token("--", &[YAML_TOKEN]),
    word_token("in", &[ID]),
    literal_token(":"),
    word_token("-", &[YAML_TOKEN]),
    match_token("ARROW_DIRECTION", arrow_direction),
    literal_token("{group}"),
    match_token("ARROW_INTO", arrow_into),
    ACC_DESCR,
    ACC_TITLE,
    TITLE,
    STRING,
    match_token("ID", identifier),
    NEWLINE,
    WHITESPACE,
    YAML,
    DIRECTIVE,
    SINGLE_LINE_COMMENT,
    match_token("ARCH_ICON", arch_icon),
    match_token("ARCH_TITLE", arch_title),
];

/// The side of a node an edge leaves from or enters at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Side {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    pub(crate) fn parse(text: &str) -> Side {
        match text {
            "R" => Side::Right,
            "T" => Side::Top,
            "B" => Side::Bottom,
            _ => Side::Left,
        }
    }

    /// The index of the side: left, right, top, bottom.
    pub(crate) fn index(self) -> usize {
        match self {
            Side::Left => 0,
            Side::Right => 1,
            Side::Top => 2,
            Side::Bottom => 3,
        }
    }
}

/// Whether an alignment puts its members in a row or in a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum AlignDirection {
    #[default]
    Row,
    Column,
}

impl AlignDirection {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            AlignDirection::Row => "row",
            AlignDirection::Column => "column",
        }
    }
}

/// A group of services, as the grammar reads it.
#[derive(Debug, Clone, Default)]
pub(crate) struct Group {
    pub id: String,
    pub icon: Option<String>,
    pub title: Option<String>,
    pub parent: Option<String>,
}

/// A service, as the grammar reads it.
#[derive(Debug, Clone, Default)]
pub(crate) struct Service {
    pub id: String,
    pub icon: Option<String>,
    pub icon_text: Option<String>,
    pub title: Option<String>,
    pub parent: Option<String>,
}

/// A junction, as the grammar reads it.
#[derive(Debug, Clone, Default)]
pub(crate) struct Junction {
    pub id: String,
    pub parent: Option<String>,
}

/// An edge between two services or junctions.
#[derive(Debug, Clone, Default)]
pub(crate) struct Edge {
    pub lhs_id: String,
    pub lhs_dir: Side,
    pub lhs_into: bool,
    pub lhs_group: bool,
    pub rhs_id: String,
    pub rhs_dir: Side,
    pub rhs_into: bool,
    pub rhs_group: bool,
    pub title: Option<String>,
}

/// An alignment of two or more services or junctions.
#[derive(Debug, Clone, Default)]
pub(crate) struct Alignment {
    pub direction: AlignDirection,
    pub members: Vec<String>,
}

/// What the grammar reads.
#[derive(Debug, Clone, Default)]
pub(crate) struct Ast {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub groups: Vec<Group>,
    pub services: Vec<Service>,
    pub junctions: Vec<Junction>,
    pub edges: Vec<Edge>,
    pub alignments: Vec<Alignment>,
}

/// Reads a diagram.
pub(crate) fn parse_architecture(src: &str) -> Result<Ast, Error> {
    let tokens = tokenize(src, TYPES)?;
    let mut r = Reader::new(tokens, TYPES);
    let mut ast = Ast::default();
    while r.kind() == NEWLINE_TOKEN {
        r.take();
    }
    r.expect(HEADER)?;
    loop {
        match r.kind() {
            END => break,
            NEWLINE_TOKEN => {
                r.take();
            }
            ACC_DESCR_TOKEN => {
                ast.acc_descr = Some(acc_descr_value(r.take()));
                end_of_line(&mut r)?;
            }
            ACC_TITLE_TOKEN => {
                ast.acc_title = Some(acc_title_value(r.take()));
                end_of_line(&mut r)?;
            }
            TITLE_TOKEN => {
                ast.title = Some(title_value(r.take()));
                end_of_line(&mut r)?;
            }
            GROUP => ast.groups.push(parse_group(&mut r)?),
            SERVICE => ast.services.push(parse_service(&mut r)?),
            JUNCTION => ast.junctions.push(parse_junction(&mut r)?),
            ALIGN => ast.alignments.push(parse_alignment(&mut r)?),
            ID => ast.edges.push(parse_edge(&mut r)?),
            _ => return Err(r.fail("a statement")),
        }
    }
    Ok(ast)
}

/// `EOL`: a line break, or the end of the diagram.
fn end_of_line(r: &mut Reader<'_>) -> Result<(), Error> {
    if r.kind() == END {
        return Ok(());
    }
    r.expect(NEWLINE_TOKEN)?;
    while r.kind() == NEWLINE_TOKEN {
        r.take();
    }
    Ok(())
}

/// `'in' in=ID?`: the group a statement is placed in.
fn parent_of(r: &mut Reader<'_>) -> Result<Option<String>, Error> {
    if !r.accept(IN) {
        return Ok(None);
    }
    Ok(Some(r.expect(ID)?.to_string()))
}

/// `Group: 'group' id=ID icon=ARCH_ICON? title=ARCH_TITLE? ('in' in=ID)? EOL;`
fn parse_group(r: &mut Reader<'_>) -> Result<Group, Error> {
    r.expect(GROUP)?;
    let id = r.expect(ID)?.to_string();
    let icon = if r.kind() == ARCH_ICON {
        Some(arch_icon_value(r.take()))
    } else {
        None
    };
    let title = if r.kind() == ARCH_TITLE {
        Some(arch_title_value(r.take()))
    } else {
        None
    };
    let parent = parent_of(r)?;
    end_of_line(r)?;
    Ok(Group {
        id,
        icon,
        title,
        parent,
    })
}

/// `Service: 'service' id=ID (iconText=STRING | icon=ARCH_ICON)? title=ARCH_TITLE?
/// ('in' in=ID)? EOL;`
fn parse_service(r: &mut Reader<'_>) -> Result<Service, Error> {
    r.expect(SERVICE)?;
    let id = r.expect(ID)?.to_string();
    let (icon, icon_text) = if r.kind() == STRING_TOKEN {
        (None, Some(string_value(r.take())))
    } else if r.kind() == ARCH_ICON {
        (Some(arch_icon_value(r.take())), None)
    } else {
        (None, None)
    };
    let title = if r.kind() == ARCH_TITLE {
        Some(arch_title_value(r.take()))
    } else {
        None
    };
    let parent = parent_of(r)?;
    end_of_line(r)?;
    Ok(Service {
        id,
        icon,
        icon_text,
        title,
        parent,
    })
}

/// `Junction: 'junction' id=ID ('in' in=ID)? EOL;`
fn parse_junction(r: &mut Reader<'_>) -> Result<Junction, Error> {
    r.expect(JUNCTION)?;
    let id = r.expect(ID)?.to_string();
    let parent = parent_of(r)?;
    end_of_line(r)?;
    Ok(Junction { id, parent })
}

/// `Alignment: 'align' direction=('row' | 'column') members+=ID (members+=ID)+ EOL;`
fn parse_alignment(r: &mut Reader<'_>) -> Result<Alignment, Error> {
    r.expect(ALIGN)?;
    let direction = if r.accept(ROW) {
        AlignDirection::Row
    } else if r.accept(COLUMN) {
        AlignDirection::Column
    } else {
        return Err(r.fail("a row or a column"));
    };
    let mut members = vec![r.expect(ID)?.to_string()];
    members.push(r.expect(ID)?.to_string());
    while r.kind() == ID {
        members.push(r.take().to_string());
    }
    end_of_line(r)?;
    Ok(Alignment { direction, members })
}

/// `Edge: lhsId=ID lhsGroup?=ARROW_GROUP? Arrow rhsId=ID rhsGroup?=ARROW_GROUP? EOL;`
fn parse_edge(r: &mut Reader<'_>) -> Result<Edge, Error> {
    let lhs_id = r.expect(ID)?.to_string();
    let lhs_group = r.accept(ARROW_GROUP);
    // LeftPort: ':' lhsDir=ARROW_DIRECTION
    r.expect(COLON)?;
    let lhs_dir = Side::parse(r.expect(ARROW_DIRECTION)?);
    let lhs_into = r.accept(ARROW_INTO);
    // ('--' | '-' title=ARCH_TITLE '-')
    let title = if r.accept(DASHES) {
        None
    } else {
        r.expect(DASH)?;
        let title = arch_title_value(r.expect(ARCH_TITLE)?);
        r.expect(DASH)?;
        Some(title)
    };
    let rhs_into = r.accept(ARROW_INTO);
    // RightPort: rhsDir=ARROW_DIRECTION ':'
    let rhs_dir = Side::parse(r.expect(ARROW_DIRECTION)?);
    r.expect(COLON)?;
    let rhs_id = r.expect(ID)?.to_string();
    let rhs_group = r.accept(ARROW_GROUP);
    end_of_line(r)?;
    Ok(Edge {
        lhs_id,
        lhs_dir,
        lhs_into,
        lhs_group,
        rhs_id,
        rhs_dir,
        rhs_into,
        rhs_group,
        title,
    })
}

/// `L|R|T|B`: the side of a node an edge leaves from.
fn arrow_direction(src: &str, at: usize) -> Found {
    match src.as_bytes().get(at) {
        Some(b'L' | b'R' | b'T' | b'B') => Found::End(at + 1),
        _ => Found::No,
    }
}

/// `<|>`: the head of an arrow.
fn arrow_into(src: &str, at: usize) -> Found {
    match src.as_bytes().get(at) {
        Some(b'<' | b'>') => Found::End(at + 1),
        _ => Found::No,
    }
}

/// `[\w]([-\w]*\w)?`: a name. It may hold dashes, but it does not end with one.
fn identifier(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    if !bytes.get(at).is_some_and(|&b| is_word(b as char)) {
        return Found::No;
    }
    let mut end = at + 1;
    while end < bytes.len() && (is_word(bytes[end] as char) || bytes[end] == b'-') {
        end += 1;
    }
    // The name ends at the last word character, so the dashes at its end are not part of it.
    let mut stop = end;
    while stop > at + 1 && bytes[stop - 1] == b'-' {
        stop -= 1;
    }
    Found::End(stop)
}

/// `\([\w-:]+\)`: the name of an icon.
fn arch_icon(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    if bytes.get(at) != Some(&b'(') {
        return Found::No;
    }
    let mut end = at + 1;
    while end < bytes.len()
        && (is_word(bytes[end] as char) || bytes[end] == b'-' || bytes[end] == b':')
    {
        end += 1;
    }
    if end == at + 1 || bytes.get(end) != Some(&b')') {
        return Found::No;
    }
    Found::End(end + 1)
}

/// `\[(?:"([^"\\]|\\.)*"|'([^'\\]|\\.)*'|[^\[\]\r\n]+)\]`: the title of a service or a group.
fn arch_title(src: &str, at: usize) -> Found {
    if src.as_bytes().get(at) != Some(&b'[') {
        return Found::No;
    }
    if let Some(end) = quoted_title(src, at) {
        return Found::End(end);
    }
    // A title without quotes holds anything but brackets and line breaks.
    let bytes = src.as_bytes();
    let mut end = at + 1;
    while end < bytes.len() {
        match bytes[end] {
            b']' => break,
            b'[' | b'\n' | b'\r' => return Found::No,
            _ => end += 1,
        }
    }
    if end == at + 1 || bytes.get(end) != Some(&b']') {
        return Found::No;
    }
    Found::End(end + 1)
}

/// The title of a service or a group, when it is written in quotes.
fn quoted_title(src: &str, at: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    let quote = *bytes.get(at + 1)?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    let mut i = at + 2;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                // The dot of the grammar is any character but a line break.
                match bytes.get(i + 1) {
                    Some(&next) if next != b'\n' && next != b'\r' => i += 2,
                    _ => return None,
                }
            }
            b'\n' | b'\r' => return None,
            c if c == quote => {
                return (bytes.get(i + 1) == Some(&b']')).then_some(i + 2);
            }
            _ => i += 1,
        }
    }
    None
}

/// The name of an icon, without its brackets.
fn arch_icon_value(text: &str) -> String {
    let inner: String = text.chars().filter(|&c| c != '(' && c != ')').collect();
    trim(&inner).to_string()
}

/// The title of a service or a group, without its brackets and quotes.
fn arch_title_value(text: &str) -> String {
    let mut inner = text;
    if let Some(rest) = inner.strip_prefix('[') {
        inner = rest;
    }
    if let Some(rest) = inner.strip_suffix(']') {
        inner = rest;
    }
    let inner = trim(inner);
    let inner = if (inner.starts_with('"') && inner.ends_with('"'))
        || (inner.starts_with('\'') && inner.ends_with('\''))
    {
        if inner.len() <= 1 {
            ""
        } else {
            &inner[1..inner.len() - 1]
        }
    } else {
        inner
    };
    let inner = inner.replace("\\\"", "\"").replace("\\'", "'");
    trim(&inner).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Ast {
        parse_architecture(src).expect("the diagram is read")
    }

    #[test]
    fn reads_the_header_and_the_statements() {
        let ast = parse(
            "architecture-beta\nservice db(database)[Database] in api\ngroup api(cloud)[API]\n",
        );
        assert_eq!(ast.services.len(), 1);
        assert_eq!(ast.services[0].id, "db");
        assert_eq!(ast.services[0].icon.as_deref(), Some("database"));
        assert_eq!(ast.services[0].title.as_deref(), Some("Database"));
        assert_eq!(ast.services[0].parent.as_deref(), Some("api"));
        assert_eq!(ast.groups.len(), 1);
        assert_eq!(ast.groups[0].title.as_deref(), Some("API"));
    }

    #[test]
    fn reads_the_titles() {
        let ast = parse("architecture-beta title a title\naccTitle: t\naccDescr: d\n");
        assert_eq!(ast.title.as_deref(), Some("a title"));
        assert_eq!(ast.acc_title.as_deref(), Some("t"));
        assert_eq!(ast.acc_descr.as_deref(), Some("d"));
    }

    #[test]
    fn reads_the_edges() {
        let ast =
            parse("architecture-beta\nservice a(server)[A]\nservice b(server)[B]\na:R --> L:b\n");
        assert_eq!(ast.edges.len(), 1);
        let edge = &ast.edges[0];
        assert_eq!(edge.lhs_id, "a");
        assert_eq!(edge.lhs_dir, Side::Right);
        assert!(edge.rhs_into);
        assert_eq!(edge.rhs_id, "b");
        assert_eq!(edge.rhs_dir, Side::Left);
        assert!(!edge.lhs_group);
    }

    #[test]
    fn reads_the_title_of_an_edge() {
        let ast = parse(
            "architecture-beta\nservice a(server)[A]\nservice b(server)[B]\na:R - [t] - L:b\n",
        );
        assert_eq!(ast.edges[0].title.as_deref(), Some("t"));
    }

    #[test]
    fn reads_the_alignments() {
        let ast =
            parse("architecture-beta\nservice a(server)[A]\nservice b(server)[B]\nalign row a b\n");
        assert_eq!(ast.alignments.len(), 1);
        assert_eq!(ast.alignments[0].direction, AlignDirection::Row);
        assert_eq!(ast.alignments[0].members, ["a", "b"]);
    }

    #[test]
    fn refuses_the_names_that_are_words_of_the_grammar() {
        // `align`, `row` and `column` are words of the grammar, so they are not names.
        for word in ["align", "row", "column"] {
            let src = format!("architecture-beta\nservice {word}(server)[x]\n");
            assert!(
                parse_architecture(&src).is_err(),
                "{word} was read as a name"
            );
        }
        // A name that starts with one of them is a name all the same.
        let ast = parse("architecture-beta\nservice rowspan(server)[x]\n");
        assert_eq!(ast.services[0].id, "rowspan");
    }

    #[test]
    fn refuses_a_name_that_starts_with_a_direction() {
        // `L`, `R`, `T` and `B` are read as directions, so a name may not start with one.
        for word in ["Lx", "Rx", "Tx", "Bx"] {
            let src = format!("architecture-beta\nservice {word}(server)[x]\n");
            assert!(
                parse_architecture(&src).is_err(),
                "{word} was read as a name"
            );
        }
    }

    #[test]
    fn reads_a_name_with_dashes() {
        let ast = parse("architecture-beta\nservice a-b-c(server)[x]\n");
        assert_eq!(ast.services[0].id, "a-b-c");
    }

    #[test]
    fn reads_the_text_of_an_icon() {
        let ast = parse("architecture-beta\nservice db\"DB\"[Database]\n");
        assert_eq!(ast.services[0].icon_text.as_deref(), Some("DB"));
        assert_eq!(ast.services[0].icon, None);
    }

    #[test]
    fn reads_a_quoted_title() {
        let ast = parse("architecture-beta\nservice a(server)[\"The \\\"Main\\\" API\"]\n");
        assert_eq!(ast.services[0].title.as_deref(), Some("The \"Main\" API"));
    }
}
