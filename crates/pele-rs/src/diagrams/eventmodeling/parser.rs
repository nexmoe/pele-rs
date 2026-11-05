//! The tokens and the parser of an event modeling diagram, over the grammar Mermaid gives with
//! Langium. Ported from Mermaid's `packages/parser/src/language/eventmodeling/
//! event-modeling.langium` at commit `21f72f07`, the grammar that pele's
//! `diagrams/eventmodeling/parser.ts` follows as well.
//!
//! Two things about that grammar are worth knowing, because they decide what is read:
//!
//! - Its terminals are tried in the order they are written, and the keywords come first. A
//!   keyword therefore wins over `EM_ID`, which is a name, unless the name is longer: `cmd` is
//!   the keyword, `cmdx` is a name.
//! - The terminals for `title`, `accTitle` and `accDescr` are written after `EM_ID`, so `EM_ID`
//!   always wins: Mermaid cannot read a title in an event modeling diagram, and pele-rs
//!   refuses such a line as well.

use crate::error::Error;

use super::lexer::{
    END, Found, Pattern, Reader, Scanner, TokenType, block_end, directive, hidden_token,
    literal_token, match_token, single_line_comment, tokenize, whitespace, word_token, yaml,
};
use crate::util::is_space;

// The kinds of the tokens, in the order they are tried, as in the grammar.
const EVENTMODELING: usize = 0;
const TF: usize = 1;
const TIMEFRAME: usize = 2;
const RF: usize = 3;
const RESETFRAME: usize = 4;
const ENTITY: usize = 5;
const DATA: usize = 6;
const NOTE: usize = 7;
const GWT: usize = 8;
const GIVEN: usize = 9;
const WHEN: usize = 10;
const THEN: usize = 11;
const RMO: usize = 12;
const READMODEL: usize = 13;
const UI: usize = 14;
const CMD: usize = 15;
const COMMAND: usize = 16;
const EVT: usize = 17;
const EVENT: usize = 18;
const PCR: usize = 19;
const PROCESSOR: usize = 20;
const JSON: usize = 21;
const JSOBJ: usize = 22;
const FIGMA: usize = 23;
const SALT: usize = 24;
const URI: usize = 25;
const MD: usize = 26;
const HTML: usize = 27;
const TEXT: usize = 28;
const OPEN: usize = 29;
const CLOSE: usize = 30;
const ARROW: usize = 31;
const DOT: usize = 32;
const TICK: usize = 33;
const ID: usize = 34;
const FID: usize = 35;
const DATA_INLINE: usize = 36;
const DATA_BLOCK: usize = 37;
const ACC_DESCR: usize = 38;
const ACC_TITLE: usize = 39;
const TITLE: usize = 40;

/// The kinds that may match a longer text than a keyword does, as Chevrotain's `LONGER_ALT`.
/// A name is longer than a keyword only when it has more characters.
const NAME: &[usize] = &[ID];
/// `event` is the beginning of `eventmodeling`, so that keyword is one of its alternatives.
const EVENT_NAME: &[usize] = &[EVENTMODELING, ID];

static TOKENS: [TokenType; 47] = [
    TokenType {
        name: "eventmodeling",
        pattern: Pattern::Match(diagram_keyword),
        hidden: false,
        longer: NAME,
    },
    word_token("tf", NAME),
    word_token("timeframe", NAME),
    word_token("rf", NAME),
    word_token("resetframe", NAME),
    word_token("entity", NAME),
    word_token("data", NAME),
    word_token("note", NAME),
    word_token("gwt", NAME),
    word_token("given", NAME),
    word_token("when", NAME),
    word_token("then", NAME),
    word_token("rmo", NAME),
    word_token("readmodel", NAME),
    word_token("ui", NAME),
    word_token("cmd", NAME),
    word_token("command", NAME),
    word_token("evt", NAME),
    word_token("event", EVENT_NAME),
    word_token("pcr", NAME),
    word_token("processor", NAME),
    word_token("json", NAME),
    word_token("jsobj", NAME),
    word_token("figma", NAME),
    word_token("salt", NAME),
    word_token("uri", NAME),
    word_token("md", NAME),
    word_token("html", NAME),
    word_token("text", NAME),
    literal_token("[["),
    literal_token("]]"),
    literal_token("->>"),
    literal_token("."),
    literal_token("`"),
    match_token("EM_ID", ident),
    match_token("EM_FID", frame_id),
    match_token("EM_DATA_INLINE", inline_data),
    TokenType {
        name: "EM_DATA_BLOCK",
        pattern: Pattern::Scanner(|| Box::new(DataBlock::default())),
        hidden: false,
        longer: &[],
    },
    match_token("EM_ACC_DESCR", acc_descr),
    match_token("EM_ACC_TITLE", acc_title),
    match_token("EM_TITLE", title),
    TokenType {
        name: "EM_WS",
        pattern: Pattern::Match(whitespace),
        hidden: true,
        longer: &[],
    },
    hidden_token("EM_YAML", Pattern::Match(yaml)),
    hidden_token("EM_DIRECTIVE", Pattern::Match(directive)),
    hidden_token(
        "EM_SINGLE_LINE_COMMENT",
        Pattern::Match(single_line_comment),
    ),
    TokenType {
        name: "EM_ML_COMMENT",
        pattern: Pattern::Scanner(|| Box::new(Comment::default())),
        hidden: true,
        longer: &[],
    },
    TokenType {
        name: "EM_SL_COMMENT",
        pattern: Pattern::Match(sl_comment),
        hidden: true,
        longer: &[],
    },
];

