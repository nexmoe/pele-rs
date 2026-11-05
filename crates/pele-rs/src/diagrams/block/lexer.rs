//! The lexer of a block diagram: Mermaid's jison lexer for `block.jison`, as pele's
//! `lexer.ts` emulates it. It reads UTF-16 code units, as the jison lexer does, so that what
//! counts as one character is the same.

use crate::diagrams::common::chars::{is_digit, is_space, is_word};

/// The tokens of the block grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum T {
    End,
    /// Not a jison token: the reference lexer would return the same empty token forever from
    /// here.
    Error,
    Key,
    IdBlock,
    Columns,
    MdStr,
    Str,
    SpaceBlock,
    Default,
    LinkStyle,
    Interpolate,
    ClassDef,
    DefaultClassDefId,
    ClassDefId,
    ClassDefStyleOpts,
    Class,
    ClassEntityIds,
    StyleClass,
    Style,
    StyleEntityIds,
    StyleDefinitionData,
    AccTitle,
    AccTitleValue,
    AccDescr,
    AccDescrValue,
    AccDescrMultilineValue,
    /// The word `end`, which closes a composite.
    EndKeyword,
    NodeDStart,
    NodeDEnd,
    BlockArrowStart,
    BlockArrowEnd,
    Dir,
    NodeId,
    Link,
    StartLink,
    LinkLabel,
    Size,
    Eof,
}

impl T {
    /// The token's name, as the error messages of the parser name it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            T::End => "$end",
            T::Error => "INVALID",
            T::Key => "BLOCK_DIAGRAM_KEY",
            T::IdBlock => "id-block",
            T::Columns => "COLUMNS",
            T::MdStr => "MD_STR",
            T::Str => "STR",
            T::SpaceBlock => "SPACE_BLOCK",
            T::Default => "DEFAULT",
            T::LinkStyle => "LINKSTYLE",
            T::Interpolate => "INTERPOLATE",
            T::ClassDef => "classDef",
            T::DefaultClassDefId => "DEFAULT_CLASSDEF_ID",
            T::ClassDefId => "CLASSDEF_ID",
            T::ClassDefStyleOpts => "CLASSDEF_STYLEOPTS",
            T::Class => "class",
            T::ClassEntityIds => "CLASSENTITY_IDS",
            T::StyleClass => "STYLECLASS",
            T::Style => "style",
            T::StyleEntityIds => "STYLE_ENTITY_IDS",
            T::StyleDefinitionData => "STYLE_DEFINITION_DATA",
            T::AccTitle => "acc_title",
            T::AccTitleValue => "acc_title_value",
            T::AccDescr => "acc_descr",
            T::AccDescrValue => "acc_descr_value",
            T::AccDescrMultilineValue => "acc_descr_multiline_value",
            T::EndKeyword => "end",
            T::NodeDStart => "NODE_DSTART",
            T::NodeDEnd => "NODE_DEND",
            T::BlockArrowStart => "BLOCK_ARROW_START",
            T::BlockArrowEnd => "BLOCK_ARROW_END",
            T::Dir => "DIR",
            T::NodeId => "NODE_ID",
            T::Link => "LINK",
            T::StartLink => "START_LINK",
            T::LinkLabel => "LINK_LABEL",
            T::Size => "SIZE",
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

/// The state the lexer is in. The jison lexer uses start conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Str,
    MdStr,
    Node,
    BlockArrow,
    ArrowDir,
    Label,
    ClassDef,
    ClassDefId,
    Class,
    ClassStyle,
    StyleStatement,
    StyleDefinition,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
}

impl S {
    /// The token the state gives for the empty rest of the input.
    fn at_end(self) -> Option<T> {
        match self {
            S::Initial => Some(T::Eof),
            S::Str => Some(T::Str),
            S::ClassDefId => Some(T::ClassDefStyleOpts),
            S::ClassStyle => Some(T::StyleClass),
            S::StyleDefinition => Some(T::StyleDefinitionData),
            S::AccTitle => Some(T::AccTitleValue),
            S::AccDescr => Some(T::AccDescrValue),
            S::AccDescrMultiline => Some(T::AccDescrMultilineValue),
            _ => None,
        }
    }
}

/// The directions a block arrow may point in.
const DIRECTIONS: [&str; 6] = ["right", "left", "x", "y", "up", "down"];

/// The characters that end a node id: `( ) [ { } < > - : =` and whitespace. A `]` does not end
/// one: Mermaid's own rule for a node id leaves it out, so `A]B` is one id.
fn is_id_stop(c: u16) -> bool {
    matches!(
        c,
        9 | 10 | 11 | 12 | 13 | 32 | 40 | 41 | 45 | 58 | 60 | 61 | 62 | 91 | 123 | 125
    )
}

