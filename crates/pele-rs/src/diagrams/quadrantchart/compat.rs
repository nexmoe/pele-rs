//! Mermaid's quadrantChart specs, as pele ports them. The cases are generated into
//! `tests/compat/quadrantchart/cases.rs` by `cargo xtask compat`, and what pele makes of each
//! into `expected.rs` by `tools/oracle/golden.mjs`: pele passes these specs, so a case on which
//! pele-rs's model matches pele's passes as well. Every case is either checked or listed in
//! `tests/compat/quadrantchart/SKIPPED.md`.

use super::db::{PointStyle, QuadrantDb};
use super::parser::parse_quadrant;
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
    "/tests/compat/quadrantchart/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/quadrantchart/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/quadrantchart/SKIPPED.md"
));

/// The style of a point or of a class, in the order the keys are read.
fn style_summary(style: &PointStyle) -> String {
    let mut out: Vec<String> = Vec::new();
    if let Some(radius) = style.radius {
        out.push(format!("radius:{}", js_number(radius)));
    }
    if let Some(color) = &style.color {
        out.push(format!("color:{color}"));
    }
    if let Some(stroke_color) = &style.stroke_color {
        out.push(format!("stroke-color:{stroke_color}"));
    }
    if let Some(stroke_width) = &style.stroke_width {
        out.push(format!("stroke-width:{stroke_width}"));
    }
    out.join(",")
}

/// The model in the form `quadrantSummary` in `tools/oracle/lib.mjs` writes pele's.
fn summary(db: &QuadrantDb) -> String {
    let mut out: Vec<String> = Vec::new();
    let quadrants: Vec<String> = db
        .quadrants
        .iter()
        .map(|quadrant| match quadrant {
            Some(quadrant) => quadrant.text.clone(),
            None => "-".to_string(),
        })
        .collect();
    out.push(format!("quadrants:{}", quadrants.join("|")));
    out.push(format!(
        "axes:{}|{}|{}|{}",
        q(db.x_axis_left.as_ref().map(|t| t.text.as_str())),
        q(db.x_axis_right.as_ref().map(|t| t.text.as_str())),
        q(db.y_axis_bottom.as_ref().map(|t| t.text.as_str())),
        q(db.y_axis_top.as_ref().map(|t| t.text.as_str())),
    ));
    for point in &db.points {
        out.push(format!(
            "point:{}|{}|{}|{}|{}",
            q(Some(point.text.text.as_str())),
            js_number(point.x),
            js_number(point.y),
            q(Some(point.class_name.as_str())),
            style_summary(&point.style),
        ));
    }
    for (name, style) in &db.classes.items {
        out.push(format!(
            "class:{}|{}",
            q(Some(name.as_str())),
            style_summary(style)
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
    let mut db = QuadrantDb::default();
    // Mermaid encodes the entities of the whole diagram before any parser sees it, and adds a
    // line break to the text it parses.
    match parse_quadrant(&(encode_entities(src) + "\n"), &mut db) {
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
    dump("quadrantchart", parsed);
}

/// The class names of a chart, in the order they were named.
#[test]
fn keeps_the_order_of_the_classes() {
    let mut db = QuadrantDb::default();
    parse_quadrant(
        "quadrantChart\nclassDef a color: #ff0000\nclassDef b color: #00ff00\n",
        &mut db,
    )
    .unwrap();
    let names: Vec<&str> = db
        .classes
        .items
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(names, ["a", "b"]);
    let _ = json("");
}