/// The keyword of the diagram. Mermaid's token builder reads it only when a comment or the
/// end of a word follows it, so `eventmodeling` at the end of a line is the keyword and
/// `eventmodeling.foo` is a name.
fn diagram_keyword(src: &str, at: usize) -> Found {
    const KEYWORD: &str = "eventmodeling";
    if !src[at..].starts_with(KEYWORD) {
        return Found::No;
    }
    let end = at + KEYWORD.len();
    let rest = &src[end..];
    if rest.starts_with("%%") {
        return Found::End(end);
    }
    match rest.chars().next() {
        None => Found::End(end),
        Some(c) if is_space(c) => Found::End(end),
        Some(_) => Found::No,
    }
}

/// `[_a-zA-Z][\w_]*`
fn ident(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    let Some(&first) = bytes.get(at) else {
        return Found::No;
    };
    if !first.is_ascii_alphabetic() && first != b'_' {
        return Found::No;
    }
    let mut end = at + 1;
    while let Some(&b) = bytes.get(end) {
        if b.is_ascii_alphanumeric() || b == b'_' {
            end += 1;
        } else {
            break;
        }
    }
    Found::End(end)
}

/// `\d{1,3}`
fn frame_id(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    let mut end = at;
    while end < bytes.len() && end - at < 3 && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end > at { Found::End(end) } else { Found::No }
}

/// `\{(.*)\}|"(.*)"|'(.*)'`, where `.` is anything but a line terminator. The match is greedy,
/// so it ends at the last closer of the line.
fn inline_data(src: &str, at: usize) -> Found {
    let close = match src.as_bytes().get(at) {
        Some(b'{') => b'}',
        Some(b'"') => b'"',
        Some(b'\'') => b'\'',
        _ => return Found::No,
    };
    let line_end = src[at..].find(['\n', '\r']).map_or(src.len(), |i| at + i);
    let last = src.as_bytes()[at..line_end]
        .iter()
        .rposition(|&b| b == close)
        .map(|i| at + i + 1);
    match last {
        Some(end) if end > at + 1 => Found::End(end),
        _ => Found::No,
    }
}

/// `//[^\n\r]*`
fn sl_comment(src: &str, at: usize) -> Found {
    if !src[at..].starts_with("//") {
        return Found::No;
    }
    let end = src[at..].find(['\n', '\r']).map_or(src.len(), |i| at + i);
    Found::End(end)
}

/// The closing braces of the data blocks of a text, and the end of each block.
#[derive(Default)]
struct DataBlock {
    /// The braces that close a block, with the end of the block each one ends.
    closers: Vec<(usize, usize)>,
    scanned: bool,
}

impl DataBlock {
    fn scan_text(&mut self, src: &str) {
        if self.scanned {
            return;
        }
        self.scanned = true;
        let bytes = src.as_bytes();
        for i in 0..bytes.len() {
            // The closing brace of a block starts a line.
            if bytes[i] != b'}' || i == 0 || bytes[i - 1] != b'\n' {
                continue;
            }
            if let Some(end) = block_end(src, i + 1) {
                self.closers.push((i, end));
            }
        }
    }
}

impl Scanner for DataBlock {
    fn scan(&mut self, src: &str, at: usize) -> Option<usize> {
        self.scan_text(src);
        // `\{[\t ]*\r?\n`: the brace, spaces and tabs, and a line break.
        if src.as_bytes().get(at) != Some(&b'{') {
            return None;
        }
        let mut body = at + 1;
        while matches!(src.as_bytes().get(body), Some(b' ' | b'\t')) {
            body += 1;
        }
        let body = match src.as_bytes().get(body) {
            Some(b'\r') if src.as_bytes().get(body + 1) == Some(&b'\n') => body + 2,
            Some(b'\n') => body + 1,
            _ => return None,
        };
        // The first brace that starts a line and ends the block.
        let at = self.closers.partition_point(|(pos, _)| *pos < body);
        self.closers.get(at).map(|&(_, end)| end)
    }
}

