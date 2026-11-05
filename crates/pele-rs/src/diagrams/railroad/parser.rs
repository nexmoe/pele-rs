//! The four grammars of a railroad diagram: Mermaid's own notation, EBNF, ABNF and PEG.
//!
//! Mermaid reads each of them with a Langium grammar and then turns the tree it gets into
//! the model of [`crate::diagrams::railroad::db`]. This module reads the same text with a
//! hand-written parser for each grammar, and builds the model directly.
//!
//! The token types are in the order Mermaid's lexer has them: the keywords first, then the
//! terminals in the order the grammar declares them, then the hidden ones. The first token
//! type that matches wins, as in Chevrotain, except that a keyword that a longer terminal
//! also matches is the terminal, as in Chevrotain's `LONGER_ALT`.

use crate::diagrams::common::tokens::{
    ACC_DESCR, ACC_TITLE, DIRECTIVE, END, Found, Pattern, Reader, SINGLE_LINE_COMMENT, TITLE,
    TokenType, YAML, acc_descr_value, acc_title_value, title_value, tokenize,
};
use crate::error::{Error, ErrorCode};
use crate::util::is_space;

use super::db::{Node, Notation, RailroadModel, Rule};

/// The token types of `railroad-beta`, Mermaid's own notation.
const IR_KEYWORD: usize = 0;
const IR_NONTERMINAL: usize = 1;
const IR_ZERO_MORE: usize = 2;
const IR_ONE_MORE: usize = 3;
const IR_SEQUENCE: usize = 4;
const IR_OPTIONAL: usize = 5;
const IR_TERMINAL: usize = 6;
const IR_SPECIAL: usize = 7;
const IR_CHOICE: usize = 8;
const IR_EQUALS: usize = 9;
const IR_SEMICOLON: usize = 10;
const IR_OPEN: usize = 11;
const IR_COMMA: usize = 12;
const IR_CLOSE: usize = 13;
const IR_TITLE: usize = 14;
const IR_ACC_TITLE: usize = 15;
const IR_ACC_DESCR: usize = 16;
const IR_ID: usize = 17;
const IR_STRING: usize = 18;

/// The token types of `railroad-ebnf-beta`.
const EBNF_KEYWORD: usize = 0;
const EBNF_DEFINES: usize = 1;
const EBNF_EQUALS: usize = 2;
const EBNF_SEMICOLON: usize = 3;
const EBNF_BAR: usize = 4;
const EBNF_COMMA: usize = 5;
const EBNF_OPEN: usize = 6;
const EBNF_CLOSE: usize = 7;
const EBNF_OPTION: usize = 8;
const EBNF_OPTION_END: usize = 9;
const EBNF_REPEAT: usize = 10;
const EBNF_REPEAT_END: usize = 11;
const EBNF_QUESTION: usize = 12;
const EBNF_STAR: usize = 13;
const EBNF_PLUS: usize = 14;
const EBNF_MINUS: usize = 15;
const EBNF_TITLE: usize = 16;
const EBNF_ACC_TITLE: usize = 17;
const EBNF_ACC_DESCR: usize = 18;
const EBNF_ID: usize = 19;
const EBNF_STRING: usize = 20;
const EBNF_SPECIAL: usize = 21;
// const EBNF_WHITESPACE: usize = 22;
const EBNF_YAML: usize = 23;
// const EBNF_DIRECTIVE: usize = 24;
// const EBNF_SINGLE_LINE_COMMENT: usize = 25;
// const EBNF_BLOCK_COMMENT: usize = 26;
const EBNF_ISO_COMMENT: usize = 27;

/// The token types of `railroad-abnf-beta`.
const ABNF_KEYWORD: usize = 0;
const ABNF_EQUALS: usize = 1;
const ABNF_SEMICOLON: usize = 2;
const ABNF_SLASH: usize = 3;
const ABNF_OPEN: usize = 4;
const ABNF_CLOSE: usize = 5;
const ABNF_OPTION: usize = 6;
const ABNF_OPTION_END: usize = 7;
const ABNF_TITLE: usize = 8;
const ABNF_ACC_TITLE: usize = 9;
const ABNF_ACC_DESCR: usize = 10;
const ABNF_RULENAME: usize = 11;
const ABNF_STRING: usize = 12;
const ABNF_NUMVAL: usize = 13;
const ABNF_REPEAT: usize = 14;
const ABNF_EXACT_REPEAT: usize = 15;
// const ABNF_WHITESPACE: usize = 16;
// const ABNF_YAML: usize = 17;
// const ABNF_DIRECTIVE: usize = 18;
// const ABNF_SINGLE_LINE_COMMENT: usize = 19;
const ABNF_COMMENT: usize = 20;

/// The token types of `railroad-peg-beta`.
const PEG_KEYWORD: usize = 0;
const PEG_ARROW: usize = 1;
const PEG_SEMICOLON: usize = 2;
const PEG_SLASH: usize = 3;
const PEG_AND: usize = 4;
const PEG_NOT: usize = 5;
const PEG_QUESTION: usize = 6;
const PEG_STAR: usize = 7;
const PEG_PLUS: usize = 8;
const PEG_OPEN: usize = 9;
const PEG_CLOSE: usize = 10;
const PEG_DOT: usize = 11;
const PEG_TITLE: usize = 12;
const PEG_ACC_TITLE: usize = 13;
const PEG_ACC_DESCR: usize = 14;
const PEG_ID: usize = 15;
const PEG_STRING: usize = 16;

