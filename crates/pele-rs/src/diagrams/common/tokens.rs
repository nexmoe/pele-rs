//! Tokenizer for the diagram types Mermaid defines with Langium grammars. Token types are tried
//! in order and the first match wins, as in Chevrotain. Ported from pele's
//! `diagrams/common/tokens.ts`, where each type is a regular expression; here each is a
//! function that answers as that expression does.

use crate::error::{Error, ErrorCode};
use crate::util::{is_line_end, is_space, trim};

/// What a matcher found at a position.
pub(crate) enum Found {
    /// A match that ends at this byte offset.
    End(usize),
    No,
    /// No match here, nor anywhere later: the construct opens here but its closer is not in
    /// the rest of the text. The type is not tried again.
    Never,
}

/// A matcher that remembers what it has scanned, for a pattern that could otherwise rescan the
/// same text from every position.
pub(crate) trait Scanner {
    fn scan(&mut self, src: &str, at: usize) -> Option<usize>;
}

pub(crate) enum Pattern {
    Literal(&'static str),
    /// A keyword, which Mermaid ends only at whitespace, a comment, or the end of input.
    Keyword(&'static str),
    Match(fn(&str, usize) -> Found),
    /// Makes a new scanner for each text.
    #[cfg_attr(
        not(feature = "pie"),
        allow(dead_code, reason = "read by the pie diagram")
    )]
    Scanner(fn() -> Box<dyn Scanner>),
}

pub(crate) struct TokenType {
    pub name: &'static str,
    pub pattern: Pattern,
    pub hidden: bool,
    /// The kinds of the token types that may match a longer text than this one does, as
    /// Chevrotain's `LONGER_ALT`. When one of them matches more, that one becomes the token.
    pub longer: &'static [usize],
}

pub(crate) struct Tokens<'a> {
    pub kinds: Vec<usize>,
    pub starts: Vec<usize>,
    pub ends: Vec<usize>,
    pub src: &'a str,
}

/// The kind of the token after the last one.
pub(crate) const END: usize = usize::MAX;

fn keyword(word: &str, src: &str, at: usize) -> Option<usize> {
    let rest = src[at..].strip_prefix(word)?;
    let ends = rest.is_empty() || rest.starts_with("%%") || rest.starts_with(is_space);
    ends.then_some(src.len() - rest.len())
}

pub(crate) fn tokenize<'a>(src: &'a str, types: &[TokenType]) -> Result<Tokens<'a>, Error> {
    let bytes = src.as_bytes();
    let mut tokens = Tokens {
        kinds: Vec::new(),
        starts: Vec::new(),
        ends: Vec::new(),
        src,
    };
    let mut dead = vec![false; types.len()];
    let mut scanners: Vec<Option<Box<dyn Scanner>>> = types.iter().map(|_| None).collect();
    let mut p = 0;
    'scan: while p < bytes.len() {
        for k in 0..types.len() {
            let Some(end) = match_at(types, &mut dead, &mut scanners, k, src, p) else {
                continue;
            };
            // The longer alternatives, as Chevrotain's `LONGER_ALT`: the first one that matches
            // more text than the keyword does becomes the token.
            let mut kind = k;
            let mut end = end;
            for &alt in types[k].longer {
                if let Some(alt_end) = match_at(types, &mut dead, &mut scanners, alt, src, p) {
                    if alt_end > end {
                        kind = alt;
                        end = alt_end;
                        break;
                    }
                }
            }
            if !types[kind].hidden {
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
    tokens.starts.push(bytes.len());
    tokens.ends.push(bytes.len());
    Ok(tokens)
}

/// The end of the match of a token type at a place, or nothing when it does not match there.
fn match_at(
    types: &[TokenType],
    dead: &mut [bool],
    scanners: &mut [Option<Box<dyn Scanner>>],
    k: usize,
    src: &str,
    p: usize,
) -> Option<usize> {
    if dead[k] {
        return None;
    }
    let kind = &types[k];
    let end = match kind.pattern {
        Pattern::Literal(text) => src.as_bytes()[p..]
            .starts_with(text.as_bytes())
            .then_some(p + text.len()),
        Pattern::Keyword(word) => keyword(word, src, p),
        Pattern::Match(f) => match f(src, p) {
            Found::End(end) => Some(end),
            Found::No => None,
            Found::Never => {
                dead[k] = true;
                None
            }
        },
        Pattern::Scanner(make) => scanners[k].get_or_insert_with(make).scan(src, p),
    };
    end.filter(|&end| end > p)
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

const fn token(name: &'static str, pattern: Pattern) -> TokenType {
    TokenType {
        name,
        pattern,
        hidden: false,
        longer: &[],
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

/// A token type that matches with a function.
#[cfg_attr(
    not(feature = "git"),
    allow(dead_code, reason = "read by the git diagram")
)]
pub(crate) const fn match_token(name: &'static str, f: fn(&str, usize) -> Found) -> TokenType {
    TokenType {
        name,
        pattern: Pattern::Match(f),
        hidden: false,
        longer: &[],
    }
}

/// A word that ends only at the end of a name: `commits` is a name, not the word `commit`. The
/// kinds that may match a longer text than the word does are the alternatives of `LONGER_ALT`.
#[cfg_attr(
    not(feature = "git"),
    allow(dead_code, reason = "read by the git diagram")
)]
pub(crate) const fn word_token(word: &'static str, longer: &'static [usize]) -> TokenType {
    TokenType {
        name: word,
        pattern: Pattern::Literal(word),
        hidden: false,
        longer,
    }
}

