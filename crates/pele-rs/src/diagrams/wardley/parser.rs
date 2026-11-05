//! The Wardley map parser.
//!
//! Mermaid reads this diagram with a Langium grammar, not with a jison one: the words of the
//! diagram are terminals of the grammar and the names are a terminal of their own. The tokenizer
//! below follows the token types Mermaid's lexer builds, in the order it builds them, and the
//! parser follows the grammar of Mermaid 12.1.0 (commit `21f72f07`):
//!
//! ```text
//! entry Wardley: NEWLINE* KW_WARDLEY (NEWLINE | TitleAndAccessibilities | Statement)*;
//! TitleAndAccessibilities: ((accDescr=ACC_DESCR | accTitle=ACC_TITLE | title=TITLE) EOL)+;
//! Statement: size=Size | evolution=Evolution | anchors+=Anchor | components+=Component
//!   | links+=Link | evolves+=Evolve | pipelines+=Pipeline | notes+=Note
//!   | annotations+=Annotations | annotation+=Annotation | accelerators+=Accelerator
//!   | deaccelerators+=Deaccelerator;
//! Size: KW_SIZE '[' width=INT ',' height=INT ']' EOL;
//! Evolution: KW_EVOLUTION stages+=EvolutionStage (ARROW stages+=EvolutionStage)+ EOL;
//! EvolutionStage: name=(ID | STRING | NAME_WITH_SPACES) ('@' boundary=WARDLEY_NUMBER)?
//!   ('/' secondName=(ID | STRING | NAME_WITH_SPACES))?;
//! Anchor: KW_ANCHOR name=(STRING | ID | NAME_WITH_SPACES) '[' visibility=WARDLEY_NUMBER ','
//!   evolution=WARDLEY_NUMBER ']' EOL;
//! Component: KW_COMPONENT name=(STRING | ID | NAME_WITH_SPACES) '[' visibility=WARDLEY_NUMBER ','
//!   evolution=WARDLEY_NUMBER ']' label=Label? decorator=Decorator?
//!   (inertia?=KW_INERTIA | '(' inertia?=KW_INERTIA ')')? EOL;
//! Label: KW_LABEL '[' negX?='-'? offsetX=INT ',' negY?='-'? offsetY=INT ']';
//! Decorator: '(' strategy=STRATEGY ')';
//! Link: from=(STRING | ID | NAME_WITH_SPACES) fromPort=LINK_PORT? arrow=(LINK_ARROW | ARROW)?
//!   to=(STRING | ID | NAME_WITH_SPACES) toPort=LINK_PORT? linkLabel=LINK_LABEL? EOL;
//! Evolve: KW_EVOLVE component=(STRING | ID | NAME_WITH_SPACES) target=WARDLEY_NUMBER EOL;
//! Pipeline: KW_PIPELINE parent=(STRING | ID | NAME_WITH_SPACES) '{' NEWLINE+
//!   components+=PipelineComponent+ '}' EOL;
//! PipelineComponent: KW_COMPONENT name=(STRING | ID | NAME_WITH_SPACES) '[' evolution=WARDLEY_NUMBER
//!   ']' label=Label? EOL;
//! Annotations: KW_ANNOTATIONS '[' x=CoordinateValue ',' y=CoordinateValue ']' EOL;
//! Annotation: KW_ANNOTATION number=INT ',' '[' x=CoordinateValue ',' y=CoordinateValue ']'
//!   text=STRING EOL;
//! Accelerator: KW_ACCELERATOR name=(STRING | ID | NAME_WITH_SPACES) '[' x=WARDLEY_NUMBER ','
//!   y=WARDLEY_NUMBER ']' EOL;
//! Deaccelerator: KW_DEACCELERATOR name=(STRING | ID | NAME_WITH_SPACES) '[' x=WARDLEY_NUMBER ','
//!   y=WARDLEY_NUMBER ']' EOL;
//! CoordinateValue: WARDLEY_NUMBER | INT;
//! EOL returns string: NEWLINE+ | EOF;
//! ```
//!
//! Mermaid's lexer takes the first token type that matches, as Chevrotain does, and a keyword
//! that a name may be longer than wins over the name only when the name is longer: `size` is
//! the word of the grammar, and `sizes` is not a name Mermaid reads, because the word is matched
//! first and the rest of the text is left over.

