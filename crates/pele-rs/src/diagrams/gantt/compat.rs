//! Mermaid's gantt specs, as pele ports them. The cases are generated into
//! `tests/compat/gantt/cases.rs` by `cargo xtask compat`, and what pele makes of each into
//! `expected.rs` by `tools/oracle/golden.mjs`: pele passes these specs, so a case on which
//! pele-rs's model matches pele's passes as well. Every case is either checked or listed in
//! `tests/compat/gantt/SKIPPED.md`.

use super::db::GanttDb;
use super::parser::parse_gantt;
use crate::diagrams::common::golden::{
    assert_skips_match_the_templates, dump, error_code, json, q, skipped,
};

struct Case {
    #[allow(
        dead_code,
        reason = "names the spec file a case comes from, for a reader"
    )]
    file: &'static str,
    name: &'static str,
    src: &'static str,
    /// Mermaid's own `it.todo` spec. It is compared with pele's like any other case; the flag
    /// only records that Mermaid does not test it either.
    #[allow(dead_code)]
    todo: bool,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/gantt/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/gantt/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/gantt/SKIPPED.md"
));

fn time(date: Option<super::dates::Date>) -> String {
    date.filter(|d| d.valid())
        .map_or("-".to_string(), |d| crate::util::js_number(d.time()))
}

/// The model in the form `ganttSummary` in `tools/oracle/lib.mjs` writes pele's, with the
/// tasks scheduled at time 0.
fn summary(db: &mut GanttDb) -> Result<String, crate::error::Error> {
    db.schedule(0.0)?;
    let mut out = vec![
        format!(
            "title:{}|{}|{}",
            q(db.title.as_deref()),
            q(db.acc_title.as_deref()),
            q(db.acc_descr.as_deref())
        ),
        format!(
            "format:{}|{}|{}|{}|{}|{}",
            json(&db.date_format),
            json(&db.axis_format),
            q(db.tick_interval.as_deref()),
            json(&db.today_marker),
            db.weekday.unwrap_or("-"),
            db.weekend
        ),
        format!(
            "days:{}|{}|{}|{}",
            json(&db.includes.join(",")),
            json(&db.excludes.join(",")),
            db.inclusive_end_dates,
            db.top_axis
        ),
        format!("sections:{}", json(&db.sections.join("|"))),
    ];
    for k in &db.tasks {
        let flags: Vec<&str> = [
            ("active", k.active),
            ("done", k.done),
            ("crit", k.crit),
            ("milestone", k.milestone),
            ("vert", k.vert),
        ]
        .into_iter()
        .filter_map(|(name, on)| on.then_some(name))
        .collect();
        out.push(format!(
            "task:{}|{}|{}|{}|{}|{}|{}|{}|{}",
            json(&k.id),
            json(&k.task),
            json(&k.section),
            time(k.start_time),
            time(k.end_time),
            time(k.render_end_time),
            flags.join(","),
            k.order,
            json(&k.classes.join(","))
        ));
    }
    // Links in the order of their tasks, which is the order the specs set them in.
    for k in &db.tasks {
        if let Some(url) = db.links.get(&k.id) {
            out.push(format!("link:{}|{}", json(&k.id), json(url)));
        }
    }
    Ok(out.join("\n"))
}

fn parsed(src: &str) -> String {
    let mut db = GanttDb::new();
    match parse_gantt(src, &mut db).and_then(|()| summary(&mut db)) {
        Ok(text) => text,
        Err(e) => format!("error:{}", error_code(e.code)),
    }
}
#[test]
fn passes_mermaid_specs() {
    assert_eq!(
        CASES.len(),
        EXPECTED.len(),
        "expected.rs is out of date; run tools/oracle/golden.mjs"
    );
    let skipped = skipped(SKIPPED);
    let mut failures = Vec::new();
    for (case, (name, expected)) in CASES.iter().zip(EXPECTED) {
        assert_eq!(case.name, *name, "expected.rs is out of date");
        if skipped.contains(&case.name) {
            // The golden file says both sides reject this source, and that has to stay true:
            // the case is listed because there is nothing to compare, not because there is
            // something that happens to be different.
            if expected.starts_with("error:") {
                let got = parsed(case.src);
                assert!(
                    got.starts_with("error:"),
                    "{} is listed in SKIPPED.md as rejected, but pele-rs draws it",
                    case.name
                );
            }
            continue;
        }
        assert_ne!(
            *expected, "template",
            "{} needs a value from the test code; list it in SKIPPED.md",
            case.name
        );
        let got = parsed(case.src);
        if got != *expected {
            failures.push(format!(
                "{}\n  src: {:?}\n  want: {expected}\n  got:  {got}",
                case.name, case.src
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        CASES.len(),
        failures.join("\n")
    );
}

#[test]
fn skips_only_known_cases() {
    for name in skipped(SKIPPED) {
        assert!(
            CASES.iter().any(|c| c.name == name),
            "SKIPPED.md lists {name}, which is not a case"
        );
    }
    assert_skips_match_the_templates(
        CASES
            .iter()
            .zip(EXPECTED)
            .map(|(c, (_, expected))| (c.name, *expected)),
        &skipped(SKIPPED),
    );
}

/// Writes pele-rs's model of the inputs of a differential run; see `tools/oracle/diff.mjs`.
#[test]
#[ignore = "run by tools/oracle/diff.mjs"]
fn oracle_dump() {
    dump("gantt", parsed);
}