const fn tok(name: &'static str, pattern: Pattern) -> TokenType {
    TokenType {
        name,
        pattern,
        hidden: false,
        longer: &[],
    }
}

const fn tok_longer(name: &'static str, pattern: Pattern, longer: &'static [usize]) -> TokenType {
    TokenType {
        name,
        pattern,
        hidden: false,
        longer,
    }
}

const fn hidden(name: &'static str, pattern: Pattern) -> TokenType {
    TokenType {
        name,
        pattern,
        hidden: true,
        longer: &[],
    }
}

/// `[\t \r\n]+`: the whitespace of the railroad grammars, which spans lines.
fn railroad_whitespace(src: &str, at: usize) -> Found {
    let mut end = at;
    while matches!(src.as_bytes().get(end), Some(b'\t' | b' ' | b'\r' | b'\n')) {
        end += 1;
    }
    if end > at { Found::End(end) } else { Found::No }
}

/// `/\/\*[\s\S]*?\*\//`: a block comment, up to its first `*/`.
fn block_comment(src: &str, at: usize) -> Found {
    if !src[at..].starts_with("/*") {
        return Found::No;
    }
    let mut i = at + 2;
    while i + 1 < src.len() {
        if src.as_bytes()[i] == b'*' && src.as_bytes()[i + 1] == b'/' {
            return Found::End(i + 2);
        }
        i += 1;
    }
    Found::Never
}

/// `/\(\*[\s\S]*?\*\)/`: an ISO comment, up to its first `*)`.
fn iso_comment(src: &str, at: usize) -> Found {
    if !src[at..].starts_with("(*") {
        return Found::No;
    }
    let mut i = at + 2;
    while i + 1 < src.len() {
        if src.as_bytes()[i] == b'*' && src.as_bytes()[i + 1] == b')' {
            return Found::End(i + 2);
        }
        i += 1;
    }
    Found::Never
}

/// `;[^\n\r]*`: a comment to the end of the line.
fn abnf_comment(src: &str, at: usize) -> Found {
    if src.as_bytes().get(at) != Some(&b';') {
        return Found::No;
    }
    let end = src[at..].find(['\n', '\r']).map_or(src.len(), |i| at + i);
    Found::End(end)
}

/// `#[^\n\r]*`: a comment to the end of the line.
fn peg_line_comment(src: &str, at: usize) -> Found {
    if src.as_bytes().get(at) != Some(&b'#') {
        return Found::No;
    }
    let end = src[at..].find(['\n', '\r']).map_or(src.len(), |i| at + i);
    Found::End(end)
}

/// `[A-Z_a-z][\w-]*`: a name.
fn identifier(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    let Some(&first) = bytes.get(at) else {
        return Found::No;
    };
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return Found::No;
    }
    let mut end = at + 1;
    while matches!(bytes.get(end), Some(b) if b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
    {
        end += 1;
    }
    Found::End(end)
}

/// `[A-Za-z][A-Za-z0-9-]*`: the name of a rule in ABNF.
fn rule_name(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    let Some(&first) = bytes.get(at) else {
        return Found::No;
    };
    if !first.is_ascii_alphabetic() {
        return Found::No;
    }
    let mut end = at + 1;
    while matches!(bytes.get(end), Some(b) if b.is_ascii_alphanumeric() || *b == b'-') {
        end += 1;
    }
    Found::End(end)
}

/// `"([^"\\]|\\.)*"|'([^'\\]|\\.)*'`: a quoted string.
fn quoted(src: &str, at: usize) -> Found {
    let Some(quote) = src[at..].chars().next().filter(|&q| q == '"' || q == '\'') else {
        return Found::No;
    };
    let mut chars = src[at + quote.len_utf8()..].char_indices();
    while let Some((i, c)) = chars.next() {
        if c == quote {
            return Found::End(at + quote.len_utf8() + i + c.len_utf8());
        }
        if c == '\\' {
            match chars.next() {
                Some((_, next)) if !matches!(next, '\n' | '\r') => {}
                _ => return Found::No,
            }
        }
    }
    Found::No
}

/// `"[^"]*": a string in ABNF, which has no escapes.
fn abnf_string(src: &str, at: usize) -> Found {
    if src.as_bytes().get(at) != Some(&b'"') {
        return Found::No;
    }
    match src[at + 1..].find('"') {
        Some(i) => Found::End(at + 1 + i + 1),
        None => Found::No,
    }
}

/// `%[xXdDbB][0-9A-Fa-f]+(?:-[0-9A-Fa-f]+|\.[0-9A-Fa-f]+)*`: a number of ABNF.
fn abnf_numval(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    if bytes.get(at) != Some(&b'%') {
        return Found::No;
    }
    let mut end = at + 1;
    match bytes.get(end) {
        Some(b'x' | b'X' | b'd' | b'D' | b'b' | b'B') => end += 1,
        _ => return Found::No,
    }
    let digits = end;
    while matches!(bytes.get(end), Some(b) if b.is_ascii_hexdigit()) {
        end += 1;
    }
    if end == digits {
        return Found::No;
    }
    while matches!(bytes.get(end), Some(b'-' | b'.')) {
        let mut next = end + 1;
        while matches!(bytes.get(next), Some(b) if b.is_ascii_hexdigit()) {
            next += 1;
        }
        if next == end + 1 {
            break;
        }
        end = next;
    }
    Found::End(end)
}

