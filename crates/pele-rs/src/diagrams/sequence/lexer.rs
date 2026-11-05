//! The sequence diagram lexer: Mermaid's jison lexer for `sequenceDiagram.jison`, as pele's
//! `lexer.ts` emulates it, over UTF-16 code units.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum T {
    End,
    Error,
    Invalid,
    Newline,
    Sd,
    Create,
    Box,
    RestOfLine,
    EndKw,
    Autonumber,
    Num,
    Off,
    Activate,
    Deactivate,
    Title,
    LegacyTitle,
    AccTitle,
    AccTitleValue,
    AccDescr,
    AccDescrValue,
    AccDescrMultilineValue,
    Loop,
    Rect,
    Opt,
    Alt,
    Par,
    ParOver,
    Critical,
    Break,
    Option,
    And,
    Else,
    Participant,
    As,
    ParticipantActor,
    Destroy,
    Note,
    Over,
    Links,
    Link,
    Properties,
    Details,
    Comma,
    LeftOf,
    RightOf,
    Plus,
    Minus,
    Central,
    Actor,
    ConfigStart,
    ConfigContent,
    ConfigEnd,
    Txt,
    SolidOpenArrow,
    DottedOpenArrow,
    SolidArrow,
    BidirectionalSolidArrow,
    DottedArrow,
    BidirectionalDottedArrow,
    SolidCross,
    DottedCross,
    SolidPoint,
    DottedPoint,
    SolidArrowTop,
    SolidArrowBottom,
    StickArrowTop,
    StickArrowBottom,
    SolidArrowTopDotted,
    SolidArrowBottomDotted,
    StickArrowTopDotted,
    StickArrowBottomDotted,
    SolidArrowTopReverse,
    SolidArrowBottomReverse,
    StickArrowTopReverse,
    StickArrowBottomReverse,
    SolidArrowTopReverseDotted,
    SolidArrowBottomReverseDotted,
    StickArrowTopReverseDotted,
    StickArrowBottomReverseDotted,
}

