//! The tokens of a packet diagram. Mermaid gives the grammar of the type with Langium, whose
//! lexer tries the terminals of the common grammar first and the keywords of the packet grammar
//! after them. This is the tokenizer of `diagrams/common/tokens.rs`, which is not compiled for
//! this type, with the parts the type does not need left out.

use crate::error::{Error, ErrorCode};
use crate::util::{is_line_end, is_space, trim};

/// What a matcher found at a position.
enum Found {
    /// A match that ends at this byte offset.
    End(usize),
    No,
    /// No match here, nor anywhere later: the construct opens here but its closer is not in
    /// the rest of the text. The type is not tried again.
    Never,
}

/// The kinds of the tokens, in the order the lexer tries them: the terminals of the common
/// grammar first, then the keywords of the packet grammar, as Langium builds them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// The kind of the token after the last one.
    End,
    AccDescr,
    AccTitle,
    Title,
    Int,
    Str,
    Newline,
    Whitespace,
    Yaml,
    Directive,
    Comment,
    PacketBeta,
    Packet,
    Dash,
    Plus,
    Colon,
}

impl Kind {
    /// The name of the kind, as an error message writes it.
    fn name(self) -> &'static str {
        match self {
            Kind::End => "end of input",
            Kind::AccDescr => "ACC_DESCR",
            Kind::AccTitle => "ACC_TITLE",
            Kind::Title => "TITLE",
            Kind::Int => "INT",
            Kind::Str => "STRING",
            Kind::Newline => "NEWLINE",
            Kind::Whitespace => "WHITESPACE",
            Kind::Yaml => "YAML",
            Kind::Directive => "DIRECTIVE",
            Kind::Comment => "SINGLE_LINE_COMMENT",
            Kind::PacketBeta => "packet-beta",
            Kind::Packet => "packet",
            Kind::Dash => "-",
            Kind::Plus => "+",
            Kind::Colon => ":",
        }
    }

    /// Whether the lexer skips the token.
    fn hidden(self) -> bool {
        matches!(
            self,
            Kind::Whitespace | Kind::Yaml | Kind::Directive | Kind::Comment
        )
    }
}

/// The kind of the token after the last one.
pub(crate) const END: Kind = Kind::End;

/// The tokens of a text.
pub(crate) struct Tokens<'a> {
    kinds: Vec<Kind>,
    starts: Vec<usize>,
    ends: Vec<usize>,
    src: &'a str,
}

/// Reads the tokens of a text.
pub(crate) fn tokenize(src: &str) -> Result<Tokens<'_>, Error> {
    let mut tokens = Tokens {
        kinds: Vec::new(),
        starts: Vec::new(),
        ends: Vec::new(),
        src,
    };
    let mut dead = [false; 15];
    let mut p = 0;
    'scan: while p < src.len() {
        for kind in ALL {
            let Some(end) = match_at(kind, &mut dead, src, p) else {
                continue;
            };
            if !kind.hidden() {
                tokens.kinds.push(kind);
                tokens.starts.push(p);
                tokens.ends.push(end);
            }
            p = end;
            continue 'scan;
        }
        return Err(Error::new(ErrorCode::Syntax, "Unrecognized text.").at(src, p));
    }
    tokens.kinds.push(END);
    tokens.starts.push(src.len());
    tokens.ends.push(src.len());
    Ok(tokens)
}

/// Every kind, in the order the lexer tries them.
const ALL: [Kind; 15] = [
    Kind::AccDescr,
    Kind::AccTitle,
    Kind::Title,
    Kind::Int,
    Kind::Str,
    Kind::Newline,
    Kind::Whitespace,
    Kind::Yaml,
    Kind::Directive,
    Kind::Comment,
    Kind::PacketBeta,
    Kind::Packet,
    Kind::Dash,
    Kind::Plus,
    Kind::Colon,
];

