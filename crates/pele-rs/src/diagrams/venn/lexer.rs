//! The lexer of a venn diagram: Mermaid's jison lexer for `venn.jison`, as pele's `lexer.ts`
//! emulates it. It reads UTF-16 code units, as the jison lexer does, so that what counts as
//! one character is the same as in the grammar.

use crate::util::{is_space, is_word};

/// The tokens of the venn grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum T {
    /// Not a jison token: the lexer has nothing more to give.
    End,
    /// The text does not fit the lexical rules of the grammar.
    Error,
    Eof,
    Newline,
    Venn,
    Title,
    Set,
    Union,
    Text,
    /// An indented `text` line, which belongs to the set or union before it.
    IndentText,
    Style,
    BracketLabel,
    Numeric,
    HexColor,
    RgbColor,
    RgbaColor,
    Identifier,
    String,
    Comma,
    Colon,
}

impl T {
    /// The token's name, as the error messages of the parser name it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            T::End => "$end",
            T::Error => "INVALID",
            T::Eof => "EOF",
            T::Newline => "NEWLINE",
            T::Venn => "VENN",
            T::Title => "TITLE",
            T::Set => "SET",
            T::Union => "UNION",
            T::Text => "TEXT",
            T::IndentText => "INDENT_TEXT",
            T::Style => "STYLE",
            T::BracketLabel => "BRACKET_LABEL",
            T::Numeric => "NUMERIC",
            T::HexColor => "HEXCOLOR",
            T::RgbColor => "RGBCOLOR",
            T::RgbaColor => "RGBACOLOR",
            T::Identifier => "IDENTIFIER",
            T::String => "STRING",
            T::Comma => "COMMA",
            T::Colon => "COLON",
        }
    }
}

/// The keywords of the grammar, and the tokens they stand for. The order is the order of the
/// rules of Mermaid's lexer: the first that matches wins.
const KEYWORDS: &[(&str, T)] = &[
    ("venn-beta", T::Venn),
    ("set", T::Set),
    ("union", T::Union),
    ("text", T::Text),
    ("style", T::Style),
];

/// A digit of a hexadecimal colour.
fn is_hex(c: char) -> bool {
    c.is_ascii_hexdigit()
}

/// Reads the tokens of a venn diagram, one at a time.
pub(crate) struct Lexer {
    units: Vec<u16>,
    /// The byte offset of each UTF-16 code unit, and one for the end of the text.
    offsets: Vec<usize>,
    /// The place the next token starts at, in UTF-16 code units.
    at: usize,
    /// The text of the token that was read last.
    text: String,
    /// The start of the token that was read last, in UTF-16 code units.
    start: usize,
    /// Whether the lexer is at the start of a line.
    line_start: bool,
    /// Whether an indented `text` line belongs to the set or union before it.
    indent: bool,
    /// Whether the text has run out.
    ended: bool,
}

impl Lexer {
    pub(crate) fn new(src: &str) -> Self {
        let units: Vec<u16> = src.encode_utf16().collect();
        let mut offsets = Vec::with_capacity(units.len() + 1);
        let mut at = 0;
        for c in src.chars() {
            for _ in 0..c.len_utf16() {
                offsets.push(at);
            }
            at += c.len_utf8();
        }
        offsets.push(at);
        Self {
            units,
            offsets,
            at: 0,
            text: String::new(),
            start: 0,
            line_start: false,
            indent: false,
            ended: false,
        }
    }

    /// The text of the token that was read last.
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// The byte offset of the token that was read last.
    pub(crate) fn start_byte(&self) -> usize {
        self.offsets[self.start.min(self.offsets.len() - 1)]
    }

    /// Sets whether an indented `text` line belongs to the set or union before it.
    pub(crate) fn set_indent(&mut self, on: bool) {
        self.indent = on;
    }

    /// The character at a place, or nothing past the end of the text.
    fn unit(&self, at: usize) -> Option<char> {
        self.units
            .get(at)
            .copied()
            .and_then(|u| char::from_u32(u as u32))
    }

    /// The text between two places.
    fn slice(&self, from: usize, to: usize) -> String {
        let len = self.units.len();
        String::from_utf16_lossy(&self.units[from.min(len)..to.min(len)])
    }

    /// The place the line that holds `from` ends at.
    fn line_end(&self, from: usize) -> usize {
        let mut at = from;
        while at < self.units.len() && self.units[at] != b'\n' as u16 {
            at += 1;
        }
        at
    }

    /// The place of the next `"` at or after `from`.
    fn find_quote(&self, from: usize) -> Option<usize> {
        let mut at = from;
        while at < self.units.len() {
            if self.units[at] == b'"' as u16 {
                return Some(at);
            }
            at += 1;
        }
        None
    }

    /// Whether the text at a place is a word, ignoring ASCII case.
    fn word_at(&self, at: usize, word: &str) -> bool {
        word.bytes().enumerate().all(|(k, w)| {
            self.unit(at + k).is_some_and(|c| {
                c == w as char || (w.is_ascii_lowercase() && c == (w as char).to_ascii_uppercase())
            })
        })
    }

