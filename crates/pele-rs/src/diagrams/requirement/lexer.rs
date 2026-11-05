//! The requirement diagram lexer: the rules of Mermaid's `requirementDiagram.jison`, over UTF-16
//! code units as Jison reads them. Ported from pele's `diagrams/requirement/lexer.ts`.

/// Token types, in the order of Mermaid's grammar; ranges of them are compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum T {
    End,
    Error,
    Title,
    AccTitle,
    AccTitleValue,
    AccDescr,
    AccDescrValue,
    AccDescrMultilineValue,
    DirectionTb,
    DirectionBt,
    DirectionRl,
    DirectionLr,
    Newline,
    Eof,
    Rd,
    StructStart,
    StructStop,
    StyleSeparator,
    Colonsep,
    Id,
    Text,
    Risk,
    Verifymthd,
    Requirement,
    FunctionalRequirement,
    InterfaceRequirement,
    PerformanceRequirement,
    PhysicalRequirement,
    DesignConstraint,
    LowRisk,
    MedRisk,
    HighRisk,
    VerifyAnalysis,
    VerifyDemonstration,
    VerifyInspection,
    VerifyTest,
    Element,
    Contains,
    Copies,
    Derives,
    Satisfies,
    Verifies,
    Refines,
    Traces,
    Type,
    Docref,
    Style,
    Classdef,
    Class,
    Alpha,
    Colon,
    Semicolon,
    Percent,
    Minus,
    Brkt,
    EndArrowL,
    EndArrowR,
    Line,
    QString,
    UnqString,
    Comma,
}

/// Every token type, in order, so that a range of them can be listed.
pub(crate) const ALL: [T; 61] = {
    use T::*;
    [
        End,
        Error,
        Title,
        AccTitle,
        AccTitleValue,
        AccDescr,
        AccDescrValue,
        AccDescrMultilineValue,
        DirectionTb,
        DirectionBt,
        DirectionRl,
        DirectionLr,
        Newline,
        Eof,
        Rd,
        StructStart,
        StructStop,
        StyleSeparator,
        Colonsep,
        Id,
        Text,
        Risk,
        Verifymthd,
        Requirement,
        FunctionalRequirement,
        InterfaceRequirement,
        PerformanceRequirement,
        PhysicalRequirement,
        DesignConstraint,
        LowRisk,
        MedRisk,
        HighRisk,
        VerifyAnalysis,
        VerifyDemonstration,
        VerifyInspection,
        VerifyTest,
        Element,
        Contains,
        Copies,
        Derives,
        Satisfies,
        Verifies,
        Refines,
        Traces,
        Type,
        Docref,
        Style,
        Classdef,
        Class,
        Alpha,
        Colon,
        Semicolon,
        Percent,
        Minus,
        Brkt,
        EndArrowL,
        EndArrowR,
        Line,
        QString,
        UnqString,
        Comma,
    ]
};

const NAMES: [&str; 61] = [
    "$end",
    "INVALID",
    "title",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "direction_tb",
    "direction_bt",
    "direction_rl",
    "direction_lr",
    "NEWLINE",
    "EOF",
    "RD",
    "STRUCT_START",
    "STRUCT_STOP",
    "STYLE_SEPARATOR",
    "COLONSEP",
    "ID",
    "TEXT",
    "RISK",
    "VERIFYMTHD",
    "REQUIREMENT",
    "FUNCTIONAL_REQUIREMENT",
    "INTERFACE_REQUIREMENT",
    "PERFORMANCE_REQUIREMENT",
    "PHYSICAL_REQUIREMENT",
    "DESIGN_CONSTRAINT",
    "LOW_RISK",
    "MED_RISK",
    "HIGH_RISK",
    "VERIFY_ANALYSIS",
    "VERIFY_DEMONSTRATION",
    "VERIFY_INSPECTION",
    "VERIFY_TEST",
    "ELEMENT",
    "CONTAINS",
    "COPIES",
    "DERIVES",
    "SATISFIES",
    "VERIFIES",
    "REFINES",
    "TRACES",
    "TYPE",
    "DOCREF",
    "STYLE",
    "CLASSDEF",
    "CLASS",
    "ALPHA",
    "COLON",
    "SEMICOLON",
    "PERCENT",
    "MINUS",
    "BRKT",
    "END_ARROW_L",
    "END_ARROW_R",
    "LINE",
    "qString",
    "unqString",
    "COMMA",
];