/// The end of the match of a kind at a place, or nothing when it does not match there.
fn match_at(kind: Kind, dead: &mut [bool; 15], src: &str, p: usize) -> Option<usize> {
    let index = ALL
        .iter()
        .position(|&k| k == kind)
        .expect("the kind is one of them");
    if dead[index] {
        return None;
    }
    let end = match kind {
        Kind::AccDescr => acc_descr(src, p),
        Kind::AccTitle => acc_title(src, p),
        Kind::Title => title(src, p),
        Kind::Int => int(src, p),
        Kind::Str => string(src, p),
        Kind::Newline => newline(src, p),
        Kind::Whitespace => whitespace(src, p),
        Kind::Yaml => yaml(src, p),
        Kind::Directive => directive(src, p),
        Kind::Comment => single_line_comment(src, p),
        Kind::PacketBeta => literal(src, p, "packet-beta"),
        Kind::Packet => keyword(src, p, "packet"),
        Kind::Dash => literal(src, p, "-"),
        Kind::Plus => literal(src, p, "+"),
        Kind::Colon => literal(src, p, ":"),
        Kind::End => Found::No,
    };
    let end = match end {
        Found::End(end) => end,
        Found::No => return None,
        Found::Never => {
            dead[index] = true;
            return None;
        }
    };
    (end > p).then_some(end)
}

/// A word that ends only at whitespace, a comment, or the end of the input.
fn keyword(src: &str, at: usize, word: &str) -> Found {
    let Some(rest) = src[at..].strip_prefix(word) else {
        return Found::No;
    };
    let ends = rest.is_empty() || rest.starts_with("%%") || rest.starts_with(is_space);
    if ends {
        Found::End(src.len() - rest.len())
    } else {
        Found::No
    }
}

/// A word that ends anywhere.
fn literal(src: &str, at: usize, word: &str) -> Found {
    if src[at..].starts_with(word) {
        Found::End(at + word.len())
    } else {
        Found::No
    }
}

fn spaces_tabs(src: &str, mut at: usize) -> usize {
    while matches!(src.as_bytes().get(at), Some(b' ' | b'\t')) {
        at += 1;
    }
    at
}

fn line_end(src: &str, from: usize) -> usize {
    src.as_bytes()[from..]
        .iter()
        .position(|&b| b == b'\n' || b == b'\r')
        .map_or(src.len(), |i| from + i)
}

/// `[^\n\r]*?(?=%%)|[^\n\r]*` from `at`: up to the first comment on the line, or its end.
fn rest_of_line(src: &str, at: usize) -> usize {
    let end = line_end(src, at);
    src[at..end].find("%%").map_or(end, |i| at + i)
}

/// `(?:\r?\n|(?!\S))` at `at`: a line break, or the end, or whitespace that is not taken.
fn block_end(src: &str, at: usize) -> Option<usize> {
    let rest = &src[at..];
    line_break(src, at).or((rest.is_empty() || rest.starts_with(is_space)).then_some(at))
}

/// `\r?\n` at `at`.
fn line_break(src: &str, at: usize) -> Option<usize> {
    match &src.as_bytes()[at..] {
        [b'\r', b'\n', ..] => Some(at + 2),
        [b'\n', ..] => Some(at + 1),
        _ => None,
    }
}

/// `[\t ]*accDescr(?:[\t ]*:([^\n\r]*?(?=%%)|[^\n\r]*)|\s*{([^}]*)})`
fn acc_descr(src: &str, at: usize) -> Found {
    let at = spaces_tabs(src, at);
    let Some(rest) = src[at..].strip_prefix("accDescr") else {
        return Found::No;
    };
    let after = src.len() - rest.len();
    let colon = spaces_tabs(src, after);
    if src.as_bytes().get(colon) == Some(&b':') {
        return Found::End(rest_of_line(src, colon + 1));
    }
    let Some(body) = rest.trim_start_matches(is_space).strip_prefix('{') else {
        return Found::No;
    };
    match body.find('}') {
        Some(i) => Found::End(src.len() - body.len() + i + 1),
        None => Found::No,
    }
}