/// `[0-9]*\*[0-9]*`: a repetition in ABNF.
fn abnf_repeat(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    let mut end = at;
    while matches!(bytes.get(end), Some(b) if b.is_ascii_digit()) {
        end += 1;
    }
    if bytes.get(end) != Some(&b'*') {
        return Found::No;
    }
    end += 1;
    while matches!(bytes.get(end), Some(b) if b.is_ascii_digit()) {
        end += 1;
    }
    Found::End(end)
}

/// `[0-9]+`: an exact repetition in ABNF.
fn abnf_exact_repeat(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    let mut end = at;
    while matches!(bytes.get(end), Some(b) if b.is_ascii_digit()) {
        end += 1;
    }
    if end == at {
        Found::No
    } else {
        Found::End(end)
    }
}

/// `\?(?=[^?;]*[^?\s;][^?;]*\?)[^?;]*\?`: a special sequence in EBNF.
fn ebnf_special(src: &str, at: usize) -> Found {
    let mut solid = false;
    for (i, c) in src[at..].char_indices() {
        if i == 0 {
            if c != '?' {
                return Found::No;
            }
            continue;
        }
        if c == '?' {
            return if solid {
                Found::End(at + i + c.len_utf8())
            } else {
                Found::No
            };
        }
        if c == ';' {
            return Found::No;
        }
        if !is_space(c) {
            solid = true;
        }
    }
    Found::No
}

const IR_TOKENS: &[TokenType] = &[
    tok_longer("railroad-beta", Pattern::Keyword("railroad-beta"), &[IR_ID]),
    tok_longer("nonterminal", Pattern::Literal("nonterminal"), &[IR_ID]),
    tok_longer("zeroOrMore", Pattern::Literal("zeroOrMore"), &[IR_ID]),
    tok_longer("oneOrMore", Pattern::Literal("oneOrMore"), &[IR_ID]),
    tok_longer("sequence", Pattern::Literal("sequence"), &[IR_ID]),
    tok_longer("optional", Pattern::Literal("optional"), &[IR_ID]),
    tok_longer("terminal", Pattern::Literal("terminal"), &[IR_ID]),
    tok_longer("special", Pattern::Literal("special"), &[IR_ID]),
    tok_longer("choice", Pattern::Literal("choice"), &[IR_ID]),
    tok("=", Pattern::Literal("=")),
    tok(";", Pattern::Literal(";")),
    tok("(", Pattern::Literal("(")),
    tok(",", Pattern::Literal(",")),
    tok(")", Pattern::Literal(")")),
    TITLE,
    ACC_TITLE,
    ACC_DESCR,
    tok("RR_ID", Pattern::Match(identifier)),
    tok("RR_STRING", Pattern::Match(quoted)),
    hidden("RR_WHITESPACE", Pattern::Match(railroad_whitespace)),
    YAML,
    DIRECTIVE,
    SINGLE_LINE_COMMENT,
    hidden("RR_BLOCK_COMMENT", Pattern::Match(block_comment)),
];

const EBNF_TOKENS: &[TokenType] = &[
    tok_longer(
        "railroad-ebnf-beta",
        Pattern::Keyword("railroad-ebnf-beta"),
        &[EBNF_ID],
    ),
    tok("::=", Pattern::Literal("::=")),
    tok("=", Pattern::Literal("=")),
    tok(";", Pattern::Literal(";")),
    tok("|", Pattern::Literal("|")),
    tok(",", Pattern::Literal(",")),
    tok_longer("(", Pattern::Literal("("), &[EBNF_ISO_COMMENT]),
    tok(")", Pattern::Literal(")")),
    tok("[", Pattern::Literal("[")),
    tok("]", Pattern::Literal("]")),
    tok("{", Pattern::Literal("{")),
    tok("}", Pattern::Literal("}")),
    tok_longer("?", Pattern::Literal("?"), &[EBNF_SPECIAL]),
    tok("*", Pattern::Literal("*")),
    tok("+", Pattern::Literal("+")),
    tok_longer("-", Pattern::Literal("-"), &[EBNF_YAML]),
    TITLE,
    ACC_TITLE,
    ACC_DESCR,
    tok("EBNF_ID", Pattern::Match(identifier)),
    tok("EBNF_STRING", Pattern::Match(quoted)),
    tok("EBNF_SPECIAL_SEQUENCE", Pattern::Match(ebnf_special)),
    hidden("EBNF_WHITESPACE", Pattern::Match(railroad_whitespace)),
    YAML,
    DIRECTIVE,
    SINGLE_LINE_COMMENT,
    hidden("EBNF_BLOCK_COMMENT", Pattern::Match(block_comment)),
    hidden("EBNF_ISO_COMMENT", Pattern::Match(iso_comment)),
];

const ABNF_TOKENS: &[TokenType] = &[
    tok_longer(
        "railroad-abnf-beta",
        Pattern::Keyword("railroad-abnf-beta"),
        &[ABNF_RULENAME],
    ),
    tok("=", Pattern::Literal("=")),
    tok_longer(";", Pattern::Literal(";"), &[ABNF_COMMENT]),
    tok("/", Pattern::Literal("/")),
    tok("(", Pattern::Literal("(")),
    tok(")", Pattern::Literal(")")),
    tok("[", Pattern::Literal("[")),
    tok("]", Pattern::Literal("]")),
    TITLE,
    ACC_TITLE,
    ACC_DESCR,
    tok("ABNF_RULENAME", Pattern::Match(rule_name)),
    tok("ABNF_STRING", Pattern::Match(abnf_string)),
    tok("ABNF_NUMVAL", Pattern::Match(abnf_numval)),
    tok("ABNF_REPEAT", Pattern::Match(abnf_repeat)),
    tok("ABNF_EXACT_REPEAT", Pattern::Match(abnf_exact_repeat)),
    hidden("ABNF_WHITESPACE", Pattern::Match(railroad_whitespace)),
    YAML,
    DIRECTIVE,
    SINGLE_LINE_COMMENT,
    hidden("ABNF_COMMENT", Pattern::Match(abnf_comment)),
];

