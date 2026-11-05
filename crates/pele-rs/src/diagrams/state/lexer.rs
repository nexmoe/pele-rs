//! The state diagram lexer: Mermaid's jison lexer for `stateDiagram.jison`, as pele's
//! `lexer.ts` emulates it, over UTF-16 code units.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum T {
    End,
    Error,
    Nl,
    Sd,
    Descr,
    Arrow,
    HideEmpty,
    Scale,
    Width,
    CompositState,
    StructStart,
    StructStop,
    StateDescr,
    As,
    Id,
    Fork,
    Join,
    Choice,
    Concurrent,
    Note,
    NoteText,
    AccTitle,
    AccTitleValue,
    AccDescr,
    AccDescrValue,
    AccDescrMultilineValue,
    Click,
    String,
    Href,
    ClassDef,
    ClassDefId,
    DefaultClassDefId,
    ClassDefStyleOpts,
    Default,
    Style,
    StyleIds,
    StyleDefStyleOpts,
    Class,
    ClassEntityIds,
    StyleClass,
    DirectionTb,
    DirectionBt,
    DirectionRl,
    DirectionLr,
    EdgeState,
    StyleSeparator,
    LeftOf,
    RightOf,
    Invalid,
}

impl T {
    /// The token's name as Mermaid's grammar spells it, for error messages.
    pub(crate) fn name(self) -> &'static str {
        use T::*;
        match self {
            End => "$end",
            Error | Invalid => "INVALID",
            Nl => "NL",
            Sd => "SD",
            Descr => "DESCR",
            Arrow => "-->",
            HideEmpty => "HIDE_EMPTY",
            Scale => "scale",
            Width => "WIDTH",
            CompositState => "COMPOSIT_STATE",
            StructStart => "STRUCT_START",
            StructStop => "STRUCT_STOP",
            StateDescr => "STATE_DESCR",
            As => "AS",
            Id => "ID",
            Fork => "FORK",
            Join => "JOIN",
            Choice => "CHOICE",
            Concurrent => "CONCURRENT",
            Note => "note",
            NoteText => "NOTE_TEXT",
            AccTitle => "acc_title",
            AccTitleValue => "acc_title_value",
            AccDescr => "acc_descr",
            AccDescrValue => "acc_descr_value",
            AccDescrMultilineValue => "acc_descr_multiline_value",
            Click => "CLICK",
            String => "STRING",
            Href => "HREF",
            ClassDef => "classDef",
            ClassDefId => "CLASSDEF_ID",
            DefaultClassDefId => "DEFAULT_CLASSDEF_ID",
            ClassDefStyleOpts => "CLASSDEF_STYLEOPTS",
            Default => "DEFAULT",
            Style => "style",
            StyleIds => "STYLE_IDS",
            StyleDefStyleOpts => "STYLEDEF_STYLEOPTS",
            Class => "class",
            ClassEntityIds => "CLASSENTITY_IDS",
            StyleClass => "STYLECLASS",
            DirectionTb => "direction_tb",
            DirectionBt => "direction_bt",
            DirectionRl => "direction_rl",
            DirectionLr => "direction_lr",
            EdgeState => "EDGE_STATE",
            StyleSeparator => "STYLE_SEPARATOR",
            LeftOf => "left_of",
            RightOf => "right_of",
        }
    }

    const DIRECTIONS: [T; 4] = [
        T::DirectionTb,
        T::DirectionBt,
        T::DirectionRl,
        T::DirectionLr,
    ];
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Struct,
    State,
    StateString,
    StateId,
    Scale,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    ClassDef,
    ClassDefId,
    Class,
    ClassStyle,
    Style,
    StyleDefStyles,
    Note,
    NoteId,
    NoteText,
    FloatingNote,
    FloatingNoteId,
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

pub(crate) struct Tokens {
    pub types: Vec<T>,
    pub texts: Vec<String>,
    pub starts: Vec<usize>,
    /// Set when lexing stopped on the grammar's own error rather than on unrecognized text.
    pub error: Option<String>,
}