/// `[\t ]*accTitle[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)`
fn acc_title(src: &str, at: usize) -> Found {
    let at = spaces_tabs(src, at);
    let Some(rest) = src[at..].strip_prefix("accTitle") else {
        return Found::No;
    };
    let colon = spaces_tabs(src, src.len() - rest.len());
    if src.as_bytes().get(colon) != Some(&b':') {
        return Found::No;
    }
    Found::End(rest_of_line(src, colon + 1))
}

/// `[\t ]*title(?:[\t ][^\n\r]*?(?=%%)|[\t ][^\n\r]*|)`
fn title(src: &str, at: usize) -> Found {
    let at = spaces_tabs(src, at);
    if !src[at..].starts_with("title") {
        return Found::No;
    }
    let end = at + 5;
    match src.as_bytes().get(end) {
        Some(b' ' | b'\t') => Found::End(rest_of_line(src, end + 1)),
        _ => Found::End(end),
    }
}

/// `0|[1-9][0-9]*(?!\.)`: a number without a leading zero, and not the start of a decimal.
fn int(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    match bytes.get(at) {
        Some(b'0') => Found::End(at + 1),
        Some(b'1'..=b'9') => {
            let mut end = at + 1;
            while matches!(bytes.get(end), Some(b'0'..=b'9')) {
                end += 1;
            }
            if bytes.get(end) == Some(&b'.') {
                Found::No
            } else {
                Found::End(end)
            }
        }
        _ => Found::No,
    }
}

/// `"([^"\\]|\\.)*"|'([^'\\]|\\.)*'`, where `.` is anything but a line terminator.
fn string(src: &str, at: usize) -> Found {
    let Some(quote) = src[at..].chars().next().filter(|&q| q == '"' || q == '\'') else {
        return Found::No;
    };
    let mut chars = src[at + 1..].char_indices();
    while let Some((i, c)) = chars.next() {
        if c == quote {
            return Found::End(at + 1 + i + 1);
        }
        if c == '\\' {
            match chars.next() {
                Some((_, next)) if !is_line_end(next) => {}
                _ => return Found::No,
            }
        }
    }
    Found::No
}

/// `\r?\n`
fn newline(src: &str, at: usize) -> Found {
    line_break(src, at).map_or(Found::No, Found::End)
}

/// `[\t ]+`
fn whitespace(src: &str, at: usize) -> Found {
    match spaces_tabs(src, at) {
        end if end > at => Found::End(end),
        _ => Found::No,
    }
}

/// `---[\t ]*\r?\n(?:[\S\s]*?\r?\n)?---(?:\r?\n|(?!\S))`
fn yaml(src: &str, at: usize) -> Found {
    if !src[at..].starts_with("---") {
        return Found::No;
    }
    let Some(body) = line_break(src, spaces_tabs(src, at + 3)) else {
        return Found::No;
    };
    let closes = |fence: usize| {
        src.as_bytes()[fence..]
            .starts_with(b"---")
            .then(|| block_end(src, fence + 3))
            .flatten()
    };
    for k in body..src.len() {
        if let Some(end) = line_break(src, k).and_then(closes) {
            return Found::End(end);
        }
    }
    closes(body).map_or(Found::Never, Found::End)
}

/// `[\t ]*%%{[\S\s]*?}%%(?:\r?\n|(?!\S))`
fn directive(src: &str, at: usize) -> Found {
    let at = spaces_tabs(src, at);
    if !src[at..].starts_with("%%{") {
        return Found::No;
    }
    for k in at + 3..src.len() {
        if src.as_bytes()[k..].starts_with(b"}%%") {
            if let Some(end) = block_end(src, k + 3) {
                return Found::End(end);
            }
        }
    }
    Found::Never
}

/// `[\t ]*%%[^\n\r]*`
fn single_line_comment(src: &str, at: usize) -> Found {
    let at = spaces_tabs(src, at);
    if !src[at..].starts_with("%%") {
        return Found::No;
    }
    Found::End(line_end(src, at))
}

/// `.replace(/[\t ]{2,}/g, ' ')`: a lone space or tab is kept as written.
fn collapse(text: &str, out: &mut String) {
    let mut run = String::new();
    for c in text.chars() {
        if c == ' ' || c == '\t' {
            run.push(c);
            continue;
        }
        out.push_str(if run.len() > 1 { " " } else { &run });
        run.clear();
        out.push(c);
    }
    out.push_str(if run.len() > 1 { " " } else { &run });
}