/// The lexer over the UTF-16 code units of the source.
pub(crate) struct Lexer {
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

    fn n(&self) -> usize {
        self.units.len()
    }

    /// The character at a place, or nothing past the end of the text.
    fn at(&self, q: usize) -> Option<u16> {
        self.units.get(q).copied()
    }

    /// The text between two places.
    fn text(&self, from: usize, to: usize) -> String {
        String::from_utf16_lossy(&self.units[from..to])
    }

    /// Whether the text at a place starts with a word.
    fn starts_at(&self, q: usize, word: &str) -> bool {
        word.bytes()
            .enumerate()
            .all(|(k, w)| self.at(q + k) == Some(u16::from(w)))
    }

    /// Whether the text at a place is a word that no name continues.
    fn keyword(&self, word: &str) -> bool {
        self.starts_at(self.p, word) && !self.at(self.p + word.len()).is_some_and(is_word)
    }

    fn push(&mut self, state: S) {
        self.stack.push(self.state);
        self.state = state;
    }

    fn pop(&mut self) {
        if let Some(state) = self.stack.pop() {
            self.state = state;
        }
    }

    /// The end of the run of whitespace at a place.
    fn spaces(&self, q: usize) -> usize {
        let mut q = q;
        while q < self.n() && self.at(q).is_some_and(is_space) {
            q += 1;
        }
        q
    }

    /// The end of the run of digits at a place.
    fn digits(&self, q: usize) -> usize {
        let mut q = q;
        while q < self.n() && self.at(q).is_some_and(is_digit) {
            q += 1;
        }
        q
    }

    /// The end of the run of name characters at a place.
    fn word(&self, q: usize) -> usize {
        let mut q = q;
        while q < self.n() && self.at(q).is_some_and(is_word) {
            q += 1;
        }
        q
    }

    /// The end of the text at a place, or the end of the source.
    fn index_of(&self, needle: u16, from: usize) -> usize {
        let mut q = from;
        while q < self.n() {
            if self.at(q) == Some(needle) {
                return q;
            }
            q += 1;
        }
        self.n()
    }

    /// Records a token and moves past it.
    fn put(&mut self, kind: T, text: String, end: usize) {
        self.types.push(kind);
        self.texts.push(text);
        self.starts.push(self.p);
        self.p = end;
    }

    /// Records a token that runs to a place.
    fn emit(&mut self, kind: T, end: usize) {
        let text = self.text(self.p, end);
        self.put(kind, text, end);
    }

    /// Opens a quoted text, and records the quote that opened it.
    fn quote(&mut self) {
        if self.at(self.p + 1) == Some(96) {
            self.push(S::MdStr);
            self.p += 2;
        } else {
            self.push(S::Str);
            self.p += 1;
        }
    }

    /// The end of a dotted link at a place, or nothing.
    fn dotted(&self, q: usize) -> Option<usize> {
        if self.at(q) != Some(46) {
            return None;
        }
        let mut q = q;
        loop {
            q += 1;
            if self.at(q) != Some(46) {
                break;
            }
        }
        if self.at(q) != Some(45) {
            return None;
        }
        match self.at(q + 1) {
            Some(120) | Some(111) | Some(62) => Some(q + 2),
            _ => Some(q + 1),
        }
    }

    /// The end of a complete link at a place, before its trailing whitespace, or nothing.
    fn link_end(&self, q: usize) -> Option<usize> {
        let mut q = q;
        match self.at(q) {
            Some(120) | Some(111) | Some(60) => q += 1,
            _ => {}
        }
        let line = self.at(q)?;
        if line == 46 {
            return self.dotted(q);
        }
        if line != 45 && line != 61 {
            return None;
        }
        let mut k = q + 1;
        while self.at(k) == Some(line) {
            k += 1;
        }
        if k - q == 1 {
            return if line == 45 { self.dotted(k) } else { None };
        }
        match self.at(k) {
            Some(120) | Some(111) | Some(62) => Some(k + 1),
            _ => {
                if k - q >= 3 {
                    Some(k)
                } else {
                    None
                }
            }
        }
    }