/// Finds the next match of a pattern at or after a position, remembering the answer so that
/// asking at each token costs one scan of the text in total.
struct Finder {
    find: fn(&[u16], usize) -> Option<usize>,
    from: usize,
    next: Option<Option<usize>>,
}

impl Finder {
    fn new(find: fn(&[u16], usize) -> Option<usize>) -> Self {
        Self {
            find,
            from: 0,
            next: None,
        }
    }

    fn at(&mut self, src: &[u16], q: usize) -> Option<usize> {
        let stale = match self.next {
            None => true,
            Some(next) => q < self.from || next.is_some_and(|n| q > n),
        };
        if stale {
            self.next = Some((self.find)(src, q));
            self.from = q;
        }
        self.next.flatten()
    }
}

/// Whether the lowercase ASCII `word` is at `q`, in any case.
fn word_at(src: &[u16], q: usize, word: &str) -> bool {
    word.bytes().enumerate().all(|(i, a)| {
        let c = src.get(q + i).map_or(NONE, |&u| u32::from(u));
        let a = u32::from(a);
        c == a || ((97..=122).contains(&a) && c == a - 32)
    })
}

fn find_direction(src: &[u16], q: usize) -> Option<usize> {
    (q..src.len()).find(|&i| word_at(src, i, "direction"))
}

/// `<<(?:fork|join|choice)>>|\[\[(?:fork|join|choice)\]\]`, in any case.
fn find_stereotype(src: &[u16], q: usize) -> Option<usize> {
    (q..src.len()).find(|&i| stereotype_len(src, i).is_some())
}

fn stereotype_len(src: &[u16], i: usize) -> Option<usize> {
    let (open, close) = match src[i] {
        60 => ("<<", ">>"),
        91 => ("[[", "]]"),
        _ => return None,
    };
    if !word_at(src, i, open) {
        return None;
    }
    ["fork", "join", "choice"].iter().find_map(|w| {
        (word_at(src, i + 2, w) && word_at(src, i + 2 + w.len(), close)).then_some(w.len() + 4)
    })
}

fn find_line_end(src: &[u16], q: usize) -> Option<usize> {
    src[q.min(src.len())..]
        .iter()
        .position(|&u| matches!(u, 10 | 13 | 0x2028 | 0x2029))
        .map(|i| q + i)
}

fn find_percent(src: &[u16], q: usize) -> Option<usize> {
    (q..src.len().saturating_sub(1)).find(|&i| src[i] == 37 && src[i + 1] == 37)
}

fn find_brace(src: &[u16], q: usize) -> Option<usize> {
    src[q.min(src.len())..]
        .iter()
        .position(|&u| u == 123)
        .map(|i| q + i)
}

struct Lexer<'a> {
    src: &'a [u16],
    n: usize,
    p: usize,
    state: S,
    stack: Vec<S>,
    out: Tokens,
    next_direction: Finder,
    next_stereotype: Finder,
    next_line_end: Finder,
    next_percent: Finder,
    next_brace: Finder,
    /// The last `direction <dir>` of each kind on the line being lexed, and where its keyword
    /// ends.
    dir_from: Option<usize>,
    dir_to: usize,
    dir_at: [Option<usize>; 4],
    dir_end: [usize; 4],
    /// The same for `<<fork>>`, `<<join>>`, `<<choice>>` and their `[[ ]]` spellings.
    stereo_from: Option<usize>,
    stereo_to: usize,
    stereo_at: [Option<usize>; 6],
}

