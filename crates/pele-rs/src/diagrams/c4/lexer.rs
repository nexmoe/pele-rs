//! The lexer of a C4 diagram: Mermaid's jison lexer for `c4Diagram.jison`, as pele's
//! `lexer.ts` emulates it. It reads UTF-16 code units, as the jison lexer does, so that what
//! counts as one character is the same.

use super::tokens::{KEYWORD_MAX, KEYWORDS, T};

/// Whitespace as `\s` sees it, on a UTF-16 code unit.
fn is_space(c: u16) -> bool {
    if c < 128 {
        return c == 32 || (9..=13).contains(&c);
    }
    c == 0xa0
        || c == 0x1680
        || (0x2000..=0x200a).contains(&c)
        || c == 0x2028
        || c == 0x2029
        || c == 0x202f
        || c == 0x205f
        || c == 0x3000
        || c == 0xfeff
}

/// A character of `\w`, on a UTF-16 code unit.
fn is_word(c: u16) -> bool {
    (48..=57).contains(&c) || (65..=90).contains(&c) || (97..=122).contains(&c) || c == 95
}

/// The tokens of a C4 source.
#[derive(Debug, Clone, Default)]
pub(crate) struct Tokens {
    pub types: Vec<T>,
    pub texts: Vec<String>,
    /// The start of each token, in UTF-16 code units.
    pub starts: Vec<usize>,
    /// The byte offset of each UTF-16 code unit, and one for the end of the text.
    pub offsets: Vec<usize>,
}

/// The state the lexer is in. The jison lexer uses start conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    /// After a keyword that takes arguments.
    Struct,
    Attribute,
    Str,
    StrKv,
    StrKvKey,
    StrKvValue,
    Acc,
    AccDescrMultiline,
}

/// Reads the tokens of a source.
pub(crate) fn tokenize(src: &str) -> Tokens {
    let mut lexer = Lexer::new(src);
    lexer.run();
    let mut tokens = lexer.tokens;
    tokens.offsets = lexer.offsets;
    tokens
}

struct Lexer {
    units: Vec<u16>,
    offsets: Vec<usize>,
    tokens: Tokens,
    state: S,
    /// The value the `acc` state returns: the title's or the description's.
    acc_value: T,
    p: usize,
    done: bool,
    /// Where the next `direction` is; `-2` until the first search, `-1` when there is none. The
    /// search is made inside the loop on purpose: made up here, one optimized version of this
    /// function in V8 ran it again for every statement, which took seconds on 50,000 characters.
    dir_next: isize,
    line_end: isize,
    dir_failed: usize,
}