impl T {
    /// The token's name as Mermaid's grammar spells it, for error messages.
    pub(crate) fn name(self) -> &'static str {
        use T::*;
        match self {
            End => "$end",
            Error | Invalid => "INVALID",
            Newline => "NEWLINE",
            Sd => "SD",
            Create => "create",
            Box => "box",
            RestOfLine => "restOfLine",
            EndKw => "end",
            Autonumber => "autonumber",
            Num => "NUM",
            Off => "off",
            Activate => "activate",
            Deactivate => "deactivate",
            Title => "title",
            LegacyTitle => "legacy_title",
            AccTitle => "acc_title",
            AccTitleValue => "acc_title_value",
            AccDescr => "acc_descr",
            AccDescrValue => "acc_descr_value",
            AccDescrMultilineValue => "acc_descr_multiline_value",
            Loop => "loop",
            Rect => "rect",
            Opt => "opt",
            Alt => "alt",
            Par => "par",
            ParOver => "par_over",
            Critical => "critical",
            Break => "break",
            Option => "option",
            And => "and",
            Else => "else",
            Participant => "participant",
            As => "AS",
            ParticipantActor => "participant_actor",
            Destroy => "destroy",
            Note => "note",
            Over => "over",
            Links => "links",
            Link => "link",
            Properties => "properties",
            Details => "details",
            Comma => ",",
            LeftOf => "left_of",
            RightOf => "right_of",
            Plus => "+",
            Minus => "-",
            Central => "()",
            Actor => "ACTOR",
            ConfigStart => "CONFIG_START",
            ConfigContent => "CONFIG_CONTENT",
            ConfigEnd => "CONFIG_END",
            Txt => "TXT",
            SolidOpenArrow => "SOLID_OPEN_ARROW",
            DottedOpenArrow => "DOTTED_OPEN_ARROW",
            SolidArrow => "SOLID_ARROW",
            BidirectionalSolidArrow => "BIDIRECTIONAL_SOLID_ARROW",
            DottedArrow => "DOTTED_ARROW",
            BidirectionalDottedArrow => "BIDIRECTIONAL_DOTTED_ARROW",
            SolidCross => "SOLID_CROSS",
            DottedCross => "DOTTED_CROSS",
            SolidPoint => "SOLID_POINT",
            DottedPoint => "DOTTED_POINT",
            SolidArrowTop => "SOLID_ARROW_TOP",
            SolidArrowBottom => "SOLID_ARROW_BOTTOM",
            StickArrowTop => "STICK_ARROW_TOP",
            StickArrowBottom => "STICK_ARROW_BOTTOM",
            SolidArrowTopDotted => "SOLID_ARROW_TOP_DOTTED",
            SolidArrowBottomDotted => "SOLID_ARROW_BOTTOM_DOTTED",
            StickArrowTopDotted => "STICK_ARROW_TOP_DOTTED",
            StickArrowBottomDotted => "STICK_ARROW_BOTTOM_DOTTED",
            SolidArrowTopReverse => "SOLID_ARROW_TOP_REVERSE",
            SolidArrowBottomReverse => "SOLID_ARROW_BOTTOM_REVERSE",
            StickArrowTopReverse => "STICK_ARROW_TOP_REVERSE",
            StickArrowBottomReverse => "STICK_ARROW_BOTTOM_REVERSE",
            SolidArrowTopReverseDotted => "SOLID_ARROW_TOP_REVERSE_DOTTED",
            SolidArrowBottomReverseDotted => "SOLID_ARROW_BOTTOM_REVERSE_DOTTED",
            StickArrowTopReverseDotted => "STICK_ARROW_TOP_REVERSE_DOTTED",
            StickArrowBottomReverseDotted => "STICK_ARROW_BOTTOM_REVERSE_DOTTED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Id,
    Alias,
    Line,
    Config,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
}

// Character classes of Mermaid's lexer rules.
const ACTOR_START: u8 = 1;
const ACTOR_REST: u8 = 2;
const ID: u8 = 4;
const ID_WIDE: u8 = 8;
const LINE: u8 = 16;
const MENU: u8 = 32;

const fn class_table() -> [u8; 128] {
    let mut t = [0u8; 128];
    let mut c = 0;
    while c < 128 {
        let ch = c as u8;
        let ws = ch == 32 || (ch >= 9 && ch <= 13);
        let mut bits = 0;
        if !contains(b"/\\+()<->:\n,;", ch) {
            bits |= ACTOR_START;
        }
        if !contains(b"+<->:\n,;", ch) {
            bits |= ACTOR_REST;
        }
        if !contains(b"<>:\n,;@", ch) {
            bits |= ID_WIDE | if ws { 0 } else { ID };
        }
        if !contains(b"#\n;", ch) {
            bits |= LINE;
        }
        if !contains(b"/\\+()<>:\n,;-", ch) && !ws {
            bits |= MENU;
        }
        t[c] = bits;
        c += 1;
    }
    t
}

const fn contains(set: &[u8], ch: u8) -> bool {
    let mut i = 0;
    while i < set.len() {
        if set[i] == ch {
            return true;
        }
        i += 1;
    }
    false
}

const CLASS: [u8; 128] = class_table();
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
    c < 128 && ((c as u8).is_ascii_alphanumeric() || c == 95)
}

fn is_digit(c: u32) -> bool {
    (48..=57).contains(&c)
}

pub(crate) struct Tokens {
    pub types: Vec<T>,
    pub texts: Vec<String>,
    pub starts: Vec<usize>,
}

