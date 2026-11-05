//! Lexer rules that Mermaid's journey and timeline grammars have in common, ported from pele's
//! `diagrams/common/scan.ts`: comments, whitespace, and the accTitle and accDescr statements.
//! The text is read as UTF-16 code units, as Jison reads it.

use crate::util::is_space;

/// What `at` returns past the end of the text.
pub(crate) const NONE: u32 = u32::MAX;

/// Tokens as parallel lists: a type, and where its text starts and ends.
#[derive(Debug, Default)]
pub(crate) struct Tokens {
    pub types: Vec<u8>,
    pub starts: Vec<usize>,
    pub ends: Vec<usize>,
}

impl Tokens {
    pub(crate) fn push(&mut self, kind: u8, start: usize, end: usize) {
        self.types.push(kind);
        self.starts.push(start);
        self.ends.push(end);
    }
}

/// The code unit at `p`, or `NONE` past the end.
pub(crate) fn at(src: &[u16], p: usize) -> u32 {
    src.get(p).map_or(NONE, |&c| u32::from(c))
}

/// JavaScript's `\s`, for one code unit.
pub(crate) fn space(c: u32) -> bool {
    char::from_u32(c).is_some_and(is_space)
}

/// `[A-Za-z0-9_]`, for one code unit.
pub(crate) fn word(c: u32) -> bool {
    char::from_u32(c).is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Where the line holding `from` ends.
pub(crate) fn line_end(src: &[u16], from: usize) -> usize {
    src.get(from..)
        .and_then(|rest| rest.iter().position(|&c| c == 10))
        .map_or(src.len(), |k| from + k)
}

/// Whether `word`, lowercase ASCII letters, is at `p`; the grammars are case-insensitive.
pub(crate) fn keyword_at(src: &[u16], p: usize, word: &str) -> bool {
    word.bytes()
        .enumerate()
        .all(|(i, b)| at(src, p + i) != NONE && (at(src, p + i) | 32) == u32::from(b))
}

/// Skips comments and whitespace. Stops at a line break that starts a token, which is a
/// NEWLINE. Jison compiles `\%%` to a single `%`, so one percent sign starts a comment, and the
/// second comment rule takes any character before `%%` with it, a line break included.
pub(crate) fn skip_trivia(src: &[u16], from: usize) -> usize {
    let n = src.len();
    let mut p = from;
    while p < n {
        let c = at(src, p);
        if c == 37 && at(src, p + 1) != 123 {
            p = line_end(src, p);
        } else if c != 125 && at(src, p + 1) == 37 && at(src, p + 2) == 37 {
            p = line_end(src, p + 3);
        } else if c == 10 {
            return p;
        } else if space(c) {
            p += 1;
            while p < n && space(at(src, p)) {
                p += 1;
            }
        } else if c == 35 {
            p = line_end(src, p);
        } else {
            return p;
        }
    }
    p
}

fn skip_space(src: &[u16], from: usize) -> usize {
    let mut p = from;
    while space(at(src, p)) {
        p += 1;
    }
    p
}

/// Lexes an accTitle or accDescr statement at `p` and returns where it ends, or `p` if there
/// is none. The token types acc_title, acc_title_value, acc_descr, acc_descr_value and
/// acc_descr_multiline_value must be numbered consecutively from `first`.
pub(crate) fn accessibility(src: &[u16], p: usize, out: &mut Tokens, first: u8) -> usize {
    let descr = keyword_at(src, p, "accdescr");
    if !descr && !keyword_at(src, p, "acctitle") {
        return p;
    }
    let n = src.len();
    let mut q = skip_space(src, p + 8);
    let c = at(src, q);
    if c == 58 {
        q = skip_space(src, q + 1);
        let end = line_end(src, q);
        out.push(if descr { first + 2 } else { first }, p, q);
        out.push(if descr { first + 3 } else { first + 1 }, q, end);
        return end;
    }
    if !descr || c != 123 {
        return p;
    }
    q = skip_space(src, q + 1);
    while at(src, q) != 125 {
        let end = src
            .get(q..)
            .and_then(|rest| rest.iter().position(|&c| c == 125))
            .map_or(n, |k| q + k);
        out.push(first + 4, q, end);
        if q >= n {
            return n;
        }
        q = end;
    }
    q + 1
}

/// The byte offset in `src` of a UTF-16 offset.
pub(crate) fn byte_offset(src: &str, units: usize) -> usize {
    let mut count = 0;
    for (i, c) in src.char_indices() {
        if count >= units {
            return i;
        }
        count += c.len_utf16();
    }
    src.len()
}
