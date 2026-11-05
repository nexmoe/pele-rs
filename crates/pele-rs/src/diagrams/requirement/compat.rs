//! Mermaid's requirement specs, as pele ports them. The cases are generated into
//! `tests/compat/requirement/cases.rs` by `cargo xtask compat`, and what pele makes of each into
//! `expected.rs` by `tools/oracle/golden.mjs`: pele passes these specs, so a case on which
//! pele-rs's model matches pele's passes as well. Every case is either checked or listed in
//! `tests/compat/requirement/SKIPPED.md`.

use super::db::RequirementDb;
use super::graph::build_requirement_graph;
use super::parser::parse_requirement;
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
    "/tests/compat/requirement/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/requirement/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/requirement/SKIPPED.md"
));

fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json(&items.join("\u{1}"))
    }
}

/// The model in the form `requirementSummary` in `tools/oracle/lib.mjs` writes pele's.
fn summary(db: &RequirementDb) -> String {
    let mut out = vec![format!("dir:{}", json(db.direction))];
    for (name, r) in &db.requirements.items {
        out.push(format!(
            "req:{}|{}|{}|{}|{}|{}|s={}|c={}",
            json(name),
            json(r.kind),
            json(&r.requirement_id),
            json(&r.text),
            json(&r.risk),
            json(&r.verify_method),
            list(&r.css_styles),
            list(&r.classes)
        ));
    }
    for (name, e) in &db.elements.items {
        out.push(format!(
            "element:{}|{}|{}|s={}|c={}",
            json(name),
            json(&e.kind),
            json(&e.doc_ref),
            list(&e.css_styles),
            list(&e.classes)
        ));
    }
    for r in &db.relations {
        out.push(format!(
            "rel:{}|{}|{}",
            json(r.kind),
            json(&r.src),
            json(&r.dst)
        ));
    }
    for (id, c) in &db.classes.items {
        out.push(format!(
            "class:{}|s={}|t={}",
            json(id),
            list(&c.styles),
            list(&c.text_styles)
        ));
    }
    let (nodes, edges) = build_requirement_graph(db);
    for n in &nodes {
        out.push(format!(
            "node:{}|{}|{}|{}",
            json(n.id),
            n.requirement.is_some(),
            list(n.css_styles),
            json(&n.css_classes)
        ));
    }
    for e in &edges {
        out.push(format!(
            "edge:{}|{}|{}|{}|{}",
            json(&e.id),
            q(e.start),
            q(e.end),
            json(e.kind),
            e.contains
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
    let mut db = RequirementDb::new();
    match parse_requirement(src, &mut db) {
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
    dump("requirement", parsed);
}