    /// Reads a link, or the start of one that carries a label.
    fn link(&mut self) -> bool {
        if let Some(e) = self.link_end(self.p) {
            let e = self.spaces(e);
            self.emit(T::Link, e);
            return true;
        }
        let q = if self.at(self.p) == Some(60) {
            self.p + 1
        } else {
            self.p
        };
        let a = self.at(q);
        let b = self.at(q + 1);
        if (a == Some(45) && (b == Some(45) || b == Some(46))) || (a == Some(61) && b == Some(61)) {
            self.push(S::Label);
            let e = self.spaces(q + 2);
            self.emit(T::StartLink, e);
            return true;
        }
        false
    }

    /// Reads the rest of the line as a token, and leaves the state it was in.
    fn line_state(&mut self, kind: T) {
        let e = self.index_of(10, self.p);
        self.pop();
        self.emit(kind, e);
    }

    /// Reads a list of names, and moves to the state that reads what follows them.
    fn id_list(&mut self, kind: T, next: S) -> bool {
        let mut k = self.word(self.p);
        if k == self.p {
            return false;
        }
        while self.at(k) == Some(44) {
            let from = self.spaces(k + 1);
            let to = self.word(from);
            if to == from {
                break;
            }
            k = to;
        }
        self.state = next;
        self.emit(kind, k);
        true
    }

    /// Reads a token of the state before the diagram's text.
    fn initial(&mut self, c: u16) -> bool {
        if is_space(c) {
            self.p = self.spaces(self.p + 1);
            return true;
        }
        let mut q;
        match c {
            98 => {
                if self.starts_at(self.p, "block") {
                    if self.keyword("block-beta") {
                        let e = self.p + 10;
                        self.emit(T::Key, e);
                        return true;
                    }
                    if self.at(self.p + 5) == Some(58) {
                        let e = self.p + 6;
                        self.emit(T::IdBlock, e);
                        return true;
                    }
                    if !self.at(self.p + 5).is_some_and(is_word) {
                        let e = self.p + 5;
                        self.emit(T::Key, e);
                        return true;
                    }
                }
            }
            99 => {
                if self.starts_at(self.p, "columns") {
                    q = self.spaces(self.p + 7);
                    if q > self.p + 7 {
                        if self.starts_at(q, "auto") && !self.at(q + 4).is_some_and(is_word) {
                            let text = "-1".to_string();
                            self.put(T::Columns, text, q + 4);
                            return true;
                        }
                        let e = self.digits(q);
                        if e > q {
                            let text = self.text(q, e);
                            self.put(T::Columns, text, e);
                            return true;
                        }
                    }
                } else if self.starts_at(self.p, "class") {
                    let def = self.starts_at(self.p + 5, "Def");
                    let from = if def { self.p + 8 } else { self.p + 5 };
                    q = self.spaces(from);
                    if q > from {
                        self.push(if def { S::ClassDef } else { S::Class });
                        self.emit(if def { T::ClassDef } else { T::Class }, q);
                        return true;
                    }
                }
            }
            34 => {
                self.quote();
                return true;
            }
            115 => {
                if self.starts_at(self.p, "space") {
                    if self.at(self.p + 5) == Some(58) {
                        let e = self.digits(self.p + 6);
                        if e > self.p + 6 {
                            let text = self.text(self.p + 6, e);
                            self.put(T::SpaceBlock, text, e);
                            return true;
                        }
                    }
                    if !self.at(self.p + 5).is_some_and(is_word) {
                        let e = self.p + 5;
                        self.put(T::SpaceBlock, "1".to_string(), e);
                        return true;
                    }
                } else if self.starts_at(self.p, "style") {
                    q = self.spaces(self.p + 5);
                    if q > self.p + 5 {
                        self.push(S::StyleStatement);
                        self.emit(T::Style, q);
                        return true;
                    }
                }
            }
            100 => {
                if self.keyword("default") {
                    let e = self.p + 7;
                    self.emit(T::Default, e);
                    return true;
                }
            }
            108 => {
                if self.keyword("linkStyle") {
                    let e = self.p + 9;
                    self.emit(T::LinkStyle, e);
                    return true;
                }
            }
            105 => {
                if self.keyword("interpolate") {
                    let e = self.p + 11;
                    self.emit(T::Interpolate, e);
                    return true;
                }
            }
            97 => {
                if self.starts_at(self.p, "accTitle") {
                    q = self.spaces(self.p + 8);
                    if self.at(q) == Some(58) {
                        self.push(S::AccTitle);
                        let e = self.spaces(q + 1);
                        self.emit(T::AccTitle, e);
                        return true;
                    }
                } else if self.starts_at(self.p, "accDescr") {
                    q = self.spaces(self.p + 8);
                    let d = self.at(q);
                    if d == Some(58) {
                        self.push(S::AccDescr);
                        let e = self.spaces(q + 1);
                        self.emit(T::AccDescr, e);
                        return true;
                    }
                    if d == Some(123) {
                        self.push(S::AccDescrMultiline);
                        self.p = self.spaces(q + 1);
                        return true;
                    }
                }
            }
            101 => {
                if self.keyword("end") {
                    let e = self.spaces(self.p + 3);
                    self.emit(T::EndKeyword, e);
                    return true;
                }
            }
            45 => {
                if self.at(self.p + 1) == Some(41) {
                    self.push(S::Node);
                    let e = self.p + 2;
                    self.emit(T::NodeDStart, e);
                    return true;
                }
                return self.link();
            }
            61 => return self.link(),
            60 => {
                if self.at(self.p + 1) == Some(91) {
                    self.push(S::BlockArrow);
                    let e = self.p + 2;
                    self.emit(T::BlockArrowStart, e);
                    return true;
                }
                return self.link();
            }
            40 => {
                let d = self.at(self.p + 1);
                let len = if d == Some(45) || d == Some(91) {
                    2
                } else if d == Some(40) {
                    if self.at(self.p + 2) == Some(40) {
                        3
                    } else {
                        2
                    }
                } else {
                    1
                };
                self.push(S::Node);
                let e = self.p + len;
                self.emit(T::NodeDStart, e);
                return true;
            }
            41 | 123 => {
                self.push(S::Node);
                let e = if self.at(self.p + 1) == Some(c) {
                    self.p + 2
                } else {
                    self.p + 1
                };
                self.emit(T::NodeDStart, e);
                return true;
            }
            62 => {
                self.push(S::Node);
                let e = self.p + 1;
                self.emit(T::NodeDStart, e);
                return true;
            }
            91 => {
                let d = self.at(self.p + 1);
                self.push(S::Node);
                let e = match d {
                    Some(91) | Some(124) | Some(40) | Some(92) | Some(47) => self.p + 2,
                    _ => self.p + 1,
                };
                self.emit(T::NodeDStart, e);
                return true;
            }
            58 => {
                let e = self.digits(self.p + 1);
                if e > self.p + 1 {
                    let text = self.text(self.p + 1, e);
                    self.put(T::Size, text, e);
                    return true;
                }
                return false;
            }
            _ => {}
        }

        q = self.p;
        while q < self.n() {
            let ch = self.at(q).unwrap_or(0);
            if ch < 128 {
                if is_id_stop(ch) {
                    break;
                }
            } else if is_space(ch) {
                break;
            }
            q += 1;
        }
        if q == self.p {
            // Nothing can be read here, so the text is not Mermaid's.
            return false;
        }
        self.emit(T::NodeId, q);
        true
    }