const PEG_TOKENS: &[TokenType] = &[
    tok_longer(
        "railroad-peg-beta",
        Pattern::Keyword("railroad-peg-beta"),
        &[PEG_ID],
    ),
    tok("<-", Pattern::Literal("<-")),
    tok(";", Pattern::Literal(";")),
    tok("/", Pattern::Literal("/")),
    tok("&", Pattern::Literal("&")),
    tok("!", Pattern::Literal("!")),
    tok("?", Pattern::Literal("?")),
    tok("*", Pattern::Literal("*")),
    tok("+", Pattern::Literal("+")),
    tok("(", Pattern::Literal("(")),
    tok(")", Pattern::Literal(")")),
    tok(".", Pattern::Literal(".")),
    TITLE,
    ACC_TITLE,
    ACC_DESCR,
    tok("PEG_ID", Pattern::Match(identifier)),
    tok("PEG_STRING", Pattern::Match(quoted)),
    hidden("PEG_WHITESPACE", Pattern::Match(railroad_whitespace)),
    YAML,
    DIRECTIVE,
    SINGLE_LINE_COMMENT,
    hidden("PEG_LINE_COMMENT", Pattern::Match(peg_line_comment)),
];

/// Mermaid's railroad value converters know three escapes; any other escaped character stands
/// for itself.
fn unescape(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    // Mermaid takes the quotes off with a slice, so a text that is too short to hold them
    // leaves nothing behind.
    let inner: String = if chars.len() < 2 {
        String::new()
    } else {
        chars[1..chars.len() - 1].iter().collect()
    };
    if !inner.contains('\\') {
        return inner;
    }
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some(next) => out.push(match next {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                c => c,
            }),
            None => out.push('\\'),
        }
    }
    out
}

/// The value of a `title` token. Mermaid's common converter trims it and collapses the
/// blanks in it, and the railroad converters take the quotes off a quoted title.
fn title_of(text: &str, decode: bool) -> String {
    let value = title_value(text);
    let quoted = match value.chars().next() {
        Some(q @ ('"' | '\'')) => value.ends_with(q),
        _ => false,
    };
    if !quoted {
        return value;
    }
    if decode {
        return unescape(&value);
    }
    if value.len() < 2 {
        return String::new();
    }
    value[1..value.len() - 1].to_string()
}

/// The title and the accessibility statements that come before the first rule.
#[derive(Default)]
struct Meta {
    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
}

/// Reads the statements that come before the first rule.
fn read_meta(
    reader: &mut Reader<'_>,
    title: usize,
    acc_title: usize,
    acc_descr: usize,
    decode_title: bool,
) -> Meta {
    let mut meta = Meta::default();
    loop {
        match reader.kind() {
            k if k == title => {
                let value = title_of(reader.take(), decode_title);
                if !value.is_empty() {
                    meta.title = Some(value);
                }
            }
            k if k == acc_title => {
                let value = acc_title_value(reader.take());
                if !value.is_empty() {
                    meta.acc_title = Some(value);
                }
            }
            k if k == acc_descr => {
                let value = acc_descr_value(reader.take());
                if !value.is_empty() {
                    meta.acc_descr = Some(value);
                }
            }
            _ => break,
        }
    }
    meta
}

/// Which of Mermaid's four grammars the text was written in. Mermaid has a detector per
/// keyword, each matching without regard to case.
fn notation_of(source: &str) -> Option<Notation> {
    let text = source.trim_start();
    for (word, notation) in [
        ("railroad-ebnf-beta", Notation::Ebnf),
        ("railroad-abnf-beta", Notation::Abnf),
        ("railroad-peg-beta", Notation::Peg),
        ("railroad-beta", Notation::Railroad),
    ] {
        if text
            .get(..word.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(word))
        {
            return Some(notation);
        }
    }
    None
}

