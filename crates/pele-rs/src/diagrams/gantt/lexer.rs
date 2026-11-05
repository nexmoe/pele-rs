//! The gantt lexer, as Mermaid's `gantt.jison` lexes. Ported from pele's
//! `diagrams/gantt/lexer.ts`. The text is read as UTF-16 code units, as Jison reads it.

use crate::diagrams::common::scan::{Tokens, at, keyword_at, space, word};

pub(crate) mod t {
    pub(crate) const END: u8 = 0;
    pub(crate) const ERROR: u8 = 1;
    pub(crate) const OPEN_DIRECTIVE: u8 = 2;
    pub(crate) const ACC_TITLE: u8 = 3;
    pub(crate) const ACC_TITLE_VALUE: u8 = 4;
    pub(crate) const ACC_DESCR: u8 = 5;
    pub(crate) const ACC_DESCR_VALUE: u8 = 6;
    pub(crate) const ACC_DESCR_MULTILINE_VALUE: u8 = 7;
    pub(crate) const NL: u8 = 8;
    pub(crate) const HREF: u8 = 9;
    pub(crate) const CALLBACKNAME: u8 = 10;
    pub(crate) const CALLBACKARGS: u8 = 11;
    pub(crate) const CLICK: u8 = 12;
    pub(crate) const GANTT: u8 = 13;
    pub(crate) const DATE_FORMAT: u8 = 14;
    pub(crate) const INCLUSIVE_END_DATES: u8 = 15;
    pub(crate) const TOP_AXIS: u8 = 16;
    pub(crate) const AXIS_FORMAT: u8 = 17;
    pub(crate) const TICK_INTERVAL: u8 = 18;
    pub(crate) const INCLUDES: u8 = 19;
    pub(crate) const EXCLUDES: u8 = 20;
    pub(crate) const TODAY_MARKER: u8 = 21;
    pub(crate) const WEEKDAY_MONDAY: u8 = 22;
    pub(crate) const WEEKEND_FRIDAY: u8 = 29;
    pub(crate) const WEEKEND_SATURDAY: u8 = 30;
    pub(crate) const DATE: u8 = 31;
    pub(crate) const TITLE: u8 = 32;
    pub(crate) const ACC_DESCRIPTION: u8 = 33;
    pub(crate) const SECTION: u8 = 34;
    pub(crate) const TASK_TXT: u8 = 35;
    pub(crate) const TASK_DATA: u8 = 36;
    pub(crate) const COLON: u8 = 37;
    pub(crate) const EOF: u8 = 38;
}

pub(crate) const TOKEN_NAMES: [&str; 39] = [
    "$end",
    "INVALID",
    "open_directive",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "NL",
    "href",
    "callbackname",
    "callbackargs",
    "click",
    "gantt",
    "dateFormat",
    "inclusiveEndDates",
    "topAxis",
    "axisFormat",
    "tickInterval",
    "includes",
    "excludes",
    "todayMarker",
    "weekday_monday",
    "weekday_tuesday",
    "weekday_wednesday",
    "weekday_thursday",
    "weekday_friday",
    "weekday_saturday",
    "weekday_sunday",
    "weekend_friday",
    "weekend_saturday",
    "date",
    "title",
    "accDescription",
    "section",
    "taskTxt",
    "taskData",
    ":",
    "EOF",
];

#[derive(Clone, Copy, PartialEq)]
enum S {
    Initial,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Href,
    CallbackName,
    CallbackArgs,
    Click,
}

const DAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

/// What follows a statement keyword: `\s` and then text up to any of `stops`.
#[derive(Clone, Copy)]
enum Rest {
    /// `\b`: the keyword stands alone.
    Alone,
    /// `\s[^...]+`, with the code units that end the text.
    Text(&'static [u16]),
}

const NO_HASH: &[u16] = &[35, 10, 59];
const NO_SEMI: &[u16] = &[10, 59];
const LINE: &[u16] = &[10];

/// Statements that start with a keyword, grouped by the keyword's first letter and kept in the
/// grammar's rule order.
fn keywords(c: u32) -> &'static [(&'static str, Rest, u8)] {
    match c {
        97 => &[
            ("axisformat", Rest::Text(NO_HASH), t::AXIS_FORMAT),
            ("accdescription", Rest::Text(NO_HASH), t::ACC_DESCRIPTION),
        ],
        103 => &[("gantt", Rest::Alone, t::GANTT)],
        100 => &[("dateformat", Rest::Text(NO_HASH), t::DATE_FORMAT)],
        105 => &[
            ("inclusiveenddates", Rest::Alone, t::INCLUSIVE_END_DATES),
            ("includes", Rest::Text(NO_HASH), t::INCLUDES),
        ],
        116 => &[
            ("topaxis", Rest::Alone, t::TOP_AXIS),
            ("tickinterval", Rest::Text(NO_HASH), t::TICK_INTERVAL),
            ("todaymarker", Rest::Text(NO_SEMI), t::TODAY_MARKER),
            ("title", Rest::Text(LINE), t::TITLE),
        ],
        101 => &[("excludes", Rest::Text(NO_HASH), t::EXCLUDES)],
        115 => &[("section", Rest::Text(LINE), t::SECTION)],
        _ => &[],
    }
}

