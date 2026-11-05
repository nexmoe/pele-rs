//! The lexer of an Ishikawa diagram: Mermaid's jison lexer for `ishikawa.jison`, as pele's
//! `lexer.ts` emulates it. The jison lexer reads the text as UTF-16 code units; every character
//! the grammar counts here fits in one code unit, so the counts are the same.

use crate::util::{is_line_end, is_space, is_word};

/// The tokens of the Ishikawa grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum T {
    /// A comment, or white space that ends in a line break.
    SpaceLine,
    /// A line break on its own.
    Nl,
    /// The keyword of the diagram.
    Ishikawa,
    /// The indentation of a line.
    SpaceList,
    /// The text of a line.
    Text,
    /// The end of the text.
    Eof,
}

impl T {
    /// The name of the token, as the parser names it in its errors.
    pub(crate) fn name(self) -> &'static str {
        match self {
            T::SpaceLine => "SPACELINE",
            T::Nl => "NL",
            T::Ishikawa => "ISHIKAWA",
            T::SpaceList => "SPACELIST",
            T::Text => "TEXT",
            T::Eof => "EOF",
        }
    }
}

/// The tokens of a source, and where each of them starts and ends.
#[derive(Debug, Default)]
pub(crate) struct Tokens {
    pub kinds: Vec<T>,
    /// The byte offset each token starts at.
    pub starts: Vec<usize>,
    /// The byte offset each token ends at.
    pub ends: Vec<usize>,
}

impl Tokens {
    fn push(&mut self, kind: T, start: usize, end: usize) {
        self.kinds.push(kind);
        self.starts.push(start);
        self.ends.push(end);
    }
}

/// The keyword of the diagram.
const KEYWORD: &str = "ishikawa";
/// The longer form of the keyword, which Mermaid's grammar reads first.
const BETA: &str = "ishikawa-beta";

/// Whether `word`, in any case, stands at `at` and ends at a word boundary. Jison turns a
/// literal of a lexer rule into a regular expression with a `\b` after it, so the keyword is
/// read only when no word character follows it.
fn keyword_at(chars: &[char], at: usize, word: &str) -> bool {
    let len = word.chars().count();
    for (k, w) in word.chars().enumerate() {
        match chars.get(at + k) {
            Some(&c) if c.to_ascii_lowercase() == w => {}
            _ => return false,
        }
    }
    !chars.get(at + len).is_some_and(|&c| is_word(c))
}

/// The tokens of Mermaid's jison lexer for an Ishikawa diagram, where the first rule that
/// matches wins: white space up to a comment is a comment line, the keyword is the keyword,
/// white space that ends in a line break is a blank line, a line break on its own is a line
/// break, other white space is the indentation of a line, and the rest of a line is its text.
pub(crate) fn tokenize(src: &str) -> Tokens {
    let chars: Vec<char> = src.chars().collect();
    // The byte offset of every character, and one for the end of the text.
    let mut offsets: Vec<usize> = Vec::with_capacity(chars.len() + 1);
    let mut at = 0;
    for c in &chars {
        offsets.push(at);
        at += c.len_utf8();
    }
    offsets.push(at);

    let mut tokens = Tokens::default();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        // The end of the white space that comes first.
        let mut j = i;
        while j < n && is_space(chars[j]) {
            j += 1;
        }
        // `\s*%%.*`: white space and a comment, up to the end of the line.
        if j + 1 < n && chars[j] == '%' && chars[j + 1] == '%' {
            let mut end = j + 2;
            while end < n && !is_line_end(chars[end]) {
                end += 1;
            }
            tokens.push(T::SpaceLine, offsets[i], offsets[end]);
            i = end;
            continue;
        }
        // The keyword, which stands at the start of a line.
        if j == i {
            let keyword = [BETA, KEYWORD]
                .into_iter()
                .find(|word| keyword_at(&chars, i, word));
            if let Some(word) = keyword {
                let len = word.chars().count();
                tokens.push(T::Ishikawa, offsets[i], offsets[i + len]);
                i += len;
                continue;
            }
        }
        if j > i {
            // `[\s]+[\n]`: white space that ends in a line break. The match ends at the last
            // line break of the run, and needs a white space character before it.
            if let Some(p) = (i..j).rev().find(|&k| chars[k] == '\n').filter(|&p| p > i) {
                tokens.push(T::SpaceLine, offsets[i], offsets[p + 1]);
                i = p + 1;
                continue;
            }
            // `[\n]+`: line breaks on their own.
            if chars[i] == '\n' {
                let mut k = i;
                while k < j && chars[k] == '\n' {
                    k += 1;
                }
                tokens.push(T::Nl, offsets[i], offsets[k]);
                i = k;
                continue;
            }
            // `[\s]+`: the indentation of a line.
            tokens.push(T::SpaceList, offsets[i], offsets[j]);
            i = j;
            continue;
        }
        // `[^\n]+`: the text of a line.
        let mut end = i;
        while end < n && chars[end] != '\n' {
            end += 1;
        }
        tokens.push(T::Text, offsets[i], offsets[end]);
        i = end;
    }
    tokens.push(T::Eof, offsets[n], offsets[n]);
    tokens
}
