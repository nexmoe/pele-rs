//! Mermaid's state specs, as pele ports them. The cases are generated into
//! `tests/compat/state/cases.rs` by `cargo xtask compat`, and what pele makes of each into
//! `expected.rs` by `tools/oracle/golden.mjs`: pele passes these specs, so a case on which
//! pele-rs's model matches pele's passes as well. Every case is either checked or listed in
//! `tests/compat/state/SKIPPED.md`.

use super::db::StateDb;
use super::parser::parse_state;
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
    "/tests/compat/state/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/state/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/state/SKIPPED.md"
));

fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json(&items.join("\u{1}"))
    }
}

/// The model in the form `stateSummary` in `tools/oracle/lib.mjs` writes pele's.
fn summary(db: &StateDb) -> String {
    let mut out = vec![format!("dir:{}", json(db.direction()))];
    for (id, st) in &db.states.items {
        let note = st.note.as_ref().map_or("-".to_string(), |n| {
            format!("{}:{}", q(n.position.as_deref()), json(&n.text))
        });
        out.push(format!(
            "state:{}|{}|{}|doc={}|n={note}|c={}|s={}",
            json(id),
            list(&st.descriptions),
            json(&st.kind),
            u8::from(st.has_doc),
            list(&st.classes),
            list(&st.styles)
        ));
    }
    for r in &db.relations {
        out.push(format!(
            "rel:{}|{}|{}",
            json(&r.id1),
            json(&r.id2),
            json(&r.title)
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
    for (id, l) in &db.links.items {
        out.push(format!(
            "link:{}|{}|{}",
            json(id),
            json(&l.url),
            json(&l.tooltip)
        ));
    }
    for n in &db.graph.nodes {
        out.push(format!(
            "node:{}|{}|{}|{}|{}|cs={}|s={}|{}|{}|p={}|d={}|{}|{}|{}",
            json(&n.id),
            q(n.shape.as_deref()),
            q(n.label.as_deref()),
            list(&n.description),
            json(&n.css_classes),
            list(&n.css_compiled_styles),
            list(&n.css_styles),
            q(n.kind.as_deref()),
            n.is_group,
            q(n.parent_id.as_deref()),
            q(n.dir.as_deref()),
            q(n.position.as_deref()),
            q(n.note_for.as_deref()),
            n.color_index.map_or("-".to_string(), |c| c.to_string())
        ));
    }
    for e in &db.graph.edges {
        out.push(format!(
            "edge:{}|{}|{}|{}|{}",
            json(&e.id),
            json(&e.start),
            json(&e.end),
            json(&e.label),
            e.dashed
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

fn outcome(src: &str) -> String {
    let mut db = StateDb::default();
    match parse_state(src, &mut db, 64) {
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
                let got = outcome(case.src);
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
        let got = outcome(case.src);
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
    dump("state", outcome);
}