use crate::diagrams::common::tokens::{
    ACC_DESCR, ACC_TITLE, DIRECTIVE, END, Found, NEWLINE, Pattern, Reader, SINGLE_LINE_COMMENT,
    STRING, TITLE, TokenType, WHITESPACE, YAML, acc_descr_value, acc_title_value, string_value,
    title_value, tokenize,
};
use crate::error::Error;
use crate::util::is_space;

/// The kinds of token, as indexes into [`TYPES`].
const LBRACKET: usize = 0;
const COMMA: usize = 1;
const RBRACKET: usize = 2;
const AT: usize = 3;
const SLASH: usize = 4;
const LPAREN: usize = 5;
const RPAREN: usize = 6;
const MINUS: usize = 7;
const LBRACE: usize = 8;
const RBRACE: usize = 9;
const WARDLEY_NUMBER: usize = 10;
const ARROW: usize = 11;
const LINK_PORT: usize = 12;
const LINK_ARROW: usize = 13;
const LINK_LABEL: usize = 14;
const STRATEGY: usize = 15;
const KW_WARDLEY: usize = 16;
const KW_SIZE: usize = 17;
const KW_EVOLUTION: usize = 18;
const KW_ANCHOR: usize = 19;
const KW_COMPONENT: usize = 20;
const KW_LABEL: usize = 21;
const KW_INERTIA: usize = 22;
const KW_EVOLVE: usize = 23;
const KW_PIPELINE: usize = 24;
const KW_NOTE: usize = 25;
const KW_ANNOTATIONS: usize = 26;
const KW_ANNOTATION: usize = 27;
const KW_ACCELERATOR: usize = 28;
const KW_DEACCELERATOR: usize = 29;
const NAME_WITH_SPACES: usize = 30;
const ACC_DESCR_TOKEN: usize = 31;
const ACC_TITLE_TOKEN: usize = 32;
const TITLE_TOKEN: usize = 33;
const INT: usize = 34;
const STRING_TOKEN: usize = 35;
const ID: usize = 36;
const NEWLINE_TOKEN: usize = 37;
/// The places of the token types that the parser does not read itself.
#[allow(dead_code, reason = "the places of the token types")]
const WHITESPACE_TOKEN: usize = 38;
#[allow(dead_code, reason = "the places of the token types")]
const YAML_TOKEN: usize = 39;
#[allow(dead_code, reason = "the places of the token types")]
const DIRECTIVE_TOKEN: usize = 40;
#[allow(dead_code, reason = "the places of the token types")]
const COMMENT_TOKEN: usize = 41;

/// A token type that is a fixed text.
const fn literal(name: &'static str, text: &'static str, longer: &'static [usize]) -> TokenType {
    TokenType {
        name,
        pattern: Pattern::Literal(text),
        hidden: false,
        longer,
    }
}

/// A token type that a function finds.
const fn matched(name: &'static str, f: fn(&str, usize) -> Found) -> TokenType {
    TokenType {
        name,
        pattern: Pattern::Match(f),
        hidden: false,
        longer: &[],
    }
}

