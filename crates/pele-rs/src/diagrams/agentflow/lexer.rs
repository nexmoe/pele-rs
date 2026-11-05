//! The agentflow lexer: Mermaid's jison lexer for `agentflow.jison`, as pele's `lexer.ts`
//! emulates it. It reads UTF-16 code units, as the jison lexer does, so that what counts as one
//! character is the same.

use crate::unicode::is_unicode_letter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum T {
    End,
    Error,
    AccTitle,
    AccTitleValue,
    AccDescr,
    AccDescrValue,
    AccDescrMultilineValue,
    ShapeData,
    CallbackName,
    CallbackArgs,
    MdStr,
    Str,
    Style,
    Default,
    LinkStyle,
    Interpolate,
    ClassDef,
    Class,
    Href,
    Click,
    Graph,
    Flow,
    Connector,
    Global,
    EndKw,
    LinkTarget,
    NoDir,
    Dir,
    DirectionTb,
    DirectionBt,
    DirectionRl,
    DirectionLr,
    DirectionTd,
    LinkId,
    Num,
    Brkt,
    StyleSeparator,
    Colon,
    Amp,
    Semi,
    Comma,
    Mult,
    Link,
    StartLink,
    EdgeText,
    EllipseEnd,
    Text,
    EllipseStart,
    StadiumEnd,
    StadiumStart,
    SubroutineEnd,
    SubroutineStart,
    VertexWithPropsStart,
    TagEnd,
    CylinderEnd,
    CylinderStart,
    DoubleCircleEnd,
    DoubleCircleStart,
    TrapEnd,
    InvTrapEnd,
    TrapStart,
    InvTrapStart,
    TagStart,
    Up,
    Sep,
    Down,
    NodeString,
    Minus,
    UnicodeText,
    Pipe,
    Pe,
    Ps,
    Sqe,
    Sqs,
    DiamondStop,
    DiamondStart,
    Newline,
    Space,
    Comment,
    Eof,
}

impl T {
    /// The token's name in Mermaid's grammar, for error messages.
    pub(crate) fn name(self) -> &'static str {
        use T::*;
        match self {
            End => "$end",
            Error => "INVALID",
            AccTitle => "acc_title",
            AccTitleValue => "acc_title_value",
            AccDescr => "acc_descr",
            AccDescrValue => "acc_descr_value",
            AccDescrMultilineValue => "acc_descr_multiline_value",
            ShapeData => "SHAPE_DATA",
            CallbackName => "CALLBACKNAME",
            CallbackArgs => "CALLBACKARGS",
            MdStr => "MD_STR",
            Str => "STR",
            Style => "STYLE",
            Default => "DEFAULT",
            LinkStyle => "LINKSTYLE",
            Interpolate => "INTERPOLATE",
            ClassDef => "CLASSDEF",
            Class => "CLASS",
            Href => "HREF",
            Click => "CLICK",
            Graph => "GRAPH",
            Flow => "flow",
            Connector => "connector",
            Global => "global",
            EndKw => "end",
            LinkTarget => "LINK_TARGET",
            NoDir => "NODIR",
            Dir => "DIR",
            DirectionTb => "direction_tb",
            DirectionBt => "direction_bt",
            DirectionRl => "direction_rl",
            DirectionLr => "direction_lr",
            DirectionTd => "direction_td",
            LinkId => "LINK_ID",
            Num => "NUM",
            Brkt => "BRKT",
            StyleSeparator => "STYLE_SEPARATOR",
            Colon => "COLON",
            Amp => "AMP",
            Semi => "SEMI",
            Comma => "COMMA",
            Mult => "MULT",
            Link => "LINK",
            StartLink => "START_LINK",
            EdgeText => "EDGE_TEXT",
            EllipseEnd => "-)",
            Text => "TEXT",
            EllipseStart => "(-",
            StadiumEnd => "STADIUMEND",
            StadiumStart => "STADIUMSTART",
            SubroutineEnd => "SUBROUTINEEND",
            SubroutineStart => "SUBROUTINESTART",
            VertexWithPropsStart => "VERTEX_WITH_PROPS_START",
            TagEnd => "TAGEND",
            CylinderEnd => "CYLINDEREND",
            CylinderStart => "CYLINDERSTART",
            DoubleCircleEnd => "DOUBLECIRCLEEND",
            DoubleCircleStart => "DOUBLECIRCLESTART",
            TrapEnd => "TRAPEND",
            InvTrapEnd => "INVTRAPEND",
            TrapStart => "TRAPSTART",
            InvTrapStart => "INVTRAPSTART",
            TagStart => "TAGSTART",
            Up => "UP",
            Sep => "SEP",
            Down => "DOWN",
            NodeString => "NODE_STRING",
            Minus => "MINUS",
            UnicodeText => "UNICODE_TEXT",
            Pipe => "PIPE",
            Pe => "PE",
            Ps => "PS",
            Sqe => "SQE",
            Sqs => "SQS",
            DiamondStop => "DIAMOND_STOP",
            DiamondStart => "DIAMOND_START",
            Newline => "NEWLINE",
            Space => "SPACE",
            Comment => "COMMENT",
            Eof => "EOF",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    String,
    MdString,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Dir,
    Text,
    EllipseText,
    TrapText,
    EdgeText,
    Click,
    CallbackName,
    CallbackArgs,
    ShapeData,
    ShapeDataStr,
}

pub(crate) struct Tokens {
    pub types: Vec<T>,
    pub texts: Vec<String>,
    /// Offsets in UTF-16 code units.
    pub starts: Vec<usize>,
}

/// A code unit, or `NONE` past the end (where JavaScript reads NaN).
const NONE: u32 = u32::MAX;

fn is_space(c: u32) -> bool {
    if c < 128 {
        return c == 32 || (9..=13).contains(&c);
    }
    matches!(
        c,
        0xa0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000 | 0xfeff
    )
}

fn is_word(c: u32) -> bool {
    c < 128 && (c as u8).is_ascii_alphanumeric() || c == 95
}

fn node_char(c: u32) -> bool {
    c < 128 && ((c as u8).is_ascii_alphanumeric() || b"!\"#$%&'*+.`?\\_/".contains(&(c as u8)))
}

struct Lexer<'a> {
    src: &'a [u16],
    n: usize,
    p: usize,
    state: S,
    stack: Vec<S>,
    out: Tokens,
    first_graph: bool,
    dir_next: Option<usize>,
    /// The next `direction` at or after the cursor; the end of the source when there is none.
    dir_stmt_next: usize,
    /// The end of the line the cursor is on.
    line_end: usize,
    at_next: Option<usize>,
    run_end: usize,
    run_at: Option<usize>,
}

