//! The timeline parser: Mermaid's `timeline.jison` grammar, as pele's `parser.ts` follows it.

use super::db::TimelineDb;
use super::lexer::{TOKEN_NAMES, t, tokenize};
use crate::diagrams::common::scan::byte_offset;
use crate::error::{Error, ErrorCode};
use crate::util::trim;

const STATEMENT: &str = "'EOF', 'SPACE', 'NEWLINE', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'section', 'period', 'event'";

pub(crate) fn parse_timeline(src: &str, db: &mut TimelineDb) -> Result<(), Error> {
    let units: Vec<u16> = src.encode_utf16().collect();
    let tokens = tokenize(&units);
    let text = |i: usize, skip: usize| {
        String::from_utf16_lossy(
            &units[(tokens.starts[i] + skip).min(tokens.ends[i])..tokens.ends[i]],
        )
    };
    let fail = |i: usize, expected: &str| {
        let kind = tokens.types[i];
        let got = if kind == t::END {
            "1"
        } else {
            TOKEN_NAMES[usize::from(kind)]
        };
        Err(Error::new(
            ErrorCode::Syntax,
            format!("Expecting {expected}, got '{got}'"),
        )
        .at(src, byte_offset(src, tokens.starts[i])))
    };

    let head = tokens.types[0];
    if !matches!(head, t::TIMELINE | t::TIMELINE_LR | t::TIMELINE_TD) {
        return fail(0, "'timeline', 'timeline_lr', 'timeline_td'");
    }
    if head != t::TIMELINE {
        db.direction = if head == t::TIMELINE_LR { "LR" } else { "TD" };
    }

    let mut i = 1;
    loop {
        match tokens.types[i] {
            t::NEWLINE => {}
            t::TITLE => db.title = Some(text(i, 6)),
            t::ACC_TITLE => {
                i += 1;
                db.set_acc_title(trim(&text(i, 0)));
            }
            t::ACC_DESCR => {
                i += 1;
                db.set_acc_description(trim(&text(i, 0)));
            }
            t::ACC_DESCR_MULTILINE_VALUE => db.set_acc_description(trim(&text(i, 0))),
            t::SECTION => db.add_section(text(i, 8)),
            t::PERIOD => db.add_period(text(i, 0)),
            t::EVENT => {
                if !db.add_event(text(i, 2)) {
                    return Err(Error::new(
                        ErrorCode::Semantic,
                        "An event needs a time period before it.",
                    ));
                }
            }
            t::EOF => return Ok(()),
            _ => return fail(i, STATEMENT),
        }
        i += 1;
    }
}