    /// Reads the end of a node's shape.
    fn node_end(&mut self, c: u16) -> bool {
        let d = self.at(self.p + 1);
        let len = match c {
            34 => {
                self.quote();
                return true;
            }
            40 => {
                if d == Some(45) {
                    2
                } else if d == Some(40) {
                    if self.at(self.p + 2) == Some(40) {
                        3
                    } else {
                        2
                    }
                } else {
                    1
                }
            }
            41 => {
                if d == Some(41) {
                    if self.at(self.p + 2) == Some(41) {
                        3
                    } else {
                        2
                    }
                } else if d == Some(93) {
                    2
                } else {
                    1
                }
            }
            125 => {
                if d == Some(125) {
                    2
                } else {
                    1
                }
            }
            45 => {
                if d == Some(41) {
                    2
                } else {
                    0
                }
            }
            93 => match d {
                Some(93) | Some(41) | Some(62) => 2,
                _ => 1,
            },
            92 | 47 => match d {
                Some(93) => 2,
                _ => 0,
            },
            _ => 0,
        };
        if len == 0 {
            return false;
        }
        self.pop();
        let e = self.p + len;
        self.emit(T::NodeDEnd, e);
        true
    }

    /// Reads the direction of a block arrow.
    fn arrow_dir(&mut self, c: u16) -> bool {
        if c == 41 {
            self.pop();
            self.pop();
            let e = self.spaces(self.p + 1);
            self.put(T::BlockArrowEnd, "]>".to_string(), e);
            return true;
        }
        let from = self.spaces(if c == 44 { self.p + 1 } else { self.p });
        for dir in DIRECTIONS {
            if self.starts_at(from, dir) {
                let e = self.spaces(from + dir.len());
                let text = self.text(if c == 44 { from } else { self.p }, e);
                self.put(T::Dir, text, e);
                return true;
            }
        }
        false
    }
}