fn skip_space(src: &[u16], from: usize) -> usize {
    let mut p = from;
    while space(at(src, p)) {
        p += 1;
    }
    p
}

/// Where `\s*` then `ch` end, after a keyword ending at `p`.
fn then(src: &[u16], p: usize, ch: u32) -> Option<usize> {
    let q = skip_space(src, p);
    (at(src, q) == ch).then_some(q + 1)
}

/// Where a keyword, `\s+`, and then a word from `words` end, with `\b` after the word.
fn word_after(src: &[u16], p: usize, keyword: &str, words: &[&str]) -> Option<(usize, usize)> {
    if !keyword_at(src, p, keyword) {
        return None;
    }
    let q = skip_space(src, p + keyword.len());
    if q == p + keyword.len() {
        return None;
    }
    words.iter().enumerate().find_map(|(i, w)| {
        (keyword_at(src, q, w) && !word(at(src, q + w.len()))).then_some((i, q + w.len()))
    })
}

fn until(src: &[u16], ch: u16, from: usize) -> usize {
    src.get(from..)
        .and_then(|rest| rest.iter().position(|&c| c == ch))
        .map_or(src.len(), |k| from + k)
}

fn digits(src: &[u16], p: usize, n: usize) -> bool {
    (p..p + n).all(|i| (48..=57).contains(&at(src, i)))
}

pub(crate) fn tokenize(src: &[u16]) -> Tokens {
    let n = src.len();
    let mut out = Tokens::default();
    let mut state = S::Initial;
    let mut p = 0;

    loop {
        if p >= n {
            let kind = match state {
                S::AccTitle => t::ACC_TITLE_VALUE,
                S::AccDescr => t::ACC_DESCR_VALUE,
                S::AccDescrMultiline => t::ACC_DESCR_MULTILINE_VALUE,
                S::Href => t::HREF,
                S::CallbackName => t::CALLBACKNAME,
                S::CallbackArgs => t::CALLBACKARGS,
                S::Click => t::CLICK,
                S::Initial => t::EOF,
            };
            out.push(kind, n, n);
            break;
        }
        let c = at(src, p);
        match state {
            S::Initial => {
                let d = at(src, p + 1);
                if c == 37 {
                    if d != 37 {
                        let mut e = until(src, 10, p + 1);
                        while at(src, e) == 10 {
                            e += 1;
                        }
                        p = e;
                    } else if at(src, p + 2) == 123 {
                        // The grammar enters a lexer state it never defines, so nothing can
                        // follow.
                        out.push(t::OPEN_DIRECTIVE, p, p + 3);
                        out.push(t::ERROR, p, p);
                        return out;
                    } else {
                        p = until(src, 10, p + 2);
                    }
                } else if c != 125 && d == 37 {
                    // Any character but `}` followed by `%` starts a comment that runs to the
                    // end of the line.
                    p = until(src, 10, p + 2);
                } else if c == 10 {
                    let mut e = p + 1;
                    while at(src, e) == 10 {
                        e += 1;
                    }
                    out.push(t::NL, p, e);
                    p = e;
                } else if space(c) {
                    p = skip_space(src, p + 1);
                } else if let Some((kind, e, next)) = keyword(src, p, c) {
                    if kind != NONE_KIND {
                        out.push(kind, p, e);
                    }
                    p = e;
                    if let Some(next) = next {
                        state = next;
                    }
                } else if (48..=57).contains(&c)
                    && digits(src, p, 4)
                    && at(src, p + 4) == 45
                    && digits(src, p + 5, 2)
                    && at(src, p + 7) == 45
                    && digits(src, p + 8, 2)
                    && !word(at(src, p + 10))
                {
                    out.push(t::DATE, p, p + 10);
                    p += 10;
                } else if c != 58 {
                    let mut e = p + 1;
                    while e < n && src[e] != 58 && src[e] != 10 {
                        e += 1;
                    }
                    out.push(t::TASK_TXT, p, e);
                    p = e;
                } else {
                    let mut e = p + 1;
                    while e < n && !matches!(src[e], 35 | 10 | 59) {
                        e += 1;
                    }
                    out.push(if e > p + 1 { t::TASK_DATA } else { t::COLON }, p, e);
                    p = e;
                }
            }
            S::AccTitle | S::AccDescr => {
                let kind = if state == S::AccTitle {
                    t::ACC_TITLE_VALUE
                } else {
                    t::ACC_DESCR_VALUE
                };
                state = S::Initial;
                let e = until(src, 10, p);
                out.push(kind, p, e);
                p = e;
            }
            S::AccDescrMultiline => {
                if c == 125 {
                    state = S::Initial;
                    p += 1;
                } else {
                    let e = until(src, 125, p);
                    out.push(t::ACC_DESCR_MULTILINE_VALUE, p, e);
                    p = e;
                }
            }
            S::Href => {
                if c == 34 {
                    state = S::Initial;
                    p += 1;
                } else {
                    let e = until(src, 34, p);
                    out.push(t::HREF, p, e);
                    p = e;
                }
            }
            S::CallbackName => {
                if c == 40 {
                    // `\(\s*\)`: no arguments.
                    let q = skip_space(src, p + 1);
                    if at(src, q) == 41 {
                        state = S::Initial;
                        p = q + 1;
                    } else {
                        state = S::CallbackArgs;
                        p += 1;
                    }
                } else {
                    let e = until(src, 40, p);
                    out.push(t::CALLBACKNAME, p, e);
                    p = e;
                }
            }
            S::CallbackArgs => {
                if c == 41 {
                    state = S::Initial;
                    p += 1;
                } else {
                    let e = until(src, 41, p);
                    out.push(t::CALLBACKARGS, p, e);
                    p = e;
                }
            }
            S::Click => {
                if space(c) {
                    state = S::Initial;
                    p += 1;
                } else {
                    let mut e = p + 1;
                    while e < n && !space(at(src, e)) {
                        e += 1;
                    }
                    out.push(t::CLICK, p, e);
                    p = e;
                }
            }
        }
    }
    out.push(t::END, n, n);
    out
}

