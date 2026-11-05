//! The gantt parser: Mermaid's `gantt.jison` grammar, as pele's `parser.ts` follows it.

use super::db::GanttDb;
use super::lexer::{TOKEN_NAMES, t, tokenize};
use crate::diagrams::common::scan::byte_offset;
use crate::error::{Error, ErrorCode};
use crate::util::trim;

const WEEKDAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

const LINE: &str = "'EOF', 'SPACE', 'NL', 'weekday_monday', 'weekday_tuesday', 'weekday_wednesday', 'weekday_thursday', 'weekday_friday', 'weekday_saturday', 'weekday_sunday', 'weekend_friday', 'weekend_saturday', 'dateFormat', 'inclusiveEndDates', 'topAxis', 'axisFormat', 'tickInterval', 'excludes', 'includes', 'todayMarker', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'section', 'taskTxt', 'click'";

/// Tokens that can start a line, which are also the only ones that can follow a complete
/// statement.
fn starts_line(kind: u8) -> bool {
    matches!(
        kind,
        t::EOF
            | t::NL
            | t::DATE_FORMAT
            | t::INCLUSIVE_END_DATES
            | t::TOP_AXIS
            | t::AXIS_FORMAT
            | t::TICK_INTERVAL
            | t::EXCLUDES
            | t::INCLUDES
            | t::TODAY_MARKER
            | t::TITLE
            | t::ACC_TITLE
            | t::ACC_DESCR
            | t::ACC_DESCR_MULTILINE_VALUE
            | t::SECTION
            | t::TASK_TXT
            | t::CLICK
            | t::WEEKDAY_MONDAY..=t::WEEKEND_SATURDAY
    )
}

pub(crate) fn parse_gantt(src: &str, db: &mut GanttDb) -> Result<(), Error> {
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
    // Mermaid's parser runs a statement's action only after seeing that the next token can
    // follow it.
    let end = |i: usize, also: &str| {
        if starts_line(tokens.types[i]) {
            Ok(())
        } else {
            fail(i, &format!("{LINE}{also}"))
        }
    };

    if tokens.types[0] != t::GANTT {
        return fail(0, "'gantt'");
    }
    let mut i = 1;
    loop {
        let kind = tokens.types[i];
        if !starts_line(kind) {
            return fail(i, LINE);
        }
        let at = i;
        i += 1;
        match kind {
            t::EOF => {
                if tokens.types[i] == t::END {
                    return Ok(());
                }
                end(i, "")?;
            }
            t::NL => {}
            t::DATE_FORMAT => {
                end(i, "")?;
                db.date_format = text(at, 11);
            }
            t::INCLUSIVE_END_DATES => {
                end(i, "")?;
                db.inclusive_end_dates = true;
            }
            t::TOP_AXIS => {
                end(i, "")?;
                db.top_axis = true;
            }
            t::AXIS_FORMAT => {
                end(i, "")?;
                db.axis_format = text(at, 11);
            }
            t::TICK_INTERVAL => {
                end(i, "")?;
                db.tick_interval = Some(text(at, 13));
            }
            t::EXCLUDES => {
                end(i, "")?;
                db.set_excludes(&text(at, 9));
            }
            t::INCLUDES => {
                end(i, "")?;
                db.set_includes(&text(at, 9));
            }
            t::TODAY_MARKER => {
                end(i, "")?;
                db.today_marker = text(at, 12);
            }
            t::WEEKEND_FRIDAY | t::WEEKEND_SATURDAY => {
                end(i, "")?;
                db.weekend = if kind == t::WEEKEND_FRIDAY {
                    "friday"
                } else {
                    "saturday"
                };
            }
            t::TITLE => {
                end(i, "")?;
                db.title = Some(text(at, 6));
            }
            t::ACC_TITLE | t::ACC_DESCR => {
                let value = if kind == t::ACC_TITLE {
                    t::ACC_TITLE_VALUE
                } else {
                    t::ACC_DESCR_VALUE
                };
                if tokens.types[i] != value {
                    return fail(i, &format!("'{}'", TOKEN_NAMES[usize::from(value)]));
                }
                let value = text(i, 0);
                i += 1;
                end(i, "")?;
                if kind == t::ACC_TITLE {
                    db.set_acc_title(trim(&value));
                } else {
                    db.set_acc_description(trim(&value));
                }
            }
            t::ACC_DESCR_MULTILINE_VALUE => {
                end(i, "")?;
                db.set_acc_description(trim(&text(at, 0)));
            }
            t::SECTION => {
                end(i, "")?;
                db.add_section(text(at, 8));
            }
            t::TASK_TXT => {
                if tokens.types[i] != t::TASK_DATA {
                    return fail(i, "'taskData'");
                }
                let data = text(i, 0);
                i += 1;
                end(i, "")?;
                db.add_task(text(at, 0), &data)?;
            }
            t::CLICK => {
                let ids = text(at, 0);
                let mut name = false;
                let mut href = None;
                let mut args = false;
                if tokens.types[i] == t::CALLBACKNAME {
                    name = true;
                    i += 1;
                    if tokens.types[i] == t::CALLBACKARGS {
                        args = true;
                        i += 1;
                    }
                    if tokens.types[i] == t::HREF {
                        href = Some(text(i, 0));
                        i += 1;
                        end(i, "")?;
                    } else {
                        end(
                            i,
                            if args {
                                ", 'href'"
                            } else {
                                ", 'callbackargs', 'href'"
                            },
                        )?;
                    }
                } else if tokens.types[i] == t::HREF {
                    href = Some(text(i, 0));
                    i += 1;
                    if tokens.types[i] == t::CALLBACKNAME {
                        name = true;
                        i += 1;
                        if tokens.types[i] == t::CALLBACKARGS {
                            i += 1;
                            end(i, "")?;
                        } else {
                            end(i, ", 'callbackargs'")?;
                        }
                    } else {
                        end(i, ", 'callbackname'")?;
                    }
                } else {
                    return fail(i, "'callbackname', 'href'");
                }
                // Callbacks are recognized and never run; the task is only marked clickable,
                // as in Mermaid.
                if name {
                    db.set_class(&ids, "clickable");
                }
                if let Some(href) = href {
                    db.set_link(&ids, &href);
                }
            }
            _ => {
                end(i, "")?;
                db.weekday = Some(WEEKDAYS[usize::from(kind - t::WEEKDAY_MONDAY)]);
            }
        }
    }
}