/// Reads a railroad diagram. The title of the diagram is the one of its front matter, unless
/// the diagram has a title of its own, as in Mermaid.
pub(crate) fn parse_railroad(
    source: &str,
    max_depth: usize,
    title: Option<&str>,
) -> Result<RailroadModel, Error> {
    let Some(notation) = notation_of(source) else {
        return Err(Error::new(
            ErrorCode::Syntax,
            "Expecting a railroad keyword but found none.",
        )
        .at(source, 0));
    };
    let (reader, meta, rules) = match notation {
        Notation::Railroad => {
            let mut reader = reader(source, IR_TOKENS)?;
            reader.expect(IR_KEYWORD)?;
            let meta = read_meta(&mut reader, IR_TITLE, IR_ACC_TITLE, IR_ACC_DESCR, true);
            let mut rules = Vec::new();
            while reader.kind() != END {
                rules.push(ir_rule(&mut reader, max_depth)?);
            }
            (reader, meta, rules)
        }
        Notation::Ebnf => {
            let mut reader = reader(source, EBNF_TOKENS)?;
            reader.expect(EBNF_KEYWORD)?;
            let meta = read_meta(
                &mut reader,
                EBNF_TITLE,
                EBNF_ACC_TITLE,
                EBNF_ACC_DESCR,
                true,
            );
            let mut rules = Vec::new();
            while reader.kind() != END {
                rules.push(ebnf_rule(&mut reader, max_depth)?);
            }
            (reader, meta, rules)
        }
        Notation::Abnf => {
            let mut reader = reader(source, ABNF_TOKENS)?;
            reader.expect(ABNF_KEYWORD)?;
            let meta = read_meta(
                &mut reader,
                ABNF_TITLE,
                ABNF_ACC_TITLE,
                ABNF_ACC_DESCR,
                false,
            );
            let mut rules = Vec::new();
            while reader.kind() != END {
                rules.push(abnf_rule(&mut reader, max_depth)?);
            }
            (reader, meta, rules)
        }
        Notation::Peg => {
            let mut reader = reader(source, PEG_TOKENS)?;
            reader.expect(PEG_KEYWORD)?;
            let meta = read_meta(&mut reader, PEG_TITLE, PEG_ACC_TITLE, PEG_ACC_DESCR, true);
            let mut rules = Vec::new();
            while reader.kind() != END {
                rules.push(peg_rule(&mut reader, max_depth)?);
            }
            (reader, meta, rules)
        }
    };
    drop(reader);
    Ok(RailroadModel {
        notation,
        title: meta.title.or_else(|| title.map(str::to_string)),
        acc_title: meta.acc_title,
        acc_descr: meta.acc_descr,
        rules,
    })
}

/// Reads the tokens of a railroad diagram.
fn reader<'a>(source: &'a str, types: &'static [TokenType]) -> Result<Reader<'a>, Error> {
    let tokens = tokenize(source, types)?;
    Ok(Reader::new(tokens, types))
}

/// The error of a diagram whose nesting is deeper than the limit.
fn too_deep(max_depth: usize) -> Error {
    Error::new(
        ErrorCode::Limit,
        format!("Expressions are nested deeper than the limit of {max_depth}."),
    )
}

// -- `railroad-beta`: Mermaid's own notation, one constructor call per node. --

/// What an open call of the railroad notation is waiting for.
enum IrFrame {
    Sequence(Vec<Node>),
    Choice(Vec<Node>),
    Wrap(IrWrap),
}

/// The calls of the railroad notation that take one expression.
#[derive(Clone, Copy)]
enum IrWrap {
    Optional,
    OneOrMore,
    ZeroOrMore,
}

/// Reads the argument of a call, such as the text of `terminal("a")`.
fn ir_argument(reader: &mut Reader<'_>) -> Result<String, Error> {
    reader.i += 1;
    reader.expect(IR_OPEN)?;
    let value = unescape(reader.expect(IR_STRING)?);
    reader.expect(IR_CLOSE)?;
    Ok(value)
}

/// Reads the expression of a rule.
fn ir_expression(reader: &mut Reader<'_>, max_depth: usize) -> Result<Node, Error> {
    let mut stack: Vec<IrFrame> = Vec::new();
    // The value that is waiting to be put into a call, if there is one.
    let mut pending: Option<Node> = None;
    loop {
        let mut value = match pending.take() {
            Some(value) => value,
            None => match reader.kind() {
                IR_TERMINAL => Node::Terminal(ir_argument(reader)?),
                IR_NONTERMINAL => Node::NonTerminal(ir_argument(reader)?),
                IR_SPECIAL => Node::Special(ir_argument(reader)?),
                IR_SEQUENCE | IR_CHOICE | IR_OPTIONAL | IR_ONE_MORE | IR_ZERO_MORE => {
                    if stack.len() >= max_depth {
                        return Err(too_deep(max_depth));
                    }
                    let frame = match reader.kind() {
                        IR_SEQUENCE => IrFrame::Sequence(Vec::new()),
                        IR_CHOICE => IrFrame::Choice(Vec::new()),
                        IR_OPTIONAL => IrFrame::Wrap(IrWrap::Optional),
                        IR_ONE_MORE => IrFrame::Wrap(IrWrap::OneOrMore),
                        _ => IrFrame::Wrap(IrWrap::ZeroOrMore),
                    };
                    reader.i += 1;
                    reader.expect(IR_OPEN)?;
                    stack.push(frame);
                    continue;
                }
                _ => return Err(reader.fail("an expression")),
            },
        };
        // The value goes into the call that is open, which may be one of the outer ones.
        loop {
            let Some(last) = stack.len().checked_sub(1) else {
                return Ok(value);
            };
            match &mut stack[last] {
                IrFrame::Sequence(elements) => {
                    elements.push(value);
                    if reader.accept(IR_COMMA) {
                        break;
                    }
                    reader.expect(IR_CLOSE)?;
                    let IrFrame::Sequence(elements) = stack.pop().expect("a frame") else {
                        unreachable!()
                    };
                    value = Node::sequence(elements);
                }
                IrFrame::Choice(alternatives) => {
                    alternatives.push(value);
                    if reader.accept(IR_COMMA) {
                        break;
                    }
                    reader.expect(IR_CLOSE)?;
                    let IrFrame::Choice(alternatives) = stack.pop().expect("a frame") else {
                        unreachable!()
                    };
                    value = Node::choice(alternatives);
                }
                IrFrame::Wrap(wrap) => {
                    let wrap = *wrap;
                    reader.expect(IR_CLOSE)?;
                    stack.pop();
                    value = match wrap {
                        IrWrap::Optional => Node::Optional(Box::new(value)),
                        IrWrap::OneOrMore => Node::repetition(value, 1, None),
                        IrWrap::ZeroOrMore => Node::repetition(value, 0, None),
                    };
                }
            }
        }
    }
}