/// The ends of the block comments of a text.
#[derive(Default)]
struct Comment {
    ends: Vec<usize>,
    scanned: bool,
}

impl Comment {
    fn scan_text(&mut self, src: &str) {
        if self.scanned {
            return;
        }
        self.scanned = true;
        let bytes = src.as_bytes();
        let mut i = 0;
        while i + 1 < bytes.len() {
            if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                self.ends.push(i + 2);
                i += 2;
            } else {
                i += 1;
            }
        }
    }
}

impl Scanner for Comment {
    fn scan(&mut self, src: &str, at: usize) -> Option<usize> {
        self.scan_text(src);
        // `/\*[\S\s]*?\*/`: the first end after the opening.
        let at = self.ends.partition_point(|&end| end <= at + 2);
        self.ends.get(at).copied()
    }
}

/// The kind of a frame: a time frame or a reset frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameKind {
    Time,
    Reset,
}

/// The kind of an entity, as the grammar names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntityKind {
    Ui,
    Processor,
    Command,
    ReadModel,
    Event,
}

impl EntityKind {
    /// The name the model and the summary use.
    pub(crate) fn name(self) -> &'static str {
        match self {
            EntityKind::Ui => "ui",
            EntityKind::Processor => "processor",
            EntityKind::Command => "command",
            EntityKind::ReadModel => "readmodel",
            EntityKind::Event => "event",
        }
    }
}

/// A frame of the timeline.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Frame {
    /// The number of the frame, as it is written, such as `01`.
    pub name: String,
    pub kind: FrameKind,
    pub entity_kind: EntityKind,
    /// The entity the frame is about, with its namespace.
    pub entity: String,
    /// The frames this one is connected to, as they are written.
    pub sources: Vec<String>,
    /// The data block the frame refers to.
    pub data_reference: Option<String>,
    /// The type of the data the frame carries.
    pub data_type: Option<String>,
    /// The data the frame carries, as it is written.
    pub data_inline: Option<String>,
    /// Where the frame starts in the source, for the error of a repeated number.
    pub offset: usize,
}

/// A data block, defined on its own.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DataEntity {
    pub name: String,
    pub data_type: Option<String>,
    pub data: String,
}

/// A note, written under the frame it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NoteEntity {
    pub frame: String,
    pub data_type: Option<String>,
    pub data: String,
}

/// A given/when/then specification, written under the frame it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Specification {
    pub frame: String,
    pub given: Vec<String>,
    pub when: Vec<String>,
    pub then: Vec<String>,
}

/// What the grammar reads out of the text.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct EventModelAst {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub model_entities: Vec<String>,
    pub frames: Vec<Frame>,
    pub data_entities: Vec<DataEntity>,
    pub notes: Vec<NoteEntity>,
    pub specifications: Vec<Specification>,
}

/// Reads an event modeling diagram out of the text.
pub(crate) fn parse_event_modeling(src: &str) -> Result<EventModelAst, Error> {
    let tokens = tokenize(src, &TOKENS)?;
    // The offsets of the tokens, to tell where a frame starts.
    let starts = tokens.starts.clone();
    let mut r = Reader::new(tokens, &TOKENS);
    let mut ast = EventModelAst::default();
    r.expect(EVENTMODELING)?;
    while r.kind() != END {
        match r.kind() {
            ACC_DESCR => {
                ast.acc_descr = Some(r.take().to_string());
            }
            ACC_TITLE => {
                ast.acc_title = Some(r.take().to_string());
            }
            TITLE => {
                ast.title = Some(r.take().to_string());
            }
            ENTITY => {
                r.i += 1;
                ast.model_entities.push(qualified_name(&mut r)?);
            }
            TF | TIMEFRAME | RF | RESETFRAME => {
                let offset = starts[r.i];
                ast.frames.push(frame(&mut r, offset)?);
            }
            DATA => {
                r.i += 1;
                let name = r.expect(ID)?.to_string();
                let (data_type, data) = data_block(&mut r)?;
                ast.data_entities.push(DataEntity {
                    name,
                    data_type,
                    data,
                });
            }
            NOTE => {
                r.i += 1;
                let frame = r.expect(FID)?.to_string();
                let (data_type, data) = data_block(&mut r)?;
                ast.notes.push(NoteEntity {
                    frame,
                    data_type,
                    data,
                });
            }
            GWT => {
                r.i += 1;
                let frame = r.expect(FID)?.to_string();
                r.expect(GIVEN)?;
                let given = statements(&mut r)?;
                let when = if r.accept(WHEN) {
                    statements(&mut r)?
                } else {
                    Vec::new()
                };
                r.expect(THEN)?;
                let then = statements(&mut r)?;
                ast.specifications.push(Specification {
                    frame,
                    given,
                    when,
                    then,
                });
            }
            _ => return Err(r.fail("a statement of the diagram")),
        }
    }
    Ok(ast)
}

