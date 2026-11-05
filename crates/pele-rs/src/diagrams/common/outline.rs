//! The indented outline grammar that Mermaid's mindmap and kanban diagrams share, ported from
//! pele's `diagrams/common/outline.ts`. Kanban adds `@{ ... }` metadata after a node and keeps
//! `@` out of node ids. The text is read as UTF-16 code units, as Jison reads it.

use super::scan::{NONE, at, byte_offset, space, word};
use crate::error::{Error, ErrorCode};

pub(crate) mod t {
    pub(crate) const END: u8 = 0;
    pub(crate) const ERROR: u8 = 1;
    pub(crate) const SPACELINE: u8 = 2;
    pub(crate) const NL: u8 = 3;
    pub(crate) const EOF: u8 = 4;
    pub(crate) const KEYWORD: u8 = 5;
    pub(crate) const SPACELIST: u8 = 6;
    pub(crate) const ICON: u8 = 7;
    pub(crate) const CLASS: u8 = 8;
    pub(crate) const NODE_DSTART: u8 = 9;
    pub(crate) const NODE_ID: u8 = 10;
    pub(crate) const NODE_DESCR: u8 = 11;
    pub(crate) const NODE_DEND: u8 = 12;
    pub(crate) const SHAPE_DATA: u8 = 13;
}

/// As the generated parser prints them. It has no name for the end of input and prints its
/// number.
const NAMES: [&str; 14] = [
    "1",
    "INVALID",
    "SPACELINE",
    "NL",
    "EOF",
    "",
    "SPACELIST",
    "ICON",
    "CLASS",
    "NODE_DSTART",
    "NODE_ID",
    "NODE_DESCR",
    "NODE_DEND",
    "SHAPE_DATA",
];

fn token_name(kind: u8, kanban: bool) -> &'static str {
    if kind == t::KEYWORD {
        if kanban { "KANBAN" } else { "MINDMAP" }
    } else {
        NAMES[usize::from(kind)]
    }
}

pub(crate) struct Tokens {
    pub types: Vec<u8>,
    pub texts: Vec<String>,
    pub starts: Vec<usize>,
}

/// Node shapes, as Mermaid numbers them.
pub(crate) mod node_type {
    pub(crate) const DEFAULT: u8 = 0;
    pub(crate) const ROUNDED_RECT: u8 = 1;
    pub(crate) const RECT: u8 = 2;
    pub(crate) const CIRCLE: u8 = 3;
    pub(crate) const CLOUD: u8 = 4;
    pub(crate) const BANG: u8 = 5;
    pub(crate) const HEXAGON: u8 = 6;
}

/// Any opening delimiter pairs with any closing one, as in Mermaid.
pub(crate) fn get_type(start: &str, end: &str) -> u8 {
    match start {
        "[" => node_type::RECT,
        "(" if end == ")" => node_type::ROUNDED_RECT,
        "(" | ")" => node_type::CLOUD,
        "((" => node_type::CIRCLE,
        "))" => node_type::BANG,
        "{{" => node_type::HEXAGON,
        _ => node_type::DEFAULT,
    }
}

/// What the grammar's actions call.
pub(crate) trait OutlineDb {
    fn add_node(
        &mut self,
        level: usize,
        id: &str,
        descr: &str,
        kind: u8,
        shape_data: Option<&str>,
    ) -> Result<(), Error>;
    fn decorate_node(&mut self, icon: Option<&str>, class: Option<&str>);
}

#[derive(Clone, Copy, PartialEq)]
enum S {
    Initial,
    Class,
    Icon,
    Node,
    Nstr,
    Nstr2,
    Data,
    DataStr,
}

/// A line terminator, which `.` does not match.
fn line_break(c: u32) -> bool {
    matches!(c, 10 | 13 | 0x2028 | 0x2029)
}

struct Lexer<'a> {
    src: &'a [u16],
    p: usize,
    out: Tokens,
}