struct Lexer<'a> {
    src: &'a [u16],
    n: usize,
    p: usize,
    state: S,
    stack: Vec<S>,
    next_nl: Option<usize>,
    out: Tokens,
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
        self.emit_text(t, s, e, text);
    }

    fn emit_text(&mut self, t: T, s: usize, e: usize, text: String) {
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

    fn eol(&mut self, q: usize) -> usize {
        if self.next_nl.is_none_or(|nl| nl < q) {
            let nl = self.src[q.min(self.n)..]
                .iter()
                .position(|&u| u == 10)
                .map_or(self.n, |i| q + i);
            self.next_nl = Some(nl);
        }
        self.next_nl.unwrap_or(self.n)
    }

    fn has(&self, q: usize, bit: u8) -> bool {
        let c = self.c(q);
        if c < 128 {
            return CLASS[c as usize] & bit != 0;
        }
        if bit == ID || bit == MENU {
            !is_space(c)
        } else {
            true
        }
    }

    /// Matches a lowercase literal without regard to the case of the source.
    fn ci(&self, q: usize, word: &str) -> bool {
        word.bytes().enumerate().all(|(k, w)| {
            let c = self.c(q + k);
            if w.is_ascii_lowercase() {
                c | 32 == u32::from(w)
            } else {
                c == u32::from(w)
            }
        })
    }

    fn word(&self, lit: &str) -> bool {
        self.ci(self.p, lit) && !is_word(self.c(self.p + lit.len()))
    }

    fn skip_ws(&self, mut q: usize) -> usize {
        while q < self.n && is_space(self.c(q)) {
            q += 1;
        }
        q
    }

    fn skip_same_line(&mut self) -> bool {
        let mut q = self.p;
        while q < self.n {
            let c = self.c(q);
            if c == 10 || !is_space(c) {
                break;
            }
            q += 1;
        }
        if q == self.p {
            return false;
        }
        self.p = q;
        true
    }

    fn line_run(&self, mut q: usize) -> usize {
        while q < self.n && self.has(q, LINE) {
            q += 1;
        }
        q
    }

    fn begin(&mut self, t: T, len: usize, next: S) {
        self.emit(t, self.p, self.p + len);
        self.push(next);
    }

    fn keyword(&mut self, lit: &str, t: T, next: Option<S>) -> bool {
        if !self.word(lit) {
            return false;
        }
        self.emit(t, self.p, self.p + lit.len());
        if let Some(next) = next {
            self.push(next);
        }
        true
    }

    /// The actor menu keywords count only when spaces and the start of a name follow.
    fn menu(&mut self, lit: &str, t: T) -> bool {
        if !self.ci(self.p, lit) {
            return false;
        }
        let mut q = self.p + lit.len();
        let mut c = self.c(q);
        if c != 32 && c != 9 {
            return false;
        }
        while c == 32 || c == 9 {
            q += 1;
            c = self.c(q);
        }
        if q >= self.n || !self.has(q, MENU) {
            return false;
        }
        self.emit(t, self.p, self.p + lit.len());
        true
    }

    fn arrow_ahead(&self, q: usize) -> bool {
        let c = self.c(q);
        let d = self.c(q + 1);
        if c == 45 {
            if matches!(d, 120 | 88 | 41 | 92 | 47 | 45) {
                return true;
            }
            let e = self.c(q + 2);
            return d == 124 && (e == 92 || e == 47);
        }
        if c == 47 || c == 92 {
            return (d == 124 || d == c) && self.c(q + 2) == 45;
        }
        c == 40 && d == 41
    }

    fn actor(&mut self) {
        let p = self.p;
        let mut q = p + 1;
        while q < self.n && self.has(q, ACTOR_START) {
            q += 1;
        }
        while q < self.n && !self.arrow_ahead(q) {
            let mut r = q;
            while self.c(r) == 45 {
                r += 1;
            }
            if r >= self.n || !self.has(r, ACTOR_REST) {
                break;
            }
            r += 1;
            while r < self.n && self.has(r, ACTOR_REST) {
                r += 1;
            }
            q = r;
        }
        let text = crate::util::trim(&self.text(p, q)).to_string();
        self.emit_text(T::Actor, p, q, text);
    }

    fn number(&mut self) -> bool {
        let p = self.p;
        let mut q = p;
        while is_digit(self.c(q)) {
            q += 1;
        }
        let whole = q > p;
        if self.c(q) == 46 && is_digit(self.c(q + 1)) {
            let r = if is_digit(self.c(q + 2)) {
                q + 3
            } else {
                q + 2
            };
            let after = self.c(r);
            if after == 32 || after == 10 {
                self.emit(T::Num, p, r);
                return true;
            }
        }
        let after = self.c(q);
        if whole && (after == 32 || after == 10) {
            self.emit(T::Num, p, q);
            return true;
        }
        false
    }

    fn dash(&mut self) {
        let p = self.p;
        let (d, e) = (self.c(p + 1), self.c(p + 2));
        let (t, len) = if d == 62 {
            if e == 62 {
                (T::SolidArrow, 3)
            } else {
                (T::SolidOpenArrow, 2)
            }
        } else if d == 120 || d == 88 {
            (T::SolidCross, 2)
        } else if d == 41 {
            (T::SolidPoint, 2)
        } else if d == 124 && e == 92 {
            (T::SolidArrowTop, 3)
        } else if d == 124 && e == 47 {
            (T::SolidArrowBottom, 3)
        } else if d == 92 && e == 92 {
            (T::StickArrowTop, 3)
        } else if d == 47 && e == 47 {
            (T::StickArrowBottom, 3)
        } else if d == 45 {
            let f = self.c(p + 3);
            if e == 62 {
                if f == 62 {
                    (T::DottedArrow, 4)
                } else {
                    (T::DottedOpenArrow, 3)
                }
            } else if e == 120 || e == 88 {
                (T::DottedCross, 3)
            } else if e == 41 {
                (T::DottedPoint, 3)
            } else if e == 124 && f == 92 {
                (T::SolidArrowTopDotted, 4)
            } else if e == 124 && f == 47 {
                (T::SolidArrowBottomDotted, 4)
            } else if e == 92 && f == 92 {
                (T::StickArrowTopDotted, 4)
            } else if e == 47 && f == 47 {
                (T::StickArrowBottomDotted, 4)
            } else {
                (T::Minus, 1)
            }
        } else {
            (T::Minus, 1)
        };
        self.emit(t, p, p + len);
    }

    fn reverse(&mut self, c: u32) -> bool {
        let p = self.p;
        let d = self.c(p + 1);
        if (d != 124 && d != c) || self.c(p + 2) != 45 {
            return false;
        }
        let dotted = self.c(p + 3) == 45;
        let top = c == 47;
        let t = match (d == 124, dotted, top) {
            (true, true, true) => T::SolidArrowTopReverseDotted,
            (true, true, false) => T::SolidArrowBottomReverseDotted,
            (true, false, true) => T::SolidArrowTopReverse,
            (true, false, false) => T::SolidArrowBottomReverse,
            (false, true, true) => T::StickArrowTopReverseDotted,
            (false, true, false) => T::StickArrowBottomReverseDotted,
            (false, false, true) => T::StickArrowTopReverse,
            (false, false, false) => T::StickArrowBottomReverse,
        };
        self.emit(t, p, p + if dotted { 4 } else { 3 });
        true
    }

    fn title(&mut self) -> bool {
        let p = self.p;
        if !self.ci(p, "title") {
            return false;
        }
        let mut q = p + 5;
        let legacy = self.c(q) == 58;
        if legacy {
            q += 1;
        }
        if !is_space(self.c(q)) || q + 1 >= self.n || !self.has(q + 1, LINE) {
            return false;
        }
        let end = self.line_run(q + 1);
        self.emit(if legacy { T::LegacyTitle } else { T::Title }, p, end);
        true
    }

    fn acc(&mut self) -> bool {
        let p = self.p;
        let descr = self.ci(p, "accdescr");
        if !descr && !self.ci(p, "acctitle") {
            return false;
        }
        let q = self.skip_ws(p + 8);
        let c = self.c(q);
        if c == 58 {
            let end = self.skip_ws(q + 1);
            self.emit(if descr { T::AccDescr } else { T::AccTitle }, p, end);
            self.push(if descr { S::AccDescr } else { S::AccTitle });
            return true;
        }
        if descr && c == 123 {
            self.p = self.skip_ws(q + 1);
            self.push(S::AccDescrMultiline);
            return true;
        }
        false
    }

    fn keywords(&mut self, c: u32) -> bool {
        match c | 32 {
            97 => {
                self.keyword("actor", T::ParticipantActor, Some(S::Id))
                    || self.keyword("alt", T::Alt, Some(S::Line))
                    || self.keyword("and", T::And, Some(S::Line))
                    || self.keyword("activate", T::Activate, Some(S::Id))
                    || self.acc()
                    || self.keyword("autonumber", T::Autonumber, None)
            }
            98 => {
                self.keyword("box", T::Box, Some(S::Line))
                    || self.keyword("break", T::Break, Some(S::Line))
            }
            99 => {
                self.keyword("create", T::Create, None)
                    || self.keyword("critical", T::Critical, Some(S::Line))
            }
            100 => {
                self.keyword("destroy", T::Destroy, Some(S::Id))
                    || self.menu("details", T::Details)
                    || self.keyword("deactivate", T::Deactivate, Some(S::Id))
            }
            101 => {
                self.keyword("else", T::Else, Some(S::Line)) || self.keyword("end", T::EndKw, None)
            }
            108 => {
                self.keyword("loop", T::Loop, Some(S::Line))
                    || self.keyword("left of", T::LeftOf, None)
                    || self.menu("links", T::Links)
                    || self.menu("link", T::Link)
            }
            110 => self.keyword("note", T::Note, None),
            111 => {
                self.keyword("opt", T::Opt, Some(S::Line))
                    || self.keyword("option", T::Option, Some(S::Line))
                    || self.keyword("over", T::Over, None)
                    || self.keyword("off", T::Off, None)
            }
            112 => {
                self.keyword("participant", T::Participant, Some(S::Id))
                    || self.keyword("par", T::Par, Some(S::Line))
                    || self.keyword("par_over", T::ParOver, Some(S::Line))
                    || self.menu("properties", T::Properties)
            }
            114 => {
                self.keyword("rect", T::Rect, Some(S::Line))
                    || self.keyword("right of", T::RightOf, None)
            }
            115 => self.keyword("sequencediagram", T::Sd, None),
            116 => self.title(),
            _ => false,
        }
    }

    fn initial(&mut self) {
        let p = self.p;
        let c = self.c(p);
        if c == 10 {
            let mut q = p + 1;
            while self.c(q) == 10 {
                q += 1;
            }
            self.emit(T::Newline, p, q);
            return;
        }
        if is_space(c) {
            self.p = self.skip_ws(p + 1);
            return;
        }
        if c == 35 || (c == 37 && self.c(p + 1) != 123) {
            self.p = self.eol(p);
            return;
        }
        if c != 125 && self.c(p + 1) == 37 && self.c(p + 2) == 37 {
            self.p = self.eol(p);
            return;
        }
        if (is_digit(c) || c == 46) && self.number() {
            return;
        }
        if c < 128 && ((c | 32) as u8).is_ascii_lowercase() && self.keywords(c) {
            return;
        }
        if c == 44 {
            self.emit(T::Comma, p, p + 1);
        } else if c == 59 {
            self.emit(T::Newline, p, p + 1);
        } else if self.has(p, ACTOR_START) {
            self.actor();
        } else if c == 45 {
            self.dash();
        } else if c == 60 && self.ci(p, "<<->>") {
            self.emit(T::BidirectionalSolidArrow, p, p + 5);
        } else if c == 60 && self.ci(p, "<<-->>") {
            self.emit(T::BidirectionalDottedArrow, p, p + 6);
        } else if (c == 47 || c == 92) && self.reverse(c) {
        } else if c == 58 {
            let end = self.line_run(p + 1);
            self.emit(T::Txt, p, end);
        } else if c == 43 {
            self.emit(T::Plus, p, p + 1);
        } else if c == 40 && self.c(p + 1) == 41 {
            self.emit(T::Central, p, p + 2);
        } else {
            self.emit(T::Invalid, p, p + 1);
        }
    }

    /// A participant name, up to an alias, a config object, or the end of the statement.
    fn id(&mut self) -> bool {
        if self.skip_same_line() {
            return true;
        }
        let p = self.p;
        let c = self.c(p);
        if c == 35 {
            self.p = self.eol(p);
            return true;
        }
        if c == 64 && self.c(p + 1) == 123 {
            self.begin(T::ConfigStart, 2, S::Config);
            return true;
        }
        let mut a = p;
        while a < self.n && self.has(a, ID) {
            a += 1;
        }
        if a > p {
            let w = self.skip_ws(a);
            if self.c(w) == 64 && self.c(w + 1) == 123 {
                self.emit(T::Actor, p, a);
                return true;
            }
            if w > a && self.ci(w, "as") && is_space(self.c(w + 2)) {
                self.emit(T::Actor, p, a);
                self.push(S::Alias);
                return true;
            }
        }
        let mut b = a;
        while b < self.n && self.has(b, ID_WIDE) {
            b += 1;
        }
        if b > p {
            let end = self.c(b);
            // Before any other delimiter the name must be followed by a comment, so it ends at
            // the last one.
            let mut cut = b;
            if b < self.n && end != 10 && end != 59 {
                cut -= 1;
                while cut > p && self.c(cut) != 35 {
                    cut -= 1;
                }
            }
            if cut > p {
                let text = crate::util::trim(&self.text(p, cut)).to_string();
                self.emit_text(T::Actor, p, cut, text);
                self.pop();
                return true;
            }
        }
        if self.c(b) == 60 {
            let e = self.eol(p);
            self.emit(T::Invalid, p, e);
            self.pop();
            return true;
        }
        if c != 10 {
            let e = self.eol(p);
            let text = crate::util::trim(&self.text(p, e)).to_string();
            self.emit_text(T::Invalid, p, e, text);
            self.pop();
            return true;
        }
        false
    }

    fn step(&mut self) -> bool {
        match self.state {
            S::Initial => {
                self.initial();
                true
            }
            S::Id => self.id(),
            S::Alias => {
                if self.skip_same_line() {
                    return true;
                }
                if self.c(self.p) == 35 {
                    self.p = self.eol(self.p);
                    return true;
                }
                self.pop();
                self.pop();
                if self.word("as") {
                    self.begin(T::As, 2, S::Line);
                } else {
                    self.emit(T::Newline, self.p, self.p);
                }
                true
            }
            S::Line => {
                if self.skip_same_line() {
                    return true;
                }
                if self.c(self.p) == 35 {
                    self.p = self.eol(self.p);
                    return true;
                }
                let end = self.line_run(self.p);
                self.emit(T::RestOfLine, self.p, end);
                self.pop();
                true
            }
            S::Config => {
                let p = self.p;
                if self.c(p) != 125 {
                    let q = self.src[p..]
                        .iter()
                        .position(|&u| u == 125)
                        .map_or(self.n, |i| p + i);
                    self.emit(T::ConfigContent, p, q);
                } else {
                    let w = self.skip_ws(p + 1);
                    let alias = w > p + 1 && self.ci(w, "as") && is_space(self.c(w + 2));
                    self.emit(T::ConfigEnd, p, p + 1);
                    self.pop();
                    if alias {
                        self.push(S::Alias);
                    } else {
                        self.pop();
                    }
                }
                true
            }
            S::AccTitle | S::AccDescr => {
                let t = if self.state == S::AccTitle {
                    T::AccTitleValue
                } else {
                    T::AccDescrValue
                };
                let e = self.eol(self.p);
                self.emit(t, self.p, e);
                self.pop();
                true
            }
            S::AccDescrMultiline => {
                let p = self.p;
                if self.c(p) == 125 {
                    self.p += 1;
                    self.pop();
                } else {
                    let q = self.src[p..]
                        .iter()
                        .position(|&u| u == 125)
                        .map_or(self.n, |i| p + i);
                    self.emit(T::AccDescrMultilineValue, p, q);
                }
                true
            }
        }
    }
}

