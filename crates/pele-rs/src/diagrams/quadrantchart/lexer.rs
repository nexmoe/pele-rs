//! The quadrantChart lexer: Mermaid's jison lexer for `quadrant.jison`, as pele's `lexer.ts`
//! emulates it. It reads UTF-16 code units, as the jison lexer does, so that what counts as one
//! character is the same.

use crate::diagrams::common::chars::{
    is_digit, is_letter, is_line_end, is_space, is_word, word_at,
};

/// The tokens of the quadrant chart grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum T {
    End,
    /// Not a jison token: the reference lexer would return the same empty token forever from
    /// here.
    Error,
    Newline,
    Title,
    TitleValue,
    AccTitle,
    AccTitleValue,
    AccDescr,
    AccDescrValue,
    AccDescrMultilineValue,
    XAxis,
    YAxis,
    Delimiter,
    Quadrant1,
    Quadrant2,
    Quadrant3,
    Quadrant4,
    Classdef,
    MdStr,
    Str,
    ClassName,
    PointStart,
    PointX,
    PointY,
    Quadrant,
    Alpha,
    UnicodeText,
    Colon,
    Plus,
    Comma,
    Equals,
    Mult,
    Brkt,
    Underscore,
    Dot,
    Amp,
    Minus,
    Num,
    Space,
    Semi,
    Punctuation,
    Eof,
}

impl T {
    /// The token's name, as the error messages of the parser name it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            T::End => "$end",
            T::Error => "INVALID",
            T::Newline => "NEWLINE",
            T::Title => "title",
            T::TitleValue => "title_value",
            T::AccTitle => "acc_title",
            T::AccTitleValue => "acc_title_value",
            T::AccDescr => "acc_descr",
            T::AccDescrValue => "acc_descr_value",
            T::AccDescrMultilineValue => "acc_descr_multiline_value",
            T::XAxis => "X-AXIS",
            T::YAxis => "Y-AXIS",
            T::Delimiter => "AXIS-TEXT-DELIMITER",
            T::Quadrant1 => "QUADRANT_1",
            T::Quadrant2 => "QUADRANT_2",
            T::Quadrant3 => "QUADRANT_3",
            T::Quadrant4 => "QUADRANT_4",
            T::Classdef => "CLASSDEF",
            T::MdStr => "MD_STR",
            T::Str => "STR",
            T::ClassName => "class_name",
            T::PointStart => "point_start",
            T::PointX => "point_x",
            T::PointY => "point_y",
            T::Quadrant => "QUADRANT",
            T::Alpha => "ALPHA",
            T::UnicodeText => "UNICODE_TEXT",
            T::Colon => "COLON",
            T::Plus => "PLUS",
            T::Comma => "COMMA",
            T::Equals => "EQUALS",
            T::Mult => "MULT",
            T::Brkt => "BRKT",
            T::Underscore => "UNDERSCORE",
            T::Dot => "DOT",
            T::Amp => "AMP",
            T::Minus => "MINUS",
            T::Num => "NUM",
            T::Space => "SPACE",
            T::Semi => "SEMI",
            T::Punctuation => "PUNCTUATION",
            T::Eof => "EOF",
        }
    }
}

/// The tokens of a source, and where each of them starts.
pub(crate) struct Tokens {
    pub types: Vec<T>,
    pub texts: Vec<String>,
    /// The start of each token, in UTF-16 code units.
    pub starts: Vec<usize>,
}

/// The state the lexer is in. The jison lexer uses start conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Title,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Str,
    MdStr,
    ClassName,
    PointStart,
    PointX,
    PointY,
}

impl S {
    /// The token the state gives for the empty rest of the input, where it gives one.
    fn at_end(self) -> Option<T> {
        match self {
            S::Initial => Some(T::Eof),
            S::Title => Some(T::TitleValue),
            S::AccTitle => Some(T::AccTitleValue),
            S::AccDescr => Some(T::AccDescrValue),
            S::AccDescrMultiline => Some(T::AccDescrMultilineValue),
            S::Str => Some(T::Str),
            S::MdStr | S::ClassName | S::PointStart | S::PointX | S::PointY => None,
        }
    }
}