/// The token types, in the order Mermaid's lexer has them: the words of the grammar first, the
/// terminals after them, and the name of a thing last of the visible ones, so that a name is
/// read as a name and not as a word of the grammar.
///
/// Mermaid's own lexer has a hidden `WS` of its own before `ACC_DESCR`; it is the same text as
/// the common `WHITESPACE` and is skipped either way.
const TYPES: &[TokenType] = &[
    literal("[", "[", &[]),
    literal(",", ",", &[]),
    literal("]", "]", &[]),
    literal("@", "@", &[]),
    literal("/", "/", &[]),
    literal("(", "(", &[]),
    literal(")", ")", &[]),
    // A dash is a word of its own, but an arrow or a front matter fence that starts with one is
    // longer, and so becomes the token.
    literal("-", "-", &[ARROW, LINK_ARROW, YAML_TOKEN]),
    literal("{", "{", &[]),
    literal("}", "}", &[]),
    matched("WARDLEY_NUMBER", wardley_number),
    matched("ARROW", arrow),
    matched("LINK_PORT", link_port),
    matched("LINK_ARROW", link_arrow),
    matched("LINK_LABEL", link_label),
    matched("STRATEGY", strategy),
    literal("KW_WARDLEY", "wardley-beta", &[]),
    literal("KW_SIZE", "size", &[]),
    literal("KW_EVOLUTION", "evolution", &[]),
    literal("KW_ANCHOR", "anchor", &[]),
    literal("KW_COMPONENT", "component", &[]),
    literal("KW_LABEL", "label", &[]),
    literal("KW_INERTIA", "inertia", &[]),
    literal("KW_EVOLVE", "evolve", &[]),
    literal("KW_PIPELINE", "pipeline", &[]),
    literal("KW_NOTE", "note", &[]),
    literal("KW_ANNOTATIONS", "annotations", &[]),
    literal("KW_ANNOTATION", "annotation", &[]),
    literal("KW_ACCELERATOR", "accelerator", &[]),
    literal("KW_DEACCELERATOR", "deaccelerator", &[]),
    matched("NAME_WITH_SPACES", name_with_spaces),
    ACC_DESCR,
    ACC_TITLE,
    TITLE,
    matched("INT", int),
    STRING,
    matched("ID", identifier),
    NEWLINE,
    WHITESPACE,
    YAML,
    DIRECTIVE,
    SINGLE_LINE_COMMENT,
];

/// `[0-9]+\.[0-9]+`
fn wardley_number(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    let mut end = at;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    if end == at || bytes.get(end) != Some(&b'.') {
        return Found::No;
    }
    let fraction = end + 1;
    let mut end = fraction;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    if end == fraction {
        return Found::No;
    }
    Found::End(end)
}

/// `->`
fn arrow(src: &str, at: usize) -> Found {
    if src[at..].starts_with("->") {
        Found::End(at + 2)
    } else {
        Found::No
    }
}

/// `\+<>|\+>|\+<`
fn link_port(src: &str, at: usize) -> Found {
    for port in ["+<>", "+>", "+<"] {
        if src[at..].starts_with(port) {
            return Found::End(at + port.len());
        }
    }
    Found::No
}

/// `-->|-\.->|>|\+'[^']*'<>|\+'[^']*'<|\+'[^']*'>`
fn link_arrow(src: &str, at: usize) -> Found {
    for arrow in ["-->", "-.->", ">"] {
        if src[at..].starts_with(arrow) {
            return Found::End(at + arrow.len());
        }
    }
    // `+'[^']*'` with a mark of its own after it.
    let bytes = src.as_bytes();
    if bytes.get(at) != Some(&b'+') || bytes.get(at + 1) != Some(&b'\'') {
        return Found::No;
    }
    let Some(quote) = src[at + 2..].find('\'').map(|i| at + 2 + i) else {
        return Found::No;
    };
    let after = quote + 1;
    if src[after..].starts_with("<>") {
        Found::End(after + 2)
    } else if bytes.get(after).is_some_and(|&c| c == b'<' || c == b'>') {
        Found::End(after + 1)
    } else {
        Found::No
    }
}

/// `;[^\n\r]+`
fn link_label(src: &str, at: usize) -> Found {
    if !src[at..].starts_with(';') {
        return Found::No;
    }
    let end = src[at + 1..]
        .find(['\n', '\r'])
        .map_or(src.len(), |i| at + 1 + i);
    if end == at + 1 {
        return Found::No;
    }
    Found::End(end)
}

/// `build|buy|outsource|market`
fn strategy(src: &str, at: usize) -> Found {
    for word in ["build", "buy", "outsource", "market"] {
        if src[at..].starts_with(word) {
            return Found::End(at + word.len());
        }
    }
    Found::No
}

/// `(?!title\s|accTitle|accDescr)[A-Za-z](?:[A-Za-z0-9_()&]|-(?!>))*
/// (?:[ \t]+[A-Za-z(](?:[A-Za-z0-9_()&]|-(?!>))*)*`
fn name_with_spaces(src: &str, at: usize) -> Found {
    if reserved(src, at) {
        return Found::No;
    }
    let bytes = src.as_bytes();
    if !bytes.get(at).is_some_and(u8::is_ascii_alphabetic) {
        return Found::No;
    }
    let mut end = name_run(src, at + 1);
    loop {
        let mut next = end;
        while matches!(bytes.get(next), Some(b' ' | b'\t')) {
            next += 1;
        }
        if next == end {
            break;
        }
        if !bytes
            .get(next)
            .is_some_and(|&c| c.is_ascii_alphabetic() || c == b'(')
        {
            break;
        }
        end = name_run(src, next + 1);
    }
    Found::End(end)
}