impl Lexer<'_> {
    fn c(&self, p: usize) -> u32 {
        at(self.src, p)
    }

    fn put(&mut self, kind: u8, text: String, end: usize) {
        self.out.types.push(kind);
        self.out.texts.push(text);
        self.out.starts.push(self.p);
        self.p = end;
    }

    fn emit(&mut self, kind: u8, end: usize) {
        let text = String::from_utf16_lossy(&self.src[self.p..end]);
        self.put(kind, text, end);
    }

    /// Where a sticky match of one or more units for which `ok` holds ends, if there is one.
    fn run(&self, from: usize, ok: impl Fn(u32) -> bool) -> Option<usize> {
        let mut e = from;
        while e < self.src.len() && ok(self.c(e)) {
            e += 1;
        }
        (e > from).then_some(e)
    }

    fn up_to(&self, ch: u16) -> usize {
        self.src[self.p..]
            .iter()
            .position(|&c| c == ch)
            .map_or(self.src.len(), |k| self.p + k)
    }
}

/// `src.slice(from, to)` with `/\n\s*/g` replaced by `<br/>`.
fn with_breaks(units: &[u16]) -> String {
    let mut out: Vec<u16> = Vec::with_capacity(units.len());
    let mut i = 0;
    while i < units.len() {
        if units[i] == 10 {
            i += 1;
            while i < units.len() && space(u32::from(units[i])) {
                i += 1;
            }
            out.extend("<br/>".encode_utf16());
        } else {
            out.push(units[i]);
            i += 1;
        }
    }
    String::from_utf16_lossy(&out)
}

/// Jison's case-insensitive flag folds ASCII letters only.
fn letters(src: &[u16], p: usize, word: &str) -> bool {
    word.bytes()
        .enumerate()
        .all(|(k, b)| at(src, p + k) != NONE && (at(src, p + k) | 32) == u32::from(b))
}