/// The token of a character that stands for itself, as Mermaid's grammar lists them.
fn single(c: u16) -> Option<T> {
    Some(match c {
        58 => T::Colon,
        43 => T::Plus,
        44 => T::Comma,
        61 => T::Equals,
        42 => T::Mult,
        35 => T::Brkt,
        95 => T::Underscore,
        46 => T::Dot,
        38 => T::Amp,
        45 => T::Minus,
        32 | 9 | 11 | 12 => T::Space,
        59 => T::Semi,
        33 | 36 | 37 | 39 | 96 | 63 | 92 | 47 => T::Punctuation,
        _ => return None,
    })
}

/// The lexer over the UTF-16 code units of the source.
struct Lexer {
    units: Vec<u16>,
    types: Vec<T>,
    texts: Vec<String>,
    starts: Vec<usize>,
    stack: Vec<S>,
    state: S,
    p: usize,
    /// The last run scanned of each kind, so that a long run is not scanned again from each of
    /// its characters.
    space_from: usize,
    space_to: usize,
    dash_from: usize,
    dash_to: usize,
    ws_from: usize,
    ws_to: usize,
    gap_from: usize,
    gap_to: usize,
}

impl Lexer {
    fn new(src: &str) -> Self {
        Self {
            units: src.encode_utf16().collect(),
            types: Vec::new(),
            texts: Vec::new(),
            starts: Vec::new(),
            stack: Vec::new(),
            state: S::Initial,
            p: 0,
            space_from: 0,
            space_to: 0,
            dash_from: 0,
            dash_to: 0,
            ws_from: 0,
            ws_to: 0,
            gap_from: 0,
            gap_to: 0,
        }
    }

    fn emit(&mut self, t: T, s: usize, e: usize) {
        self.types.push(t);
        self.texts.push(String::from_utf16_lossy(&self.units[s..e]));
        self.starts.push(s);
        self.p = e;
    }

    fn push(&mut self, s: S) {
        self.stack.push(self.state);
        self.state = s;
    }

    fn pop(&mut self) {
        if let Some(state) = self.stack.pop() {
            self.state = state;
        }
    }

    /// The end of the line that starts at `q`.
    fn line_end(&self, q: usize) -> usize {
        let n = self.units.len();
        let from = q.min(n);
        match self.units[from..].iter().position(|&c| c == 10) {
            Some(i) => from + i,
            None => n,
        }
    }

    /// The first place at or after `q` that is not a space.
    fn spaces(&self, q: usize) -> usize {
        let mut q = q;
        while self.units.get(q) == Some(&32) {
            q += 1;
        }
        q
    }

    /// The first place at or after `q` that is not a space of any kind.
    fn run(&self, q: usize) -> usize {
        let mut q = q;
        while q < self.units.len() && is_space(self.units[q]) {
            q += 1;
        }
        q
    }

    /// The end of Mermaid's `(1)|(0(.\d+)?)`, where the dot is any character within a line.
    fn coordinate(&self, q: usize) -> Option<usize> {
        let c = *self.units.get(q)?;
        if c == 49 {
            return Some(q + 1);
        }
        if c != 48 {
            return None;
        }
        let mut e = q + 1;
        if e < self.units.len()
            && !is_line_end(self.units[e])
            && self.units.get(e + 1).copied().is_some_and(is_digit)
        {
            e += 2;
            while self.units.get(e).copied().is_some_and(is_digit) {
                e += 1;
            }
        }
        Some(e)
    }

    /// The end of the run of spaces that starts at `p`, remembered so that a long run is not
    /// scanned again from each of its characters.
    fn space_run(&mut self, p: usize) -> usize {
        if p < self.space_from || p >= self.space_to {
            self.space_from = p;
            self.space_to = self.spaces(p);
        }
        self.space_to
    }

    /// The end of the run of dashes that starts at `j`, remembered as the spaces are.
    fn dash_run(&mut self, j: usize) -> usize {
        if j < self.dash_from || j >= self.dash_to {
            self.dash_from = j;
            self.dash_to = j;
            while self.units.get(self.dash_to) == Some(&45) {
                self.dash_to += 1;
            }
        }
        self.dash_to
    }

    /// The end of the run of spaces of any kind that starts at `p`.
    fn ws_run(&mut self, p: usize) -> usize {
        if p < self.ws_from || p >= self.ws_to {
            self.ws_from = p;
            self.ws_to = self.run(p);
        }
        self.ws_to
    }

    /// The end of the run of spaces of any kind that starts at `q`, after a colon.
    fn gap_run(&mut self, q: usize) -> usize {
        if q < self.gap_from || q >= self.gap_to {
            self.gap_from = q;
            self.gap_to = self.run(q);
        }
        self.gap_to
    }