/// The text of a name that starts with a word the grammar keeps for itself.
fn reserved(src: &str, at: usize) -> bool {
    let rest = &src[at..];
    if rest.starts_with("accTitle") || rest.starts_with("accDescr") {
        return true;
    }
    rest.starts_with("title") && rest[5..].chars().next().is_some_and(is_space)
}

/// The run of a name that follows its first letter.
fn name_run(src: &str, mut at: usize) -> usize {
    let bytes = src.as_bytes();
    while let Some(&c) = bytes.get(at) {
        let keeps = c.is_ascii_alphanumeric()
            || matches!(c, b'_' | b'(' | b')' | b'&')
            || (c == b'-' && bytes.get(at + 1) != Some(&b'>'));
        if !keeps {
            break;
        }
        at += 1;
    }
    at
}

/// `0|[1-9][0-9]*(?!\.)`
fn int(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    let Some(&first) = bytes.get(at) else {
        return Found::No;
    };
    if first == b'0' {
        return Found::End(at + 1);
    }
    if !first.is_ascii_digit() {
        return Found::No;
    }
    let mut end = at + 1;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    // The lookahead may make the number shorter, as the expression does when it gives digits back.
    while end > at + 1 && bytes.get(end) == Some(&b'.') {
        end -= 1;
    }
    if bytes.get(end) == Some(&b'.') {
        return Found::No;
    }
    Found::End(end)
}

/// `[\w]([-\w]*\w)?`
fn identifier(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    let Some(&first) = bytes.get(at) else {
        return Found::No;
    };
    if !(first.is_ascii_alphanumeric() || first == b'_') {
        return Found::No;
    }
    let mut end = at + 1;
    while bytes
        .get(end)
        .is_some_and(|&c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        end += 1;
    }
    // The name cannot end with a dash, so the last letter is kept and the rest given back.
    while end > at + 1 && !bytes[end - 1].is_ascii_alphanumeric() && bytes[end - 1] != b'_' {
        end -= 1;
    }
    Found::End(end)
}

/// A stage of the evolution axis.
#[derive(Debug, Clone)]
pub(crate) struct Stage {
    pub name: String,
    pub boundary: Option<f64>,
    pub second_name: Option<String>,
}

/// The size of the canvas.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Size {
    pub width: f64,
    pub height: f64,
}

/// The offset of the label of a node from the node.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LabelOffset {
    pub neg_x: bool,
    pub offset_x: f64,
    pub neg_y: bool,
    pub offset_y: f64,
}

/// An anchor, which is a node without a shape of its own.
#[derive(Debug, Clone)]
pub(crate) struct Anchor {
    pub name: String,
    pub visibility: f64,
    pub evolution: f64,
}

/// A component of the map.
#[derive(Debug, Clone)]
pub(crate) struct Component {
    pub name: String,
    pub visibility: f64,
    pub evolution: f64,
    pub label: Option<LabelOffset>,
    pub strategy: Option<String>,
    pub inertia: bool,
}

/// A component of a pipeline.
#[derive(Debug, Clone)]
pub(crate) struct PipelineComponent {
    pub name: String,
    pub evolution: f64,
    pub label: Option<LabelOffset>,
}

/// A link between two nodes.
#[derive(Debug, Clone)]
pub(crate) struct Link {
    pub from: String,
    pub from_port: Option<String>,
    pub arrow: Option<String>,
    pub to: String,
    pub to_port: Option<String>,
    pub link_label: Option<String>,
}

/// The evolution of a component, which is where it is heading.
#[derive(Debug, Clone)]
pub(crate) struct Evolve {
    pub component: String,
    pub target: f64,
}

/// A pipeline, which is a set of components under a parent.
#[derive(Debug, Clone)]
pub(crate) struct Pipeline {
    pub parent: String,
    pub components: Vec<PipelineComponent>,
}