impl T {
    /// The token's name in Mermaid's grammar, as parse errors print it.
    pub(crate) fn name(self) -> &'static str {
        NAMES[self as usize]
    }
}

/// Every keyword is matched without regard to case and must end at a word boundary.
const KEYWORDS: [(&str, T); 31] = [
    ("requirementdiagram", T::Rd),
    ("id", T::Id),
    ("text", T::Text),
    ("risk", T::Risk),
    ("verifymethod", T::Verifymthd),
    ("requirement", T::Requirement),
    ("functionalrequirement", T::FunctionalRequirement),
    ("interfacerequirement", T::InterfaceRequirement),
    ("performancerequirement", T::PerformanceRequirement),
    ("physicalrequirement", T::PhysicalRequirement),
    ("designconstraint", T::DesignConstraint),
    ("low", T::LowRisk),
    ("medium", T::MedRisk),
    ("high", T::HighRisk),
    ("analysis", T::VerifyAnalysis),
    ("demonstration", T::VerifyDemonstration),
    ("inspection", T::VerifyInspection),
    ("test", T::VerifyTest),
    ("element", T::Element),
    ("contains", T::Contains),
    ("copies", T::Copies),
    ("derives", T::Derives),
    ("satisfies", T::Satisfies),
    ("verifies", T::Verifies),
    ("refines", T::Refines),
    ("traces", T::Traces),
    ("type", T::Type),
    ("docref", T::Docref),
    ("style", T::Style),
    ("classdef", T::Classdef),
    ("class", T::Class),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    String,
    Style,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
}

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

fn is_line_end(c: u32) -> bool {
    matches!(c, 10 | 13 | 0x2028 | 0x2029)
}

/// Characters that end an unquoted string: `:,{<>-=` and line breaks.
fn is_stop(c: u32) -> bool {
    matches!(c, 58 | 44 | 13 | 10 | 123 | 60 | 62 | 45 | 61)
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
    out: Tokens,
    /// Where each kind of direction statement starts and where its keyword ends.
    dirs: [Vec<(usize, usize)>; 4],
    /// How many statements of each kind start before the end of the current line.
    dir_seen: [usize; 4],
    line_end: Option<usize>,
}

