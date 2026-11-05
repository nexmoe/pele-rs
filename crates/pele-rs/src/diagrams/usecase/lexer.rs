//! The lexer of a use case diagram: Mermaid's Chevrotain lexer for use cases, as pele's
//! `lexer.ts` emulates it. It reads UTF-16 code units, as Chevrotain does, so that what counts
//! as one character is the same.

use crate::error::{Error, ErrorCode};

use super::tokens::T;

/// The source of a diagram, read as UTF-16 code units, with the byte offset of each of them.
pub(crate) struct Source {
    pub text: String,
    units: Vec<u16>,
    /// The byte offset of each UTF-16 code unit, and one for the end of the text.
    offsets: Vec<usize>,
}

impl Source {
    pub(crate) fn new(src: &str) -> Self {
        let mut units = Vec::with_capacity(src.len());
        let mut offsets = Vec::with_capacity(src.len() + 1);
        let mut at = 0;
        for c in src.chars() {
            for _ in 0..c.len_utf16() {
                units.push(c as u16);
                offsets.push(at);
            }
            at += c.len_utf8();
        }
        offsets.push(at);
        Self {
            text: src.to_string(),
            units,
            offsets,
        }
    }

    /// The number of UTF-16 code units of the text.
    pub(crate) fn len(&self) -> usize {
        self.units.len()
    }

    /// The code unit at `at`, or 0 past the end.
    pub(crate) fn at(&self, at: usize) -> u16 {
        self.units.get(at).copied().unwrap_or(0)
    }

    /// The text between two code unit offsets.
    pub(crate) fn slice(&self, from: usize, to: usize) -> String {
        let n = self.units.len();
        String::from_utf16_lossy(&self.units[from.min(n)..to.min(n)])
    }

    /// Whether the text at `at` starts with `word`.
    pub(crate) fn starts_with(&self, at: usize, word: &str) -> bool {
        word.bytes()
            .enumerate()
            .all(|(k, w)| self.at(at + k) == u16::from(w))
    }

    /// Where `needle` is written, at or after `from`.
    pub(crate) fn find(&self, needle: &str, from: usize) -> Option<usize> {
        let first = needle.chars().next().map_or(0, |c| c as u16);
        let mut at = from;
        while at < self.units.len() {
            if self.at(at) == first && self.starts_with(at, needle) {
                return Some(at);
            }
            at += 1;
        }
        None
    }

    /// The byte offset of a code unit offset.
    pub(crate) fn byte(&self, at: usize) -> usize {
        self.offsets[at.min(self.offsets.len() - 1)]
    }
}

/// The tokens of a source, with the offsets of each of them in UTF-16 code units.
pub(crate) struct Tokens {
    pub types: Vec<T>,
    pub starts: Vec<usize>,
    pub ends: Vec<usize>,
}

const WORD: u8 = 1;
const PUNCTUATION: u8 = 2;

/// The ASCII characters that are a word character, a CSS punctuation or neither.
fn class_of(c: u16) -> u8 {
    match c {
        48..=57 | 65..=90 | 97..=122 | 95 => WORD,
        33 | 35 | 36 | 38 | 42 | 43 | 47 | 61 | 63 | 94 | 124 | 126 => PUNCTUATION,
        _ => 0,
    }
}

fn is_digit(c: u16) -> bool {
    (48..=57).contains(&c)
}

fn is_letter(c: u16) -> bool {
    (65..=90).contains(&c) || (97..=122).contains(&c)
}

fn is_hex(c: u16) -> bool {
    is_digit(c) || (65..=70).contains(&c) || (97..=102).contains(&c)
}

fn is_blank(c: u16) -> bool {
    c == 32 || c == 9
}

fn is_word(c: u16) -> bool {
    class_of(c) & WORD != 0
}

/// Whether a code unit is a character of `\w`.
pub(crate) fn is_word_char(c: u16) -> bool {
    is_word(c)
}