/// A note on the map.
#[derive(Debug, Clone)]
pub(crate) struct Note {
    pub text: String,
    pub visibility: f64,
    pub evolution: f64,
}

/// The place of the list of the annotations.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Annotations {
    pub x: f64,
    pub y: f64,
}

/// An annotation of the map.
#[derive(Debug, Clone)]
pub(crate) struct Annotation {
    pub number: f64,
    pub x: f64,
    pub y: f64,
    pub text: String,
}

/// An accelerator or a deaccelerator.
#[derive(Debug, Clone)]
pub(crate) struct Force {
    pub name: String,
    pub x: f64,
    pub y: f64,
}

/// The diagram, as Mermaid's syntax tree holds it.
#[derive(Debug, Clone, Default)]
pub(crate) struct Ast {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub size: Option<Size>,
    pub stages: Option<Vec<Stage>>,
    pub anchors: Vec<Anchor>,
    pub components: Vec<Component>,
    pub links: Vec<Link>,
    pub evolves: Vec<Evolve>,
    pub pipelines: Vec<Pipeline>,
    pub notes: Vec<Note>,
    pub annotations: Vec<Annotations>,
    pub annotation: Vec<Annotation>,
    pub accelerators: Vec<Force>,
    pub deaccelerators: Vec<Force>,
}