impl Lexer<'_> {
    fn c(&self, i: usize) -> u32 {
        self.src.get(i).map_or(NONE, |&u| u32::from(u))
    }

    fn text(&self, s: usize, e: usize) -> String {
        String::from_utf16_lossy(&self.src[s.min(self.n)..e.min(self.n)])
    }

    fn emit(&mut self, t: T, s: usize, e: usize) -> bool {
        let text = self.text(s, e);
        self.out.types.push(t);
        self.out.texts.push(text);
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

    fn find(&self, unit: u16, from: usize) -> Option<usize> {
        self.src[from.min(self.n)..]
            .iter()
            .position(|&u| u == unit)
            .map(|i| from + i)
    }

    fn line_break(&self, q: usize) -> usize {
        self.find(10, q).unwrap_or(self.n)
    }

    /// Whether the source has the lower-case `word` at `q`, ignoring case.
    fn starts_ci(&self, word: &str, q: usize) -> bool {
        word.bytes().enumerate().all(|(i, k)| {
            let c = self.c(q + i);
            c == u32::from(k) || (k.is_ascii_lowercase() && c < 128 && (c | 32) == u32::from(k))
        })
    }

    fn skip_ws(&self, mut q: usize) -> usize {
        while is_space(self.c(q)) {
            q += 1;
        }
        q
    }

    /// Matches `accTitle\s*:\s*` and its siblings at `p`, ignoring case.
    fn acc(&self, word: &str, punct: u32) -> Option<usize> {
        if !self.starts_ci(word, self.p) {
            return None;
        }
        let q = self.skip_ws(self.p + word.len());
        (self.c(q) == punct).then(|| self.skip_ws(q + 1))
    }

    /// `.*direction\s+XX[^\n]*`: the last such statement that starts on this line decides the
    /// token.
    fn direction(&mut self) -> bool {
        let p = self.p;
        if self.line_end.is_none_or(|e| e < p) {
            let end = self.src[p..]
                .iter()
                .position(|&u| is_line_end(u32::from(u)))
                .map_or(self.n, |i| p + i);
            self.line_end = Some(end);
            for k in 0..4 {
                let list = &self.dirs[k];
                let mut j = self.dir_seen[k];
                while j < list.len() && list[j].0 <= end {
                    j += 1;
                }
                self.dir_seen[k] = j;
            }
        }
        for k in 0..4 {
            let Some(j) = self.dir_seen[k].checked_sub(1) else {
                continue;
            };
            let (start, end) = self.dirs[k][j];
            if start >= p {
                let stop = self.line_break(end);
                return self.emit(ALL[T::DirectionTb as usize + k], p, stop);
            }
        }
        false
    }

    fn initial(&mut self, c: u32) -> bool {
        let p = self.p;
        let lower = c | 32;
        if lower == 116 && self.starts_ci("itle", p + 1) && is_space(self.c(p + 5)) {
            let mut q = p + 6;
            while q < self.n && !matches!(self.c(q), 35 | 10 | 59) {
                q += 1;
            }
            if q > p + 6 {
                return self.emit(T::Title, p, q);
            }
        }
        if lower == 97 && (self.c(p + 1) | 32) == 99 && (self.c(p + 2) | 32) == 99 {
            if let Some(e) = self.acc("acctitle", 58) {
                self.push(S::AccTitle);
                return self.emit(T::AccTitle, p, e);
            }
            if let Some(e) = self.acc("accdescr", 58) {
                self.push(S::AccDescr);
                return self.emit(T::AccDescr, p, e);
            }
            if let Some(e) = self.acc("accdescr", 123) {
                self.push(S::AccDescrMultiline);
                self.p = e;
                return true;
            }
        }
        if self.dirs.iter().any(|d| !d.is_empty()) && !is_line_end(c) && self.direction() {
            return true;
        }

        if c == 10 || (c == 13 && self.c(p + 1) == 10) {
            let mut q = p;
            while q < self.n {
                match self.c(q) {
                    10 => q += 1,
                    13 if self.c(q + 1) == 10 => q += 2,
                    _ => break,
                }
            }
            return self.emit(T::Newline, p, q);
        }
        if is_space(c) {
            self.p = self.skip_ws(p + 1);
            return true;
        }
        if c == 35 || c == 37 {
            self.p = self.line_break(p);
            return true;
        }

        if is_word(c) {
            let mut q = p + 1;
            while is_word(self.c(q)) {
                q += 1;
            }
            let word = self.text(p, q).to_ascii_lowercase();
            if let Some(&(_, keyword)) = KEYWORDS.iter().find(|(k, _)| *k == word) {
                if matches!(keyword, T::Style | T::Classdef | T::Class) {
                    self.push(S::Style);
                }
                return self.emit(keyword, p, q);
            }
            while q < self.n && !is_stop(self.c(q)) {
                q += 1;
            }
            let text = crate::util::trim(&self.text(p, q)).to_string();
            self.out.types.push(T::UnqString);
            self.out.texts.push(text);
            self.out.starts.push(p);
            self.p = q;
            return true;
        }

        match c {
            123 => self.emit(T::StructStart, p, p + 1),
            125 => self.emit(T::StructStop, p, p + 1),
            58 => {
                if self.c(p + 1) == 58 && self.c(p + 2) == 58 {
                    self.emit(T::StyleSeparator, p, p + 3)
                } else {
                    self.emit(T::Colonsep, p, p + 1)
                }
            }
            60 => self.c(p + 1) == 45 && self.emit(T::EndArrowL, p, p + 2),
            45 => {
                if self.c(p + 1) == 62 {
                    self.emit(T::EndArrowR, p, p + 2)
                } else {
                    self.emit(T::Line, p, p + 1)
                }
            }
            34 => {
                self.push(S::String);
                self.p += 1;
                true
            }
            44 => self.emit(T::Comma, p, p + 1),
            _ => false,
        }
    }

    fn style(&mut self, c: u32) -> bool {
        let p = self.p;
        if is_word(c) {
            let mut q = p + 1;
            while is_word(self.c(q)) {
                q += 1;
            }
            return self.emit(T::Alpha, p, q);
        }
        match c {
            58 => self.emit(T::Colon, p, p + 1),
            59 => self.emit(T::Semicolon, p, p + 1),
            37 => self.emit(T::Percent, p, p + 1),
            45 => self.emit(T::Minus, p, p + 1),
            35 => self.emit(T::Brkt, p, p + 1),
            32 => {
                self.p += 1;
                true
            }
            34 => {
                self.push(S::String);
                self.p += 1;
                true
            }
            10 => {
                self.pop();
                self.p += 1;
                true
            }
            44 => self.emit(T::Comma, p, p + 1),
            _ => false,
        }
    }
}

