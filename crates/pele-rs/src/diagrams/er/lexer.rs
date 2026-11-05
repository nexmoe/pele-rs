//! The ER diagram lexer: the rules of Mermaid's `erDiagram.jison`, over UTF-16 code units as
//! Jison reads them. Ported from pele's `diagrams/er/lexer.ts`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum T {
    End,
    Error,
    ErDiagram,
    Eof,
    Newline,
    Colon,
    StyleSeparator,
    BlockStart,
    BlockStop,
    Sqs,
    Sqe,
    AccTitle,
    AccTitleValue,
    AccDescr,
    AccDescrValue,
    AccDescrMultilineValue,
    EndKw,
    Subgraph,
    DirectionTb,
    DirectionBt,
    DirectionRl,
    DirectionLr,
    ClassDef,
    UnicodeText,
    StyleText,
    Comma,
    Class,
    Style,
    Semi,
    Num,
    Brkt,
    EntityName,
    DecimalNum,
    EntityOne,
    AttributeWord,
    AttributeKey,
    Comment,
    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
    OnlyOne,
    MdParent,
    NonIdentifying,
    Identifying,
    Word,
    /// A character no rule names; the grammar gives it meaning only as `?` and `,` inside a
    /// block.
    Char,
}

impl T {
    /// The token's name in Mermaid's grammar, as parse errors print it.
    pub(crate) fn name(self) -> &'static str {
        use T::*;
        match self {
            End => "$end",
            Error => "INVALID",
            ErDiagram => "ER_DIAGRAM",
            Eof => "EOF",
            Newline => "NEWLINE",
            Colon => "COLON",
            StyleSeparator => "STYLE_SEPARATOR",
            BlockStart => "BLOCK_START",
            BlockStop => "BLOCK_STOP",
            Sqs => "SQS",
            Sqe => "SQE",
            AccTitle => "acc_title",
            AccTitleValue => "acc_title_value",
            AccDescr => "acc_descr",
            AccDescrValue => "acc_descr_value",
            AccDescrMultilineValue => "acc_descr_multiline_value",
            EndKw => "END",
            Subgraph => "SUBGRAPH",
            DirectionTb => "direction_tb",
            DirectionBt => "direction_bt",
            DirectionRl => "direction_rl",
            DirectionLr => "direction_lr",
            ClassDef => "CLASSDEF",
            UnicodeText => "UNICODE_TEXT",
            StyleText => "STYLE_TEXT",
            Comma => "COMMA",
            Class => "CLASS",
            Style => "STYLE",
            Semi => "SEMI",
            Num => "NUM",
            Brkt => "BRKT",
            EntityName => "ENTITY_NAME",
            DecimalNum => "DECIMAL_NUM",
            EntityOne => "ENTITY_ONE",
            AttributeWord => "ATTRIBUTE_WORD",
            AttributeKey => "ATTRIBUTE_KEY",
            Comment => "COMMENT",
            ZeroOrOne => "ZERO_OR_ONE",
            ZeroOrMore => "ZERO_OR_MORE",
            OneOrMore => "ONE_OR_MORE",
            OnlyOne => "ONLY_ONE",
            MdParent => "MD_PARENT",
            NonIdentifying => "NON_IDENTIFYING",
            Identifying => "IDENTIFYING",
            Word => "WORD",
            Char => "CHAR",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Block,
    BlockBq,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Style,
}

const NONE: u32 = u32::MAX;
const DIRECTIONS: [T; 4] = [
    T::DirectionTb,
    T::DirectionBt,
    T::DirectionRl,
    T::DirectionLr,
];

const TEXT: u8 = 1;
const STYLE_TEXT: u8 = 2;
const ATTR_START: u8 = 4;
const ATTR_REST: u8 = 8;

const fn classes() -> [u8; 128] {
    let mut table = [0u8; 128];
    let mut c = 0;
    while c < 128 {
        let b = c as u8;
        let word = b.is_ascii_alphanumeric() || b == b'_';
        if word {
            table[c] |= TEXT | STYLE_TEXT | ATTR_REST;
            if !b.is_ascii_digit() {
                table[c] |= ATTR_START;
            }
        }
        c += 1;
    }
    table[b'-' as usize] |= TEXT | STYLE_TEXT | ATTR_REST;
    table[b'*' as usize] |= TEXT | STYLE_TEXT | ATTR_REST | ATTR_START;
    table[b'.' as usize] |= TEXT | ATTR_REST;
    let rest = b"[](),";
    let mut k = 0;
    while k < rest.len() {
        table[rest[k] as usize] |= ATTR_REST;
        k += 1;
    }
    table
}

const CLASS: [u8; 128] = classes();

fn has(c: u32, kind: u8) -> bool {
    c < 128 && CLASS[c as usize] & kind != 0
}

/// A character of a text run: any non-ASCII unit, or an ASCII one of the class.
fn in_run(c: u32, kind: u8) -> bool {
    c != NONE && (c >= 128 || has(c, kind))
}

/// Mermaid's attribute pattern is case-insensitive, which lets the micro sign into its
/// `À-￿` range.
fn is_attr(c: u32, kind: u8) -> bool {
    if c < 128 {
        has(c, kind)
    } else {
        c != NONE && (c >= 0xc0 || c == 0xb5)
    }
}

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

fn is_digit(c: u32) -> bool {
    (48..=57).contains(&c)
}

fn is_line_end(c: u32) -> bool {
    matches!(c, 10 | 13 | 0x2028 | 0x2029)
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

/// What the generic type rule last found: the next `~` (`None` once there is none left), the
/// end of the run of non-space characters, and the end of the line with its last `~`.
#[derive(Default)]
struct Tilde {
    next: Option<Option<usize>>,
    run_end: Option<usize>,
    line: Option<(usize, Option<usize>)>,
}

struct Lexer<'a> {
    src: &'a [u16],
    n: usize,
    p: usize,
    state: S,
    out: Tokens,
    dir: Option<DirLine>,
    tilde: Tilde,
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

    /// Whether the source has the lower-case `word` at `q`, ignoring case.
    fn starts_ci(&self, word: &str, q: usize) -> bool {
        word.bytes().enumerate().all(|(i, k)| {
            let c = self.c(q + i);
            c == u32::from(k) || (k.is_ascii_lowercase() && c < 128 && (c | 32) == u32::from(k))
        })
    }

    fn kw(&self, word: &str) -> bool {
        self.starts_ci(word, self.p)
    }

    fn kwb(&self, word: &str) -> bool {
        self.kw(word) && !is_word(self.c(self.p + word.len()))
    }

    fn lower(&self, q: usize) -> u32 {
        let c = self.c(q);
        if (65..=90).contains(&c) { c + 32 } else { c }
    }

    fn find(&self, unit: u16, from: usize) -> Option<usize> {
        self.src[from.min(self.n)..]
            .iter()
            .position(|&u| u == unit)
            .map(|i| from + i)
    }

    fn line_end(&self, q: usize) -> usize {
        self.src[q.min(self.n)..]
            .iter()
            .position(|&u| is_line_end(u32::from(u)))
            .map_or(self.n, |i| q + i)
    }

    fn digits(&self, mut q: usize) -> usize {
        while is_digit(self.c(q)) {
            q += 1;
        }
        q
    }

    fn skip_ws(&self, mut q: usize) -> usize {
        while is_space(self.c(q)) {
            q += 1;
        }
        q
    }

    /// Matches `accTitle\s*:\s*` and its siblings at `p`, ignoring case.
    fn acc(&self, word: &str, punct: u32) -> Option<usize> {
        if !self.kw(word) {
            return None;
        }
        let q = self.skip_ws(self.p + word.len());
        (self.c(q) == punct).then(|| self.skip_ws(q + 1))
    }

    /// Rules `.*direction\s+TB[^\n]*` and their three siblings, tried in that order at `p`.
    fn direction(&mut self) -> bool {
        let p = self.p;
        if self.dir.as_ref().is_none_or(|d| p > d.end) {
            let end = self.line_end(p);
            let mut last = [None; 4];
            let mut d = p;
            while d + 9 <= end {
                if self.starts_ci("direction", d) {
                    let w = self.skip_ws(d + 9);
                    if w > d + 9 {
                        for (k, code) in ["tb", "bt", "rl", "lr"].iter().enumerate() {
                            if self.starts_ci(code, w) {
                                let stop = self.find(10, w + 2).unwrap_or(self.n);
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

    fn quoted(&mut self) {
        let p = self.p;
        let Some(e) = self.find(34, p + 1) else {
            self.emit(T::Char, p, p + 1);
            return;
        };
        let name = e > p + 1
            && self.src[p + 1..e]
                .iter()
                .all(|&u| !matches!(u, 37 | 92 | 13 | 10 | 11 | 8));
        self.emit(if name { T::EntityName } else { T::Word }, p, e + 1);
    }

    fn number(&mut self, c: u32) {
        let p = self.p;
        let d = self.c(p + 1);
        if d == 43 && (c == 49 || c == 48) {
            let t = if c == 49 { T::OneOrMore } else { T::ZeroOrMore };
            self.emit(t, p, p + 2);
            return;
        }
        let q = self.digits(p + 1);
        if self.c(q) == 46 && is_digit(self.c(q + 1)) {
            let e = self.digits(q + 2);
            self.emit(T::DecimalNum, p, e);
            return;
        }
        if c == 49 {
            if is_space(d) {
                let r = self.skip_ws(p + 2);
                let f = self.c(r);
                if is_word(f) || f == 34 || f == 39 {
                    self.emit(T::OnlyOne, p, p + 1);
                    return;
                }
            } else if d == 45 || d == 46 {
                let f = self.c(p + 2);
                if f == 45 || f == 46 {
                    self.emit(T::OnlyOne, p, p + 1);
                    return;
                }
            }
            if !is_word(d) {
                self.emit(T::EntityOne, p, p + 1);
                return;
            }
        }
        self.emit(T::Num, p, q);
    }

    fn keyword(&mut self, lc: u32) -> bool {
        let p = self.p;
        let d = self.c(p + 1);
        match lc {
            101 => {
                if self.kwb("erdiagram") {
                    self.emit(T::ErDiagram, p, p + 9)
                } else if self.kwb("end") {
                    let e = self.skip_ws(p + 3);
                    self.emit(T::EndKw, p, e)
                } else {
                    false
                }
            }
            115 => {
                if self.kwb("style") {
                    self.state = S::Style;
                    self.emit(T::Style, p, p + 5)
                } else if self.kwb("subgraph") {
                    self.emit(T::Subgraph, p, p + 8)
                } else {
                    false
                }
            }
            99 => {
                if self.kwb("classdef") {
                    self.state = S::Style;
                    self.emit(T::ClassDef, p, p + 8)
                } else if self.kwb("class") {
                    self.emit(T::Class, p, p + 5)
                } else {
                    false
                }
            }
            111 => {
                if self.kwb("one or zero") {
                    self.emit(T::ZeroOrOne, p, p + 11)
                } else if self.kwb("one or more") || self.kwb("one or many") {
                    self.emit(T::OneOrMore, p, p + 11)
                } else if self.kwb("one") {
                    self.emit(T::OnlyOne, p, p + 3)
                } else if self.kwb("only one") {
                    self.emit(T::OnlyOne, p, p + 8)
                } else if d == 124 {
                    self.emit(T::ZeroOrOne, p, p + 2)
                } else if d == 123 {
                    self.emit(T::ZeroOrMore, p, p + 2)
                } else if self.kwb("optionally to") {
                    self.emit(T::NonIdentifying, p, p + 13)
                } else {
                    false
                }
            }
            122 => {
                if self.kwb("zero or one") {
                    self.emit(T::ZeroOrOne, p, p + 11)
                } else if self.kwb("zero or more") || self.kwb("zero or many") {
                    self.emit(T::ZeroOrMore, p, p + 12)
                } else {
                    false
                }
            }
            109 => {
                if self.kw("many(0)") {
                    self.emit(T::ZeroOrMore, p, p + 7)
                } else if self.kw("many(1)") {
                    self.emit(T::OneOrMore, p, p + 7)
                } else if self.kwb("many") {
                    self.emit(T::ZeroOrMore, p, p + 4)
                } else {
                    false
                }
            }
            116 => self.kwb("to") && self.emit(T::Identifying, p, p + 2),
            117 => matches!(d, 46 | 45 | 124) && self.emit(T::MdParent, p, p + 1),
            124 => {
                if self.lower(p + 1) == 111 && !is_word(self.c(p + 2)) {
                    self.emit(T::ZeroOrOne, p, p + 2)
                } else if d == 124 {
                    self.emit(T::OnlyOne, p, p + 2)
                } else if d == 123 {
                    self.emit(T::OneOrMore, p, p + 2)
                } else {
                    false
                }
            }
            125 => {
                if self.lower(p + 1) == 111 && !is_word(self.c(p + 2)) {
                    self.emit(T::ZeroOrMore, p, p + 2)
                } else if d == 124 {
                    self.emit(T::OneOrMore, p, p + 2)
                } else {
                    false
                }
            }
            46 => matches!(d, 46 | 45) && self.emit(T::NonIdentifying, p, p + 2),
            45 => {
                if d == 45 {
                    self.emit(T::Identifying, p, p + 2)
                } else if d == 46 {
                    self.emit(T::NonIdentifying, p, p + 2)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn initial(&mut self, c: u32) {
        let p = self.p;
        if (c == 97 || c == 65) && self.kw("acc") {
            if let Some(e) = self.acc("acctitle", 58) {
                self.state = S::AccTitle;
                self.emit(T::AccTitle, p, e);
                return;
            }
            if let Some(e) = self.acc("accdescr", 58) {
                self.state = S::AccDescr;
                self.emit(T::AccDescr, p, e);
                return;
            }
            if let Some(e) = self.acc("accdescr", 123) {
                self.state = S::AccDescrMultiline;
                self.p = e;
                return;
            }
        }
        if self.direction() {
            return;
        }
        match c {
            32 | 9 | 13 => {
                let mut e = p + 1;
                while matches!(self.c(e), 32 | 9 | 13) {
                    e += 1;
                }
                self.p = e;
                return;
            }
            10 => {
                let mut e = p + 1;
                while self.c(e) == 10 {
                    e += 1;
                }
                self.emit(T::Newline, p, e);
                return;
            }
            34 => return self.quoted(),
            123 => {
                self.state = S::Block;
                self.emit(T::BlockStart, p, p + 1);
                return;
            }
            35 => {
                self.emit(T::Brkt, p, p + 1);
                return;
            }
            44 => {
                self.emit(T::Comma, p, p + 1);
                return;
            }
            58 => {
                if self.c(p + 1) == 58 && self.c(p + 2) == 58 {
                    self.emit(T::StyleSeparator, p, p + 3);
                } else {
                    self.emit(T::Colon, p, p + 1);
                }
                return;
            }
            91 => {
                self.emit(T::Sqs, p, p + 1);
                return;
            }
            93 => {
                self.emit(T::Sqe, p, p + 1);
                return;
            }
            _ => {}
        }
        if is_digit(c) {
            return self.number(c);
        }
        if self.keyword(if (65..=90).contains(&c) { c + 32 } else { c }) {
            return;
        }
        if in_run(c, TEXT) {
            let mut e = p + 1;
            while in_run(self.c(e), TEXT) {
                e += 1;
            }
            self.emit(T::UnicodeText, p, e);
            return;
        }
        self.emit(T::Char, p, p + 1);
    }

    /// Mermaid's generic type rule: a run of non-space characters holding a `~`, through the
    /// last `~` on the line.
    fn generic(&mut self) -> bool {
        let p = self.p;
        let next = match self.tilde.next {
            Some(None) => return false,
            Some(Some(q)) if q >= p => q,
            _ => {
                let found = self.find(126, p);
                self.tilde.next = Some(found);
                match found {
                    Some(q) => q,
                    None => return false,
                }
            }
        };
        let run_end = match self.tilde.run_end {
            Some(q) if q >= p => q,
            _ => {
                let mut q = p;
                while q < self.n && !is_space(self.c(q)) {
                    q += 1;
                }
                self.tilde.run_end = Some(q);
                q
            }
        };
        if next >= run_end {
            return false;
        }
        let last = match self.tilde.line {
            Some((end, last)) if end >= p => last,
            _ => {
                let end = self.line_end(p);
                let last = self.src[..end].iter().rposition(|&u| u == 126);
                self.tilde.line = Some((end, last));
                last
            }
        };
        let Some(last) = last.filter(|&last| last > next) else {
            return false;
        };
        let mut e = last + 1;
        while e < self.n && !is_space(self.c(e)) {
            e += 1;
        }
        self.emit(T::AttributeWord, p, e)
    }

    fn block(&mut self, c: u32) {
        let p = self.p;
        if is_space(c) {
            self.p = self.skip_ws(p + 1);
            return;
        }
        let lc = c | 32;
        if matches!(lc, 112 | 102 | 117) && (self.c(p + 1) | 32) == 107 && !is_word(self.c(p + 2)) {
            self.emit(T::AttributeKey, p, p + 2);
            return;
        }
        if self.generic() {
            return;
        }
        if is_attr(c, ATTR_START) {
            let mut e = p + 1;
            while e < self.n && is_attr(self.c(e), ATTR_REST) {
                e += 1;
            }
            self.emit(T::AttributeWord, p, e);
            return;
        }
        if c == 96 {
            self.state = S::BlockBq;
            self.p += 1;
            return;
        }
        if c == 34 {
            if let Some(e) = self.find(34, p + 1) {
                self.emit(T::Comment, p, e + 1);
                return;
            }
        } else if c == 125 {
            self.state = S::Initial;
            self.emit(T::BlockStop, p, p + 1);
            return;
        }
        self.emit(T::Char, p, p + 1);
    }

    /// Returns false when the input holds text no rule matches.
    fn style(&mut self, c: u32) -> bool {
        let p = self.p;
        match c {
            10 => {
                let mut e = p + 1;
                while self.c(e) == 10 {
                    e += 1;
                }
                self.state = S::Initial;
                self.emit(T::Newline, p, e);
            }
            _ if is_space(c) => self.p = self.skip_ws(p + 1),
            58 => _ = self.emit(T::Colon, p, p + 1),
            44 => _ = self.emit(T::Comma, p, p + 1),
            35 => _ = self.emit(T::Brkt, p, p + 1),
            59 => _ = self.emit(T::Semi, p, p + 1),
            _ if in_run(c, STYLE_TEXT) => {
                let mut e = p + 1;
                while in_run(self.c(e), STYLE_TEXT) {
                    e += 1;
                }
                self.emit(T::StyleText, p, e);
            }
            _ => return false,
        }
        true
    }
}

pub(crate) fn tokenize(src: &[u16]) -> Tokens {
    let mut lx = Lexer {
        src,
        n: src.len(),
        p: 0,
        state: S::Initial,
        out: Tokens {
            types: Vec::new(),
            texts: Vec::new(),
            starts: Vec::new(),
        },
        dir: None,
        tilde: Tilde::default(),
    };
    let n = lx.n;
    while lx.p < n {
        let c = lx.c(lx.p);
        let p = lx.p;
        match lx.state {
            S::Initial => lx.initial(c),
            S::Block => lx.block(c),
            S::BlockBq => {
                if c == 96 {
                    lx.state = S::Block;
                    lx.p += 1;
                } else {
                    let e = lx.find(96, p).unwrap_or(n);
                    lx.emit(T::AttributeWord, p, e);
                }
            }
            S::AccTitle | S::AccDescr => {
                let e = lx.find(10, p).unwrap_or(n);
                let t = if lx.state == S::AccTitle {
                    T::AccTitleValue
                } else {
                    T::AccDescrValue
                };
                lx.emit(t, p, e);
                lx.state = S::Initial;
            }
            S::AccDescrMultiline => {
                if c == 125 {
                    lx.state = S::Initial;
                    lx.p += 1;
                } else {
                    let e = lx.find(125, p).unwrap_or(n);
                    lx.emit(T::AccDescrMultilineValue, p, e);
                }
            }
            S::Style => {
                if !lx.style(c) {
                    let out = &mut lx.out;
                    out.types.push(T::Error);
                    out.texts.push(String::new());
                    out.starts.push(p);
                    return lx.out;
                }
            }
        }
    }

    // At the end of input Jison's INITIAL state yields EOF, the title and description states
    // yield an empty value, and the block and style states yield nothing.
    let last = match lx.state {
        S::Initial => Some(T::Eof),
        S::AccTitle => Some(T::AccTitleValue),
        S::AccDescr => Some(T::AccDescrValue),
        S::AccDescrMultiline => Some(T::AccDescrMultilineValue),
        S::Block | S::BlockBq | S::Style => None,
    };
    let mut out = lx.out;
    if let Some(last) = last {
        out.types.push(last);
        out.texts.push(String::new());
        out.starts.push(n);
    }
    out.types.push(T::End);
    out.texts.push(String::new());
    out.starts.push(n);
    out
}
