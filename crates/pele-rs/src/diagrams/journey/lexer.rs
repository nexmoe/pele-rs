//! The journey lexer, as Mermaid's `journey.jison` lexes. Ported from pele's
//! `diagrams/journey/lexer.ts`.

use crate::diagrams::common::scan::{
    Tokens, accessibility, at, keyword_at, skip_trivia, space, word,
};

pub(crate) mod t {
    pub(crate) const END: u8 = 0;
    pub(crate) const EOF: u8 = 1;
    pub(crate) const INVALID: u8 = 2;
    pub(crate) const NEWLINE: u8 = 3;
    pub(crate) const JOURNEY: u8 = 4;
    pub(crate) const TITLE: u8 = 5;
    pub(crate) const ACC_TITLE: u8 = 6;
    pub(crate) const ACC_DESCR: u8 = 8;
    pub(crate) const ACC_DESCR_MULTILINE_VALUE: u8 = 10;
    pub(crate) const SECTION: u8 = 11;
    pub(crate) const TASK_NAME: u8 = 12;
    pub(crate) const TASK_DATA: u8 = 13;
    pub(crate) const COLON: u8 = 14;
}

pub(crate) const TOKEN_NAMES: [&str; 15] = [
    "$end",
    "EOF",
    "INVALID",
    "NEWLINE",
    "journey",
    "title",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "section",
    "taskName",
    "taskData",
    ":",
];

/// `#`, a line break, and `;` end every kind of text; a colon also ends a task name or a
/// section.
fn text_end(src: &[u16], from: usize, colon: bool) -> usize {
    let mut e = from;
    while e < src.len() {
        let c = src[e];
        if c == 35 || c == 10 || c == 59 || (colon && c == 58) {
            break;
        }
        e += 1;
    }
    e
}

pub(crate) fn tokenize(src: &[u16]) -> Tokens {
    let n = src.len();
    let mut out = Tokens::default();
    let mut p = 0;
    loop {
        p = skip_trivia(src, p);
        if p >= n {
            break;
        }
        let c = at(src, p);
        let mut e;
        if c == 10 {
            e = p + 1;
            while at(src, e) == 10 {
                e += 1;
            }
            out.push(t::NEWLINE, p, e);
            p = e;
            continue;
        }
        if keyword_at(src, p, "journey") && !word(at(src, p + 7)) {
            out.push(t::JOURNEY, p, p + 7);
            p += 7;
            continue;
        }
        // `\s` after a keyword also takes a line break, so `title` alone on a line takes the
        // next line as its text.
        if keyword_at(src, p, "title") && space(at(src, p + 5)) {
            e = text_end(src, p + 6, false);
            if e > p + 6 {
                out.push(t::TITLE, p, e);
                p = e;
                continue;
            }
        }
        e = accessibility(src, p, &mut out, t::ACC_TITLE);
        if e != p {
            p = e;
            continue;
        }
        if keyword_at(src, p, "section") && space(at(src, p + 7)) {
            e = text_end(src, p + 8, true);
            if e > p + 8 {
                out.push(t::SECTION, p, e);
                p = e;
                continue;
            }
        }
        if c == 58 {
            e = text_end(src, p + 1, false);
            if e > p + 1 {
                out.push(t::TASK_DATA, p, e);
                p = e;
            } else {
                out.push(t::COLON, p, p + 1);
                p += 1;
            }
        } else if c == 59 {
            out.push(t::INVALID, p, p + 1);
            p += 1;
        } else {
            e = text_end(src, p + 1, true);
            out.push(t::TASK_NAME, p, e);
            p = e;
        }
    }
    // Jison marks its lexer done when a rule runs on empty input, so an accessibility value
    // that matches nothing at the very end of the text is followed by no EOF token.
    if out.starts.last().is_none_or(|&s| s < n) {
        out.push(t::EOF, n, n);
    }
    out.push(t::END, n, n);
    out
}
