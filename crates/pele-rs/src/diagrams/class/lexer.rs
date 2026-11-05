//! The class diagram lexer: Mermaid's jison lexer for `classDiagram.jison`, as pele's
//! `lexer.ts` emulates it, over UTF-16 code units.

use crate::unicode::is_unicode_letter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum T {
    End,
    Error,
    ClassDiagram,
    Newline,
    Eof,
    Sqs,
    Str,
    Sqe,
    Dot,
    GenericType,
    Label,
    AccTitle,
    AccTitleValue,
    AccDescr,
    AccDescrValue,
    AccDescrMultilineValue,
    StructStart,
    StructStop,
    Namespace,
    StyleSeparator,
    AnnotationStart,
    AnnotationEnd,
    Class,
    Member,
    NoteFor,
    Note,
    ClassDef,
    Alpha,
    Comma,
    DirectionTb,
    DirectionBt,
    DirectionRl,
    DirectionLr,
    Aggregation,
    Extension,
    Composition,
    Dependency,
    Lollipop,
    Line,
    DottedLine,
    Callback,
    Link,
    LinkTarget,
    Click,
    CallbackName,
    CallbackArgs,
    Href,
    Style,
    CssClass,
    Colon,
    Brkt,
    Pct,
    Minus,
    UnicodeText,
    BquoteStr,
    EdgeState,
    EofInStruct,
    OpenInStruct,
    Plus,
    Equals,
    Punctuation,
}

impl T {
    /// The token's name as Mermaid's grammar spells it, for error messages.
    pub(crate) fn name(self) -> &'static str {
        use T::*;
        match self {
            End => "$end",
            Error => "INVALID",
            ClassDiagram => "CLASS_DIAGRAM",
            Newline => "NEWLINE",
            Eof => "EOF",
            Sqs => "SQS",
            Str => "STR",
            Sqe => "SQE",
            Dot => "DOT",
            GenericType => "GENERICTYPE",
            Label => "LABEL",
            AccTitle => "acc_title",
            AccTitleValue => "acc_title_value",
            AccDescr => "acc_descr",
            AccDescrValue => "acc_descr_value",
            AccDescrMultilineValue => "acc_descr_multiline_value",
            StructStart => "STRUCT_START",
            StructStop => "STRUCT_STOP",
            Namespace => "NAMESPACE",
            StyleSeparator => "STYLE_SEPARATOR",
            AnnotationStart => "ANNOTATION_START",
            AnnotationEnd => "ANNOTATION_END",
            Class => "CLASS",
            Member => "MEMBER",
            NoteFor => "NOTE_FOR",
            Note => "NOTE",
            ClassDef => "CLASSDEF",
            Alpha => "ALPHA",
            Comma => "COMMA",
            DirectionTb => "direction_tb",
            DirectionBt => "direction_bt",
            DirectionRl => "direction_rl",
            DirectionLr => "direction_lr",
            Aggregation => "AGGREGATION",
            Extension => "EXTENSION",
            Composition => "COMPOSITION",
            Dependency => "DEPENDENCY",
            Lollipop => "LOLLIPOP",
            Line => "LINE",
            DottedLine => "DOTTED_LINE",
            Callback => "CALLBACK",
            Link => "LINK",
            LinkTarget => "LINK_TARGET",
            Click => "CLICK",
            CallbackName => "CALLBACK_NAME",
            CallbackArgs => "CALLBACK_ARGS",
            Href => "HREF",
            Style => "STYLE",
            CssClass => "CSSCLASS",
            Colon => "COLON",
            Brkt => "BRKT",
            Pct => "PCT",
            Minus => "MINUS",
            UnicodeText => "UNICODE_TEXT",
            BquoteStr => "BQUOTE_STR",
            EdgeState => "EDGE_STATE",
            EofInStruct => "EOF_IN_STRUCT",
            OpenInStruct => "OPEN_IN_STRUCT",
            Plus => "PLUS",
            Equals => "EQUALS",
            Punctuation => "PUNCTUATION",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    String,
    BqString,
    Generic,
    CallbackName,
    CallbackArgs,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Cls,
    ClassBody,
    Namespace,
    NamespaceBody,
}

impl S {
    /// The token each state gives at the end of input, before the parser sees the end itself.
    fn at_end(self) -> T {
        match self {
            S::String => T::Str,
            S::Generic => T::GenericType,
            S::CallbackName => T::CallbackName,
            S::CallbackArgs => T::CallbackArgs,
            S::AccTitle => T::AccTitleValue,
            S::AccDescr => T::AccDescrValue,
            S::AccDescrMultiline => T::AccDescrMultilineValue,
            S::ClassBody | S::NamespaceBody => T::EofInStruct,
            S::Initial | S::BqString | S::Cls | S::Namespace => T::Eof,
        }
    }
}

const NONE: u32 = u32::MAX;
const DIRECTIONS: [T; 4] = [
    T::DirectionTb,
    T::DirectionBt,
    T::DirectionRl,
    T::DirectionLr,
];

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
    c < 128 && ((c as u8).is_ascii_alphanumeric() || c == 95)
}