/// Reads a Wardley map.
pub(crate) fn parse_wardley(src: &str) -> Result<Ast, Error> {
    let tokens = tokenize(src, TYPES)?;
    let mut r = Reader::new(tokens, TYPES);
    let mut ast = Ast::default();

    while r.kind() == NEWLINE_TOKEN {
        r.i += 1;
    }
    r.expect(KW_WARDLEY)?;

    while r.kind() != END {
        match r.kind() {
            NEWLINE_TOKEN => r.i += 1,
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
            KW_SIZE => {
                r.i += 1;
                let (width, height) = pair(&mut r, int_value)?;
                end_of_line(&mut r)?;
                ast.size = Some(Size { width, height });
            }
            KW_EVOLUTION => {
                r.i += 1;
                let mut stages = vec![stage(&mut r)?];
                r.expect(ARROW)?;
                stages.push(stage(&mut r)?);
                while r.accept(ARROW) {
                    stages.push(stage(&mut r)?);
                }
                end_of_line(&mut r)?;
                ast.stages = Some(stages);
            }
            KW_ANCHOR => {
                r.i += 1;
                let name = name(&mut r)?;
                let (visibility, evolution) = pair(&mut r, number_value)?;
                end_of_line(&mut r)?;
                ast.anchors.push(Anchor {
                    name,
                    visibility,
                    evolution,
                });
            }
            KW_COMPONENT => {
                r.i += 1;
                let name = name(&mut r)?;
                let (visibility, evolution) = pair(&mut r, number_value)?;
                let label = if r.kind() == KW_LABEL {
                    Some(label(&mut r)?)
                } else {
                    None
                };
                let mut strategy = None;
                let mut inertia = false;
                if r.accept(LPAREN) {
                    if r.kind() == STRATEGY {
                        strategy = Some(r.expect(STRATEGY)?.to_string());
                        r.expect(RPAREN)?;
                        // The inertia may follow the decorator.
                        if r.accept(LPAREN) {
                            r.expect(KW_INERTIA)?;
                            r.expect(RPAREN)?;
                            inertia = true;
                        } else if r.accept(KW_INERTIA) {
                            inertia = true;
                        }
                    } else {
                        r.expect(KW_INERTIA)?;
                        r.expect(RPAREN)?;
                        inertia = true;
                    }
                } else if r.accept(KW_INERTIA) {
                    inertia = true;
                }
                end_of_line(&mut r)?;
                ast.components.push(Component {
                    name,
                    visibility,
                    evolution,
                    label,
                    strategy,
                    inertia,
                });
            }
            KW_EVOLVE => {
                r.i += 1;
                let component = name(&mut r)?;
                let target = number_value(&mut r)?;
                end_of_line(&mut r)?;
                ast.evolves.push(Evolve { component, target });
            }
            KW_PIPELINE => {
                r.i += 1;
                let parent = name(&mut r)?;
                r.expect(LBRACE)?;
                r.expect(NEWLINE_TOKEN)?;
                while r.kind() == NEWLINE_TOKEN {
                    r.i += 1;
                }
                let mut components = Vec::new();
                loop {
                    r.expect(KW_COMPONENT)?;
                    let name = name(&mut r)?;
                    r.expect(LBRACKET)?;
                    let evolution = number_value(&mut r)?;
                    r.expect(RBRACKET)?;
                    let label = if r.kind() == KW_LABEL {
                        Some(label(&mut r)?)
                    } else {
                        None
                    };
                    end_of_line(&mut r)?;
                    components.push(PipelineComponent {
                        name,
                        evolution,
                        label,
                    });
                    if r.kind() != KW_COMPONENT {
                        break;
                    }
                }
                r.expect(RBRACE)?;
                end_of_line(&mut r)?;
                ast.pipelines.push(Pipeline { parent, components });
            }
            KW_NOTE => {
                r.i += 1;
                let text = string_value(r.expect(STRING_TOKEN)?);
                let (visibility, evolution) = pair(&mut r, number_value)?;
                end_of_line(&mut r)?;
                ast.notes.push(Note {
                    text,
                    visibility,
                    evolution,
                });
            }
            KW_ANNOTATIONS => {
                r.i += 1;
                let (x, y) = pair(&mut r, coordinate_value)?;
                end_of_line(&mut r)?;
                ast.annotations.push(Annotations { x, y });
            }
            KW_ANNOTATION => {
                r.i += 1;
                let number = int_value(&mut r)?;
                r.expect(COMMA)?;
                let (x, y) = pair(&mut r, coordinate_value)?;
                let text = string_value(r.expect(STRING_TOKEN)?);
                end_of_line(&mut r)?;
                ast.annotation.push(Annotation { number, x, y, text });
            }
            KW_ACCELERATOR | KW_DEACCELERATOR => {
                let slow = r.kind() == KW_DEACCELERATOR;
                r.i += 1;
                let name = name(&mut r)?;
                let (x, y) = pair(&mut r, number_value)?;
                end_of_line(&mut r)?;
                let force = Force { name, x, y };
                if slow {
                    ast.deaccelerators.push(force);
                } else {
                    ast.accelerators.push(force);
                }
            }
            STRING_TOKEN | ID | NAME_WITH_SPACES => {
                let from = name(&mut r)?;
                let from_port = if r.kind() == LINK_PORT {
                    Some(r.take().to_string())
                } else {
                    None
                };
                let arrow = if r.kind() == LINK_ARROW || r.kind() == ARROW {
                    Some(r.take().to_string())
                } else {
                    None
                };
                let to = name(&mut r)?;
                let to_port = if r.kind() == LINK_PORT {
                    Some(r.take().to_string())
                } else {
                    None
                };
                let link_label = if r.kind() == LINK_LABEL {
                    Some(r.take()[1..].trim().to_string())
                } else {
                    None
                };
                end_of_line(&mut r)?;
                ast.links.push(Link {
                    from,
                    from_port,
                    arrow,
                    to,
                    to_port,
                    link_label,
                });
            }
            _ => return Err(r.fail("a statement")),
        }
    }
    Ok(ast)
}

/// The end of a statement: a line break or the end of the text.
fn end_of_line(r: &mut Reader<'_>) -> Result<(), Error> {
    if r.kind() == END {
        return Ok(());
    }
    if r.kind() != NEWLINE_TOKEN {
        return Err(r.fail("a line break or the end of input"));
    }
    while r.kind() == NEWLINE_TOKEN {
        r.i += 1;
    }
    Ok(())
}

/// A name: a string, an identifier or a name of words.
fn name(r: &mut Reader<'_>) -> Result<String, Error> {
    match r.kind() {
        STRING_TOKEN => Ok(string_value(r.take())),
        ID | NAME_WITH_SPACES => Ok(r.take().to_string()),
        _ => Err(r.fail("a name")),
    }
}