impl Lexer<'_> {
    fn c(&self, i: usize) -> u32 {
        self.src.get(i).map_or(NONE, |&u| u32::from(u))
    }

    fn text(&self, s: usize, e: usize) -> String {
        String::from_utf16_lossy(&self.src[s.min(self.n)..e.min(self.n)])
    }

    fn emit(&mut self, t: T, s: usize, e: usize) {
        let text = self.text(s, e);
        self.emit_text(t, text, s, e);
    }

    fn emit_text(&mut self, t: T, text: String, s: usize, e: usize) {
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

    fn word(&self, w: &str, q: usize) -> bool {
        word_at(self.src, q, w)
    }

    fn ws_end(&self, q: usize) -> usize {
        let mut j = q;
        while j < self.n && is_space(self.c(j)) {
            j += 1;
        }
        j
    }

    fn word_end(&self, q: usize) -> usize {
        let mut j = q;
        while j < self.n && is_word(self.c(j)) {
            j += 1;
        }
        j
    }

    fn eol(&self, q: usize) -> usize {
        self.src[q.min(self.n)..]
            .iter()
            .position(|&u| u == 10)
            .map_or(self.n, |i| q + i)
    }

    fn index_of(&self, unit: u16, q: usize) -> Option<usize> {
        self.src[q.min(self.n)..]
            .iter()
            .position(|&u| u == unit)
            .map(|i| q + i)
    }

    fn line_end(&mut self, q: usize) -> usize {
        self.next_line_end.at(self.src, q).unwrap_or(self.n)
    }

    /// Rules `.*direction\s+TB[^\n]*` and their three siblings: the first kind, in grammar
    /// order, found between q and the end of its line.
    fn direction(&mut self, q: usize) -> Option<usize> {
        if self.dir_from.is_none_or(|from| q < from || q > self.dir_to) {
            let first = self.next_direction.at(self.src, q);
            let end = if first.is_none() {
                self.n
            } else {
                self.line_end(q)
            };
            self.dir_from = Some(q);
            self.dir_to = end;
            self.dir_at = [None; 4];
            let mut d = first;
            while let Some(at) = d.filter(|&at| at < end) {
                let w = self.ws_end(at + 9);
                if w > at + 9 {
                    let kind = ["tb", "bt", "rl", "lr"]
                        .iter()
                        .position(|k| self.word(k, w));
                    if let Some(kind) = kind {
                        self.dir_at[kind] = Some(at);
                        self.dir_end[kind] = w + 2;
                    }
                }
                d = self.next_direction.at(self.src, at + 1);
            }
        }
        (0..4).find(|&k| self.dir_at[k].is_some_and(|at| at >= q))
    }

    fn stereotype(&mut self, q: usize) -> Option<usize> {
        if self
            .stereo_from
            .is_none_or(|from| q < from || q > self.stereo_to)
        {
            let first = self.next_stereotype.at(self.src, q);
            let end = if first.is_none() {
                self.n
            } else {
                self.line_end(q)
            };
            self.stereo_from = Some(q);
            self.stereo_to = end;
            self.stereo_at = [None; 6];
            let mut d = first;
            while let Some(at) = d.filter(|&at| at < end) {
                let letter = self.c(at + 2) | 32;
                let base = if self.c(at) == 60 { 0 } else { 3 };
                let kind = match letter {
                    102 => 0,
                    106 => 1,
                    _ => 2,
                };
                self.stereo_at[base + kind] = Some(at);
                d = self.next_stereotype.at(self.src, at + 1);
            }
        }
        (0..6).find(|&k| self.stereo_at[k].is_some_and(|at| at >= q))
    }

    /// The grammar's `processId()`: an id ends at an inline `%%`, and a text that starts with
    /// one is dropped.
    fn emit_id(&mut self, s: usize, e: usize) -> bool {
        match self.next_percent.at(self.src, s) {
            Some(at) if at + 2 <= e => {
                if at == s {
                    self.p = e;
                    return false;
                }
                self.emit(T::Id, s, at);
                true
            }
            _ => {
                self.emit(T::Id, s, e);
                true
            }
        }
    }

    fn finish(mut self, t: T) -> Tokens {
        self.out.types.push(t);
        self.out.texts.push(String::new());
        self.out.starts.push(self.p);
        self.out
    }

    fn comment(&self, c: u32) -> bool {
        c == 35 || (c == 37 && self.c(self.p + 1) == 37 && self.c(self.p + 2) != 123)
    }

    fn skip_inline_space(&mut self) {
        let mut j = self.p + 1;
        while j < self.n && self.c(j) != 10 && is_space(self.c(j)) {
            j += 1;
        }
        self.p = j;
    }

    fn run(mut self) -> Tokens {
        loop {
            let p = self.p;
            if p >= self.n {
                // Jison tries the current state's rules once more on the empty remainder.
                let n = self.n;
                match self.state {
                    S::Initial => self.emit(T::Nl, n, n),
                    S::AccTitle => self.emit(T::AccTitleValue, n, n),
                    S::AccDescr => self.emit(T::AccDescrValue, n, n),
                    S::AccDescrMultiline => self.emit(T::AccDescrMultilineValue, n, n),
                    S::ClassDefId => self.emit(T::ClassDefStyleOpts, n, n),
                    S::ClassStyle => self.emit(T::StyleClass, n, n),
                    S::StyleDefStyles => self.emit(T::StyleDefStyleOpts, n, n),
                    S::StateString => self.emit(T::StateDescr, n, n),
                    S::FloatingNote => self.emit(T::NoteText, n, n),
                    S::StateId | S::FloatingNoteId => self.emit(T::Id, n, n),
                    _ => {}
                }
                return self.finish(T::End);
            }
            let c = self.c(p);
            let done = match self.state {
                S::Initial | S::Struct => self.document(c),
                S::State => self.state_rules(c),
                S::StateString => {
                    if c == 34 {
                        self.p += 1;
                        self.pop();
                    } else {
                        let close = self.index_of(34, p).unwrap_or(self.n);
                        self.emit(T::StateDescr, p, close);
                    }
                    None
                }
                S::StateId => {
                    let mut j = p;
                    while j < self.n && !matches!(self.c(j), 10 | 123) {
                        j += 1;
                    }
                    if self.emit_id(p, j) {
                        self.pop();
                    }
                    None
                }
                S::Scale => {
                    if (48..=57).contains(&c) {
                        let mut j = p + 1;
                        while j < self.n && (48..=57).contains(&self.c(j)) {
                            j += 1;
                        }
                        self.emit(T::Width, p, j);
                        None
                    } else {
                        let q = self.ws_end(p);
                        if q > p && self.word("width", q) && !is_word(self.c(q + 5)) {
                            self.p = q + 5;
                            self.pop();
                            None
                        } else {
                            Some(T::Error)
                        }
                    }
                }
                S::AccTitle => {
                    let e = self.eol(p);
                    self.emit(T::AccTitleValue, p, e);
                    self.pop();
                    None
                }
                S::AccDescr => {
                    let e = self.eol(p);
                    self.emit(T::AccDescrValue, p, e);
                    self.pop();
                    None
                }
                S::AccDescrMultiline => {
                    if c == 125 {
                        self.p += 1;
                        self.pop();
                    } else {
                        let close = self.index_of(125, p).unwrap_or(self.n);
                        self.emit(T::AccDescrMultilineValue, p, close);
                    }
                    None
                }
                S::ClassDef => {
                    let is_default = self.word("default", p) && self.ws_end(p + 7) > p + 7;
                    let j = if is_default { p + 7 } else { self.word_end(p) };
                    let e = self.ws_end(j);
                    if j == p || e == j {
                        Some(T::Error)
                    } else {
                        self.emit(
                            if is_default {
                                T::DefaultClassDefId
                            } else {
                                T::ClassDefId
                            },
                            p,
                            e,
                        );
                        self.pop();
                        self.push(S::ClassDefId);
                        None
                    }
                }
                S::ClassDefId => {
                    let e = self.eol(p);
                    self.emit(T::ClassDefStyleOpts, p, e);
                    self.pop();
                    None
                }
                S::Class => {
                    let mut j = self.word_end(p);
                    if j == p {
                        Some(T::Error)
                    } else {
                        while self.c(j) == 44 {
                            let k = self.ws_end(j + 1);
                            let e = self.word_end(k);
                            if e == k {
                                break;
                            }
                            j = e;
                        }
                        self.emit(T::ClassEntityIds, p, j);
                        self.pop();
                        self.push(S::ClassStyle);
                        None
                    }
                }
                S::ClassStyle => {
                    let e = self.eol(p);
                    self.emit(T::StyleClass, p, e);
                    self.pop();
                    None
                }
                S::Style => {
                    let mut j = p;
                    while j < self.n && (is_word(self.c(j)) || self.c(j) == 44) {
                        j += 1;
                    }
                    let e = self.ws_end(j);
                    if j == p || e == j {
                        Some(T::Error)
                    } else {
                        self.emit(T::StyleIds, p, e);
                        self.pop();
                        self.push(S::StyleDefStyles);
                        None
                    }
                }
                S::StyleDefStyles => {
                    let e = self.eol(p);
                    self.emit(T::StyleDefStyleOpts, p, e);
                    self.pop();
                    None
                }
                S::Note => {
                    if self.word("left of", p) && !is_word(self.c(p + 7)) {
                        self.emit(T::LeftOf, p, p + 7);
                        self.pop();
                        self.push(S::NoteId);
                        None
                    } else if self.word("right of", p) && !is_word(self.c(p + 8)) {
                        self.emit(T::RightOf, p, p + 8);
                        self.pop();
                        self.push(S::NoteId);
                        None
                    } else if c == 34 {
                        self.p += 1;
                        self.pop();
                        self.push(S::FloatingNote);
                        None
                    } else {
                        Some(T::Error)
                    }
                }
                S::NoteId => {
                    let q = self.ws_end(p);
                    let mut j = q;
                    while j < self.n {
                        let d = self.c(j);
                        if d == 58 || d == 45 || is_space(d) {
                            break;
                        }
                        j += 1;
                    }
                    if j == q {
                        Some(T::Error)
                    } else {
                        if self.emit_id(p, j) {
                            self.pop();
                            self.push(S::NoteText);
                        }
                        None
                    }
                }
                S::NoteText => self.note_text(p),
                S::FloatingNote => {
                    let q = self.ws_end(p);
                    if self.word("as", q) {
                        let e = self.ws_end(q + 2);
                        self.emit(T::As, p, e);
                        self.pop();
                        self.push(S::FloatingNoteId);
                    } else if c == 34 {
                        self.p += 1;
                    } else {
                        let close = self.index_of(34, p).unwrap_or(self.n);
                        self.emit(T::NoteText, p, close);
                    }
                    None
                }
                S::FloatingNoteId => {
                    let e = self.eol(p);
                    if self.emit_id(p, e) {
                        self.pop();
                    }
                    None
                }
            };
            if let Some(t) = done {
                return self.finish(t);
            }
        }
    }

    fn note_text(&mut self, p: usize) -> Option<T> {
        let q = self.ws_end(p);
        if self.c(q) == 58 {
            let mut j = q + 1;
            while j < self.n && !matches!(self.c(j), 58 | 10 | 59) {
                j += 1;
            }
            if j > q + 1 {
                // Mermaid drops two characters here, expecting " :"; with no space before the
                // colon the first character of the note goes too.
                let text = crate::util::trim(&self.text(p + 2, j)).to_string();
                self.emit_text(T::NoteText, text, p, j);
                self.pop();
                return None;
            }
        }
        let mut k = self.index_of(10, p);
        let mut e = 0;
        while let Some(nl) = k {
            e = self.ws_end(nl);
            if self.word("end note", e) && !is_word(self.c(e + 8)) {
                break;
            }
            k = self.index_of(10, e);
        }
        if k.is_none() {
            return Some(T::Error);
        }
        let text = crate::util::trim(&self.text(p, e)).to_string();
        self.emit_text(T::NoteText, text, p, e + 8);
        self.pop();
        None
    }

    fn state_rules(&mut self, c: u32) -> Option<T> {
        let p = self.p;
        if c != 10 && is_space(c) {
            self.skip_inline_space();
            return None;
        }
        if self.comment(c) {
            self.p = self.eol(p);
            return None;
        }
        if let Some(kind) = self.stereotype(p) {
            let at = self.stereo_at[kind].unwrap_or(p);
            let choice = kind == 2 || kind == 5;
            let t = if choice {
                T::Choice
            } else if kind % 3 == 0 {
                T::Fork
            } else {
                T::Join
            };
            let text = crate::util::trim(&self.text(p, at)).to_string();
            self.emit_text(t, text, p, at + if choice { 10 } else { 8 });
            self.pop();
            return None;
        }
        if c == 34 {
            self.p += 1;
            self.push(S::StateString);
            return None;
        }
        let q = if c == 10 { self.ws_end(p) } else { p };
        if self.word("as", q) {
            let e = self.ws_end(q + 2);
            if e > q + 2 {
                self.emit(T::As, p, e);
                self.push(S::StateId);
                return None;
            }
        }
        if c == 10 {
            self.p += 1;
            self.pop();
            return None;
        }
        if c == 123 {
            self.emit(T::StructStart, p, p + 1);
            self.pop();
            self.push(S::Struct);
            return None;
        }
        if is_word(c) {
            let first = self.word_end(p);
            let second = self.ws_end(first);
            if second > first && is_word(self.c(second)) {
                let brace = self.next_brace.at(self.src, second + 1);
                let end = self.line_end(second);
                if let Some(brace) = brace.filter(|&b| b < end) {
                    let found = crate::util::trim(&self.text(p, brace + 1)).to_string();
                    self.out.error = Some(format!(
                        "Error: State name must be a single word. Found: \"{found}\""
                    ));
                    return Some(T::Error);
                }
            }
        }
        let mut j = p + 1;
        while j < self.n && !(self.c(j) == 123 || is_space(self.c(j))) {
            j += 1;
        }
        self.emit(T::CompositState, p, j);
        None
    }

    fn document(&mut self, c: u32) -> Option<T> {
        let p = self.p;
        let root = self.state == S::Initial;
        let lc = c | 32;
        if root {
            if lc == 99 && self.word("click", p) && !is_word(self.c(p + 5)) {
                self.emit(T::Click, p, p + 5);
                return None;
            }
            if lc == 104 && self.word("href", p) && !is_word(self.c(p + 4)) {
                self.emit(T::Href, p, p + 4);
                return None;
            }
            if c == 34 {
                if let Some(close) = self.index_of(34, p + 1) {
                    self.emit(T::String, p, close + 1);
                    return None;
                }
            }
            if lc == 100 && self.word("default", p) && !is_word(self.c(p + 7)) {
                self.emit(T::Default, p, p + 7);
                return None;
            }
            if let Some(kind) = self.direction(p) {
                let e = self.eol(self.dir_end[kind]);
                self.emit(T::DIRECTIONS[kind], p, e);
                return None;
            }
            if c == 10 {
                let mut j = p + 1;
                while self.c(j) == 10 {
                    j += 1;
                }
                self.emit(T::Nl, p, j);
                return None;
            }
            if is_space(c) {
                self.p = self.ws_end(p);
                return None;
            }
        } else if c != 10 && is_space(c) {
            self.skip_inline_space();
            return None;
        }
        if self.comment(c) {
            self.p = self.eol(p);
            return None;
        }
        if lc == 115 {
            if root && self.word("scale", p) {
                let e = self.ws_end(p + 5);
                if e > p + 5 {
                    self.emit(T::Scale, p, e);
                    self.push(S::Scale);
                    return None;
                }
            }
            if self.word("style", p) {
                let e = self.ws_end(p + 5);
                if e > p + 5 {
                    self.emit(T::Style, p, e);
                    self.push(S::Style);
                    return None;
                }
            }
            if self.word("state", p) {
                let e = self.ws_end(p + 5);
                if e > p + 5 {
                    self.p = e;
                    self.push(S::State);
                    return None;
                }
                if root && self.word("diagram", p + 5) {
                    let from = if self.word("-v2", p + 12) {
                        p + 15
                    } else {
                        p + 12
                    };
                    let e = self.ws_end(from);
                    if e > from {
                        self.emit(T::Sd, p, e);
                        return None;
                    }
                }
            }
        } else if lc == 99 {
            if self.word("class", p) {
                let def = self.word("def", p + 5);
                let from = if def { p + 8 } else { p + 5 };
                let e = self.ws_end(from);
                if e > from {
                    self.emit(if def { T::ClassDef } else { T::Class }, p, e);
                    self.push(if def { S::ClassDef } else { S::Class });
                    return None;
                }
            }
        } else if lc == 97 && root && (self.word("acctitle", p) || self.word("accdescr", p)) {
            let title = self.c(p + 3) | 32 == 116;
            let q = self.ws_end(p + 8);
            let d = self.c(q);
            if d == 58 {
                let e = self.ws_end(q + 1);
                self.emit(if title { T::AccTitle } else { T::AccDescr }, p, e);
                self.push(if title { S::AccTitle } else { S::AccDescr });
                return None;
            }
            if d == 123 && !title {
                self.p = self.ws_end(q + 1);
                self.push(S::AccDescrMultiline);
                return None;
            }
        }
        if root {
            if c == 123 {
                self.emit(T::StructStart, p, p + 1);
                self.push(S::Struct);
                return None;
            }
        } else {
            if let Some(kind) = self.direction(p) {
                let e = self.eol(self.dir_end[kind]);
                self.emit(T::DIRECTIONS[kind], p, e);
                return None;
            }
            if c == 125 {
                self.emit(T::StructStop, p, p + 1);
                self.pop();
                return None;
            }
            if c == 10 {
                self.p += 1;
                return None;
            }
        }
        if lc == 110 && self.word("note", p) {
            let e = self.ws_end(p + 4);
            if e > p + 4 {
                self.emit(T::Note, p, e);
                self.push(S::Note);
                return None;
            }
        }
        if root && lc == 104 && self.word("hide empty description", p) && !is_word(self.c(p + 22)) {
            self.emit(T::HideEmpty, p, p + 22);
            return None;
        }
        if c == 91 && self.c(p + 1) == 42 && self.c(p + 2) == 93 {
            self.emit(T::EdgeState, p, p + 3);
            return None;
        }
        let mut j = p;
        while j < self.n {
            let d = self.c(j);
            if d == 58 || d == 45 || d == 123 || is_space(d) {
                break;
            }
            j += 1;
        }
        if j > p {
            self.emit_id(p, j);
            return None;
        }
        if c == 58 {
            j = p + 1;
            while j < self.n {
                let d = self.c(j);
                if d == 10 || d == 59 {
                    break;
                }
                if d == 58 {
                    let after = self.c(j + 1);
                    if j + 1 >= self.n || after == 58 || after == 10 || after == 59 {
                        break;
                    }
                    j += 1;
                }
                j += 1;
            }
            if j > p + 1 {
                let text = crate::util::trim(&self.text(p, j)).to_string();
                self.emit_text(T::Descr, text, p, j);
                return None;
            }
            if self.c(p + 1) == 58 && self.c(p + 2) == 58 {
                self.emit(T::StyleSeparator, p, p + 3);
                return None;
            }
        } else if c == 45 && self.c(p + 1) == 45 {
            if self.c(p + 2) == 62 {
                self.emit(T::Arrow, p, p + 3);
                return None;
            }
            if !root {
                self.emit(T::Concurrent, p, p + 2);
                return None;
            }
        }
        if !root {
            return Some(T::Error);
        }
        self.emit(T::Invalid, p, p + 1);
        None
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
            error: None,
        },
        next_direction: Finder::new(find_direction),
        next_stereotype: Finder::new(find_stereotype),
        next_line_end: Finder::new(find_line_end),
        next_percent: Finder::new(find_percent),
        next_brace: Finder::new(find_brace),
        dir_from: None,
        dir_to: 0,
        dir_at: [None; 4],
        dir_end: [0; 4],
        stereo_from: None,
        stereo_to: 0,
        stereo_at: [None; 6],
    }
    .run()
}