/// The length of each keyword, in the order of [`T`].
fn keyword_length(t: T) -> usize {
    match t {
        T::Usecase => 12,
        T::Actor => 5,
        T::SystemBoundary => 14,
        T::End => 3,
        T::Direction => 9,
        T::Td | T::Tb | T::Bt | T::Lr | T::Rl => 2,
        T::Note => 4,
        T::For => 3,
        T::Json => 4,
        T::ClassDef => 8,
        T::Class => 5,
        T::Style => 5,
        T::Include => 7,
        T::Extend => 6,
        T::True => 4,
        T::False => 5,
        _ => 0,
    }
}

/// Mermaid's `INCLUDE` and `EXTEND` are the only keywords matched without regard to case.
fn starts_with_no_case(src: &Source, at: usize, word: &str) -> bool {
    word.bytes()
        .enumerate()
        .all(|(k, w)| src.at(at + k) | 32 == u16::from(w))
}

/// The keyword that is a prefix of the text at `at`, tried in the order of Mermaid's token list.
fn keyword_at(src: &Source, at: usize, c: u16) -> Option<T> {
    let starts = |word: &str| src.starts_with(at, word);
    match c as u8 as char {
        'a' => {
            if starts("actor") {
                Some(T::Actor)
            } else {
                None
            }
        }
        's' => {
            if starts("systemBoundary") {
                Some(T::SystemBoundary)
            } else if starts("style") {
                Some(T::Style)
            } else {
                None
            }
        }
        'e' => {
            if starts("end") {
                Some(T::End)
            } else if starts_with_no_case(src, at, "extend") {
                Some(T::Extend)
            } else {
                None
            }
        }
        'E' => {
            if starts_with_no_case(src, at, "extend") {
                Some(T::Extend)
            } else {
                None
            }
        }
        'd' => {
            if starts("direction") {
                Some(T::Direction)
            } else {
                None
            }
        }
        'T' => {
            if starts("TD") {
                Some(T::Td)
            } else if starts("TB") {
                Some(T::Tb)
            } else {
                None
            }
        }
        'B' => {
            if starts("BT") {
                Some(T::Bt)
            } else {
                None
            }
        }
        'L' => {
            if starts("LR") {
                Some(T::Lr)
            } else {
                None
            }
        }
        'R' => {
            if starts("RL") {
                Some(T::Rl)
            } else {
                None
            }
        }
        'n' => {
            if starts("note") {
                Some(T::Note)
            } else {
                None
            }
        }
        'f' => {
            if starts("for") {
                Some(T::For)
            } else if starts("false") {
                Some(T::False)
            } else {
                None
            }
        }
        'j' => {
            if starts("json") {
                Some(T::Json)
            } else {
                None
            }
        }
        'c' => {
            if starts("classDef") {
                Some(T::ClassDef)
            } else if starts("class") {
                Some(T::Class)
            } else {
                None
            }
        }
        'i' | 'I' => {
            if starts_with_no_case(src, at, "include") {
                Some(T::Include)
            } else {
                None
            }
        }
        't' => {
            if starts("true") {
                Some(T::True)
            } else {
                None
            }
        }
        'u' => {
            if starts("usecase-beta") {
                Some(T::Usecase)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The line and column of an offset, counted in UTF-16 code units as JavaScript counts them.
pub(crate) fn line_column(src: &Source, offset: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut line_start = 0usize;
    let mut i = 0usize;
    while i < offset {
        let c = src.at(i);
        if c != 10 && c != 13 {
            i += 1;
            continue;
        }
        if c == 13 && i + 1 < offset && src.at(i + 1) == 10 {
            i += 1;
        }
        line += 1;
        line_start = i + 1;
        i += 1;
    }
    (line, (offset - line_start + 1) as u32)
}

/// Where a span of the source is, as the error messages of the parser write it.
pub(crate) fn location_text(src: &Source, start: usize, end: usize) -> String {
    let (line, column) = line_column(src, start);
    format!("line {line}, column {column} [{start},{end})")
}

/// The end of a quoted string that opens at `at`, or `None` when the line ends first.
fn string_end(src: &Source, at: usize, quote: u16) -> Option<usize> {
    let mut k = at + 1;
    while k < src.len() {
        let c = src.at(k);
        if c == quote {
            return Some(k + 1);
        }
        if c == 10 || c == 13 {
            return None;
        }
        k += 1;
    }
    None
}

fn starts_token(src: &Source, at: usize) -> bool {
    let c = src.at(at);
    if c != 34 && c != 39 {
        return true;
    }
    (c == 34 && src.at(at + 1) == 96) || string_end(src, at, c).is_some()
}

/// Mermaid reports the first character no token starts with, and how many characters its lexer
/// skipped before one did.
fn lex_error(src: &Source, offset: usize, length: usize) -> Error {
    let (line, column) = line_column(src, offset);
    let character = src.slice(offset, offset + 1);
    Error::new(
        ErrorCode::Syntax,
        format!(
            "Error lexing usecase diagram: unexpected character: ->{character}<- at offset: {offset}, \
             skipped {length} characters. at line {line}, column {column} [{offset},{})",
            offset + length
        ),
    )
    .at(&src.text, src.byte(offset))
}

/// Produces the tokens of Mermaid's Chevrotain lexer for use case diagrams: the first pattern in
/// its list that matches wins, a keyword gives way to a longer identifier, and `json id@` and
/// `<<` switch to modes that read a JSON object and a stereotype.
pub(crate) fn tokenize(src: &Source) -> Result<Tokens, Error> {
    let mut types: Vec<T> = Vec::new();
    let mut starts: Vec<usize> = Vec::new();
    let mut ends: Vec<usize> = Vec::new();
    let n = src.len();
    let mut i = 0usize;
    // Whether only blanks came before on this line, where comments and accessibility lines may
    // start.
    let mut line_start = true;
    let mut no_brace = false;

    while i < n {
        let c = src.at(i);
        if c == 32 || c == 9 {
            i += 1;
            continue;
        }
        let mut kind: T;
        let mut end = i + 1;
        let mut fresh = false;

        if is_word(c) {
            kind = T::Eof;
            if c == 97 && line_start && src.starts_with(i, "acc") {
                let descr = src.starts_with(i + 3, "Descr");
                if descr || src.starts_with(i + 3, "Title") {
                    let mut j = i + 8;
                    while is_blank(src.at(j)) {
                        j += 1;
                    }
                    let next = src.at(j);
                    if next == 58 {
                        kind = if descr {
                            T::AccDescrLine
                        } else {
                            T::AccTitleLine
                        };
                        end = j + 1;
                        while end < n && src.at(end) != 10 && src.at(end) != 13 {
                            end += 1;
                        }
                    } else if next == 123 && descr && !no_brace {
                        match src.find("}", j + 1) {
                            None => no_brace = true,
                            Some(at) => {
                                kind = T::AccDescrBlock;
                                end = at + 1;
                            }
                        }
                    }
                }
            } else if c == 106 && src.starts_with(i, "json") && is_blank(src.at(i + 4)) {
                let mut j = i + 5;
                while is_blank(src.at(j)) {
                    j += 1;
                }
                let id = j;
                while is_word(src.at(j)) {
                    j += 1;
                }
                if j > id {
                    while is_blank(src.at(j)) {
                        j += 1;
                    }
                    if src.at(j) == 64 {
                        j += 1;
                        while is_blank(src.at(j)) {
                            j += 1;
                        }
                        if src.at(j) == 123 {
                            types.push(T::JsonDeclarationStart);
                            starts.push(i);
                            ends.push(j);
                            let mut depth = 0i32;
                            let mut quoted = false;
                            let mut escaped = false;
                            let mut k = j;
                            while k < n {
                                let ch = src.at(k);
                                if quoted {
                                    if escaped {
                                        escaped = false;
                                    } else if ch == 92 {
                                        escaped = true;
                                    } else if ch == 34 {
                                        quoted = false;
                                    }
                                } else if ch == 34 {
                                    quoted = true;
                                } else if ch == 123 {
                                    depth += 1;
                                } else if ch == 125 {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                k += 1;
                            }
                            types.push(if k < n {
                                T::JsonObjectLiteral
                            } else {
                                T::UnclosedJsonObjectLiteral
                            });
                            starts.push(j);
                            i = if k < n { k + 1 } else { n };
                            ends.push(i);
                            line_start = false;
                            continue;
                        }
                    }
                }
            }
            if kind == T::Eof {
                while end < n && is_word(src.at(end)) {
                    end += 1;
                }
                match keyword_at(src, i, c) {
                    Some(keyword) => {
                        // A keyword that is only the start of a longer word is an identifier.
                        if keyword == T::Usecase || end - i == keyword_length(keyword) {
                            kind = keyword;
                            end = i + keyword_length(keyword);
                        } else {
                            kind = T::Identifier;
                        }
                    }
                    None => {
                        if (c == 111 || c == 120) && src.at(i + 1) == 45 && src.at(i + 2) == 45 {
                            kind = if c == 111 {
                                T::BackwardCircle
                            } else {
                                T::BackwardCross
                            };
                            end = i + 3;
                        } else if is_digit(c) {
                            // `1mg` is an identifier, and `1.5px` is a number because the identifier
                            // stops at the dot.
                            let mut digits = i + 1;
                            while is_digit(src.at(digits)) {
                                digits += 1;
                            }
                            if src.at(digits) == 46 && is_digit(src.at(digits + 1)) {
                                kind = T::Number;
                                end = digits + 2;
                                while is_digit(src.at(end)) {
                                    end += 1;
                                }
                                while is_letter(src.at(end)) {
                                    end += 1;
                                }
                            } else {
                                kind = T::Identifier;
                            }
                        } else {
                            kind = T::Identifier;
                            while src.at(end) == 45 && is_word(src.at(end + 1)) {
                                kind = T::CssIdentifier;
                                end += 2;
                                while is_word(src.at(end)) {
                                    end += 1;
                                }
                            }
                        }
                    }
                }
            }
        } else if c == 10 {
            kind = T::Newline;
            fresh = true;
        } else if c == 13 {
            kind = T::Newline;
            fresh = true;
            if src.at(end) == 10 {
                end += 1;
            }
        } else if c == 34 || c == 39 {
            if c == 34 && src.at(i + 1) == 96 {
                let close = src.find("`\"", i + 2);
                kind = match close {
                    None => T::UnclosedMarkdownString,
                    Some(_) => T::MarkdownString,
                };
                end = match close {
                    None => n,
                    Some(at) => at + 2,
                };
            } else {
                kind = T::PlainString;
                match string_end(src, i, c) {
                    None => {
                        let mut resume = i + 1;
                        while resume < n && !starts_token(src, resume) {
                            resume += 1;
                        }
                        return Err(lex_error(src, i, resume - i));
                    }
                    Some(at) => end = at,
                }
            }
        } else if c == 37 {
            if line_start && src.at(end) == 37 {
                kind = T::Comment;
                while end < n && src.at(end) != 10 && src.at(end) != 13 {
                    end += 1;
                }
            } else {
                kind = T::Percent;
            }
        } else if c == 45 {
            if src.at(end) != 45 {
                kind = T::Dash;
            } else {
                while src.at(end) == 45 {
                    end += 1;
                }
                let next = src.at(end);
                let two = end - i == 2;
                if two && next == 124 && src.at(end + 1) == 62 {
                    kind = T::Generalization;
                    end += 2;
                } else if next == 62 {
                    kind = T::ForwardSolid;
                    end += 1;
                } else if two && (next == 111 || next == 120) {
                    kind = if next == 111 {
                        T::ForwardCircle
                    } else {
                        T::ForwardCross
                    };
                    end += 1;
                } else {
                    kind = T::MarkerlessSolid;
                }
            }
        } else if c == 46 {
            let next = src.at(end);
            if next == 46 && src.at(end + 1) == 62 {
                kind = T::DependencyArrow;
                end += 2;
            } else if is_digit(next) {
                kind = T::Number;
                while is_digit(src.at(end)) {
                    end += 1;
                }
                while is_letter(src.at(end)) {
                    end += 1;
                }
            } else {
                kind = T::Dot;
            }
        } else if c == 60 {
            if src.at(end) == 60 {
                types.push(T::StereotypeStart);
                starts.push(i);
                ends.push(i + 2);
                i += 2;
                if i >= n {
                    break;
                }
                if src.at(i) == 62 && src.at(i + 1) == 62 {
                    kind = T::StereotypeEnd;
                    end = i + 2;
                } else {
                    let mut close = i;
                    let mut found = false;
                    while close < n {
                        let ch = src.at(close);
                        if ch == 10 || ch == 13 {
                            break;
                        }
                        if ch == 62 && src.at(close + 1) == 62 {
                            found = true;
                            break;
                        }
                        close += 1;
                    }
                    if found && !src.slice(i, close).trim().is_empty() {
                        types.push(T::StereotypeText);
                        starts.push(i);
                        ends.push(close);
                        kind = T::StereotypeEnd;
                        i = close;
                        end = close + 2;
                    } else {
                        while close < n && src.at(close) != 10 && src.at(close) != 13 {
                            close += 1;
                        }
                        if close == i {
                            while close < n && (src.at(close) == 10 || src.at(close) == 13) {
                                close += 1;
                            }
                            return Err(lex_error(src, i, close - i));
                        }
                        kind = T::UnclosedStereotypeText;
                        end = close;
                    }
                }
            } else if src.at(end) == 45 && src.at(end + 1) == 45 {
                kind = T::BackwardSolid;
                end += 2;
                while src.at(end) == 45 {
                    end += 1;
                }
            } else {
                kind = T::LabelPunctuation;
            }
        } else if c == 58 {
            if src.at(end) == 58 && src.at(end + 1) == 58 {
                kind = T::ClassSeparator;
                end += 2;
            } else {
                kind = T::Colon;
            }
        } else if c == 64 {
            if src.at(end) == 123 {
                kind = T::MetadataStart;
                end += 1;
            } else {
                kind = T::At;
            }
        } else if c == 123 {
            kind = T::LBrace;
        } else if c == 125 {
            kind = T::RBrace;
        } else if c == 91 {
            kind = T::LBracket;
        } else if c == 93 {
            kind = T::RBracket;
        } else if c == 40 {
            kind = T::LParen;
        } else if c == 41 {
            kind = T::RParen;
        } else if c == 44 {
            kind = T::Comma;
        } else if c == 35 {
            while is_hex(src.at(end)) {
                end += 1;
            }
            kind = if end > i + 1 {
                T::HashColor
            } else {
                T::CssPunctuation
            };
        } else if c == 92 {
            if src.at(end) == 44 {
                kind = T::CssEscapedComma;
                end += 1;
            } else {
                kind = T::LabelPunctuation;
            }
        } else if c < 127 && c > 32 {
            kind = if class_of(c) & PUNCTUATION != 0 {
                T::CssPunctuation
            } else {
                T::LabelPunctuation
            };
        } else {
            kind = T::LabelSymbol;
            while end < n {
                let ch = src.at(end);
                if (32..127).contains(&ch) || ch == 9 || ch == 10 || ch == 13 {
                    break;
                }
                end += 1;
            }
        }

        types.push(kind);
        starts.push(i);
        ends.push(end);
        i = end;
        line_start = fresh;
    }

    for _ in 0..3 {
        types.push(T::Eof);
        starts.push(n);
        ends.push(n);
    }
    Ok(Tokens {
        types,
        starts,
        ends,
    })
}
