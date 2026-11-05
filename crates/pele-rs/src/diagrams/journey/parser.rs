//! The journey parser: Mermaid's `journey.jison` grammar, as pele's `parser.ts` follows it.

use super::db::JourneyDb;
use super::lexer::{TOKEN_NAMES, t, tokenize};
use crate::diagrams::common::scan::byte_offset;
use crate::error::{Error, ErrorCode};
use crate::util::trim;

const STATEMENT: &str = "'EOF', 'SPACE', 'NEWLINE', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'section', 'taskName'";

pub(crate) fn parse_journey(src: &str, db: &mut JourneyDb) -> Result<(), Error> {
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

    if tokens.types[0] != t::JOURNEY {
        return fail(0, "'journey'");
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
            t::TASK_NAME => {
                let name = text(i, 0);
                i += 1;
                if tokens.types[i] != t::TASK_DATA {
                    return fail(i, "'taskData'");
                }
                db.add_task(name, &text(i, 0));
            }
            t::EOF => return Ok(()),
            _ => return fail(i, STATEMENT),
        }
        i += 1;
    }
}
