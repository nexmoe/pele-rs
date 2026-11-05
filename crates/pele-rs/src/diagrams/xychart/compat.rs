//! Mermaid's xychart specs, as pele ports them. The cases are generated into
//! `tests/compat/xychart/cases.rs` by `cargo xtask compat`, and what pele makes of each into
//! `expected.rs` by `tools/oracle/golden.mjs`: pele passes these specs, so a case on which
//! pele-rs's model matches pele's passes as well. Every case is either checked or listed in
//! `tests/compat/xychart/SKIPPED.md`.

use super::db::{XAxis, XyChartDb};
use super::parser::parse_xychart;
use crate::diagrams::common::golden::{
    assert_skips_match_the_templates, dump, error_code, json, q, skipped,
};
use crate::text::entities::encode_entities;
use crate::util::js_number;

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
    "/tests/compat/xychart/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/xychart/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/xychart/SKIPPED.md"
));

/// A list of texts, as `list` in `tools/oracle/lib.mjs` writes it.
fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json(&items.join("\u{1}"))
    }
}

/// The model in the form `xychartSummary` in `tools/oracle/lib.mjs` writes pele's.
fn summary(db: &XyChartDb) -> String {
    let mut out: Vec<String> = Vec::new();
    match &db.x_axis {
        XAxis::Band { title, categories } => out.push(format!(
            "xaxis:band|{}|{}",
            q(Some(title.as_str())),
            list(categories)
        )),
        XAxis::Linear { title, min, max } => out.push(format!(
            "xaxis:linear|{}|{}|{}",
            q(Some(title.as_str())),
            js_number(*min),
            js_number(*max)
        )),
    }
    out.push(format!(
        "yaxis:linear|{}|{}|{}",
        q(Some(db.y_axis.title.as_str())),
        js_number(db.y_axis.min),
        js_number(db.y_axis.max)
    ));
    out.push(format!(
        "hasx:{}|hasy:{}",
        db.has_set_x_axis, db.has_set_y_axis
    ));
    out.push(format!(
        "orientation:{}",
        q(match db.orientation {
            Some(super::db::Orientation::Vertical) => Some("vertical"),
            Some(super::db::Orientation::Horizontal) => Some("horizontal"),
            None => None,
        })
    ));
    for plot in &db.plots {
        let data = plot
            .data
            .entries
            .iter()
            .map(|(key, value)| {
                format!(
                    "{}={}",
                    json(key),
                    value.map(js_number).unwrap_or_else(|| "-".to_string())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let labels = match &plot.point_labels {
            Some(labels) => list(labels),
            None => "-".to_string(),
        };
        out.push(format!(
            "plot:{}|{}|{}|{}",
            plot.kind.name(),
            q(Some(plot.title.as_str())),
            data,
            labels
        ));
    }
    out.push(format!(
        "title:{}|acc:{}|{}",
        q(db.title.as_deref()),
        q(db.acc_title.as_deref()),
        q(db.acc_descr.as_deref())
    ));
    out.join("\n")
}

fn parsed(src: &str) -> String {
    let mut db = XyChartDb::default();
    // Mermaid encodes the entities of the whole diagram before any parser sees it, and adds a
    // line break to the text it parses.
    match parse_xychart(&(encode_entities(src) + "\n"), &mut db) {
        Ok(()) => summary(&db),
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
    dump("xychart", parsed);
}
