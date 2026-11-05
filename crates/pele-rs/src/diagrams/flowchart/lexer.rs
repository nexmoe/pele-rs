//! The flowchart lexer: Mermaid's jison lexer for `flow.jison`, as pele's `lexer.ts` emulates
//! it. It reads UTF-16 code units, as the jison lexer does, so that what counts as one
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
    Subgraph,
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
            Subgraph => "subgraph",
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
    ThickEdgeText,
    DottedEdgeText,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum LinkKind {
    Normal,
    Thick,
    Dotted,
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
    at_next: Option<usize>,
    line_end: Option<usize>,
    dir_failed: usize,
    run_end: usize,
    run_at: Option<usize>,
    ws: Option<(usize, usize)>,
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

    fn kw(&self, word: &str) -> bool {
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

    /// The first character after any run of whitespace at q, skipping one x, o or <.
    fn skip_ws_cached(&mut self, q: usize) -> usize {
        if let Some((from, to)) = self.ws {
            if q >= from && q <= to {
                return to;
            }
        }
        if is_space(self.c(q)) {
            let j = self.ws_end(q);
            self.ws = Some((q, j));
            return j;
        }
        q
    }

    fn link_char(&mut self, q: usize) -> u32 {
        let j = self.skip_ws_cached(q);
        let d = self.c(j);
        if d == 120 || d == 111 || d == 60 {
            self.c(j + 1)
        } else {
            d
        }
    }

    fn invis_ahead(&mut self, q: usize) -> bool {
        let j = self.skip_ws_cached(q);
        self.c(j) == 126
    }

    /// The patterns of the link tokens: `\s*[xo<]?--+[-xo>]\s*` and the like.
    fn link_re(&self, kind: LinkKind, full: bool, q: usize) -> Option<usize> {
        let q = self.ws_end(q);
        let opt = matches!(self.c(q), 120 | 111 | 60);
        let at = if opt { q + 1 } else { q };
        let end = match kind {
            LinkKind::Normal | LinkKind::Thick => {
                let ch = if kind == LinkKind::Normal { 45 } else { 61 };
                let mut r = at;
                while self.c(r) == ch {
                    r += 1;
                }
                let m = r - at;
                if !full {
                    (m >= 2).then_some(at + 2)?
                } else if m >= 2 && matches!(self.c(r), 120 | 111 | 62) {
                    r + 1
                } else if m >= 3 {
                    r
                } else {
                    return None;
                }
            }
            LinkKind::Dotted => {
                if !full {
                    (self.c(at) == 45 && self.c(at + 1) == 46).then_some(at + 2)?
                } else {
                    let mut r = at;
                    if self.c(r) == 45 {
                        r += 1;
                    }
                    let dots = r;
                    while self.c(r) == 46 {
                        r += 1;
                    }
                    if r == dots || self.c(r) != 45 {
                        return None;
                    }
                    r += 1;
                    if matches!(self.c(r), 120 | 111 | 62) {
                        r += 1;
                    }
                    r
                }
            }
        };
        // For a start link, `--` must not be part of a longer run only when the pattern says so;
        // `\s*[xo<]?--\s*` takes the first two and stops.
        Some(self.ws_end(end))
    }

    /// `\s*~~~+\s*`
    fn invis_re(&self, q: usize) -> Option<usize> {
        let mut r = self.ws_end(q);
        let s = r;
        while self.c(r) == 126 {
            r += 1;
        }
        (r - s >= 3).then(|| self.ws_end(r))
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

    fn invis_link(&mut self) -> bool {
        if !self.invis_ahead(self.p) {
            return false;
        }
        match self.invis_re(self.p) {
            Some(e) => {
                self.emit(T::Link, self.p, e);
                true
            }
            None => false,
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

    fn graph(&mut self, len: usize) {
        if self.first_graph {
            self.first_graph = false;
            self.push(S::Dir);
        }
        self.emit(T::Graph, self.p, self.p + len);
    }

    fn node_string(&mut self) -> bool {
        let mut q = self.p;
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
            } else {
                break;
            }
        }
        if q == self.p {
            return false;
        }
        self.emit(T::NodeString, self.p, q);
        true
    }

    fn link(&mut self, c: u32) -> bool {
        let p = self.p;
        let d = self.link_char(p);
        if d == 45 {
            if let Some(e) = self.link_re(LinkKind::Normal, true, p) {
                self.emit(T::Link, p, e);
                return true;
            }
            if let Some(e) = self.link_re(LinkKind::Normal, false, p) {
                self.push(S::EdgeText);
                self.emit(T::StartLink, p, e);
                return true;
            }
            if let Some(e) = self.link_re(LinkKind::Dotted, true, p) {
                self.emit(T::Link, p, e);
                return true;
            }
            if let Some(e) = self.link_re(LinkKind::Dotted, false, p) {
                self.push(S::DottedEdgeText);
                self.emit(T::StartLink, p, e);
                return true;
            }
        } else if d == 61 {
            if let Some(e) = self.link_re(LinkKind::Thick, true, p) {
                self.emit(T::Link, p, e);
                return true;
            }
            if let Some(e) = self.link_re(LinkKind::Thick, false, p) {
                self.push(S::ThickEdgeText);
                self.emit(T::StartLink, p, e);
                return true;
            }
        } else if d == 46 {
            if let Some(e) = self.link_re(LinkKind::Dotted, true, p) {
                self.emit(T::Link, p, e);
                return true;
            }
        }
        c != 45 && c != 61 && c != 46 && self.invis_link()
    }

    /// `.*direction\s+XX[^\n]*` at p, within the current line.
    fn direction_re(&self, which: &str, line_end: usize) -> Option<usize> {
        let p = self.p;
        for k in (p..line_end).rev() {
            if !self.starts("direction", k) {
                continue;
            }
            let q = k + 9;
            let after = self.ws_end(q);
            if after == q || !self.starts(which, after) {
                continue;
            }
            let from = after + 2;
            return Some(self.find(10, from).unwrap_or(self.n));
        }
        None
    }

    fn initial(&mut self, c: u32) -> bool {
        let p = self.p;
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
                if self.kw("classDef") {
                    self.emit(T::ClassDef, p, p + 8);
                    return true;
                }
                if self.kw("class") {
                    self.emit(T::Class, p, p + 5);
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
                if self.kw("style") {
                    self.emit(T::Style, p, p + 5);
                    return true;
                }
                if self.kw("swimlane-beta") {
                    self.graph(13);
                    return true;
                }
                if self.kw("subgraph") {
                    self.emit(T::Subgraph, p, p + 8);
                    return true;
                }
            }
            100 if self.kw("default") => {
                self.emit(T::Default, p, p + 7);
                return true;
            }
            108 if self.kw("linkStyle") => {
                self.emit(T::LinkStyle, p, p + 9);
                return true;
            }
            105 if self.kw("interpolate") => {
                self.emit(T::Interpolate, p, p + 11);
                return true;
            }
            104 if self.starts("href", p) && is_space(self.c(p + 4)) => {
                self.emit(T::Href, p, p + 5);
                return true;
            }
            102 => {
                if self.kw("flowchart-elk") {
                    self.graph(13);
                    return true;
                }
                if self.kw("flowchart") {
                    self.graph(9);
                    return true;
                }
            }
            103 if self.kw("graph") => {
                self.graph(5);
                return true;
            }
            101 if self.kw("end") => {
                let e = self.ws_end(p + 3);
                self.emit(T::EndKw, p, e);
                return true;
            }
            95 => {
                for word in ["_self", "_blank", "_parent", "_top"] {
                    if self.kw(word) {
                        self.emit(T::LinkTarget, p, p + word.len());
                        return true;
                    }
                }
            }
            _ => {}
        }

        if let Some(mut next) = self.dir_next {
            if p >= self.dir_failed {
                if next < p {
                    self.dir_next = self.find_word("direction", p);
                }
                if let Some(n2) = self.dir_next {
                    next = n2;
                    if self.line_end.is_none_or(|l| l < p) {
                        let end = (p..self.n)
                            .find(|&i| matches!(self.c(i), 10 | 13 | 0x2028 | 0x2029))
                            .unwrap_or(self.n);
                        self.line_end = Some(end);
                    }
                    let line_end = self.line_end.unwrap_or(self.n);
                    if next < line_end {
                        let kinds = [
                            ("TB", T::DirectionTb),
                            ("BT", T::DirectionBt),
                            ("RL", T::DirectionRl),
                            ("LR", T::DirectionLr),
                            ("TD", T::DirectionTd),
                        ];
                        for (which, t) in kinds {
                            if let Some(e) = self.direction_re(which, line_end) {
                                self.emit(t, p, e);
                                return true;
                            }
                        }
                        // No direction statement starts anywhere in the rest of this line.
                        self.dir_failed = line_end;
                    }
                }
            }
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
            120 | 111 => return self.link(c) || self.node_string(),
            60 => {
                if self.link(c) {
                    return true;
                }
                self.emit(T::TagStart, p, p + 1);
                return true;
            }
            45 => {
                if self.link(c) || self.node_string() {
                    return true;
                }
                self.emit(T::Minus, p, p + 1);
                return true;
            }
            61 => return self.link(c),
            46 => return self.link(c) || self.node_string(),
            126 => return self.invis_link(),
            40 => return self.opener(c),
            91 => {
                if self.c(p + 1) == 124 {
                    self.emit(T::VertexWithPropsStart, p, p + 2);
                    return true;
                }
                return self.opener(c);
            }
            124 | 123 => return self.opener(c),
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
            _ => {}
        }

        if is_space(c) {
            if self.link(c) {
                return true;
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

    fn edge_text(&mut self, c: u32, kind: LinkKind, a: u32, b: u32) -> bool {
        let p = self.p;
        if c == 34 {
            self.quote();
            return true;
        }
        let d = self.link_char(p);
        if d == a || d == b {
            if let Some(e) = self.link_re(kind, true, p) {
                self.pop();
                self.emit(T::Link, p, e);
                return true;
            }
        }
        if c == a && (a != 45 || self.c(p + 1) == 45) {
            return false;
        }
        let mut q = p + 1;
        while q < self.n {
            let ch = self.c(q);
            if ch == 34 {
                break;
            }
            if ch == a {
                if a != 45 || self.c(q + 1) == 45 {
                    break;
                }
            } else if ch == 120 || ch == 111 || ch == 60 || ch == b || is_space(ch) {
                let dd = self.link_char(q);
                if (dd == a || dd == b) && self.link_re(kind, true, q).is_some() {
                    break;
                }
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
                    return true;
                }
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
                    None => return self.invis_link() || self.opener(c),
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
                    _ => {
                        if (c == 126 || is_space(c)) && self.invis_link() {
                            return true;
                        }
                        let mut q = p + 1;
                        while q < n
                            && !matches!(self.c(q), 91 | 93 | 40 | 41 | 123 | 125 | 124 | 34)
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
                } else if (c == 126 || is_space(c)) && self.invis_link() {
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
                        if (ch == 126 || is_space(ch))
                            && self.invis_ahead(q)
                            && self.invis_re(q).is_some()
                        {
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
                if (c == 126 || is_space(c)) && self.invis_link() {
                    return true;
                }
                match c {
                    40 => {
                        if d == 93 && !(d == 45 || d == 91 || (d == 40 && self.c(p + 2) == 40)) {
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
                    _ => {
                        if (c == 63 || c == 61) && d == 93 {
                            self.pop();
                            self.emit(T::TrapEnd, p, p + 2);
                            return true;
                        }
                        let mut q = p + 1;
                        while q < n && !matches!(self.c(q), 92 | 91 | 93 | 40 | 41 | 123 | 125 | 47)
                        {
                            q += 1;
                        }
                        self.emit(T::Text, p, q);
                    }
                }
            }
            S::EdgeText => return self.edge_text(c, LinkKind::Normal, 45, NONE - 1),
            S::ThickEdgeText => return self.edge_text(c, LinkKind::Thick, 61, NONE - 1),
            S::DottedEdgeText => return self.edge_text(c, LinkKind::Dotted, 46, 45),
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

/// Splits flowchart text into tokens. Stops at the first character no rule matches, with an
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
        at_next: None,
        line_end: None,
        dir_failed: 0,
        run_end: 0,
        run_at: None,
        ws: None,
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
    fn reads_a_simple_graph() {
        let toks = kinds("graph TD\nA[Start] --> B{Is it?}\n");
        let types: Vec<T> = toks.iter().map(|t| t.0).collect();
        assert_eq!(
            types,
            [
                T::Graph,
                T::Dir,
                T::Newline,
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
        assert_eq!(toks[7].1, " --> ");
    }

    #[test]
    fn reads_edge_text_and_link_kinds() {
        let toks = kinds("flowchart LR\nA -- text --> B\nC -.-> D\nE == x ==> F\nG ~~~ H\n");
        let links: Vec<&str> = toks
            .iter()
            .filter(|t| matches!(t.0, T::Link | T::StartLink | T::EdgeText))
            .map(|t| t.1.as_str())
            .collect();
        assert_eq!(
            links,
            [
                " -- ", "text", " --> ", " -.-> ", " == ", "x", " ==> ", " ~~~ "
            ]
        );
    }

    #[test]
    fn reads_link_ids_and_shape_data() {
        let toks = kinds("flowchart TD\nA e1@--> B\nC@{ shape: rect }\n");
        assert!(toks.iter().any(|t| t.0 == T::LinkId && t.1 == "e1@"));
        assert!(
            toks.iter()
                .any(|t| t.0 == T::ShapeData && t.1.contains("shape: rect"))
        );
    }

    #[test]
    fn reads_directions_and_accessibility() {
        let toks = kinds("graph\nsubgraph a\ndirection LR\nend\naccTitle: T\n");
        assert_eq!(toks[1].0, T::NoDir);
        assert!(toks.iter().any(|t| t.0 == T::DirectionLr));
        assert!(toks.iter().any(|t| t.0 == T::AccTitleValue && t.1 == "T"));
    }
}
