//! The xychart lexer: Mermaid's jison lexer for `xychart.jison`, as pele's `lexer.ts` emulates
//! it. It reads UTF-16 code units, as the jison lexer does, so that what counts as one
//! character is the same.

use crate::diagrams::common::chars::{is_digit, is_letter, is_space, is_word, word_at};

/// The tokens of the xychart grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum T {
    End,
    /// Not a jison token: the reference lexer would return the same empty token forever from
    /// here.
    Error,
    Newline,
    Title,
    AccTitle,
    AccTitleValue,
    AccDescr,
    AccDescrValue,
    AccDescrMultilineValue,
    Xychart,
    ChartOrientation,
    XAxis,
    YAxis,
    ArrowDelimiter,
    Line,
    Bar,
    NumberWithDecimal,
    SquareBracesStart,
    SquareBracesEnd,
    Str,
    Alpha,
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
    Semi,
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
            T::AccTitle => "acc_title",
            T::AccTitleValue => "acc_title_value",
            T::AccDescr => "acc_descr",
            T::AccDescrValue => "acc_descr_value",
            T::AccDescrMultilineValue => "acc_descr_multiline_value",
            T::Xychart => "XYCHART",
            T::ChartOrientation => "CHART_ORIENTATION",
            T::XAxis => "X_AXIS",
            T::YAxis => "Y_AXIS",
            T::ArrowDelimiter => "ARROW_DELIMITER",
            T::Line => "LINE",
            T::Bar => "BAR",
            T::NumberWithDecimal => "NUMBER_WITH_DECIMAL",
            T::SquareBracesStart => "SQUARE_BRACES_START",
            T::SquareBracesEnd => "SQUARE_BRACES_END",
            T::Str => "STR",
            T::Alpha => "ALPHA",
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
            T::Semi => "SEMI",
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
    /// The byte offset of each UTF-16 code unit, and one for the end of the text.
    pub offsets: Vec<usize>,
}

/// The state the lexer is in. The jison lexer uses start conditions; the ones that matter here
/// are the text of the chart, the text of an axis, the text of a series, the text of a quoted
/// string and the text of an accessible title or description.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    AxisData,
    AxisBandData,
    Data,
    DataInner,
    Str,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
}

impl S {
    /// The token the state gives for the empty rest of the input.
    fn at_end(self) -> T {
        match self {
            S::Initial | S::AxisData | S::AxisBandData | S::Data | S::DataInner => T::Eof,
            S::Str => T::Str,
            S::AccTitle => T::AccTitleValue,
            S::AccDescr => T::AccDescrValue,
            S::AccDescrMultiline => T::AccDescrMultilineValue,
        }
    }
}

/// Mermaid's grammar means to open a markdown string at a backtick followed by a backtick, but
/// its quoting turns the rule into one that matches this text and leaves the current state.
/// Markdown strings are never read.
const BROKEN_RULE: &str = concat!(
    "`)",
    "                                    ",
    "{ this.pushstate(md_string); }\n<md_string>(?:(?!`\").)+",
    "                  ",
    "{ return md_str; }\n<md_string>(?:`"
);

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
        59 => T::Semi,
        _ => return None,
    })
}

/// The lexer over the UTF-16 code units of the source.
struct Lexer {
    units: Vec<u16>,
    /// The byte offset of each UTF-16 code unit, and one for the end of the text.
    offsets: Vec<usize>,
    types: Vec<T>,
    texts: Vec<String>,
    starts: Vec<usize>,
    stack: Vec<S>,
    state: S,
    p: usize,
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
            types: Vec::new(),
            texts: Vec::new(),
            starts: Vec::new(),
            stack: Vec::new(),
            state: S::Initial,
            p: 0,
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
    fn run(&self, q: usize) -> usize {
        let mut q = q;
        while q < self.units.len() && is_space(self.units[q]) {
            q += 1;
        }
        q
    }

    /// The first place at or after `q` that is not a digit.
    fn digits(&self, q: usize) -> usize {
        let mut q = q;
        while q < self.units.len() && is_digit(self.units[q]) {
            q += 1;
        }
        q
    }

    /// Whether a keyword is at the current place, and what follows it is not a word.
    fn keyword(&self, word: &str) -> bool {
        word_at(&self.units, self.p, word)
            && !self
                .units
                .get(self.p + word.len())
                .copied()
                .is_some_and(is_word)
    }

    /// The index of the next `c` at or after `p`.
    fn index_of(&self, c: u16) -> Option<usize> {
        self.units[self.p..]
            .iter()
            .position(|&u| u == c)
            .map(|i| self.p + i)
    }