pub(crate) fn tokenize(src: &[u16], kanban: bool) -> Tokens {
    let n = src.len();
    let keyword = if kanban { "kanban" } else { "mindmap" };
    let id_char = |c: u32| !(matches!(c, 40 | 91 | 10 | 41 | 123 | 125) || (kanban && c == 64));
    let mut lx = Lexer {
        src,
        p: 0,
        out: Tokens {
            types: Vec::new(),
            texts: Vec::new(),
            starts: Vec::new(),
        },
    };
    let mut state = S::Initial;
    let mut next_break: Option<usize> = None;

    while lx.p < n {
        let p = lx.p;
        let c = lx.c(p);
        let d = lx.c(p + 1);
        match state {
            S::Initial => {
                let w = if c <= 32 || c >= 160 {
                    lx.run(p, space).unwrap_or(p)
                } else {
                    p
                };
                if kanban && c == 64 && d == 123 {
                    lx.put(t::SHAPE_DATA, String::new(), p + 2);
                    state = S::Data;
                } else if lx.c(w) == 37 && lx.c(w + 1) == 37 {
                    let e = lx.run(w, |c| !line_break(c)).unwrap_or(w);
                    lx.emit(t::SPACELINE, e);
                } else if w > p {
                    if next_break.is_none_or(|b| b < p) {
                        next_break = Some(lx.up_to(10));
                    }
                    let last_break = src[p + 1..w].iter().rposition(|&c| c == 10);
                    if next_break.is_some_and(|b| b >= w) {
                        lx.emit(t::SPACELIST, w);
                    } else if let Some(k) = last_break {
                        lx.emit(t::SPACELINE, p + 1 + k + 1);
                    } else {
                        lx.emit(t::NL, p + 1);
                    }
                } else if letters(src, p, keyword) && !word(lx.c(p + keyword.len())) {
                    lx.emit(t::KEYWORD, p + keyword.len());
                } else if c == 58 && d == 58 && lx.c(p + 2) == 58 {
                    lx.p += 3;
                    state = S::Class;
                } else if c == 58 && d == 58 && letters(src, p + 2, "icon") && lx.c(p + 6) == 40 {
                    lx.p += 7;
                    state = S::Icon;
                } else if (c == 45 && d == 41)
                    || (c == 40 && (d == 45 || d == 40))
                    || ((c == 41 || c == 123) && d == c)
                {
                    lx.emit(t::NODE_DSTART, p + 2);
                    state = S::Node;
                } else if c == 41 || c == 40 || c == 91 {
                    lx.emit(t::NODE_DSTART, p + 1);
                    state = S::Node;
                } else if let Some(e) = lx.run(p, id_char) {
                    lx.emit(t::NODE_ID, e);
                } else {
                    break;
                }
            }
            S::Node => {
                if c == 34 {
                    state = if d == 96 { S::Nstr2 } else { S::Nstr };
                    lx.p += if d == 96 { 2 } else { 1 };
                } else if c == 41 || c == 40 {
                    let e = if d == c || (c == 40 && d == 45) {
                        p + 2
                    } else {
                        p + 1
                    };
                    lx.emit(t::NODE_DEND, e);
                    state = S::Initial;
                } else if c == 93 {
                    lx.emit(t::NODE_DEND, p + 1);
                    state = S::Initial;
                } else if (c == 125 && d == 125) || (c == 45 && d == 41) {
                    lx.emit(t::NODE_DEND, p + 2);
                    state = S::Initial;
                } else {
                    // A lone closing brace takes the rest of its line as text.
                    // Neither match can fail here: the unit at `p` is in both.
                    let e = if c == 125 {
                        lx.run(p, |c| !line_break(c))
                    } else {
                        lx.run(p, |c| !matches!(c, 41 | 93 | 40 | 125))
                    };
                    lx.emit(t::NODE_DESCR, e.unwrap_or(p + 1));
                }
            }
            S::Nstr => {
                if c == 34 {
                    lx.p += 1;
                    state = S::Node;
                } else {
                    let e = lx.up_to(34);
                    lx.emit(t::NODE_DESCR, e);
                }
            }
            S::Nstr2 => {
                if c == 96 && d == 34 {
                    lx.p += 2;
                    state = S::Node;
                } else if let Some(e) = lx.run(p, |c| c != 96 && c != 34) {
                    lx.emit(t::NODE_DESCR, e);
                } else {
                    break;
                }
            }
            S::Class => {
                if let Some(e) = lx.run(p, |c| !line_break(c)) {
                    lx.emit(t::CLASS, e);
                    state = S::Initial;
                } else if c == 10 {
                    lx.p += 1;
                    state = S::Initial;
                } else {
                    break;
                }
            }
            S::Icon => {
                if c == 41 {
                    lx.p += 1;
                    state = S::Initial;
                } else {
                    let e = lx.up_to(41);
                    lx.emit(t::ICON, e);
                }
            }
            S::Data => {
                if c == 34 {
                    lx.emit(t::SHAPE_DATA, p + 1);
                    state = S::DataStr;
                } else if c == 125 {
                    lx.p += 1;
                    state = S::Initial;
                } else if let Some(e) = lx.run(p, |c| !matches!(c, 125 | 94 | 34)) {
                    lx.emit(t::SHAPE_DATA, e);
                } else {
                    break;
                }
            }
            S::DataStr => {
                if c == 34 {
                    lx.emit(t::SHAPE_DATA, p + 1);
                    state = S::Data;
                } else {
                    let e = lx.up_to(34);
                    let text = with_breaks(&src[p..e]);
                    lx.put(t::SHAPE_DATA, text, e);
                }
            }
        }
    }

    if lx.p < n {
        lx.put(t::ERROR, String::new(), lx.p);
    } else {
        if state == S::Initial {
            lx.put(t::EOF, String::new(), n);
        }
        lx.put(t::END, String::new(), n);
    }
    lx.out
}

const STOP: &str = "'SPACELINE', 'NL', 'EOF'";

fn is_stop(kind: u8) -> bool {
    matches!(kind, t::SPACELINE | t::NL | t::EOF)
}

fn is_start(kind: u8) -> bool {
    matches!(
        kind,
        t::SPACELINE | t::SPACELIST | t::ICON | t::CLASS | t::NODE_DSTART | t::NODE_ID
    )
}