/// Reads the tokens of a source.
pub(crate) fn tokenize(src: &str) -> Tokens {
    let mut lexer = Lexer::new(src);
    let n = lexer.n();

    loop {
        if lexer.p >= n {
            let kind = lexer.state.at_end();
            lexer.p = n;
            if let Some(kind) = kind {
                lexer.put(kind, String::new(), n);
            }
            break;
        }

        let c = lexer.at(lexer.p).unwrap_or(0);
        let before = lexer.p;
        let before_state = lexer.state;
        let mut ok = true;
        match lexer.state {
            S::Initial => ok = lexer.initial(c),
            S::Str => {
                if c == 34 {
                    lexer.pop();
                    lexer.p += 1;
                } else {
                    let e = lexer.index_of(34, lexer.p);
                    lexer.emit(T::Str, e);
                }
            }
            S::MdStr => {
                if c == 96 {
                    ok = lexer.at(lexer.p + 1) == Some(34);
                    if ok {
                        lexer.pop();
                        lexer.p += 2;
                    }
                } else if c == 34 {
                    ok = false;
                } else {
                    let mut q = lexer.p + 1;
                    while q < n {
                        let ch = lexer.at(q).unwrap_or(0);
                        if ch == 96 || ch == 34 {
                            break;
                        }
                        q += 1;
                    }
                    lexer.emit(T::MdStr, q);
                }
            }
            S::Node => ok = lexer.node_end(c),
            S::BlockArrow => {
                if c == 34 {
                    lexer.quote();
                } else if c == 93 && lexer.at(lexer.p + 1) == Some(62) {
                    let q = lexer.spaces(lexer.p + 2);
                    ok = lexer.at(q) == Some(40);
                    if ok {
                        lexer.push(S::ArrowDir);
                        lexer.p = q + 1;
                    }
                } else {
                    ok = false;
                }
            }
            S::ArrowDir => ok = lexer.arrow_dir(c),
            S::Label => {
                if c == 34 {
                    if lexer.at(lexer.p + 1) == Some(96) {
                        lexer.quote();
                    } else {
                        let e = lexer.p + 1;
                        lexer.emit(T::LinkLabel, e);
                        lexer.push(S::Str);
                    }
                } else {
                    let q = lexer.spaces(lexer.p);
                    match lexer.link_end(q) {
                        Some(e) => {
                            lexer.pop();
                            let e = lexer.spaces(e);
                            lexer.emit(T::Link, e);
                        }
                        None => ok = false,
                    }
                }
            }
            S::ClassDef => {
                let mut kind = T::DefaultClassDefId;
                let mut k = if lexer.starts_at(lexer.p, "DEFAULT") {
                    lexer.p + 7
                } else {
                    lexer.p
                };
                let mut q = lexer.spaces(k);
                if k == lexer.p || q == k {
                    kind = T::ClassDefId;
                    k = lexer.word(lexer.p);
                    q = lexer.spaces(k);
                }
                ok = k > lexer.p && q > k;
                if ok {
                    lexer.state = S::ClassDefId;
                    lexer.emit(kind, q);
                }
            }
            S::ClassDefId => lexer.line_state(T::ClassDefStyleOpts),
            S::Class => ok = lexer.id_list(T::ClassEntityIds, S::ClassStyle),
            S::ClassStyle => lexer.line_state(T::StyleClass),
            S::StyleStatement => ok = lexer.id_list(T::StyleEntityIds, S::StyleDefinition),
            S::StyleDefinition => lexer.line_state(T::StyleDefinitionData),
            S::AccTitle => lexer.line_state(T::AccTitleValue),
            S::AccDescr => lexer.line_state(T::AccDescrValue),
            S::AccDescrMultiline => {
                if c == 125 {
                    lexer.pop();
                    lexer.p += 1;
                } else {
                    let e = lexer.index_of(125, lexer.p);
                    lexer.emit(T::AccDescrMultilineValue, e);
                }
            }
        }

        if !ok {
            lexer.types.push(T::Error);
            lexer.texts.push(String::new());
            lexer.starts.push(lexer.p);
            break;
        }
        if lexer.p == before && lexer.state == before_state {
            // A rule that reads nothing would have the lexer read the same text for ever.
            lexer.types.push(T::Error);
            lexer.texts.push(String::new());
            lexer.starts.push(lexer.p);
            break;
        }
    }

    lexer.types.push(T::End);
    lexer.texts.push(String::new());
    lexer.starts.push(n);
    Tokens {
        types: lexer.types,
        texts: lexer.texts,
        starts: lexer.starts,
        offsets: lexer.offsets,
    }
}
