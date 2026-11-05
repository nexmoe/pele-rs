//! The tokenizer of an event modeling diagram: the tokens of Mermaid's grammar, tried in the
//! order it writes them, as Chevrotain does. It is the part of `diagrams/common/tokens.rs`
//! that this type needs, so that the type builds with its own feature alone.

use crate::error::{Error, ErrorCode};
use crate::util::is_space;

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
    Match(fn(&str, usize) -> Found),
    /// Makes a new scanner for each text.
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

/// Reads the tokens of a text. The token types are tried in order and the first match wins, as
/// in Chevrotain.
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

/// A token type that matches with a function.
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
pub(crate) const fn word_token(word: &'static str, longer: &'static [usize]) -> TokenType {
    TokenType {
        name: word,
        pattern: Pattern::Literal(word),
        hidden: false,
        longer,
    }
}

pub(crate) const fn literal_token(text: &'static str) -> TokenType {
    TokenType {
        name: text,
        pattern: Pattern::Literal(text),
        hidden: false,
        longer: &[],
    }
}

/// A token type that is not part of the diagram.
pub(crate) const fn hidden_token(name: &'static str, pattern: Pattern) -> TokenType {
    TokenType {
        name,
        pattern,
        hidden: true,
        longer: &[],
    }
}

/// `\s+`
pub(crate) fn whitespace(src: &str, at: usize) -> Found {
    let mut end = at;
    for (i, c) in src[at..].char_indices() {
        if !is_space(c) {
            break;
        }
        end = at + i + c.len_utf8();
    }
    if end > at { Found::End(end) } else { Found::No }
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

/// `\r?\n` at `at`.
fn line_break(src: &str, at: usize) -> Option<usize> {
    match &src.as_bytes()[at..] {
        [b'\r', b'\n', ..] => Some(at + 2),
        [b'\n', ..] => Some(at + 1),
        _ => None,
    }
}

/// `(?:\r?\n|(?!\S))` at `at`: a line break, or the end, or whitespace that is not taken.
pub(crate) fn block_end(src: &str, at: usize) -> Option<usize> {
    if let Some(end) = line_break(src, at) {
        return Some(end);
    }
    let rest = &src[at..];
    (rest.is_empty() || rest.starts_with(is_space)).then_some(at)
}

/// `---[\t ]*\r?\n(?:[\S\s]*?\r?\n)?---(?:\r?\n|(?!\S))`
pub(crate) fn yaml(src: &str, at: usize) -> Found {
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
    // The content of the block may be empty, so the fence may follow the header at once.
    if let Some(end) = closes(body) {
        return Found::End(end);
    }
    for k in body..src.len() {
        if let Some(end) = line_break(src, k).and_then(closes) {
            return Found::End(end);
        }
    }
    Found::Never
}

/// `[\t ]*%%{[\S\s]*?}%%(?:\r?\n|(?!\S))`
pub(crate) fn directive(src: &str, at: usize) -> Found {
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
pub(crate) fn single_line_comment(src: &str, at: usize) -> Found {
    let at = spaces_tabs(src, at);
    if !src[at..].starts_with("%%") {
        return Found::No;
    }
    Found::End(line_end(src, at))
}