/// Makes the calls on `db` that the grammar's actions make, in the same order, and reports
/// errors in the state where the generated parser would.
pub(crate) fn parse_outline(src: &str, db: &mut dyn OutlineDb, kanban: bool) -> Result<(), Error> {
    let units: Vec<u16> = src.encode_utf16().collect();
    let tokens = tokenize(&units, kanban);
    let types = &tokens.types;
    let texts = &tokens.texts;
    let data = if kanban { ", 'SHAPE_DATA'" } else { "" };
    let kind = |i: usize| types.get(i).copied().unwrap_or(t::END);
    let fail = |i: usize, expected: &str| -> Result<(), Error> {
        let k = kind(i);
        let offset = byte_offset(src, tokens.starts[i.min(tokens.starts.len() - 1)]);
        if k == t::ERROR {
            return Err(Error::new(ErrorCode::Syntax, "Unrecognized text.").at(src, offset));
        }
        Err(Error::new(
            ErrorCode::Syntax,
            format!("Expecting {expected}, got '{}'", token_name(k, kanban)),
        )
        .at(src, offset))
    };

    let keyword = format!("'{}'", token_name(t::KEYWORD, kanban));
    let mut i = 0;
    if kind(i) == t::SPACELINE {
        i += 1;
        while matches!(kind(i), t::SPACELINE | t::NL) {
            i += 1;
        }
        if kind(i) != t::KEYWORD {
            return fail(i, &format!("'SPACELINE', 'NL', {keyword}"));
        }
    } else if kind(i) != t::KEYWORD {
        return fail(i, &format!("'SPACELINE', {keyword}"));
    }
    i += 1;
    let mut first = "'SPACELINE', 'NL', 'SPACELIST', 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'";
    if kind(i) == t::NL {
        i += 1;
        first = "'SPACELINE', 'SPACELIST', 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'";
    }

    loop {
        let mut k = kind(i);
        let mut level = 0;
        let indented = k == t::SPACELIST;
        if indented {
            level = texts[i].encode_utf16().count();
            i += 1;
            k = kind(i);
        }
        if k == t::NODE_ID || k == t::NODE_DSTART {
            let mut id: Option<&str> = None;
            let descr: &str;
            let mut node = 0;
            if k == t::NODE_ID {
                id = Some(&texts[i]);
                i += 1;
            }
            if kind(i) == t::NODE_DSTART {
                let start = &texts[i];
                i += 1;
                if kind(i) != t::NODE_DESCR {
                    return fail(i, "'NODE_DESCR'");
                }
                descr = &texts[i];
                i += 1;
                if kind(i) != t::NODE_DEND {
                    return fail(i, "'NODE_DEND'");
                }
                let end = &texts[i];
                i += 1;
                if !is_stop(kind(i)) && kind(i) != t::SHAPE_DATA {
                    return fail(i, &format!("{STOP}{data}"));
                }
                node = get_type(start, end);
            } else {
                if !is_stop(kind(i)) && kind(i) != t::SHAPE_DATA {
                    return fail(i, &format!("{STOP}, 'NODE_DSTART'{data}"));
                }
                descr = id.unwrap_or_default();
            }
            let id = id.unwrap_or(descr);
            if kind(i) == t::SHAPE_DATA {
                let mut shape_data = texts[i].clone();
                i += 1;
                while kind(i) == t::SHAPE_DATA {
                    shape_data += &texts[i];
                    i += 1;
                }
                if !is_stop(kind(i)) {
                    return fail(i, &format!("{STOP}{data}"));
                }
                db.add_node(level, id, descr, node, Some(&shape_data))?;
            } else {
                db.add_node(level, id, descr, node, None)?;
            }
        } else if k == t::ICON || k == t::CLASS {
            let text = &texts[i];
            i += 1;
            if !is_stop(kind(i)) {
                return fail(i, STOP);
            }
            if k == t::ICON {
                db.decorate_node(Some(text), None);
            } else {
                db.decorate_node(None, Some(text));
            }
        } else if indented {
            if !is_stop(k) {
                return fail(
                    i,
                    &format!("{STOP}, 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'"),
                );
            }
        } else if k == t::SPACELINE {
            i += 1;
            if !is_stop(kind(i)) {
                return fail(i, STOP);
            }
        } else {
            return fail(i, first);
        }

        i += 1;
        while matches!(kind(i), t::NL | t::EOF) {
            i += 1;
        }
        if kind(i) == t::END {
            return Ok(());
        }
        if !is_start(kind(i)) {
            return fail(
                i,
                &format!("{STOP}, 'SPACELIST', 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'"),
            );
        }
    }
}