    fn lex(&mut self) {
        let n = self.units.len();
        while self.p < n {
            let c = self.units[self.p];
            let next = self.units.get(self.p + 1).copied();

            match self.state {
                S::Initial => {}
                S::Title | S::AccTitle | S::AccDescr => {
                    let e = self.line_end(self.p);
                    let t = self.state.at_end().unwrap_or(T::Eof);
                    self.emit(t, self.p, e);
                    self.pop();
                    continue;
                }
                S::AccDescrMultiline => {
                    if c == 125 {
                        self.p += 1;
                        self.pop();
                    } else {
                        let e = self
                            .units
                            .get(self.p..)
                            .and_then(|rest| rest.iter().position(|&u| u == 125))
                            .map(|i| self.p + i)
                            .unwrap_or(n);
                        self.emit(T::AccDescrMultilineValue, self.p, e);
                    }
                    continue;
                }
                S::Str => {
                    if c == 34 {
                        self.p += 1;
                        self.pop();
                    } else {
                        let e = self
                            .units
                            .get(self.p..)
                            .and_then(|rest| rest.iter().position(|&u| u == 34))
                            .map(|i| self.p + i)
                            .unwrap_or(n);
                        self.emit(T::Str, self.p, e);
                    }
                    continue;
                }
                S::MdStr => {
                    if c == 96 && next == Some(34) {
                        self.p += 2;
                        self.pop();
                        continue;
                    }
                    if c != 96 && c != 34 {
                        let mut e = self.p + 1;
                        while e < n && self.units[e] != 96 && self.units[e] != 34 {
                            e += 1;
                        }
                        self.emit(T::MdStr, self.p, e);
                        continue;
                    }
                    break;
                }
                S::ClassName => {
                    if is_word(c) {
                        let mut e = self.p + 1;
                        while self.units.get(e).copied().is_some_and(is_word) {
                            e += 1;
                        }
                        self.emit(T::ClassName, self.p, e);
                        self.pop();
                        continue;
                    }
                    break;
                }
                S::PointStart => {
                    if let Some(e) = self.coordinate(self.p) {
                        self.emit(T::PointX, self.p, e);
                        self.push(S::PointX);
                        continue;
                    }
                    let k = self.run(self.p);
                    if self.units.get(k) == Some(&93) {
                        self.p = self.spaces(k + 1);
                        self.pop();
                        continue;
                    }
                    break;
                }
                S::PointX => {
                    let k = self.run(self.p);
                    if self.units.get(k) == Some(&44) {
                        self.p = self.run(k + 1);
                        self.state = S::PointY;
                        continue;
                    }
                    break;
                }
                S::PointY => {
                    if let Some(e) = self.coordinate(self.p) {
                        self.emit(T::PointY, self.p, e);
                        self.pop();
                        continue;
                    }
                    break;
                }
            }

            if c == 37 && next == Some(37) {
                self.p = self.line_end(self.p + 2);
                continue;
            }
            // Mermaid's second comment rule takes any character before `%%` with it, a line
            // break included.
            if c != 125 && next == Some(37) && self.units.get(self.p + 2) == Some(&37) {
                self.p = self.line_end(self.p + 3);
                continue;
            }
            if c == 10 || c == 13 {
                let mut e = self.p + 1;
                while self.units.get(e).is_some_and(|&d| d == 10 || d == 13) {
                    e += 1;
                }
                self.emit(T::Newline, self.p, e);
                continue;
            }

            let lower = c | 32;
            if lower == 116
                && word_at(&self.units, self.p, "title")
                && !is_word_at(&self.units, self.p + 5)
            {
                self.emit(T::Title, self.p, self.p + 5);
                self.push(S::Title);
                continue;
            }
            if lower == 97 && word_at(&self.units, self.p, "acc") {
                let descr = word_at(&self.units, self.p + 3, "descr");
                if descr || word_at(&self.units, self.p + 3, "title") {
                    let k = self.run(self.p + 8);
                    match self.units.get(k) {
                        Some(&58) => {
                            let e = self.run(k + 1);
                            let t = if descr { T::AccDescr } else { T::AccTitle };
                            self.emit(t, self.p, e);
                            self.push(if descr { S::AccDescr } else { S::AccTitle });
                            continue;
                        }
                        Some(&123) if descr => {
                            self.p = self.run(k + 1);
                            self.push(S::AccDescrMultiline);
                            continue;
                        }
                        _ => {}
                    }
                }
            }

            // The axis, quadrant and chart keywords and the axis arrow take the spaces around
            // them.
            let mut j = self.p;
            if c == 32 {
                j = self.space_run(self.p);
            }
            let d = self.units.get(j).copied();
            let dl = d.unwrap_or(0) | 32;
            let mut kind: Option<T> = None;
            let mut end = 0usize;
            if dl == 120 && word_at(&self.units, j, "x-axis") {
                kind = Some(T::XAxis);
                end = j + 6;
            } else if dl == 121 && word_at(&self.units, j, "y-axis") {
                kind = Some(T::YAxis);
                end = j + 6;
            } else if d == Some(45) {
                let dash_to = self.dash_run(j);
                if dash_to - j >= 2 && self.units.get(dash_to) == Some(&62) {
                    kind = Some(T::Delimiter);
                    end = dash_to + 1;
                }
            } else if dl == 113 && word_at(&self.units, j, "quadrant") {
                let q = self.units.get(j + 9).copied().unwrap_or(0);
                if self.units.get(j + 8) == Some(&45) && (49..=52).contains(&q) {
                    kind = Some(match q {
                        49 => T::Quadrant1,
                        50 => T::Quadrant2,
                        51 => T::Quadrant3,
                        _ => T::Quadrant4,
                    });
                    end = j + 10;
                } else if word_at(&self.units, j + 8, "chart") {
                    kind = Some(T::Quadrant);
                    end = j + 13;
                }
            }
            if let Some(kind) = kind {
                let e = self.spaces(end);
                self.emit(kind, self.p, e);
                continue;
            }

            if lower == 99
                && word_at(&self.units, self.p, "classdef")
                && !is_word_at(&self.units, self.p + 8)
            {
                self.emit(T::Classdef, self.p, self.p + 8);
                continue;
            }
            if c == 34 {
                if next == Some(96) {
                    self.p += 2;
                    self.push(S::MdStr);
                } else {
                    self.p += 1;
                    self.push(S::Str);
                }
                continue;
            }
            if c == 58 && next == Some(58) && self.units.get(self.p + 2) == Some(&58) {
                self.p += 3;
                self.push(S::ClassName);
                continue;
            }
            // A point's `: [`, with any whitespace before, between and after, line breaks
            // included.
            if c == 58 || is_space(c) {
                let mut k = self.p;
                if c != 58 {
                    k = self.ws_run(self.p);
                }
                if self.units.get(k) == Some(&58) {
                    let gap_to = self.gap_run(k + 1);
                    if self.units.get(gap_to) == Some(&91) {
                        let e = self.run(gap_to + 1);
                        self.emit(T::PointStart, self.p, e);
                        self.push(S::PointStart);
                        continue;
                    }
                }
            }

            if is_letter(c) {
                let mut e = self.p + 1;
                while self.units.get(e).copied().is_some_and(is_letter) {
                    e += 1;
                }
                self.emit(T::Alpha, self.p, e);
                continue;
            }
            if c >= 128 {
                let mut e = self.p + 1;
                while self.units.get(e).is_some_and(|&u| u >= 128) {
                    e += 1;
                }
                self.emit(T::UnicodeText, self.p, e);
                continue;
            }
            if is_digit(c) {
                let mut e = self.p + 1;
                while self.units.get(e).copied().is_some_and(is_digit) {
                    e += 1;
                }
                self.emit(T::Num, self.p, e);
                continue;
            }
            if let Some(t) = single(c) {
                self.emit(t, self.p, self.p + 1);
                continue;
            }
            break;
        }

        if self.p < n {
            self.types.push(T::Error);
            self.texts.push(String::new());
            self.starts.push(self.p);
            return;
        }
        // Jison tries the rules of the current state once more against the empty rest of the
        // input.
        if let Some(t) = self.state.at_end() {
            self.types.push(t);
            self.texts.push(String::new());
            self.starts.push(n);
        }
        self.types.push(T::End);
        self.texts.push(String::new());
        self.starts.push(n);
    }
}

/// Whether the character at `at` is a character of `\w`.
fn is_word_at(units: &[u16], at: usize) -> bool {
    units.get(at).copied().is_some_and(is_word)
}

/// Reads a source as the tokens of the quadrant chart grammar.
pub(crate) fn tokenize(src: &str) -> Tokens {
    let mut lexer = Lexer::new(src);
    lexer.lex();
    Tokens {
        types: lexer.types,
        texts: lexer.texts,
        starts: lexer.starts,
    }
}