    fn lex(&mut self) {
        let n = self.units.len();
        while self.p < n {
            let c = self.units[self.p];
            let next = self.units.get(self.p + 1).copied();

            match self.state {
                S::Str => {
                    if c == 34 {
                        self.p += 1;
                        self.pop();
                    } else {
                        let e = self.index_of(34).unwrap_or(n);
                        self.emit(T::Str, self.p, e);
                    }
                    continue;
                }
                S::AccTitle | S::AccDescr => {
                    let e = self.line_end(self.p);
                    let t = self.state.at_end();
                    self.emit(t, self.p, e);
                    self.pop();
                    continue;
                }
                S::AccDescrMultiline => {
                    if c == 125 {
                        self.p += 1;
                        self.pop();
                    } else {
                        let e = self.index_of(125).unwrap_or(n);
                        self.emit(T::AccDescrMultilineValue, self.p, e);
                    }
                    continue;
                }
                _ => {}
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
                // Only a line break that directly follows an axis or a series ends it; one after
                // spaces is skipped with them, and the state carries on to the next line.
                if matches!(self.state, S::AxisData | S::Data) && (c == 10 || next == Some(10)) {
                    let e = if c == 10 { self.p + 1 } else { self.p + 2 };
                    self.emit(T::Newline, self.p, e);
                    self.pop();
                    continue;
                }
                let mut e = self.p + 1;
                while self.units.get(e).is_some_and(|&d| d == 10 || d == 13) {
                    e += 1;
                }
                self.emit(T::Newline, self.p, e);
                continue;
            }

            match c | 32 {
                116 => {
                    if self.keyword("title") {
                        let p = self.p;
                        self.emit(T::Title, p, p + 5);
                        continue;
                    }
                }
                97 => {
                    if word_at(&self.units, self.p, "acc") {
                        let descr = word_at(&self.units, self.p + 3, "descr");
                        if descr || word_at(&self.units, self.p + 3, "title") {
                            let k = self.run(self.p + 8);
                            match self.units.get(k) {
                                Some(&58) => {
                                    let e = self.run(k + 1);
                                    let p = self.p;
                                    let t = if descr { T::AccDescr } else { T::AccTitle };
                                    self.emit(t, p, e);
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
                }
                120 => {
                    if self.keyword("xychart-beta") {
                        let p = self.p;
                        self.emit(T::Xychart, p, p + 12);
                        continue;
                    }
                    if self.keyword("xychart") {
                        let p = self.p;
                        self.emit(T::Xychart, p, p + 7);
                        continue;
                    }
                    if self.keyword("x-axis") {
                        let p = self.p;
                        self.emit(T::XAxis, p, p + 6);
                        self.push(S::AxisData);
                        continue;
                    }
                }
                121 => {
                    if self.keyword("y-axis") {
                        let p = self.p;
                        self.emit(T::YAxis, p, p + 6);
                        self.push(S::AxisData);
                        continue;
                    }
                }
                118 => {
                    if word_at(&self.units, self.p, "vertical") {
                        let p = self.p;
                        self.emit(T::ChartOrientation, p, p + 8);
                        continue;
                    }
                }
                104 => {
                    if word_at(&self.units, self.p, "horizontal") {
                        let p = self.p;
                        self.emit(T::ChartOrientation, p, p + 10);
                        continue;
                    }
                }
                108 => {
                    if self.keyword("line") {
                        let p = self.p;
                        self.emit(T::Line, p, p + 4);
                        self.push(S::Data);
                        continue;
                    }
                }
                98 if self.keyword("bar") => {
                    let p = self.p;
                    self.emit(T::Bar, p, p + 3);
                    self.push(S::Data);
                    continue;
                }
                _ => {}
            }

            if c == 91 {
                match self.state {
                    S::AxisData => self.push(S::AxisBandData),
                    S::Data => self.push(S::DataInner),
                    _ => {}
                }
                let p = self.p;
                self.emit(T::SquareBracesStart, p, p + 1);
                continue;
            }
            if c == 93 {
                if matches!(self.state, S::DataInner | S::AxisBandData) {
                    self.pop();
                }
                let p = self.p;
                self.emit(T::SquareBracesEnd, p, p + 1);
                continue;
            }
            if self.state == S::AxisData
                && c == 45
                && next == Some(45)
                && self.units.get(self.p + 2) == Some(&62)
            {
                let p = self.p;
                self.emit(T::ArrowDelimiter, p, p + 3);
                continue;
            }
            if matches!(self.state, S::AxisData | S::DataInner) {
                let q = if c == 43 || c == 45 {
                    self.p + 1
                } else {
                    self.p
                };
                let mut end = None;
                if let Some(d) = self.units.get(q).copied() {
                    if is_digit(d) {
                        let mut e = self.digits(q + 1);
                        if self.units.get(e) == Some(&46)
                            && self.units.get(e + 1).copied().is_some_and(is_digit)
                        {
                            e = self.digits(e + 2);
                        }
                        end = Some(e);
                    } else if d == 46 && self.units.get(q + 1).copied().is_some_and(is_digit) {
                        end = Some(self.digits(q + 2));
                    }
                }
                if let Some(e) = end {
                    let p = self.p;
                    self.emit(T::NumberWithDecimal, p, e);
                    continue;
                }
            }
            if c == 96 && word_at(&self.units, self.p, BROKEN_RULE) {
                self.p += BROKEN_RULE.len();
                self.pop();
                continue;
            }
            if c == 34 {
                self.p += 1;
                self.push(S::Str);
                continue;
            }
            if is_letter(c) {
                let mut e = self.p + 1;
                while self.units.get(e).copied().is_some_and(is_letter) {
                    e += 1;
                }
                let p = self.p;
                self.emit(T::Alpha, p, e);
                continue;
            }
            if is_digit(c) {
                let e = self.digits(self.p + 1);
                let p = self.p;
                self.emit(T::Num, p, e);
                continue;
            }
            if is_space(c) {
                self.p = self.run(self.p + 1);
                continue;
            }
            if c < 128 {
                if let Some(t) = single(c) {
                    let p = self.p;
                    self.emit(t, p, p + 1);
                    continue;
                }
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
        let t = self.state.at_end();
        self.types.push(t);
        self.texts.push(String::new());
        self.starts.push(n);
        self.types.push(T::End);
        self.texts.push(String::new());
        self.starts.push(n);
    }
}

/// Reads a source as the tokens of the xychart grammar.
pub(crate) fn tokenize(src: &str) -> Tokens {
    let mut lexer = Lexer::new(src);
    lexer.lex();
    Tokens {
        types: lexer.types,
        texts: lexer.texts,
        starts: lexer.starts,
        offsets: lexer.offsets,
    }
}