/// The places of `direction\s+(TB|BT|RL|LR)` in `src`, ignoring case, found left to right
/// without overlap as a global regular expression finds them.
fn directions(src: &[u16]) -> [Vec<(usize, usize)>; 4] {
    let mut out: [Vec<(usize, usize)>; 4] = Default::default();
    let lx = Lexer {
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
        dirs: Default::default(),
        dir_seen: [0; 4],
        line_end: None,
    };
    let mut d = 0;
    while d + 9 <= src.len() {
        if lx.starts_ci("direction", d) {
            let w = lx.skip_ws(d + 9);
            if w > d + 9 {
                if let Some(k) = ["tb", "bt", "rl", "lr"]
                    .iter()
                    .position(|code| lx.starts_ci(code, w))
                {
                    out[k].push((d, w + 2));
                    d = w + 2;
                    continue;
                }
            }
        }
        d += 1;
    }
    out
}

pub(crate) fn tokenize(src: &[u16]) -> Tokens {
    let mut lx = Lexer {
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
        dirs: directions(src),
        dir_seen: [0; 4],
        line_end: None,
    };
    let n = lx.n;
    while lx.p < n {
        let c = lx.c(lx.p);
        let p = lx.p;
        let ok = match lx.state {
            S::Initial => lx.initial(c),
            S::Style => lx.style(c),
            S::String => {
                if c == 34 {
                    lx.pop();
                    lx.p += 1;
                } else {
                    let e = lx.find(34, p).unwrap_or(n);
                    lx.emit(T::QString, p, e);
                }
                true
            }
            S::AccTitle | S::AccDescr => {
                let t = if lx.state == S::AccTitle {
                    T::AccTitleValue
                } else {
                    T::AccDescrValue
                };
                lx.pop();
                let e = lx.line_break(p);
                lx.emit(t, p, e)
            }
            S::AccDescrMultiline => {
                if c == 125 {
                    lx.pop();
                    lx.p += 1;
                } else {
                    let e = lx.find(125, p).unwrap_or(n);
                    lx.emit(T::AccDescrMultilineValue, p, e);
                }
                true
            }
        };
        if !ok {
            let out = &mut lx.out;
            out.types.push(T::Error);
            out.texts.push(String::new());
            out.starts.push(lx.p);
            return lx.out;
        }
    }

    // At the end of input Jison still tries the rules that can match nothing, once.
    let last = match lx.state {
        S::Initial => Some(T::Eof),
        S::String => Some(T::QString),
        S::AccTitle => Some(T::AccTitleValue),
        S::AccDescr => Some(T::AccDescrValue),
        S::AccDescrMultiline => Some(T::AccDescrMultilineValue),
        S::Style => None,
    };
    if let Some(last) = last {
        lx.emit(last, n, n);
    }
    lx.emit(T::End, n, n);
    lx.out
}