/// Reads one rule of the railroad notation.
fn ir_rule(reader: &mut Reader<'_>, max_depth: usize) -> Result<Rule, Error> {
    let name = reader.expect(IR_ID)?.to_string();
    reader.expect(IR_EQUALS)?;
    let definition = ir_expression(reader, max_depth)?;
    reader.expect(IR_SEMICOLON)?;
    Ok(Rule { name, definition })
}

// -- `railroad-ebnf-beta`: EBNF in the W3C and ISO 14977 styles. --

/// The bracket an EBNF group is written in.
#[derive(Clone, Copy)]
enum EbnfBracket {
    Group,
    Optional,
    Repetition,
}

/// One open bracket of EBNF, or the rule itself.
struct EbnfFrame {
    bracket: Option<EbnfBracket>,
    /// The alternatives of the choice; the last one is the one being written.
    alts: Vec<Vec<Node>>,
    /// The term being written, which is not yet in an alternative.
    term: Option<Node>,
    /// Whether this frame is the exception of the term around it.
    as_except: bool,
}

impl EbnfFrame {
    fn new(bracket: Option<EbnfBracket>) -> Self {
        Self {
            bracket,
            alts: vec![Vec::new()],
            term: None,
            as_except: false,
        }
    }

    /// Puts the term that is being written into the alternative it belongs to.
    fn flush(&mut self) {
        if let Some(term) = self.term.take() {
            if let Some(alt) = self.alts.last_mut() {
                alt.push(term);
            }
        }
    }

    /// The model of the choice this frame holds.
    fn finish(mut self) -> Result<Node, Error> {
        self.flush();
        let mut alts = Vec::with_capacity(self.alts.len());
        for alt in self.alts {
            if alt.is_empty() {
                return Err(Error::new(
                    ErrorCode::Syntax,
                    "Expecting a term but found none.",
                ));
            }
            alts.push(Node::sequence(alt));
        }
        Ok(Node::choice(alts))
    }
}

/// Whether a token starts a term of EBNF.
fn ebnf_starts_term(kind: usize) -> bool {
    matches!(
        kind,
        EBNF_STRING | EBNF_ID | EBNF_SPECIAL | EBNF_OPEN | EBNF_OPTION | EBNF_REPEAT
    )
}

/// Reads the definition of an EBNF rule.
fn ebnf_definition(reader: &mut Reader<'_>, max_depth: usize) -> Result<Node, Error> {
    let mut stack: Vec<EbnfFrame> = Vec::new();
    let mut top = EbnfFrame::new(None);
    // Whether the term being written waits for the exception of a postfix.
    let mut except = false;
    loop {
        let mut primary = match reader.kind() {
            EBNF_STRING => Node::Terminal(unescape(reader.take())),
            EBNF_ID => Node::NonTerminal(reader.take().to_string()),
            EBNF_SPECIAL => Node::Special(special_text(reader.take())),
            EBNF_OPEN | EBNF_OPTION | EBNF_REPEAT => {
                if stack.len() >= max_depth {
                    return Err(too_deep(max_depth));
                }
                let bracket = match reader.kind() {
                    EBNF_OPEN => EbnfBracket::Group,
                    EBNF_OPTION => EbnfBracket::Optional,
                    _ => EbnfBracket::Repetition,
                };
                reader.i += 1;
                let mut frame = EbnfFrame::new(Some(bracket));
                frame.as_except = except;
                stack.push(top);
                top = frame;
                except = false;
                continue;
            }
            _ => return Err(reader.fail("a terminal, a name, or an opening bracket")),
        };
        loop {
            if except {
                let Some(term) = top.term.take() else {
                    return Err(reader.fail("a terminal, a name, or an opening bracket"));
                };
                top.term = Some(Node::Sequence(vec![
                    term,
                    Node::Terminal("-".to_string()),
                    primary,
                ]));
                except = false;
            } else {
                top.flush();
                top.term = Some(primary);
            }
            // The postfixes apply to everything before them in the term.
            loop {
                match reader.kind() {
                    EBNF_QUESTION => {
                        reader.i += 1;
                        let term = top.term.take().expect("a term");
                        top.term = Some(Node::Optional(Box::new(term)));
                    }
                    EBNF_STAR => {
                        reader.i += 1;
                        let term = top.term.take().expect("a term");
                        top.term = Some(Node::repetition(term, 0, None));
                    }
                    EBNF_PLUS => {
                        reader.i += 1;
                        let term = top.term.take().expect("a term");
                        top.term = Some(Node::repetition(term, 1, None));
                    }
                    _ => break,
                }
            }
            let kind = reader.kind();
            let close = match top.bracket {
                Some(EbnfBracket::Group) => EBNF_CLOSE,
                Some(EbnfBracket::Optional) => EBNF_OPTION_END,
                Some(EbnfBracket::Repetition) => EBNF_REPEAT_END,
                None => END,
            };
            if kind == close && top.bracket.is_some() {
                reader.i += 1;
                let bracket = top.bracket;
                let was_except = top.as_except;
                let choice = top.finish()?;
                top = stack
                    .pop()
                    .ok_or_else(|| reader.fail("a closing bracket"))?;
                primary = match bracket {
                    Some(EbnfBracket::Optional) => Node::Optional(Box::new(choice)),
                    Some(EbnfBracket::Repetition) => Node::repetition(choice, 0, None),
                    _ => choice,
                };
                except = was_except;
                continue;
            }
            if kind == EBNF_MINUS {
                reader.i += 1;
                except = true;
            } else if kind == EBNF_COMMA {
                reader.i += 1;
            } else if kind == EBNF_BAR {
                reader.i += 1;
                top.flush();
                top.alts.push(Vec::new());
            } else if !ebnf_starts_term(kind) {
                if stack.is_empty() {
                    return top.finish();
                }
                return Err(reader.fail("a closing bracket"));
            }
            break;
        }
    }
}