/// A keyword that Mermaid ends only at whitespace, a comment, or the end of input, with the
/// alternatives of `LONGER_ALT`.
#[cfg_attr(
    not(feature = "git"),
    allow(dead_code, reason = "read by the git diagram")
)]
pub(crate) const fn keyword_token_with(word: &'static str, longer: &'static [usize]) -> TokenType {
    TokenType {
        name: word,
        pattern: Pattern::Keyword(word),
        hidden: false,
        longer,
    }
}

pub(crate) const ACC_DESCR: TokenType = token("ACC_DESCR", Pattern::Match(acc_descr));
pub(crate) const ACC_TITLE: TokenType = token("ACC_TITLE", Pattern::Match(acc_title));
pub(crate) const TITLE: TokenType = token("TITLE", Pattern::Match(title));
pub(crate) const STRING: TokenType = token("STRING", Pattern::Match(string));
pub(crate) const NEWLINE: TokenType = token("NEWLINE", Pattern::Match(newline));
pub(crate) const WHITESPACE: TokenType = hidden("WHITESPACE", Pattern::Match(whitespace));
pub(crate) const YAML: TokenType = hidden("YAML", Pattern::Match(yaml));
pub(crate) const DIRECTIVE: TokenType = hidden("DIRECTIVE", Pattern::Match(directive));
pub(crate) const SINGLE_LINE_COMMENT: TokenType =
    hidden("SINGLE_LINE_COMMENT", Pattern::Match(single_line_comment));

#[cfg_attr(
    not(feature = "pie"),
    allow(dead_code, reason = "read by the pie diagram")
)]
pub(crate) const fn keyword_token(word: &'static str) -> TokenType {
    token(word, Pattern::Keyword(word))
}

pub(crate) const fn literal_token(text: &'static str) -> TokenType {
    token(text, Pattern::Literal(text))
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

/// `.replace(/^\s*/gm, '').replace(/\s+$/gm, '').replace(/[\t ]{2,}/gm, ' ')
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

/// Reads tokens for a hand-written parser, with Chevrotain-style messages on failure.
pub(crate) struct Reader<'a> {
    pub i: usize,
    tokens: Tokens<'a>,
    types: &'a [TokenType],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(tokens: Tokens<'a>, types: &'a [TokenType]) -> Self {
        Self {
            i: 0,
            tokens,
            types,
        }
    }

    pub(crate) fn kind(&self) -> usize {
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

    pub(crate) fn accept(&mut self, kind: usize) -> bool {
        if self.kind() != kind {
            return false;
        }
        self.i += 1;
        true
    }

    pub(crate) fn expect(&mut self, kind: usize) -> Result<&'a str, Error> {
        if self.kind() != kind {
            return Err(self.fail(&format!("token of type '{}'", self.types[kind].name)));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn end(f: fn(&str, usize) -> Found, src: &str) -> Option<usize> {
        match f(src, 0) {
            Found::End(end) => Some(end),
            _ => None,
        }
    }

    #[test]
    fn matches_accessibility_tokens() {
        assert_eq!(end(acc_descr, " accDescr: a b %% c"), Some(15));
        assert_eq!(end(acc_descr, "accDescr\n {\n x\n}y"), Some(16));
        assert_eq!(end(acc_descr, "accDescr {"), None);
        assert_eq!(end(acc_title, "accTitle : t\nx"), Some(12));
        assert_eq!(end(acc_title, "accTitle t"), None);
        assert_eq!(end(title, "title"), Some(5));
        assert_eq!(end(title, "titlex"), Some(5));
        assert_eq!(end(title, "\ttitle a %%b"), Some(9));
    }

    #[test]
    fn matches_strings() {
        assert_eq!(end(string, r#""a\"b" x"#), Some(6));
        assert_eq!(end(string, "'a\nb'"), Some(5));
        assert_eq!(end(string, "\"a\\\nb\""), None);
        assert_eq!(end(string, "\"open"), None);
        assert_eq!(end(string, "\"é\""), Some(4));
    }

    #[test]
    fn matches_blocks() {
        assert_eq!(end(yaml, "---\nx: 1\n---\nrest"), Some(13));
        assert_eq!(end(yaml, "---\n---"), Some(7));
        assert!(matches!(yaml("---\nno end", 0), Found::Never));
        assert_eq!(end(directive, "%%{init: {}}%%\nx"), Some(15));
        assert_eq!(end(directive, "%%{a}%%x}%% "), Some(11));
        assert!(matches!(directive("%%{ open", 0), Found::Never));
        assert_eq!(end(single_line_comment, " %% c\nx"), Some(5));
    }

    #[test]
    fn reads_values() {
        assert_eq!(title_value(" title  a   b  "), "a b");
        assert_eq!(acc_title_value("accTitle:  x\ty "), "x\ty");
        assert_eq!(acc_descr_value("accDescr: one"), "one");
        assert_eq!(
            acc_descr_value("accDescr {\n  a  neat\n\n  on\tlines \n}"),
            "a neat\non\tlines"
        );
        assert_eq!(multiline("a \r\n\n b"), "a\rb");
        assert_eq!(string_value(r#""a\"b\n\q""#), "a\"b\nq");
        assert_eq!(string_value("'x'"), "x");
    }

    #[test]
    fn ends_keywords_at_spaces_and_comments() {
        assert_eq!(keyword("pie", "pie", 0), Some(3));
        assert_eq!(keyword("pie", "pie%%x", 0), Some(3));
        assert_eq!(keyword("pie", "pie\u{3000}", 0), Some(3));
        assert_eq!(keyword("pie", "piex", 0), None);
        assert_eq!(keyword("pie", "pi", 0), None);
    }
}