impl Lexer<'_> {
    fn c(&self, i: usize) -> u32 {
        self.src.get(i).map_or(NONE, |&u| u32::from(u))
    }

    fn text(&self, s: usize, e: usize) -> String {
        String::from_utf16_lossy(&self.src[s..e])
    }

    fn emit(&mut self, t: T, s: usize, e: usize) {
        let text = self.text(s, e);
        self.out.types.push(t);
        self.out.texts.push(text);
        self.out.starts.push(s);
        self.p = e;
    }

    fn push(&mut self, s: S) {
        self.stack.push(self.state);
        self.state = s;
    }

    fn pop(&mut self) {
        if let Some(s) = self.stack.pop() {
            self.state = s;
        }
    }

    fn starts(&self, word: &str, at: usize) -> bool {
        word.bytes()
            .enumerate()
            .all(|(i, b)| self.c(at + i) == u32::from(b))
    }

    /// Whether the word at the cursor is the whole of the run of node characters there: the
    /// jison lexer takes the longest match, so a word followed by more of such a run is a
    /// node name and not the keyword.
    fn keyword(&self, word: &str) -> bool {
        self.starts(word, self.p) && self.node_run(self.p) == word.len()
    }

    /// Whether the word at the cursor ends at a word boundary, as `\b` sees it.
    fn word_boundary(&self, word: &str) -> bool {
        self.starts(word, self.p) && !is_word(self.c(self.p + word.len()))
    }

    fn find(&self, unit: u16, from: usize) -> Option<usize> {
        self.src
            .get(from..)?
            .iter()
            .position(|&u| u == unit)
            .map(|i| from + i)
    }

    fn find_word(&self, word: &str, from: usize) -> Option<usize> {
        let w: Vec<u16> = word.encode_utf16().collect();
        (from..self.n.saturating_sub(w.len() - 1)).find(|&i| self.src[i..].starts_with(&w))
    }

    fn ws_end(&self, mut q: usize) -> usize {
        while q < self.n && is_space(self.c(q)) {
            q += 1;
        }
        q
    }

    /// The end of a run of spaces and tabs.
    fn blank_end(&self, mut q: usize) -> usize {
        while q < self.n && matches!(self.c(q), 32 | 9) {
            q += 1;
        }
        q
    }

    /// `word\s*sep\s*`
    fn keyword_then(&self, word: &str, sep: u8, q: usize) -> Option<usize> {
        if !self.starts(word, q) {
            return None;
        }
        let q = self.ws_end(q + word.len());
        (self.c(q) == u32::from(sep)).then(|| self.ws_end(q + 1))
    }

    /// `word\s+`
    fn word_spaces(&self, word: &str, q: usize) -> Option<usize> {
        if !self.starts(word, q) || !is_space(self.c(q + word.len())) {
            return None;
        }
        Some(self.ws_end(q + word.len()))
    }

    /// The length of the run of node characters at `p`: the `NODE_STRING` pattern.
    fn node_run(&self, p: usize) -> usize {
        let mut q = p;
        while q < self.n {
            let ch = self.c(q);
            if node_char(ch) {
                q += 1;
            } else if ch == 45 && q + 1 < self.n {
                let d = self.c(q + 1);
                if d == 62 || d == 45 || d == 46 {
                    break;
                }
                q += 1;
            } else if ch == 61 && self.c(q + 1) != 61 {
                q += 1;
            } else {
                break;
            }
        }
        q - p
    }

    fn node_string(&mut self) -> bool {
        let q = self.p + self.node_run(self.p);
        if q == self.p {
            return false;
        }
        self.emit(T::NodeString, self.p, q);
        true
    }

    /// The end of the `%%` comment that starts at `q`, or `None`. A `%%{` opens a metadata
    /// block and is not a comment.
    fn comment_end(&self, q: usize) -> Option<usize> {
        if self.c(q) != 37 || self.c(q + 1) != 37 || self.c(q + 2) == 123 {
            return None;
        }
        Some(self.find(10, q + 2).unwrap_or(self.n))
    }

    fn quote(&mut self) {
        if self.c(self.p + 1) == 96 {
            self.push(S::MdString);
            self.p += 2;
        } else {
            self.push(S::String);
            self.p += 1;
        }
    }

    fn opener(&mut self, c: u32) -> bool {
        let p = self.p;
        let d = self.c(p + 1);
        match c {
            40 => {
                if d == 45 {
                    self.push(S::EllipseText);
                    self.emit(T::EllipseStart, p, p + 2);
                } else if d == 91 {
                    self.push(S::Text);
                    self.emit(T::StadiumStart, p, p + 2);
                } else if d == 40 && self.c(p + 2) == 40 {
                    self.push(S::Text);
                    self.emit(T::DoubleCircleStart, p, p + 3);
                } else {
                    self.push(S::Text);
                    self.emit(T::Ps, p, p + 1);
                }
                true
            }
            91 => {
                match d {
                    91 => {
                        self.push(S::Text);
                        self.emit(T::SubroutineStart, p, p + 2);
                    }
                    40 => {
                        self.push(S::Text);
                        self.emit(T::CylinderStart, p, p + 2);
                    }
                    47 => {
                        self.push(S::TrapText);
                        self.emit(T::TrapStart, p, p + 2);
                    }
                    92 => {
                        self.push(S::TrapText);
                        self.emit(T::InvTrapStart, p, p + 2);
                    }
                    _ => {
                        self.push(S::Text);
                        self.emit(T::Sqs, p, p + 1);
                    }
                }
                true
            }
            124 => {
                self.push(S::Text);
                self.emit(T::Pipe, p, p + 1);
                true
            }
            123 => {
                self.push(S::Text);
                self.emit(T::DiamondStart, p, p + 1);
                true
            }
            _ => false,
        }
    }

    /// The end of `\s*--+[x>]\s*` at `q`, or `None`.
    fn arrow_end(&self, q: usize) -> Option<usize> {
        let j = self.ws_end(q);
        if self.c(j) != 45 || self.c(j + 1) != 45 {
            return None;
        }
        let mut k = j + 2;
        while self.c(k) == 45 {
            k += 1;
        }
        let t = self.c(k);
        if t == 120 || t == 62 {
            Some(self.ws_end(k + 1))
        } else {
            None
        }
    }

    /// The `--` and `-.-` links of the agentflow grammar: `\s*\-\-+[x>]\s*`,
    /// `\s*\-\-\s*` and `\s*\-\.+\-\s*`.
    fn link(&mut self) -> bool {
        let p = self.p;
        let j = self.ws_end(p);
        if self.c(j) != 45 {
            return false;
        }
        let d = self.c(j + 1);
        if d == 45 {
            let mut k = j + 2;
            while self.c(k) == 45 {
                k += 1;
            }
            let t = self.c(k);
            if t == 120 || t == 62 {
                self.emit(T::Link, p, self.ws_end(k + 1));
            } else {
                self.push(S::EdgeText);
                self.emit(T::StartLink, p, self.ws_end(j + 2));
            }
            return true;
        }
        if d == 46 {
            let mut k = j + 2;
            while self.c(k) == 46 {
                k += 1;
            }
            if self.c(k) == 45 {
                self.emit(T::Link, p, self.ws_end(k + 1));
                return true;
            }
        }
        false
    }

    /// `.*direction\s+XX[^\n]*` at p: the direction statement of the grammar. The token takes
    /// the whole line, from the cursor to its end. A bare `TB` is a node, not a direction.
    fn direction_statement(&mut self) -> Option<(T, usize)> {
        let p = self.p;
        // The next `direction` at or after the cursor, and the end of the line the cursor is
        // on. Both are looked for once and kept, so that a long diagram stays quick.
        if self.dir_stmt_next < p {
            self.dir_stmt_next = self.find_word("direction", p).unwrap_or(self.n);
        }
        if p >= self.line_end {
            self.line_end = self.find(10, p).unwrap_or(self.n);
        }
        let line_end = self.line_end;
        let mut at = self.dir_stmt_next;
        if at >= line_end {
            return None;
        }
        // The rule takes the last `direction` of the line.
        let mut from = at + 1;
        while let Some(found) = self.find_word("direction", from) {
            if found >= line_end {
                break;
            }
            at = found;
            from = found + 1;
        }
        let mut q = at + 9;
        // The spaces may cross a line break.
        let start = q;
        while q < self.n && is_space(self.c(q)) {
            q += 1;
        }
        if q == start {
            return None;
        }
        let word = ["TB", "BT", "RL", "LR", "TD"];
        let kind = word.iter().find(|w| self.starts(w, q))?;
        if self.node_run(q) > kind.len() {
            return None;
        }
        let t = match *kind {
            "TB" => T::DirectionTb,
            "BT" => T::DirectionBt,
            "RL" => T::DirectionRl,
            "LR" => T::DirectionLr,
            _ => T::DirectionTd,
        };
        Some((t, self.find(10, q).unwrap_or(self.n)))
    }

    fn initial(&mut self, c: u32) -> bool {
        let p = self.p;
        // The direction statement takes the whole line, so it wins the longest match.
        if let Some((t, end)) = self.direction_statement() {
            self.emit(t, p, end);
            return true;
        }
        match c {
            97 if self.starts("acc", p) => {
                if let Some(e) = self.keyword_then("accTitle", b':', p) {
                    self.push(S::AccTitle);
                    self.emit(T::AccTitle, p, e);
                    return true;
                }
                if let Some(e) = self.keyword_then("accDescr", b':', p) {
                    self.push(S::AccDescr);
                    self.emit(T::AccDescr, p, e);
                    return true;
                }
                if let Some(e) = self.keyword_then("accDescr", b'{', p) {
                    self.push(S::AccDescrMultiline);
                    self.p = e;
                    return true;
                }
            }
            64 if self.c(p + 1) == 123 => {
                self.push(S::ShapeData);
                self.out.types.push(T::ShapeData);
                self.out.texts.push(String::new());
                self.out.starts.push(p);
                self.p += 2;
                return true;
            }
            99 => {
                if let Some(e) = self.word_spaces("call", p) {
                    self.push(S::CallbackName);
                    self.p = e;
                    return true;
                }
                if self.keyword("classDef") {
                    self.emit(T::ClassDef, p, p + 8);
                    return true;
                }
                if self.keyword("class") {
                    self.emit(T::Class, p, p + 5);
                    return true;
                }
                if self.keyword("connector") {
                    self.emit(T::Connector, p, p + 9);
                    return true;
                }
                if let Some(e) = self.word_spaces("click", p) {
                    self.push(S::Click);
                    self.p = e;
                    return true;
                }
            }
            34 => {
                self.quote();
                return true;
            }
            115 => {
                if self.keyword("style") {
                    self.emit(T::Style, p, p + 5);
                    return true;
                }
            }
            100 if self.keyword("default") => {
                self.emit(T::Default, p, p + 7);
                return true;
            }
            108 if self.keyword("linkStyle") => {
                self.emit(T::LinkStyle, p, p + 9);
                return true;
            }
            105 if self.keyword("interpolate") => {
                self.emit(T::Interpolate, p, p + 11);
                return true;
            }
            104 if self.starts("href", p) && is_space(self.c(p + 4)) => {
                self.emit(T::Href, p, p + 5);
                return true;
            }
            102 => {
                if self.keyword("flow") {
                    self.emit(T::Flow, p, p + 4);
                    return true;
                }
            }
            84 => {}
            103 if self.keyword("global") => {
                let e = self.blank_end(p + 6);
                self.emit(T::Global, p, e);
                return true;
            }
            97 if self.keyword("agentflow-beta") => {
                if self.first_graph {
                    self.first_graph = false;
                    self.push(S::Dir);
                }
                self.emit(T::Graph, p, p + 14);
                return true;
            }
            101 if self.word_boundary("end") => {
                let e = self.blank_end(p + 3);
                self.emit(T::EndKw, p, e);
                return true;
            }
            95 => {
                for word in ["_self", "_blank", "_parent", "_top"] {
                    if self.keyword(word) {
                        self.emit(T::LinkTarget, p, p + word.len());
                        return true;
                    }
                }
            }
            _ => {}
        }

        // A link id is a run of non-space characters up to its last `@` that is not followed
        // by `{` or `"`.
        if self.at_next.is_some() && c != 34 && !is_space(c) {
            if p >= self.run_end {
                if self.at_next.is_some_and(|a| a < p) {
                    self.at_next = self.find(64, p);
                }
                let mut q = p;
                while q < self.n {
                    let ch = self.c(q);
                    if ch == 34 || is_space(ch) {
                        break;
                    }
                    q += 1;
                }
                self.run_end = q;
                self.run_at = None;
                if let Some(at) = self.at_next.filter(|&a| a < q) {
                    let top = q.min(self.n - 1);
                    let mut k = top;
                    while k > at {
                        k -= 1;
                        if self.c(k) != 64 {
                            continue;
                        }
                        let after = self.c(k + 1);
                        if after != 123 && after != 34 {
                            self.run_at = Some(k);
                            break;
                        }
                    }
                }
            }
            if let Some(at) = self.run_at.filter(|&a| a > p) {
                self.emit(T::LinkId, p, at + 1);
                return true;
            }
        }

        if (48..=57).contains(&c) {
            let mut q = p + 1;
            while (48..=57).contains(&self.c(q)) {
                q += 1;
            }
            self.emit(T::Num, p, q);
            return true;
        }

        match c {
            35 => {
                self.emit(T::Brkt, p, p + 1);
                return true;
            }
            58 => {
                if self.c(p + 1) == 58 && self.c(p + 2) == 58 {
                    self.emit(T::StyleSeparator, p, p + 3);
                } else {
                    self.emit(T::Colon, p, p + 1);
                }
                return true;
            }
            38 => {
                self.emit(T::Amp, p, p + 1);
                return true;
            }
            59 => {
                self.emit(T::Semi, p, p + 1);
                return true;
            }
            44 => {
                self.emit(T::Comma, p, p + 1);
                return true;
            }
            42 => {
                self.emit(T::Mult, p, p + 1);
                return true;
            }
            45 => {
                if self.link() || self.node_string() {
                    return true;
                }
                self.emit(T::Minus, p, p + 1);
                return true;
            }
            46 => {
                if self.link() || self.node_string() {
                    return true;
                }
                return false;
            }
            60 => {
                self.emit(T::TagStart, p, p + 1);
                return true;
            }
            124 | 123 => return self.opener(c),
            91 => {
                if self.c(p + 1) == 124 {
                    self.emit(T::VertexWithPropsStart, p, p + 2);
                    return true;
                }
                return self.opener(c);
            }
            40 => return self.opener(c),
            62 => {
                self.push(S::Text);
                self.emit(T::TagEnd, p, p + 1);
                return true;
            }
            94 => {
                self.emit(T::Up, p, p + 1);
                return true;
            }
            92 => {
                if self.c(p + 1) == 124 {
                    self.emit(T::Sep, p, p + 2);
                    return true;
                }
                return self.node_string();
            }
            118 => {
                if !is_word(self.c(p + 1)) {
                    self.emit(T::Down, p, p + 1);
                    return true;
                }
                return self.node_string();
            }
            37 => {
                if let Some(e) = self.comment_end(p) {
                    self.emit(T::Comment, p, e);
                    return true;
                }
                return self.node_string();
            }
            _ => {}
        }

        if is_space(c) {
            if self.link() {
                return true;
            }
            if matches!(c, 32 | 9) {
                if let Some(e) = self.comment_end(self.blank_end(p)) {
                    self.emit(T::Comment, p, e);
                    return true;
                }
            }
            // `(?:\r?\n)+`
            let mut q = p;
            loop {
                if self.c(q) == 10 {
                    q += 1;
                } else if self.c(q) == 13 && self.c(q + 1) == 10 {
                    q += 2;
                } else {
                    break;
                }
            }
            if q > p {
                self.emit(T::Newline, p, q);
                return true;
            }
            // `[^\S\n\r]+`
            let mut q = p;
            while is_space(self.c(q)) && self.c(q) != 10 && self.c(q) != 13 {
                q += 1;
            }
            if q > p {
                self.emit(T::Space, p, q);
                return true;
            }
            return false;
        }

        if c < 128 {
            return self.node_string();
        }
        if is_unicode_letter(c) {
            self.emit(T::UnicodeText, p, p + 1);
            return true;
        }
        false
    }

    fn edge_text(&mut self, c: u32) -> bool {
        let p = self.p;
        if c == 34 {
            self.quote();
            return true;
        }
        if let Some(e) = self.arrow_end(p) {
            self.pop();
            self.emit(T::Link, p, e);
            return true;
        }
        if c == 45 && self.c(p + 1) == 45 {
            return false;
        }
        let mut q = p + 1;
        while q < self.n {
            let ch = self.c(q);
            if ch == 34 {
                break;
            }
            if ch == 45 {
                if self.c(q + 1) == 45 {
                    break;
                }
            } else if is_space(ch) && self.arrow_end(q).is_some() {
                break;
            }
            q += 1;
        }
        self.emit(T::EdgeText, p, q);
        true
    }

    fn step(&mut self, c: u32) -> bool {
        let p = self.p;
        let n = self.n;
        match self.state {
            S::Initial => return self.initial(c),
            S::String => {
                if c == 34 {
                    if self.c(p + 1) == 96 {
                        self.push(S::MdString);
                        self.p += 2;
                    } else {
                        self.pop();
                        self.p += 1;
                    }
                } else {
                    let e = self.find(34, p).unwrap_or(n);
                    self.emit(T::Str, p, e);
                }
            }
            S::MdString => {
                if c == 96 {
                    if self.c(p + 1) == 34 {
                        self.pop();
                        self.p += 2;
                    } else {
                        return false;
                    }
                } else if c == 34 {
                    self.quote();
                } else {
                    let mut q = p + 1;
                    while q < n && self.c(q) != 96 && self.c(q) != 34 {
                        q += 1;
                    }
                    self.emit(T::MdStr, p, q);
                }
            }
            S::AccTitle | S::AccDescr => {
                let e = self.find(10, p).unwrap_or(n);
                let t = if self.state == S::AccTitle {
                    T::AccTitleValue
                } else {
                    T::AccDescrValue
                };
                self.pop();
                self.emit(t, p, e);
            }
            S::AccDescrMultiline => {
                if c == 125 {
                    self.pop();
                    self.p += 1;
                } else {
                    let e = self.find(125, p).unwrap_or(n);
                    self.emit(T::AccDescrMultilineValue, p, e);
                }
            }
            S::Dir => {
                if c == 34 {
                    self.quote();
                } else {
                    // `(?:\r?\n)*\s*\n`: the whitespace up to the last line break in it.
                    let w = self.ws_end(p);
                    if let Some(nl) = (p..w).rev().find(|&i| self.c(i) == 10) {
                        self.pop();
                        self.emit(T::NoDir, p, nl + 1);
                        return true;
                    }
                    let q = w;
                    let two = (self.c(q), self.c(q + 1));
                    let dirs = [(76, 82), (82, 76), (84, 66), (66, 84), (84, 68), (66, 82)];
                    let e = if dirs.contains(&two) && !is_word(self.c(q + 2)) {
                        Some(q + 2)
                    } else if matches!(self.c(q), 60 | 62 | 94)
                        || (self.c(q) == 118 && !is_word(self.c(q + 1)))
                    {
                        Some(q + 1)
                    } else {
                        None
                    };
                    match e {
                        Some(e) => {
                            self.pop();
                            self.emit(T::Dir, p, e);
                        }
                        None => return self.opener(c),
                    }
                }
            }
            S::Text => {
                let d = self.c(p + 1);
                match c {
                    34 => self.quote(),
                    40 | 91 | 123 => {
                        self.opener(c);
                    }
                    93 => {
                        self.pop();
                        if d == 41 {
                            self.emit(T::StadiumEnd, p, p + 2);
                        } else if d == 93 {
                            self.emit(T::SubroutineEnd, p, p + 2);
                        } else {
                            self.emit(T::Sqe, p, p + 1);
                        }
                    }
                    41 => {
                        self.pop();
                        if d == 93 {
                            self.emit(T::CylinderEnd, p, p + 2);
                        } else if d == 41 && self.c(p + 2) == 41 {
                            self.emit(T::DoubleCircleEnd, p, p + 3);
                        } else {
                            self.emit(T::Pe, p, p + 1);
                        }
                    }
                    124 => {
                        self.pop();
                        self.emit(T::Pipe, p, p + 1);
                    }
                    125 => {
                        self.pop();
                        self.emit(T::DiamondStop, p, p + 1);
                    }
                    37 => {
                        // The text pattern leaves a lone `%` out, so only a comment may start
                        // with one.
                        match self.comment_end(p) {
                            Some(e) => self.p = e,
                            None => return false,
                        }
                    }
                    _ => {
                        let mut q = p + 1;
                        while q < n
                            && !matches!(self.c(q), 91 | 93 | 40 | 41 | 123 | 125 | 124 | 34 | 37)
                        {
                            q += 1;
                        }
                        self.emit(T::Text, p, q);
                    }
                }
            }
            S::EllipseText => {
                let d = self.c(p + 1);
                if c == 34 {
                    self.quote();
                } else if c == 37 {
                    match self.comment_end(p) {
                        Some(e) => self.p = e,
                        None => return false,
                    }
                } else if matches!(c, 45 | 47 | 41) && d == 41 {
                    self.pop();
                    self.emit(T::EllipseEnd, p, p + 2);
                } else if matches!(c, 40 | 91 | 123) {
                    self.opener(c);
                } else if matches!(c, 41 | 93 | 125) {
                    return false;
                } else {
                    let mut q = p + 1;
                    while q < n {
                        let ch = self.c(q);
                        if matches!(ch, 34 | 40 | 41 | 91 | 93 | 123 | 125) {
                            break;
                        }
                        if matches!(ch, 45 | 47) && self.c(q + 1) == 41 {
                            break;
                        }
                        if ch == 37 && self.c(q + 1) == 37 && self.c(q + 2) != 123 {
                            break;
                        }
                        q += 1;
                    }
                    self.emit(T::Text, p, q);
                }
            }
            S::TrapText => {
                let d = self.c(p + 1);
                if c == 34 {
                    self.quote();
                    return true;
                }
                match c {
                    40 => {
                        if d == 45 || d == 91 || (d == 40 && self.c(p + 2) == 40) {
                            self.opener(c);
                        } else if d == 93 {
                            self.pop();
                            self.emit(T::TrapEnd, p, p + 2);
                        } else {
                            self.opener(c);
                        }
                    }
                    91 | 123 => {
                        self.opener(c);
                    }
                    92 => {
                        if d == 93 {
                            self.pop();
                            self.emit(T::TrapEnd, p, p + 2);
                        } else {
                            self.emit(T::Text, p, p + 1);
                        }
                    }
                    47 => {
                        if d == 93 {
                            self.pop();
                            self.emit(T::InvTrapEnd, p, p + 2);
                        } else {
                            self.emit(T::Text, p, p + 1);
                        }
                    }
                    93 | 41 => {
                        if d == 93 {
                            self.pop();
                            self.emit(T::TrapEnd, p, p + 2);
                        } else {
                            return false;
                        }
                    }
                    125 => return false,
                    37 => match self.comment_end(p) {
                        Some(e) => self.p = e,
                        None => return false,
                    },
                    _ => {
                        if (c == 63 || c == 61) && d == 93 {
                            self.pop();
                            self.emit(T::TrapEnd, p, p + 2);
                            return true;
                        }
                        let mut q = p + 1;
                        while q < n
                            && !matches!(self.c(q), 92 | 91 | 93 | 40 | 41 | 123 | 125 | 47 | 37)
                        {
                            q += 1;
                        }
                        self.emit(T::Text, p, q);
                    }
                }
            }
            S::EdgeText => return self.edge_text(c),
            S::Click => {
                if c == 34 {
                    self.quote();
                } else if is_space(c) {
                    self.pop();
                    self.p += 1;
                } else {
                    let mut q = p + 1;
                    while q < n && !is_space(self.c(q)) {
                        q += 1;
                    }
                    self.emit(T::Click, p, q);
                }
            }
            S::CallbackName => {
                if c == 40 {
                    // `\(\s*\)`
                    let q = self.ws_end(p + 1);
                    self.pop();
                    if self.c(q) == 41 {
                        self.p = q + 1;
                    } else {
                        self.push(S::CallbackArgs);
                        self.p += 1;
                    }
                } else {
                    let e = self.find(40, p).unwrap_or(n);
                    self.emit(T::CallbackName, p, e);
                }
            }
            S::CallbackArgs => {
                if c == 41 {
                    self.pop();
                    self.p += 1;
                } else {
                    let e = self.find(41, p).unwrap_or(n);
                    self.emit(T::CallbackArgs, p, e);
                }
            }
            S::ShapeData => {
                if c == 34 {
                    self.push(S::ShapeDataStr);
                    self.emit(T::ShapeData, p, p + 1);
                } else if c == 125 {
                    self.pop();
                    self.p += 1;
                } else if c == 94 {
                    return false;
                } else {
                    let mut q = p + 1;
                    while q < n && !matches!(self.c(q), 125 | 94 | 34) {
                        q += 1;
                    }
                    self.emit(T::ShapeData, p, q);
                }
            }
            S::ShapeDataStr => {
                if c == 34 {
                    self.pop();
                    self.emit(T::ShapeData, p, p + 1);
                } else {
                    let e = self.find(34, p).unwrap_or(n);
                    // `\n\s*` becomes `<br/>`.
                    let text = self.text(p, e);
                    let mut out = String::with_capacity(text.len());
                    let mut chars = text.chars().peekable();
                    while let Some(ch) = chars.next() {
                        if ch == '\n' {
                            out.push_str("<br/>");
                            while chars.next_if(|&c| crate::util::is_space(c)).is_some() {}
                        } else {
                            out.push(ch);
                        }
                    }
                    self.out.types.push(T::ShapeData);
                    self.out.texts.push(out);
                    self.out.starts.push(p);
                    self.p = e;
                }
            }
        }
        true
    }
}