/// Reads one rule of EBNF.
fn ebnf_rule(reader: &mut Reader<'_>, max_depth: usize) -> Result<Rule, Error> {
    let name = reader.expect(EBNF_ID)?.to_string();
    if !reader.accept(EBNF_EQUALS) {
        reader.expect(EBNF_DEFINES)?;
    }
    let definition = ebnf_definition(reader, max_depth)?;
    reader.expect(EBNF_SEMICOLON)?;
    Ok(Rule { name, definition })
}

// -- `railroad-abnf-beta`: ABNF as in RFC 5234. --

/// The bracket an ABNF group is written in.
#[derive(Clone, Copy)]
enum AbnfBracket {
    Group,
    Optional,
}

/// One open bracket of ABNF, or the rule itself.
struct AbnfFrame {
    bracket: Option<AbnfBracket>,
    /// The alternatives of the alternation; the last one is the one being written.
    alts: Vec<Vec<(Option<String>, Node)>>,
    /// The repetition written before the bracket this frame holds.
    repeat: Option<String>,
}

impl AbnfFrame {
    fn new(bracket: Option<AbnfBracket>) -> Self {
        Self {
            bracket,
            alts: vec![Vec::new()],
            repeat: None,
        }
    }

    /// The model of the alternation this frame holds.
    fn finish(self) -> Result<Node, Error> {
        let mut alts = Vec::with_capacity(self.alts.len());
        for alt in self.alts {
            if alt.is_empty() {
                return Err(Error::new(
                    ErrorCode::Syntax,
                    "Expecting an element but found none.",
                ));
            }
            alts.push(Node::sequence(
                alt.into_iter()
                    .map(|(repeat, node)| abnf_element(repeat, node))
                    .collect(),
            ));
        }
        Ok(Node::choice(alts))
    }
}

/// The model of one element of ABNF, with the repetition that was written before it.
fn abnf_element(repeat: Option<String>, primary: Node) -> Node {
    let Some(repeat) = repeat else {
        return primary;
    };
    let (min, max) = if let Some(star) = repeat.find('*') {
        let min = if star > 0 {
            repeat[..star].parse().unwrap_or(u64::MAX)
        } else {
            0
        };
        let max = if star + 1 < repeat.len() {
            repeat[star + 1..].parse().ok()
        } else {
            None
        };
        (min, max)
    } else {
        let exact = repeat.parse().unwrap_or(u64::MAX);
        (exact, Some(exact))
    };
    if min == 0 && max == Some(1) {
        return Node::Optional(Box::new(primary));
    }
    Node::repetition(primary, min, max)
}

/// Whether a token starts an element of ABNF.
fn abnf_starts_element(kind: usize) -> bool {
    matches!(
        kind,
        ABNF_STRING | ABNF_NUMVAL | ABNF_RULENAME | ABNF_OPEN | ABNF_OPTION
    )
}

/// Reads the definition of an ABNF rule.
fn abnf_definition(reader: &mut Reader<'_>, max_depth: usize) -> Result<Node, Error> {
    let mut stack: Vec<AbnfFrame> = Vec::new();
    let mut top = AbnfFrame::new(None);
    loop {
        let mut repeat = match reader.kind() {
            ABNF_REPEAT | ABNF_EXACT_REPEAT => Some(reader.take().to_string()),
            _ => None,
        };
        let mut primary = match reader.kind() {
            ABNF_STRING => Node::Terminal(abnf_string_value(reader.take())),
            ABNF_NUMVAL => Node::Terminal(reader.take().to_string()),
            ABNF_RULENAME => Node::NonTerminal(reader.take().to_string()),
            ABNF_OPEN | ABNF_OPTION => {
                if stack.len() >= max_depth {
                    return Err(too_deep(max_depth));
                }
                let bracket = if reader.kind() == ABNF_OPEN {
                    AbnfBracket::Group
                } else {
                    AbnfBracket::Optional
                };
                reader.i += 1;
                let mut frame = AbnfFrame::new(Some(bracket));
                frame.repeat = repeat.take();
                stack.push(top);
                top = frame;
                continue;
            }
            _ => return Err(reader.fail("a string, a number, a name, or an opening bracket")),
        };
        loop {
            if let Some(alt) = top.alts.last_mut() {
                alt.push((repeat.take(), primary));
            }
            let kind = reader.kind();
            let close = match top.bracket {
                Some(AbnfBracket::Group) => ABNF_CLOSE,
                Some(AbnfBracket::Optional) => ABNF_OPTION_END,
                None => END,
            };
            if kind == close && top.bracket.is_some() {
                reader.i += 1;
                let bracket = top.bracket;
                let group_repeat = top.repeat.clone();
                let choice = top.finish()?;
                top = stack
                    .pop()
                    .ok_or_else(|| reader.fail("a closing bracket"))?;
                primary = match bracket {
                    Some(AbnfBracket::Optional) => Node::Optional(Box::new(choice)),
                    _ => choice,
                };
                repeat = group_repeat;
                continue;
            }
            if kind == ABNF_SLASH {
                reader.i += 1;
                top.alts.push(Vec::new());
            } else if !abnf_starts_element(kind) && kind != ABNF_REPEAT && kind != ABNF_EXACT_REPEAT
            {
                if stack.is_empty() {
                    return top.finish();
                }
                return Err(reader.fail("a closing bracket"));
            }
            break;
        }
    }
}