    /// Whether the text at a place is a word that no name continues.
    fn keyword(&self, at: usize, word: &str) -> bool {
        self.word_at(at, word) && !self.unit(at + word.len()).is_some_and(is_word)
    }

    /// The end of `rgb(1, 2, 3)` or `rgba(1, 2, 3, 0.5)` whose parenthesis opens at `at`, or
    /// nothing when the colour is not one.
    fn color(&self, at: usize, parts: usize) -> Option<usize> {
        let mut i = at + 1;
        for part in 0..parts {
            while self.unit(i).is_some_and(is_space) {
                i += 1;
            }
            let from = i;
            while self.unit(i).is_some_and(|c| c.is_ascii_digit() || c == '.') {
                i += 1;
            }
            if i == from {
                return None;
            }
            while self.unit(i).is_some_and(is_space) {
                i += 1;
            }
            let close = if part == parts - 1 { ')' } else { ',' };
            if self.unit(i) != Some(close) {
                return None;
            }
            i += 1;
        }
        Some(i)
    }

    /// A token that ends at `end`, with the text it covers.
    fn token(&mut self, kind: T, end: usize) -> T {
        self.text = self.slice(self.at, end);
        self.at = end;
        kind
    }

    /// The next token of the source.
    pub(crate) fn next(&mut self) -> T {
        let n = self.units.len();
        loop {
            let mut i = self.at;
            self.start = i;
            self.text.clear();
            if i >= n {
                if self.ended {
                    return T::End;
                }
                self.ended = true;
                return T::Eof;
            }
            let unit = self.units[i];
            let Some(c) = char::from_u32(unit as u32) else {
                return T::Error;
            };
            // A comment runs to the end of the line. It also takes the character before it along,
            // whatever that is: a line break, or the last letter of a word.
            if (c == '%' && self.unit(i + 1) == Some('%') && self.unit(i + 2) != Some('{'))
                || (c != '}' && self.unit(i + 1) == Some('%') && self.unit(i + 2) == Some('%'))
            {
                self.at = self.line_end(i + 2);
                continue;
            }
            if self.line_start {
                if c == ' ' || c == '\t' {
                    let mut j = i + 1;
                    while self.unit(j) == Some(' ') || self.unit(j) == Some('\t') {
                        j += 1;
                    }
                    self.at = j;
                    if self.keyword(j, "text") && self.indent {
                        // The word `text` itself is dropped; the indentation stands for it.
                        self.at = j + 4;
                        self.line_start = false;
                        self.text = self.slice(i, j);
                        return T::IndentText;
                    }
                    continue;
                }
                if c != '\n' && c != '\r' {
                    self.indent = false;
                    self.line_start = false;
                }
            }
            if c == '\n' || c == '\r' {
                i += 1;
                while self.unit(i) == Some('\n') || self.unit(i) == Some('\r') {
                    i += 1;
                }
                self.line_start = true;
                return self.token(T::Newline, i);
            }
            if c == '%' && self.unit(i + 1) == Some('%') {
                self.at = self.line_end(i);
                continue;
            }
            if c == ' ' || c == '\t' {
                i += 1;
                while self.unit(i) == Some(' ') || self.unit(i) == Some('\t') {
                    i += 1;
                }
                self.at = i;
                continue;
            }

            if c.is_ascii_alphabetic() || c == '_' {
                // The title runs to the end of the line, or to a `#` or a `;`.
                if self.word_at(i, "title") && self.unit(i + 5).is_some_and(is_space) {
                    let mut end = i + 6;
                    while end < n {
                        let ch = self.units[end];
                        if ch == b'#' as u16 || ch == b'\n' as u16 || ch == b';' as u16 {
                            break;
                        }
                        end += 1;
                    }
                    if end > i + 6 {
                        return self.token(T::Title, end);
                    }
                }
                for (word, kind) in KEYWORDS {
                    if self.keyword(i, word) {
                        return self.token(*kind, i + word.len());
                    }
                }
                if self.word_at(i, "rgb") {
                    let alpha = self
                        .unit(i + 3)
                        .is_some_and(|c| c.eq_ignore_ascii_case(&'a'));
                    let open = i + if alpha { 4 } else { 3 };
                    if self.unit(open) == Some('(') {
                        if let Some(end) = self.color(open, if alpha { 4 } else { 3 }) {
                            return self.token(if alpha { T::RgbaColor } else { T::RgbColor }, end);
                        }
                    }
                }
                i += 1;
                while self.unit(i).is_some_and(|c| is_word(c) || c == '-') {
                    i += 1;
                }
                return self.token(T::Identifier, i);
            }
            if c == '[' {
                // A label in brackets: `["text"]` or `[text]`.
                if self.unit(i + 1) == Some('"') {
                    let Some(quote) = self.find_quote(i + 2) else {
                        return T::Error;
                    };
                    if self.unit(quote + 1) != Some(']') {
                        return T::Error;
                    }
                    self.at = quote + 2;
                    self.text = self.slice(i + 2, quote);
                    return T::BracketLabel;
                }
                let mut end = i + 1;
                while end < n && self.units[end] != b']' as u16 && self.units[end] != b'"' as u16 {
                    end += 1;
                }
                if end == i + 1 || self.unit(end) != Some(']') {
                    return T::Error;
                }
                self.at = end + 1;
                self.text = crate::util::trim(&self.slice(i + 1, end)).to_string();
                return T::BracketLabel;
            }
            if c.is_ascii_digit() || c == '.' || c == '+' || c == '-' {
                if c == '+' || c == '-' {
                    i += 1;
                }
                let from = i;
                while self.unit(i).is_some_and(|c| c.is_ascii_digit()) {
                    i += 1;
                }
                if i > from {
                    if self.unit(i) == Some('.')
                        && self.unit(i + 1).is_some_and(|c| c.is_ascii_digit())
                    {
                        i += 2;
                        while self.unit(i).is_some_and(|c| c.is_ascii_digit()) {
                            i += 1;
                        }
                    }
                } else if self.unit(i) == Some('.')
                    && self.unit(i + 1).is_some_and(|c| c.is_ascii_digit())
                {
                    i += 2;
                    while self.unit(i).is_some_and(|c| c.is_ascii_digit()) {
                        i += 1;
                    }
                } else {
                    return T::Error;
                }
                return self.token(T::Numeric, i);
            }
            if c == '#' {
                let mut end = i + 1;
                while end < i + 9 && self.unit(end).is_some_and(is_hex) {
                    end += 1;
                }
                return if end >= i + 4 {
                    self.token(T::HexColor, end)
                } else {
                    T::Error
                };
            }
            if c == '"' {
                let Some(quote) = self.find_quote(i + 1) else {
                    return T::Error;
                };
                return self.token(T::String, quote + 1);
            }
            if c == ',' {
                return self.token(T::Comma, i + 1);
            }
            if c == ':' {
                return self.token(T::Colon, i + 1);
            }
            return T::Error;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(src: &str) -> Vec<(T, String)> {
        let mut lexer = Lexer::new(src);
        let mut out = Vec::new();
        loop {
            let kind = lexer.next();
            if kind == T::End || kind == T::Eof {
                break;
            }
            out.push((kind, lexer.text().to_string()));
        }
        out
    }

    #[test]
    fn reads_the_words_of_the_grammar() {
        let out = tokens("venn-beta\n  set A\n");
        assert_eq!(out[0].0, T::Venn);
        assert_eq!(out[1].0, T::Newline);
        assert_eq!(out[2].0, T::Set);
        assert_eq!(out[3], (T::Identifier, "A".to_string()));
    }

    #[test]
    fn ignores_case() {
        let out = tokens("Venn-Beta\n  SET A\n");
        assert_eq!(out[0].0, T::Venn);
        assert_eq!(out[2].0, T::Set);
    }

    #[test]
    fn reads_a_title() {
        let out = tokens("venn-beta\n  title What a good feature\n");
        assert_eq!(out[2].0, T::Title);
        assert_eq!(out[2].1, "title What a good feature");
    }

    #[test]
    fn reads_the_labels_in_brackets() {
        let out = tokens("venn-beta\n  set A[\"Alpha\"]\n");
        assert_eq!(out[4], (T::BracketLabel, "Alpha".to_string()));
        let out = tokens("venn-beta\n  set A[  Alpha  ]\n");
        assert_eq!(out[4], (T::BracketLabel, "Alpha".to_string()));
    }

    #[test]
    fn reads_the_colours() {
        let out = tokens("venn-beta\n  style A fill:rgb(1, 2, 3)\n");
        assert!(
            out.iter()
                .any(|(k, t)| *k == T::RgbColor && t == "rgb(1, 2, 3)")
        );
        let out = tokens("venn-beta\n  style A fill:rgba(1, 2, 3, 0.5)\n");
        assert!(
            out.iter()
                .any(|(k, t)| *k == T::RgbaColor && t == "rgba(1, 2, 3, 0.5)")
        );
        let out = tokens("venn-beta\n  style A fill:#ff0000\n");
        assert!(out.iter().any(|(k, t)| *k == T::HexColor && t == "#ff0000"));
    }

    #[test]
    fn skips_the_comments() {
        let out = tokens("venn-beta\n%% a comment\n  set A\n");
        assert!(out.iter().any(|(k, t)| *k == T::Set && t == "set"));
    }

    #[test]
    fn reads_an_indented_text_line() {
        let mut lexer = Lexer::new("venn-beta\n  set A\n    text A1\n");
        let mut out = Vec::new();
        loop {
            let kind = lexer.next();
            if kind == T::End || kind == T::Eof {
                break;
            }
            out.push((kind, lexer.text().to_string()));
            // The parser tells the lexer that a set was declared, so that an indented `text`
            // line belongs to it.
            if kind == T::Identifier {
                lexer.set_indent(true);
            }
        }
        // The indented `text` line is its own token, and the word `text` is dropped.
        assert!(out.iter().any(|(k, _)| *k == T::IndentText));
        assert!(!out.iter().any(|(k, t)| *k == T::Text && t == "text"));
    }
}