impl Lexer {
    fn new(src: &str) -> Self {
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
            tokens: Tokens::default(),
            state: S::Initial,
            acc_value: T::AccTitleValue,
            p: 0,
            done: false,
            dir_next: -2,
            line_end: -1,
            dir_failed: 0,
        }
    }

    fn n(&self) -> usize {
        self.units.len()
    }

    /// The character at a place, or nothing past the end of the text.
    fn at(&self, q: usize) -> Option<u16> {
        self.units.get(q).copied()
    }

    fn text(&self, from: usize, to: usize) -> String {
        String::from_utf16_lossy(&self.units[from..to])
    }

    /// Whether the text at a place starts with a word.
    fn starts_at(&self, q: usize, word: &str) -> bool {
        word.bytes()
            .enumerate()
            .all(|(k, w)| self.at(q + k) == Some(u16::from(w)))
    }

    /// Records a token that runs from a place to another.
    fn emit_from(&mut self, kind: T, from: usize, end: usize) {
        let text = self.text(from, end);
        self.tokens.types.push(kind);
        self.tokens.texts.push(text);
        self.tokens.starts.push(from);
        self.p = end;
    }

    /// The index of the first of `needle` at or after `from`, before `limit`.
    fn find(&self, needle: &str, from: usize, limit: usize) -> Option<usize> {
        let mut at = from;
        while at + needle.len() <= limit {
            if self.starts_at(at, needle) {
                return Some(at);
            }
            at += 1;
        }
        None
    }

    /// The place the line ends: the first line terminator at or after `from`, or the end of the
    /// text.
    fn line_terminator(&self, from: usize) -> usize {
        let mut at = from;
        while at < self.n() {
            let c = self.units[at];
            if c == 10 || c == 13 || c == 0x2028 || c == 0x2029 {
                return at;
            }
            at += 1;
        }
        self.n()
    }

    /// The place a line of text ends: the first newline in it, or the end of the text.
    fn line_end(&self, from: usize) -> usize {
        let mut at = from;
        while at < self.n() {
            if self.units[at] == 10 {
                return at;
            }
            at += 1;
        }
        self.n()
    }

    /// Skips the whitespace at a place.
    fn skip_ws(&self, mut q: usize) -> usize {
        while q < self.n() && is_space(self.units[q]) {
            q += 1;
        }
        q
    }

    /// The end of a direction statement that starts at the cursor, if there is one. The jison
    /// rule is `.*direction\s+TB[^\n]*`: the `.*` takes the last `direction` of the line that a
    /// direction follows, the whitespace after it may reach over a line break, and the text runs
    /// to the end of the line.
    fn direction_end(&self, word: &str) -> Option<usize> {
        let line = self.line_terminator(self.p);
        let mut at = self.p;
        let mut found: Vec<usize> = Vec::new();
        while let Some(k) = self.find("direction", at, line) {
            found.push(k);
            at = k + 1;
        }
        for k in found.into_iter().rev() {
            let run_start = k + "direction".len();
            let mut run_end = run_start;
            while run_end < self.n() && is_space(self.units[run_end]) {
                run_end += 1;
            }
            // The whitespace run is greedy, so the word has to follow the longest one that works.
            let mut q = run_end;
            loop {
                if self.starts_at(q, word) {
                    return Some(self.line_end(q + word.len()));
                }
                if q <= run_start + 1 {
                    break;
                }
                q -= 1;
            }
        }
        None
    }

    /// `word\s[^#\n;]+`: a statement that runs to the end of its line.
    fn statement(&mut self, word: &str, kind: T) -> bool {
        let q = self.p + word.len();
        if !self.starts_at(self.p, word) {
            return false;
        }
        if !self.at(q).is_some_and(is_space) {
            return false;
        }
        let from = q + 1;
        let mut q = from;
        while q < self.n() {
            let c = self.units[q];
            if c == 35 || c == 10 || c == 59 {
                break;
            }
            q += 1;
        }
        if q == from {
            return false;
        }
        self.emit_from(kind, self.p, q);
        true
    }

    fn run(&mut self) {
        loop {
            // Jison notes the end of input one call before it reports it, so a rule that matches
            // nothing gets one last turn.
            if self.done {
                let n = self.n();
                self.emit_from(T::End, n, n);
                break;
            }
            if self.p >= self.n() {
                self.done = true;
            }
            let c = self.at(self.p).unwrap_or(0);

            match self.state {
                S::Initial => {
                    if self.p >= self.n() {
                        let n = self.n();
                        self.emit_from(T::Eof, n, n);
                        continue;
                    }
                    if self.direction() {
                        continue;
                    }
                    if c == 116 && self.statement("title", T::Title) {
                        continue;
                    }
                    if c == 97 {
                        if self.statement("accDescription", T::AccDescription) {
                            continue;
                        }
                        let title = self.starts_at(self.p, "accTitle");
                        if title || self.starts_at(self.p, "accDescr") {
                            let q = self.skip_ws(self.p + 8);
                            let d = self.at(q).unwrap_or(0);
                            if d == 58 {
                                let end = self.skip_ws(q + 1);
                                self.acc_value = if title {
                                    T::AccTitleValue
                                } else {
                                    T::AccDescrValue
                                };
                                self.emit_from(
                                    if title { T::AccTitle } else { T::AccDescr },
                                    self.p,
                                    end,
                                );
                                self.state = S::Acc;
                                continue;
                            }
                            if d == 123 && !title {
                                self.p = self.skip_ws(q + 1);
                                self.state = S::AccDescrMultiline;
                                continue;
                            }
                        }
                    }
                    if c == 37 && self.at(self.p + 1) == Some(37) {
                        // A comment takes the line breaks after it, so no NEWLINE follows.
                        let mut q = self.line_end(self.p + 2);
                        while q < self.n() && (self.units[q] == 10 || self.units[q] == 13) {
                            q += 1;
                        }
                        self.p = q;
                        continue;
                    }
                    if is_space(c) {
                        let mut q = self.p;
                        let mut last = None;
                        while q < self.n() {
                            let d = self.units[q];
                            if !is_space(d) {
                                break;
                            }
                            if d == 10 {
                                last = Some(q);
                            }
                            q += 1;
                        }
                        match last {
                            Some(at) => self.emit_from(T::Newline, self.p, at + 1),
                            None => self.p = q,
                        }
                        continue;
                    }
                    if c == 123 {
                        self.emit_from(T::LBrace, self.p, self.p + 1);
                        continue;
                    }
                    if c == 125 {
                        self.emit_from(T::RBrace, self.p, self.p + 1);
                        continue;
                    }
                    if is_word(c) {
                        let mut q = self.p + 1;
                        while q < self.n() && is_word(self.units[q]) {
                            q += 1;
                        }
                        if q - self.p <= KEYWORD_MAX {
                            let word = self.text(self.p, q);
                            if let Some(kind) =
                                KEYWORDS.iter().find(|(k, _)| *k == word).map(|(_, t)| *t)
                            {
                                if !kind.is_header() {
                                    self.state = S::Struct;
                                }
                                self.emit_from(kind, self.p, q);
                                continue;
                            }
                        }
                    }
                    self.emit_from(T::Error, self.p, self.p);
                    break;
                }
                S::Struct => {
                    if self.p >= self.n() {
                        let n = self.n();
                        self.emit_from(T::EofInStruct, n, n);
                    } else if c == 40 {
                        let mut q = self.p + 1;
                        while self.at(q) == Some(32) {
                            q += 1;
                        }
                        if self.at(q) == Some(44) {
                            self.emit_from(T::AttributeEmpty, self.p, q + 1);
                        } else {
                            self.p += 1;
                        }
                        self.state = S::Attribute;
                    } else if c == 41 {
                        self.p += 1;
                        self.state = S::Initial;
                    } else {
                        self.emit_from(T::Error, self.p, self.p);
                        break;
                    }
                    continue;
                }
                S::Attribute => {
                    if self.p >= self.n() {
                        continue;
                    }
                    if c == 41 {
                        self.p += 1;
                        self.state = S::Initial;
                    } else if c == 44 {
                        if self.at(self.p + 1) == Some(44) {
                            self.emit_from(T::AttributeEmpty, self.p, self.p + 2);
                        } else {
                            self.p += 1;
                        }
                    } else {
                        let mut q = self.p;
                        while self.at(q) == Some(32) {
                            q += 1;
                        }
                        let d = self.at(q).unwrap_or(0);
                        if d == 34 {
                            if self.at(q + 1) == Some(34) {
                                self.emit_from(T::AttributeEmpty, self.p, q + 2);
                            } else {
                                self.p = q + 1;
                                self.state = S::Str;
                            }
                        } else if d == 36 {
                            self.p = q + 1;
                            self.state = S::StrKv;
                        } else {
                            // An unquoted value runs to the next comma, through closing parentheses
                            // and line breaks.
                            let end = self.up_to(self.p, 44);
                            self.emit_from(T::Str, self.p, end);
                        }
                    }
                    continue;
                }
                S::Str => {
                    if c == 34 {
                        self.p += 1;
                        self.state = S::Attribute;
                    } else {
                        let end = self.up_to(self.p, 34);
                        self.emit_from(T::Str, self.p, end);
                    }
                    continue;
                }
                S::StrKv => {
                    let end = self.up_to(self.p, 61);
                    self.emit_from(T::StrKey, self.p, end);
                    self.state = S::StrKvKey;
                    continue;
                }
                S::StrKvKey => {
                    if self.p >= self.n() {
                        continue;
                    }
                    let mut q = self.p + 1;
                    if c == 61 {
                        while self.at(q) == Some(32) {
                            q += 1;
                        }
                    }
                    if c != 61 || self.at(q) != Some(34) {
                        self.emit_from(T::Error, self.p, self.p);
                        break;
                    }
                    self.p = q + 1;
                    self.state = S::StrKvValue;
                    continue;
                }
                S::StrKvValue => {
                    if self.p >= self.n() {
                        continue;
                    }
                    if c == 34 {
                        self.p += 1;
                        self.state = S::Attribute;
                    } else {
                        let end = self.up_to(self.p, 34);
                        self.emit_from(T::StrValue, self.p, end);
                    }
                    continue;
                }
                S::Acc => {
                    let end = self.line_end(self.p);
                    let kind = self.acc_value;
                    self.emit_from(kind, self.p, end);
                    self.state = S::Initial;
                    continue;
                }
                S::AccDescrMultiline => {
                    if c == 125 {
                        self.p += 1;
                        self.state = S::Initial;
                    } else {
                        let end = self.up_to(self.p, 125);
                        self.emit_from(T::AccDescrMultilineValue, self.p, end);
                    }
                    continue;
                }
            }
        }
    }

    /// The direction statements of the line the cursor is on.
    fn direction(&mut self) -> bool {
        if self.dir_next != -1 && self.p >= self.dir_failed {
            if self.dir_next < self.p as isize {
                self.dir_next = self
                    .find("direction", self.p, self.n())
                    .map_or(-1, |k| k as isize);
            }
            if self.dir_next != -1 {
                if self.line_end < self.p as isize {
                    self.line_end = self.line_terminator(self.p) as isize;
                }
                if self.dir_next < self.line_end {
                    for (word, kind) in [
                        ("TB", T::DirectionTb),
                        ("BT", T::DirectionBt),
                        ("RL", T::DirectionRl),
                        ("LR", T::DirectionLr),
                    ] {
                        if let Some(end) = self.direction_end(word) {
                            self.emit_from(kind, self.p, end);
                            return true;
                        }
                    }
                    // No direction statement starts anywhere in the rest of this line either.
                    self.dir_failed = self.line_end as usize;
                }
            }
        }
        false
    }

    /// The place the next of a character is at, or the end of the text.
    fn up_to(&self, from: usize, what: u16) -> usize {
        let mut at = from;
        while at < self.n() {
            if self.units[at] == what {
                return at;
            }
            at += 1;
        }
        self.n()
    }
}
