//! The timeline lexer, as Mermaid's `timeline.jison` lexes. Ported from pele's
//! `diagrams/timeline/lexer.ts`.

use crate::diagrams::common::scan::{
    Tokens, accessibility, at, keyword_at, line_end, skip_trivia, space, word,
};

pub(crate) mod t {
    pub(crate) const END: u8 = 0;
    pub(crate) const EOF: u8 = 1;
    pub(crate) const INVALID: u8 = 2;
    pub(crate) const NEWLINE: u8 = 3;
    pub(crate) const TIMELINE: u8 = 4;
    pub(crate) const TIMELINE_LR: u8 = 5;
    pub(crate) const TIMELINE_TD: u8 = 6;
    pub(crate) const TITLE: u8 = 7;
    pub(crate) const ACC_TITLE: u8 = 8;
    pub(crate) const ACC_DESCR: u8 = 10;
    pub(crate) const ACC_DESCR_MULTILINE_VALUE: u8 = 12;
    pub(crate) const SECTION: u8 = 13;
    pub(crate) const PERIOD: u8 = 14;
    pub(crate) const EVENT: u8 = 15;
}

pub(crate) const TOKEN_NAMES: [&str; 16] = [
    "$end",
    "EOF",
    "INVALID",
    "NEWLINE",
    "timeline",
    "timeline_lr",
    "timeline_td",
    "title",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "section",
    "period",
    "event",
];

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
        if keyword_at(src, p, "timeline") {
            e = p + 8;
            while at(src, e) == 32 || at(src, e) == 9 {
                e += 1;
            }
            let lr = keyword_at(src, e, "lr");
            if e > p + 8 && (lr || keyword_at(src, e, "td")) && !word(at(src, e + 2)) {
                out.push(if lr { t::TIMELINE_LR } else { t::TIMELINE_TD }, p, e + 2);
                p = e + 2;
                continue;
            }
            if !word(at(src, p + 8)) {
                out.push(t::TIMELINE, p, p + 8);
                p += 8;
                continue;
            }
        }
        // `\s` after a keyword also takes a line break, so `title` alone on a line takes the
        // next line as its text.
        if keyword_at(src, p, "title") && space(at(src, p + 5)) && p + 6 < n && at(src, p + 6) != 10
        {
            e = line_end(src, p + 6);
            out.push(t::TITLE, p, e);
            p = e;
            continue;
        }
        e = accessibility(src, p, &mut out, t::ACC_TITLE);
        if e != p {
            p = e;
            continue;
        }
        if keyword_at(src, p, "section") && space(at(src, p + 7)) {
            e = p + 8;
            while e < n && at(src, e) != 58 && at(src, e) != 10 {
                e += 1;
            }
            if e > p + 8 {
                out.push(t::SECTION, p, e);
                p = e;
                continue;
            }
        }
        if c != 58 {
            e = p + 1;
            while e < n && !matches!(at(src, e), 35 | 58 | 10) {
                e += 1;
            }
            out.push(t::PERIOD, p, e);
            p = e;
            continue;
        }
        // An event runs to the end of the line or to a colon that is followed by whitespace.
        if space(at(src, p + 1)) {
            e = p + 2;
            while e < n {
                let k = at(src, e);
                if k == 10 || (k == 58 && space(at(src, e + 1))) {
                    break;
                }
                e += 1;
            }
            if e > p + 2 {
                out.push(t::EVENT, p, e);
                p = e;
                continue;
            }
        }
        out.push(t::INVALID, p, p + 1);
        p += 1;
    }
    // Jison marks its lexer done when a rule runs on empty input, so an accessibility value
    // that matches nothing at the very end of the text is followed by no EOF token.
    if out.starts.last().is_none_or(|&s| s < n) {
        out.push(t::EOF, n, n);
    }
    out.push(t::END, n, n);
    out
}