/// A keyword match that starts a lexer state without a token.
const NONE_KIND: u8 = u8::MAX;

/// A statement keyword at `p`: its token (or `NONE_KIND`), where it ends, and the state it
/// enters.
fn keyword(src: &[u16], p: usize, c: u32) -> Option<(u8, usize, Option<S>)> {
    match c | 32 {
        97 => {
            if keyword_at(src, p, "acctitle") {
                if let Some(e) = then(src, p + 8, 58) {
                    return Some((t::ACC_TITLE, skip_space(src, e), Some(S::AccTitle)));
                }
            }
            if keyword_at(src, p, "accdescr") {
                if let Some(e) = then(src, p + 8, 58) {
                    return Some((t::ACC_DESCR, skip_space(src, e), Some(S::AccDescr)));
                }
                if let Some(e) = then(src, p + 8, 123) {
                    return Some((NONE_KIND, skip_space(src, e), Some(S::AccDescrMultiline)));
                }
            }
        }
        104 => {
            if keyword_at(src, p, "href") {
                let q = skip_space(src, p + 4);
                if q > p + 4 && at(src, q) == 34 {
                    return Some((NONE_KIND, q + 1, Some(S::Href)));
                }
            }
        }
        99 => {
            for (kw, next) in [("call", S::CallbackName), ("click", S::Click)] {
                if keyword_at(src, p, kw) {
                    let q = skip_space(src, p + kw.len());
                    if q > p + kw.len() {
                        return Some((NONE_KIND, q, Some(next)));
                    }
                }
            }
        }
        119 => {
            if let Some((i, e)) = word_after(src, p, "weekday", &DAYS) {
                return Some((t::WEEKDAY_MONDAY + i as u8, e, None));
            }
            if let Some((i, e)) = word_after(src, p, "weekend", &["friday", "saturday"]) {
                let kind = if i == 0 {
                    t::WEEKEND_FRIDAY
                } else {
                    t::WEEKEND_SATURDAY
                };
                return Some((kind, e, None));
            }
        }
        _ => {}
    }
    for &(kw, rest, kind) in keywords(c | 32) {
        if !keyword_at(src, p, kw) {
            continue;
        }
        let q = p + kw.len();
        match rest {
            Rest::Alone => {
                if !word(at(src, q)) {
                    return Some((kind, q, None));
                }
            }
            Rest::Text(stops) => {
                if space(at(src, q)) {
                    let e = src[q + 1..]
                        .iter()
                        .position(|c| stops.contains(c))
                        .map_or(src.len(), |k| q + 1 + k);
                    if e > q + 1 {
                        return Some((kind, e, None));
                    }
                }
            }
        }
    }
    None
}