/// Trims the text and collapses runs of spaces and tabs.
fn single(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    collapse(trim(text), &mut out);
    out
}

/// The value of a `title` token.
pub(crate) fn title_value(text: &str) -> String {
    let start = text.find("title").map_or(text.len(), |i| i + 5);
    single(&text[start..])
}

/// The value of an `accTitle` token.
pub(crate) fn acc_title_value(text: &str) -> String {
    let start = text.find(':').map_or(text.len(), |i| i + 1);
    single(&text[start..])
}

/// The value of an `accDescr` token, on one line or in braces.
pub(crate) fn acc_descr_value(text: &str) -> String {
    let after = text.find("accDescr").map_or(text.len(), |i| i + 8);
    let rest = &text[after..];
    let head = rest.trim_start_matches([' ', '\t']);
    if let Some(line) = head.strip_prefix(':') {
        return single(line);
    }
    let open = rest.find('{').map_or(rest.len(), |i| i + 1);
    let body = rest[open..].strip_suffix('}').unwrap_or(&rest[open..]);
    multiline(body)
}

/// `.replace(/^\s*/gm, '').replace(/\s+$/gm, '').replace(/[\t ]{2,}/gm, ' ')`
/// .replace(/[\n\r]{2,}/gm, '\n')`: each line trimmed, blank lines dropped, and each line
/// kept joined to the one before by the first line break after that one.
fn multiline(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut sep = None;
    let push = |line: &str, out: &mut String, sep: &mut Option<char>| {
        let line = trim(line);
        if !line.is_empty() {
            if let Some(c) = sep.take() {
                out.push(c);
            }
            collapse(line, out);
        }
    };
    let mut start = 0;
    for (i, c) in body.char_indices() {
        if !is_line_end(c) {
            continue;
        }
        push(&body[start..i], &mut out, &mut sep);
        if !out.is_empty() && sep.is_none() {
            sep = Some(c);
        }
        start = i + c.len_utf8();
    }
    push(&body[start..], &mut out, &mut sep);
    out
}

/// The text of a string token, without its quotes and with its escapes resolved.
pub(crate) fn string_value(text: &str) -> String {
    let inner = &text[1..text.len() - 1];
    if !inner.contains('\\') {
        return inner.to_string();
    }
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(next) = chars.next() else { break };
        out.push(match next {
            'b' => '\u{8}',
            'f' => '\u{c}',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'v' => '\u{b}',
            '0' => '\0',
            c => c,
        });
    }
    out
}

/// Reads tokens for a hand-written parser.
pub(crate) struct Reader<'a> {
    pub i: usize,
    tokens: Tokens<'a>,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(tokens: Tokens<'a>) -> Self {
        Self { i: 0, tokens }
    }

    pub(crate) fn kind(&self) -> Kind {
        self.tokens.kinds[self.i]
    }

    pub(crate) fn text(&self) -> &'a str {
        &self.tokens.src[self.tokens.starts[self.i]..self.tokens.ends[self.i]]
    }

    pub(crate) fn take(&mut self) -> &'a str {
        let text = self.text();
        self.i += 1;
        text
    }

    pub(crate) fn accept(&mut self, kind: Kind) -> bool {
        if self.kind() != kind {
            return false;
        }
        self.i += 1;
        true
    }

    pub(crate) fn expect(&mut self, kind: Kind) -> Result<&'a str, Error> {
        if self.kind() != kind {
            return Err(self.fail(&format!("token of type '{}'", kind.name())));
        }
        Ok(self.take())
    }

    pub(crate) fn fail(&self, expected: &str) -> Error {
        let found = if self.kind() == END {
            "end of input".to_string()
        } else {
            format!("`{}`", self.text())
        };
        Error::new(
            ErrorCode::Syntax,
            format!("Expecting {expected} but found {found}."),
        )
        .at(self.tokens.src, self.tokens.starts[self.i])
    }
}
