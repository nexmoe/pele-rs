//! Mermaid's ER specs, as pele ports them. The cases are generated into
//! `tests/compat/er/cases.rs` by `cargo xtask compat`, and what pele makes of each into
//! `expected.rs` by `tools/oracle/golden.mjs`: pele passes these specs, so a case on which
//! pele-rs's model matches pele's passes as well. Every case is either checked or listed in
//! `tests/compat/er/SKIPPED.md`.

use super::db::ErDb;
use super::graph::build_er_graph;
use super::parser::parse_er;
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
    "/tests/compat/er/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/er/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/er/SKIPPED.md"
));

fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json(&items.join("\u{1}"))
    }
}

/// The model in the form `erSummary` in `tools/oracle/lib.mjs` writes pele's.
fn summary(db: &ErDb) -> String {
    let mut out = vec![format!("dir:{}", json(db.direction))];
    for (name, e) in &db.entities.items {
        out.push(format!(
            "entity:{}|{}|{}|{}|{}|s={}",
            json(name),
            json(&e.id),
            json(&e.label),
            json(&e.alias),
            json(&e.css_classes),
            list(&e.css_styles)
        ));
        for a in &e.attributes {
            out.push(format!(
                "  attr:{}|{}|{}|{}",
                json(&a.kind),
                json(&a.name),
                list(&a.keys),
                json(&a.comment)
            ));
        }
    }
    for r in &db.relationships {
        out.push(format!(
            "rel:{}|{}|{}|{}|{}|{}",
            json(&r.entity_a),
            json(&r.role_a),
            json(&r.entity_b),
            r.rel_spec.card_a.name(),
            if r.rel_spec.identifying {
                "IDENTIFYING"
            } else {
                "NON_IDENTIFYING"
            },
            r.rel_spec.card_b.name()
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
    for g in &db.subgraphs {
        out.push(format!(
            "subgraph:{}|{}|{}|c={}|s={}|{}",
            json(&g.id),
            list(&g.nodes),
            json(&g.title),
            list(&g.classes),
            list(&g.css_styles),
            q(g.dir)
        ));
    }
    let graph = build_er_graph(db);
    for n in &graph.nodes {
        out.push(format!(
            "node:{}|{}|{}|{}|p={}|{}|{}|{}",
            json(&n.id),
            json(n.label),
            json(n.alias),
            n.is_group,
            q(n.parent_id),
            list(&n.css_compiled_styles),
            json(&n.css_classes),
            q(n.dir)
        ));
    }
    for e in &graph.edges {
        out.push(format!(
            "edge:{}|{}|{}|{}|{}|{}|{}",
            json(&e.id),
            json(e.start),
            json(e.end),
            json(e.label),
            e.arrow_start.name().to_lowercase(),
            e.arrow_end.name().to_lowercase(),
            if e.dashed { "dashed" } else { "solid" }
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
    let mut db = ErDb::new();
    match parse_er(src, &mut db, 64) {
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
    dump("er", parsed);
}