/// A frame: its number, its entity, the frames it is connected to, and the data it carries.
fn frame(r: &mut Reader<'_>, offset: usize) -> Result<Frame, Error> {
    let kind = match r.kind() {
        TF | TIMEFRAME => FrameKind::Time,
        RF | RESETFRAME => FrameKind::Reset,
        _ => return Err(r.fail("a time frame")),
    };
    r.i += 1;
    let name = r.expect(FID)?.to_string();
    let entity_kind = entity_kind(r)?;
    let entity = qualified_name(r)?;
    let mut sources = Vec::new();
    while r.accept(ARROW) {
        sources.push(r.expect(FID)?.to_string());
    }
    let data_reference = if r.accept(OPEN) {
        let name = r.expect(ID)?.to_string();
        r.expect(CLOSE)?;
        Some(name)
    } else {
        None
    };
    let (data_type, data_inline) = data_inline(r)?;
    Ok(Frame {
        name,
        kind,
        entity_kind,
        entity,
        sources,
        data_reference,
        data_type,
        data_inline,
        offset,
    })
}

/// A name that may have a namespace: `Inventory.AddItem`.
fn qualified_name(r: &mut Reader<'_>) -> Result<String, Error> {
    let mut parts = vec![r.expect(ID)?.to_string()];
    while r.accept(DOT) {
        parts.push(r.expect(ID)?.to_string());
    }
    Ok(parts.join("."))
}

/// The kind of entity a frame is about.
fn entity_kind(r: &mut Reader<'_>) -> Result<EntityKind, Error> {
    let kind = match r.kind() {
        UI => EntityKind::Ui,
        PCR | PROCESSOR => EntityKind::Processor,
        CMD | COMMAND => EntityKind::Command,
        RMO | READMODEL => EntityKind::ReadModel,
        EVT | EVENT => EntityKind::Event,
        _ => return Err(r.fail("an entity type")),
    };
    r.i += 1;
    Ok(kind)
}

/// The type of a data block, between backticks.
fn data_type(r: &mut Reader<'_>) -> Result<String, Error> {
    let name = match r.kind() {
        JSON => "json",
        JSOBJ => "jsobj",
        FIGMA => "figma",
        SALT => "salt",
        URI => "uri",
        MD => "md",
        HTML => "html",
        TEXT => "text",
        _ => return Err(r.fail("a data type")),
    };
    r.i += 1;
    Ok(name.to_string())
}

/// The data a frame carries on its own line.
fn data_inline(r: &mut Reader<'_>) -> Result<(Option<String>, Option<String>), Error> {
    if r.kind() == TICK {
        r.i += 1;
        let kind = data_type(r)?;
        r.expect(TICK)?;
        let value = r.expect(DATA_INLINE)?.to_string();
        return Ok((Some(kind), Some(value)));
    }
    if r.kind() == DATA_INLINE {
        return Ok((None, Some(r.take().to_string())));
    }
    Ok((None, None))
}

/// The data block a statement defines.
fn data_block(r: &mut Reader<'_>) -> Result<(Option<String>, String), Error> {
    if r.kind() == TICK {
        r.i += 1;
        let kind = data_type(r)?;
        r.expect(TICK)?;
        let value = r.expect(DATA_BLOCK)?.to_string();
        return Ok((Some(kind), value));
    }
    let value = r.expect(DATA_BLOCK)?.to_string();
    Ok((None, value))
}

/// The statements of a given/when/then specification: an entity type and an entity.
fn statements(r: &mut Reader<'_>) -> Result<Vec<String>, Error> {
    let mut out = Vec::new();
    while matches!(
        r.kind(),
        RMO | READMODEL | UI | CMD | COMMAND | EVT | EVENT | PCR | PROCESSOR
    ) {
        entity_kind(r)?;
        out.push(r.expect(ID)?.to_string());
    }
    if out.is_empty() {
        return Err(r.fail("a statement of the specification"));
    }
    Ok(out)
}