/// Reads one rule of ABNF.
fn abnf_rule(reader: &mut Reader<'_>, max_depth: usize) -> Result<Rule, Error> {
    let name = reader.expect(ABNF_RULENAME)?.to_string();
    reader.expect(ABNF_EQUALS)?;
    let definition = abnf_definition(reader, max_depth)?;
    reader.expect(ABNF_SEMICOLON)?;
    Ok(Rule { name, definition })
}

// -- `railroad-peg-beta`: parsing expression grammars. --

/// One open group of PEG, or the rule itself.
struct PegFrame {
    /// The alternatives of the ordered choice; the last one is the one being written.
    alts: Vec<Vec<Node>>,
    /// The predicate written before the group this frame holds.
    operator: Option<char>,
}

impl PegFrame {
    fn new() -> Self {
        Self {
            alts: vec![Vec::new()],
            operator: None,
        }
    }

    /// The model of the ordered choice this frame holds.
    fn finish(self) -> Result<Node, Error> {
        let mut alts = Vec::with_capacity(self.alts.len());
        for alt in self.alts {
            if alt.is_empty() {
                return Err(Error::new(
                    ErrorCode::Syntax,
                    "Expecting an element but found none.",
                ));
            }
            alts.push(Node::sequence(alt));
        }
        Ok(Node::choice(alts))
    }
}

/// What a predicate shows of the expression it looks ahead for.
fn label(node: &Node) -> String {
    match node {
        Node::Terminal(value) => format!("\"{value}\""),
        Node::NonTerminal(name) => name.clone(),
        Node::Special(text) => text.clone(),
        _ => "(...)".to_string(),
    }
}

/// Whether a token starts an element of PEG.
fn peg_starts_element(kind: usize) -> bool {
    matches!(kind, PEG_STRING | PEG_ID | PEG_DOT | PEG_OPEN)
}

/// Reads the definition of a PEG rule.
fn peg_definition(reader: &mut Reader<'_>, max_depth: usize) -> Result<Node, Error> {
    let mut stack: Vec<PegFrame> = Vec::new();
    let mut top = PegFrame::new();
    loop {
        let mut operator = match reader.kind() {
            PEG_AND => Some('&'),
            PEG_NOT => Some('!'),
            _ => None,
        };
        if operator.is_some() {
            reader.i += 1;
        }
        let mut primary = match reader.kind() {
            PEG_STRING => Node::Terminal(unescape(reader.take())),
            PEG_ID => Node::NonTerminal(reader.take().to_string()),
            PEG_DOT => Node::Special(reader.take().to_string()),
            PEG_OPEN => {
                if stack.len() >= max_depth {
                    return Err(too_deep(max_depth));
                }
                reader.i += 1;
                let mut frame = PegFrame::new();
                frame.operator = operator.take();
                stack.push(top);
                top = frame;
                continue;
            }
            _ => return Err(reader.fail("a literal, a name, a dot, or an opening parenthesis")),
        };
        loop {
            let suffix = match reader.kind() {
                PEG_QUESTION => Some('?'),
                PEG_STAR => Some('*'),
                PEG_PLUS => Some('+'),
                _ => None,
            };
            if suffix.is_some() {
                reader.i += 1;
            }
            let inner = match suffix {
                Some('?') => Node::Optional(Box::new(primary)),
                Some('*') => Node::repetition(primary, 0, None),
                Some('+') => Node::repetition(primary, 1, None),
                _ => primary,
            };
            let value = match operator {
                Some(op) => Node::Special(format!("{op}{}", label(&inner))),
                None => inner,
            };
            if let Some(alt) = top.alts.last_mut() {
                alt.push(value);
            }
            let kind = reader.kind();
            if kind == PEG_CLOSE && !stack.is_empty() {
                reader.i += 1;
                let group_operator = top.operator;
                let choice = top.finish()?;
                top = stack
                    .pop()
                    .ok_or_else(|| reader.fail("a closing parenthesis"))?;
                primary = choice;
                operator = group_operator;
                continue;
            }
            if kind == PEG_SLASH {
                reader.i += 1;
                top.alts.push(Vec::new());
            } else if !peg_starts_element(kind) && kind != PEG_AND && kind != PEG_NOT {
                if stack.is_empty() {
                    return top.finish();
                }
                return Err(reader.fail("a closing parenthesis"));
            }
            break;
        }
    }
}

/// Reads one rule of PEG.
fn peg_rule(reader: &mut Reader<'_>, max_depth: usize) -> Result<Rule, Error> {
    let name = reader.expect(PEG_ID)?.to_string();
    reader.expect(PEG_ARROW)?;
    let definition = peg_definition(reader, max_depth)?;
    reader.expect(PEG_SEMICOLON)?;
    Ok(Rule { name, definition })
}

/// The text of a special sequence in EBNF, without the question marks around it.
fn special_text(text: &str) -> String {
    text[1..text.len() - 1].trim().to_string()
}

/// The text of a string in ABNF, which has no escapes.
fn abnf_string_value(text: &str) -> String {
    text[1..text.len() - 1].to_string()
}