pub(crate) fn tokenize(src: &[u16]) -> Tokens {
    let n = src.len();
    let mut lx = Lexer {
        src,
        n,
        p: 0,
        state: S::Initial,
        stack: Vec::new(),
        next_nl: None,
        out: Tokens {
            types: Vec::new(),
            texts: Vec::new(),
            starts: Vec::new(),
        },
    };
    while lx.p < n {
        if !lx.step() {
            lx.out.types.push(T::Error);
            lx.out.texts.push(String::new());
            lx.out.starts.push(lx.p);
            return lx.out;
        }
    }
    // Mermaid's lexer tries its rules once more on the empty remainder before reporting the end.
    match lx.state {
        S::Initial | S::Alias => lx.emit(T::Newline, n, n),
        S::Line => lx.emit(T::RestOfLine, n, n),
        S::AccTitle => lx.emit(T::AccTitleValue, n, n),
        S::AccDescr => lx.emit(T::AccDescrValue, n, n),
        S::AccDescrMultiline => lx.emit(T::AccDescrMultilineValue, n, n),
        _ => {}
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
    fn reads_messages() {
        let toks = kinds("sequenceDiagram\nAlice->>+John: Hello\nJohn-->>-Alice: Hi\n");
        let types: Vec<T> = toks.iter().map(|t| t.0).collect();
        assert_eq!(
            types,
            [
                T::Sd,
                T::Newline,
                T::Actor,
                T::SolidArrow,
                T::Plus,
                T::Actor,
                T::Txt,
                T::Newline,
                T::Actor,
                T::DottedArrow,
                T::Minus,
                T::Actor,
                T::Txt,
                T::Newline,
                T::Newline,
                T::End
            ]
        );
        assert_eq!(toks[2].1, "Alice");
        assert_eq!(toks[6].1, ": Hello");
    }

    #[test]
    fn reads_participants_and_blocks() {
        let toks =
            kinds("sequenceDiagram\nparticipant A as Alice\nloop Every minute\nA->>A: x\nend\n");
        assert!(toks.iter().any(|t| t.0 == T::As));
        assert!(toks.iter().any(|t| t.0 == T::RestOfLine && t.1 == "Alice"));
        assert!(
            toks.iter()
                .any(|t| t.0 == T::RestOfLine && t.1 == "Every minute")
        );
        assert!(toks.iter().any(|t| t.0 == T::EndKw));
    }
}