pub(crate) struct Tokens {
    pub types: Vec<T>,
    pub texts: Vec<String>,
    pub starts: Vec<usize>,
}

/// The direction statements of one line: for each of TB, BT, RL and LR, the last place on the
/// line where `.*direction\s+XX[^\n]*` can match and where that match ends.
struct DirLine {
    end: usize,
    last: [Option<(usize, usize)>; 4],
}

struct Lexer<'a> {
    src: &'a [u16],
    n: usize,
    p: usize,
    state: S,
    stack: Vec<S>,
    out: Tokens,
    dir: Option<DirLine>,
}

impl Lexer<'_> {
    fn c(&self, i: usize) -> u32 {
        self.src.get(i).map_or(NONE, |&u| u32::from(u))
    }

    fn emit(&mut self, t: T, s: usize, e: usize) -> bool {
        self.out.types.push(t);
        self.out.texts.push(String::from_utf16_lossy(
            &self.src[s.min(self.n)..e.min(self.n)],
        ));
        self.out.starts.push(s);
        self.p = e;
        true
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

    /// Whether the source has `word` at `q`, as written.
    fn starts(&self, word: &str, q: usize) -> bool {
        word.bytes()
            .enumerate()
            .all(|(i, b)| self.c(q + i) == u32::from(b))
    }

    fn kw(&self, word: &str) -> bool {
        self.starts(word, self.p) && !is_word(self.c(self.p + word.len()))
    }

    fn skip_ws(&self, mut q: usize) -> usize {
        while q < self.n && is_space(self.c(q)) {
            q += 1;
        }
        q
    }

    fn until(&self, unit: u16) -> usize {
        self.src[self.p.min(self.n)..]
            .iter()
            .position(|&u| u == unit)
            .map_or(self.n, |i| self.p + i)
    }

    /// A run of whitespace is a NEWLINE up to its last line feed; the rest is skipped.
    fn space(&mut self, leaves: bool) {
        let mut q = self.p;
        let mut last_nl = None;
        while q < self.n && is_space(self.c(q)) {
            if self.c(q) == 10 {
                last_nl = Some(q);
            }
            q += 1;
        }
        match last_nl {
            None => self.p = q,
            Some(k) => {
                if leaves {
                    self.pop();
                }
                self.emit(T::Newline, self.p, k + 1);
            }
        }
    }

    /// The keywords that every state recognizes, even inside generics and backtick names.
    fn keyword(&mut self, c: u32) -> bool {
        let p = self.p;
        match c {
            99 => {
                if self.kw("cssClass") {
                    return self.emit(T::CssClass, p, p + 8);
                }
                if self.kw("callback") {
                    return self.emit(T::Callback, p, p + 8);
                }
                if self.kw("click") {
                    return self.emit(T::Click, p, p + 5);
                }
            }
            108 if self.kw("link") => return self.emit(T::Link, p, p + 4),
            110 => {
                if self.kw("note for") {
                    return self.emit(T::NoteFor, p, p + 8);
                }
                if self.kw("note") {
                    return self.emit(T::Note, p, p + 4);
                }
            }
            60 if self.c(p + 1) == 60 => return self.emit(T::AnnotationStart, p, p + 2),
            62 if self.c(p + 1) == 62 => return self.emit(T::AnnotationEnd, p, p + 2),
            104 if self.kw("href") => return self.emit(T::Href, p, p + 4),
            _ => {}
        }
        false
    }

    fn common(&mut self, c: u32) -> bool {
        if self.keyword(c) {
            return true;
        }
        let p = self.p;
        let d = self.c(p + 1);
        match c {
            34 => {
                self.push(S::String);
                self.p += 1;
                return true;
            }
            126 => {
                self.push(S::Generic);
                self.p += 1;
                return true;
            }
            96 => {
                self.push(S::BqString);
                self.p += 1;
                return true;
            }
            95 => {
                for target in ["_self", "_blank", "_parent", "_top"] {
                    if self.kw(target) {
                        return self.emit(T::LinkTarget, p, p + target.len());
                    }
                }
            }
            60 => {
                return if d == 124 {
                    self.emit(T::Extension, p, p + 2)
                } else {
                    self.emit(T::Dependency, p, p + 1)
                };
            }
            124 => return d == 62 && self.emit(T::Extension, p, p + 2),
            62 => return self.emit(T::Dependency, p, p + 1),
            42 => return self.emit(T::Composition, p, p + 1),
            111 if !is_word(d) => return self.emit(T::Aggregation, p, p + 1),
            40 => return d == 41 && self.emit(T::Lollipop, p, p + 2),
            45 => {
                return if d == 45 {
                    self.emit(T::Line, p, p + 2)
                } else {
                    self.emit(T::Minus, p, p + 1)
                };
            }
            46 => {
                return if d == 46 {
                    self.emit(T::DottedLine, p, p + 2)
                } else {
                    self.emit(T::Dot, p, p + 1)
                };
            }
            58 => {
                let mut q = p + 1;
                while q < self.n && !matches!(self.c(q), 58 | 10 | 59) {
                    q += 1;
                }
                if q > p + 1 {
                    return self.emit(T::Label, p, q);
                }
                return d == 58 && self.c(p + 2) == 58 && self.emit(T::StyleSeparator, p, p + 3);
            }
            43 => return self.emit(T::Plus, p, p + 1),
            37 => return self.emit(T::Pct, p, p + 1),
            61 => return self.emit(T::Equals, p, p + 1),
            91 => return self.emit(T::Sqs, p, p + 1),
            93 => return self.emit(T::Sqe, p, p + 1),
            33 | 35 | 36 | 38 | 39 | 44 | 63 | 92 | 47 => {
                return self.emit(T::Punctuation, p, p + 1);
            }
            _ => {}
        }
        if is_word(c) {
            let mut q = p + 1;
            while is_word(self.c(q)) {
                q += 1;
            }
            return self.emit(T::Alpha, p, q);
        }
        c != NONE && is_unicode_letter(c) && self.emit(T::UnicodeText, p, p + 1)
    }

    /// Rules `.*direction\s+TB[^\n]*` and their three siblings, tried in that order at `p`.
    fn direction(&mut self) -> bool {
        let p = self.p;
        if self.dir.as_ref().is_none_or(|d| p > d.end) {
            let end = self.src[p..]
                .iter()
                .position(|&u| matches!(u, 10 | 13 | 0x2028 | 0x2029))
                .map_or(self.n, |i| p + i);
            let mut last = [None; 4];
            let mut d = p;
            while d + 9 <= end {
                if self.starts("direction", d) {
                    let w = self.skip_ws(d + 9);
                    if w > d + 9 {
                        for (k, code) in ["TB", "BT", "RL", "LR"].iter().enumerate() {
                            if self.starts(code, w) {
                                let stop = self.src[w + 2..]
                                    .iter()
                                    .position(|&u| u == 10)
                                    .map_or(self.n, |i| w + 2 + i);
                                last[k] = Some((d, stop));
                            }
                        }
                    }
                }
                d += 1;
            }
            self.dir = Some(DirLine { end, last });
        }
        let line = self.dir.as_ref().expect("line");
        // `.*` is greedy, so the last place on the line wins; it must not lie before `p`.
        let found = (0..4).find_map(|k| {
            line.last[k]
                .filter(|&(d, _)| d >= p)
                .map(|(_, stop)| (k, stop))
        });
        match found {
            Some((k, stop)) => self.emit(DIRECTIONS[k], p, stop),
            None => false,
        }
    }

    fn initial(&mut self, c: u32) -> bool {
        if self.direction() {
            return true;
        }
        if is_space(c) {
            self.space(false);
            return true;
        }
        let p = self.p;
        match c {
            37 if self.c(p + 1) == 37 => {
                let mut e = self.until(10);
                while e < self.n && matches!(self.c(e), 10 | 13) {
                    e += 1;
                }
                self.p = e;
                return true;
            }
            97 => {
                let title = self.starts("accTitle", p);
                if title || self.starts("accDescr", p) {
                    let q = self.skip_ws(p + 8);
                    let d = self.c(q);
                    if d == 58 {
                        let e = self.skip_ws(q + 1);
                        self.emit(if title { T::AccTitle } else { T::AccDescr }, p, e);
                        self.push(if title { S::AccTitle } else { S::AccDescr });
                        return true;
                    }
                    if d == 123 && !title {
                        self.p = self.skip_ws(q + 1);
                        self.push(S::AccDescrMultiline);
                        return true;
                    }
                }
            }
            99 => {
                if self.kw("classDiagram-v2") {
                    return self.emit(T::ClassDiagram, p, p + 15);
                }
                if self.kw("classDiagram") {
                    return self.emit(T::ClassDiagram, p, p + 12);
                }
                if self.starts("call", p) && is_space(self.c(p + 4)) {
                    self.p = self.skip_ws(p + 4);
                    self.push(S::CallbackName);
                    return true;
                }
                if self.kw("classDef") {
                    return self.emit(T::ClassDef, p, p + 8);
                }
                if self.kw("class") {
                    self.push(S::Cls);
                    return self.emit(T::Class, p, p + 5);
                }
            }
            91 if self.starts("*]", p + 1) => return self.emit(T::EdgeState, p, p + 3),
            115 if self.kw("style") => return self.emit(T::Style, p, p + 5),
            110 if self.kw("namespace") => {
                self.push(S::Namespace);
                return self.emit(T::Namespace, p, p + 9);
            }
            44 => return self.emit(T::Comma, p, p + 1),
            35 => return self.emit(T::Brkt, p, p + 1),
            _ => {}
        }
        self.common(c) || (c == 58 && self.emit(T::Colon, p, p + 1))
    }

    fn run(mut self) -> Tokens {
        loop {
            if self.p >= self.n {
                let t = self.state.at_end();
                self.out.types.push(t);
                self.out.texts.push(String::new());
                self.out.starts.push(self.n);
                break;
            }
            let p = self.p;
            let c = self.c(p);
            let ok = match self.state {
                S::Initial => self.initial(c),
                S::Cls => {
                    if is_space(c) {
                        self.space(true);
                        true
                    } else if c == 125 {
                        self.pop();
                        self.pop();
                        self.emit(T::StructStop, p, p + 1)
                    } else if c == 123 {
                        self.push(S::ClassBody);
                        self.emit(T::StructStart, p, p + 1)
                    } else {
                        self.common(c)
                    }
                }
                S::ClassBody => {
                    if c == 34 {
                        self.push(S::String);
                        self.p += 1;
                    } else if c == 125 {
                        self.pop();
                        self.emit(T::StructStop, p, p + 1);
                    } else if c == 91 && self.starts("*]", p + 1) {
                        self.emit(T::EdgeState, p, p + 3);
                    } else if c == 123 {
                        self.emit(T::OpenInStruct, p, p + 1);
                    } else if c == 10 {
                        self.p += 1;
                    } else {
                        let mut q = p + 1;
                        while q < self.n && !matches!(self.c(q), 123 | 125 | 10) {
                            q += 1;
                        }
                        self.emit(T::Member, p, q);
                    }
                    true
                }
                S::Namespace => {
                    if is_space(c) {
                        self.space(true);
                        true
                    } else if c == 123 {
                        self.push(S::NamespaceBody);
                        self.emit(T::StructStart, p, p + 1)
                    } else if c == 125 {
                        // The brace is read again in the state the namespace was opened from.
                        self.pop();
                        true
                    } else if c == 110 && self.kw("namespace") {
                        self.push(S::Namespace);
                        self.emit(T::Namespace, p, p + 9)
                    } else {
                        self.common(c)
                    }
                }
                S::NamespaceBody => {
                    if is_space(c) {
                        self.space(false);
                        true
                    } else if c == 125 {
                        self.pop();
                        self.emit(T::StructStop, p, p + 1)
                    } else if c == 110 && self.kw("namespace") {
                        self.push(S::Namespace);
                        self.emit(T::Namespace, p, p + 9)
                    } else if c == 91 && self.starts("*]", p + 1) {
                        self.emit(T::EdgeState, p, p + 3)
                    } else if c == 99 && self.kw("class") {
                        self.push(S::Cls);
                        self.emit(T::Class, p, p + 5)
                    } else {
                        self.common(c)
                    }
                }
                S::String => {
                    if c == 34 {
                        self.pop();
                        self.p += 1;
                    } else {
                        let e = self.until(34);
                        self.emit(T::Str, p, e);
                    }
                    true
                }
                S::Generic => {
                    if c == 34 {
                        self.push(S::String);
                        self.p += 1;
                    } else if self.keyword(c) {
                    } else if c == 126 {
                        self.pop();
                        self.p += 1;
                    } else {
                        let e = self.until(126);
                        self.emit(T::GenericType, p, e);
                    }
                    true
                }
                S::BqString => {
                    if c == 34 {
                        self.push(S::String);
                        self.p += 1;
                    } else if self.keyword(c) {
                    } else if c == 126 {
                        self.push(S::Generic);
                        self.p += 1;
                    } else if c == 96 {
                        self.pop();
                        self.p += 1;
                    } else {
                        let e = self.until(96);
                        self.emit(T::BquoteStr, p, e);
                    }
                    true
                }
                S::CallbackName => {
                    if c == 40 {
                        let q = self.skip_ws(p + 1);
                        self.pop();
                        if self.c(q) == 41 {
                            self.p = q + 1;
                        } else {
                            self.push(S::CallbackArgs);
                            self.p += 1;
                        }
                    } else {
                        let e = self.until(40);
                        self.emit(T::CallbackName, p, e);
                    }
                    true
                }
                S::CallbackArgs => {
                    if c == 41 {
                        self.pop();
                        self.p += 1;
                    } else {
                        let e = self.until(41);
                        self.emit(T::CallbackArgs, p, e);
                    }
                    true
                }
                S::AccTitle | S::AccDescr => {
                    let t = if self.state == S::AccTitle {
                        T::AccTitleValue
                    } else {
                        T::AccDescrValue
                    };
                    let e = self.until(10);
                    self.emit(t, p, e);
                    self.pop();
                    true
                }
                S::AccDescrMultiline => {
                    if c == 125 {
                        self.pop();
                        self.p += 1;
                    } else {
                        let e = self.until(125);
                        self.emit(T::AccDescrMultilineValue, p, e);
                    }
                    true
                }
            };
            if !ok {
                self.out.types.push(T::Error);
                self.out.texts.push(String::new());
                self.out.starts.push(self.p);
                return self.out;
            }
        }
        self.out.types.push(T::End);
        self.out.texts.push(String::new());
        self.out.starts.push(self.n);
        self.out
    }
}

pub(crate) fn tokenize(src: &[u16]) -> Tokens {
    Lexer {
        src,
        n: src.len(),
        p: 0,
        state: S::Initial,
        stack: Vec::new(),
        out: Tokens {
            types: Vec::new(),
            texts: Vec::new(),
            starts: Vec::new(),
        },
        dir: None,
    }
    .run()
}