/// Splits agentflow text into tokens. Stops at the first character no rule matches, with an
/// `Error` token there.
pub(crate) fn tokenize(src: &[u16]) -> Tokens {
    let n = src.len();
    let mut lx = Lexer {
        src,
        n,
        p: 0,
        state: S::Initial,
        stack: Vec::new(),
        out: Tokens {
            types: Vec::new(),
            texts: Vec::new(),
            starts: Vec::new(),
        },
        first_graph: true,
        dir_next: None,
        dir_stmt_next: 0,
        line_end: 0,
        at_next: None,
        run_end: 0,
        run_at: None,
    };
    lx.dir_next = lx.find_word("direction", 0);
    lx.at_next = lx.find(64, 0);
    loop {
        if lx.p >= n {
            let t = match lx.state {
                S::Initial => Some(T::Eof),
                S::AccTitle => Some(T::AccTitleValue),
                S::AccDescr => Some(T::AccDescrValue),
                S::AccDescrMultiline => Some(T::AccDescrMultilineValue),
                S::Click => Some(T::Click),
                S::CallbackName => Some(T::CallbackName),
                S::CallbackArgs => Some(T::CallbackArgs),
                _ => None,
            };
            if let Some(t) = t {
                lx.emit(t, n, n);
            }
            break;
        }
        let c = lx.c(lx.p);
        let before = lx.p;
        let before_len = lx.out.types.len();
        if !lx.step(c) {
            lx.out.types.push(T::Error);
            lx.out.texts.push(String::new());
            lx.out.starts.push(lx.p);
            return lx.out;
        }
        // Every rule consumes text or emits a token; guard against a rule that does neither.
        if lx.p == before && lx.out.types.len() == before_len && lx.stack.len() > 10_000 {
            lx.out.types.push(T::Error);
            lx.out.texts.push(String::new());
            lx.out.starts.push(lx.p);
            return lx.out;
        }
    }
    lx.out.types.push(T::End);
    lx.out.texts.push(String::new());
    lx.out.starts.push(n);
    lx.out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<(T, String)> {
        let units: Vec<u16> = src.encode_utf16().collect();
        let t = tokenize(&units);
        t.types.into_iter().zip(t.texts).collect()
    }

    #[test]
    fn reads_a_simple_diagram() {
        let toks = kinds("agentflow-beta\nA[Start] --> B{Is it?}\n");
        let types: Vec<T> = toks.iter().map(|t| t.0).collect();
        assert_eq!(
            types,
            [
                T::Graph,
                T::NoDir,
                T::NodeString,
                T::Sqs,
                T::Text,
                T::Sqe,
                T::Link,
                T::NodeString,
                T::DiamondStart,
                T::Text,
                T::DiamondStop,
                T::Newline,
                T::Eof,
                T::End
            ]
        );
        assert_eq!(toks[6].1, " --> ");
    }

    #[test]
    fn reads_the_agentflow_keywords() {
        let toks = kinds("flow f1[Title]\nend\nconnector c1[Hub]\nglobal g1[Shared]\n");
        assert!(toks.iter().any(|t| t.0 == T::Flow && t.1 == "flow"));
        assert!(toks.iter().any(|t| t.0 == T::EndKw && t.1 == "end"));
        assert!(
            toks.iter()
                .any(|t| t.0 == T::Connector && t.1 == "connector")
        );
        // The keyword takes the spaces that follow it, as Mermaid's rule does.
        assert!(toks.iter().any(|t| t.0 == T::Global && t.1 == "global "));
    }

    #[test]
    fn a_longer_node_name_wins_over_a_keyword() {
        let toks = kinds("flowchart\n");
        assert!(
            toks.iter()
                .any(|t| t.0 == T::NodeString && t.1 == "flowchart")
        );
    }

    #[test]
    fn reads_the_reference_edge() {
        let toks = kinds("agentflow-beta\nA -.- B\n");
        // The link takes the spaces around it, as Mermaid's rule does.
        assert!(toks.iter().any(|t| t.0 == T::Link && t.1 == " -.- "));
    }
}