/// A stage of the evolution axis.
fn stage(r: &mut Reader<'_>) -> Result<Stage, Error> {
    let stage_name = name(r)?;
    let boundary = if r.accept(AT) {
        Some(number_value(r)?)
    } else {
        None
    };
    let second_name = if r.accept(SLASH) {
        Some(name(r)?)
    } else {
        None
    };
    Ok(Stage {
        name: stage_name,
        boundary,
        second_name,
    })
}

/// The offset of the label of a node from the node.
fn label(r: &mut Reader<'_>) -> Result<LabelOffset, Error> {
    r.expect(KW_LABEL)?;
    r.expect(LBRACKET)?;
    let neg_x = r.accept(MINUS);
    let offset_x = int_value(r)?;
    r.expect(COMMA)?;
    let neg_y = r.accept(MINUS);
    let offset_y = int_value(r)?;
    r.expect(RBRACKET)?;
    Ok(LabelOffset {
        neg_x,
        offset_x,
        neg_y,
        offset_y,
    })
}

/// A pair of numbers in brackets.
fn pair(
    r: &mut Reader<'_>,
    read: fn(&mut Reader<'_>) -> Result<f64, Error>,
) -> Result<(f64, f64), Error> {
    r.expect(LBRACKET)?;
    let first = read(r)?;
    r.expect(COMMA)?;
    let second = read(r)?;
    r.expect(RBRACKET)?;
    Ok((first, second))
}

/// A number of the grammar, which is a decimal.
fn number_value(r: &mut Reader<'_>) -> Result<f64, Error> {
    Ok(r.expect(WARDLEY_NUMBER)?.parse().unwrap_or(f64::NAN))
}

/// A whole number.
fn int_value(r: &mut Reader<'_>) -> Result<f64, Error> {
    Ok(r.expect(INT)?.parse().unwrap_or(f64::NAN))
}

/// A number that may be a decimal or a whole number.
fn coordinate_value(r: &mut Reader<'_>) -> Result<f64, Error> {
    if r.kind() == WARDLEY_NUMBER {
        number_value(r)
    } else {
        int_value(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The end of a match of the whole number, or nothing when there is none.
    fn whole(src: &str) -> Option<usize> {
        match int(src, 0) {
            Found::End(end) => Some(end),
            _ => None,
        }
    }

    fn text(src: &str) -> String {
        let tokens = tokenize(src, TYPES).expect("the text is read");
        tokens
            .kinds
            .iter()
            .take_while(|&&k| k != END)
            .map(|&k| TYPES[k].name)
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn reads_the_words_of_the_grammar_before_a_name() {
        assert_eq!(text("size"), "KW_SIZE");
        assert_eq!(text("sizes"), "KW_SIZE NAME_WITH_SPACES");
        assert_eq!(text("inerti"), "NAME_WITH_SPACES");
        assert_eq!(text("build"), "STRATEGY");
    }

    #[test]
    fn reads_an_arrow_before_the_dash_of_a_name() {
        assert_eq!(text("->"), "ARROW");
        assert_eq!(text("-->"), "LINK_ARROW");
        assert_eq!(text("-.->"), "LINK_ARROW");
        assert_eq!(text("-50"), "- INT");
    }

    #[test]
    fn reads_a_title_as_a_title_and_not_as_a_name() {
        assert_eq!(text("title a b"), "TITLE");
        assert_eq!(text("title"), "NAME_WITH_SPACES");
        assert_eq!(text("titleX"), "NAME_WITH_SPACES");
    }

    #[test]
    fn reads_the_names_of_the_grammar() {
        assert_eq!(text("Cup of Tea"), "NAME_WITH_SPACES");
        assert_eq!(text("byte pair encoding (BPE)"), "NAME_WITH_SPACES");
        assert_eq!(text("real-time processing"), "NAME_WITH_SPACES");
        assert_eq!(text("end-user"), "NAME_WITH_SPACES");
        assert_eq!(text("_x"), "ID");
    }

    #[test]
    fn reads_the_numbers_of_the_grammar() {
        assert_eq!(text("0.5"), "WARDLEY_NUMBER");
        assert_eq!(text("10"), "INT");
        // The lookahead of the whole number gives the digits back when a dot follows them.
        assert_eq!(whole("10."), Some(1));
        assert_eq!(whole("1."), None);
        assert_eq!(whole("0."), Some(1));
    }
}