fn common_match(token: crate::diagrams::common::tokens::TokenType, src: &str, at: usize) -> Found {
    use crate::diagrams::common::tokens::{Found as F, Pattern as P};
    let P::Match(f) = token.pattern else {
        unreachable!()
    };
    match f(src, at) {
        F::End(e) => Found::End(e),
        F::No => Found::No,
        F::Never => Found::Never,
    }
}
fn acc_descr(src: &str, at: usize) -> Found {
    common_match(crate::diagrams::common::tokens::ACC_DESCR, src, at)
}
fn acc_title(src: &str, at: usize) -> Found {
    common_match(crate::diagrams::common::tokens::ACC_TITLE, src, at)
}
fn title(src: &str, at: usize) -> Found {
    common_match(crate::diagrams::common::tokens::TITLE_WORD, src, at)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Result<EventModelAst, Error> {
        parse_event_modeling(src)
    }

    #[test]
    fn reads_the_frames() {
        let ast = parse("eventmodeling\ntf 01 ui CartUI\ntf 02 cmd AddItem\n").unwrap();
        assert_eq!(ast.frames.len(), 2);
        assert_eq!(ast.frames[0].name, "01");
        assert_eq!(ast.frames[0].entity_kind, EntityKind::Ui);
        assert_eq!(ast.frames[0].entity, "CartUI");
        assert_eq!(ast.frames[1].entity_kind, EntityKind::Command);
        assert_eq!(ast.frames[1].kind, FrameKind::Time);
    }

    #[test]
    fn reads_the_relaxed_notation() {
        let ast = parse("eventmodeling\ntimeframe 01 ui CartUI\nresetframe 02 evt Item\n").unwrap();
        assert_eq!(ast.frames[0].entity_kind, EntityKind::Ui);
        assert_eq!(ast.frames[1].kind, FrameKind::Reset);
    }

    #[test]
    fn reads_a_namespace() {
        let ast = parse("eventmodeling\ntf 01 cmd Inventory.AddItem\n").unwrap();
        assert_eq!(ast.frames[0].entity, "Inventory.AddItem");
    }

    #[test]
    fn reads_the_sources_and_the_data() {
        let ast = parse(
            "eventmodeling\ndata D {\n a: 1\n}\ntf 01 rmo C ->> 02 [[D]] `json`{ \"a\": 1 }\n",
        )
        .unwrap();
        let frame = &ast.frames[0];
        assert_eq!(frame.sources, vec!["02".to_string()]);
        assert_eq!(frame.data_reference.as_deref(), Some("D"));
        assert_eq!(frame.data_type.as_deref(), Some("json"));
        assert_eq!(frame.data_inline.as_deref(), Some("{ \"a\": 1 }"));
        assert_eq!(ast.data_entities[0].data, "{\n a: 1\n}\n");
    }

    #[test]
    fn reads_the_notes_and_the_specifications() {
        let ast = parse(
            "eventmodeling\nentity Cart\ntf 01 evt A\nnote 01 {\n a note\n}\ngwt 01 given cmd Cart when evt Cart then evt Cart\n",
        )
        .unwrap();
        assert_eq!(ast.notes[0].frame, "01");
        assert_eq!(ast.notes[0].data, "{\n a note\n}\n");
        assert_eq!(ast.specifications[0].given, vec!["Cart".to_string()]);
        assert_eq!(ast.specifications[0].when, vec!["Cart".to_string()]);
        assert_eq!(ast.specifications[0].then, vec!["Cart".to_string()]);
        assert_eq!(ast.model_entities, vec!["Cart".to_string()]);
    }

    #[test]
    fn refuses_what_meraid_cannot_read() {
        // A title: Mermaid's grammar has a terminal for it, but the name token comes first, so
        // the line is a name and the diagram does not parse.
        assert!(parse("eventmodeling\ntitle T\ntf 01 evt A\n").is_err());
        // The same for the accessible text.
        assert!(parse("eventmodeling\naccTitle: T\ntf 01 evt A\n").is_err());
        // A number that is not one to three digits.
        assert!(parse("eventmodeling\ntf 0001 evt A\n").is_err());
        // A name that starts with a digit.
        assert!(parse("eventmodeling\ntf 01 evt 1A\n").is_err());
        // A data block on one line is inline data, which a data entity cannot carry.
        assert!(parse("eventmodeling\ndata D { a: 1 }\n").is_err());
        // A statement that is not one of the grammar's.
        assert!(parse("eventmodeling\nnonsense\n").is_err());
    }

    #[test]
    fn reads_the_comments() {
        let ast =
            parse("eventmodeling\n%% a comment\n// another\n/* a third */\ntf 01 evt A\n").unwrap();
        assert_eq!(ast.frames.len(), 1);
    }
}
