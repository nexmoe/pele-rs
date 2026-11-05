//! Mermaid's agentflow specs, as pele ports them. The cases are generated into
//! `tests/compat/agentflow/cases.rs` by `cargo xtask compat`, and what the parser Mermaid
//! generates from its own grammar makes of each into `expected.rs` by
//! `tools/oracle/golden.mjs`: pele passes these specs, so a case on which pele-rs's model
//! matches passes as well. Every case is either checked or listed in
//! `tests/compat/agentflow/SKIPPED.md`.

use super::db::AgentflowDb;
use super::parser::parse_agentflow;
use super::shapes::resolve_shape_alias;
use crate::diagrams::common::golden::{dump, error_code, json, q, skipped};

struct Case {
    #[allow(
        dead_code,
        reason = "names the spec file a case comes from, for a reader"
    )]
    file: &'static str,
    name: &'static str,
    src: &'static str,
    /// Mermaid's own `it.todo` spec. It is compared with the others like any other case; the
    /// flag only records that Mermaid does not test it either.
    #[allow(dead_code)]
    todo: bool,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/agentflow/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/agentflow/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/agentflow/SKIPPED.md"
));

/// The items of a list, as `list` in `tools/oracle/lib.mjs` writes them.
fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json(&items.join("\u{1}"))
    }
}

/// The model in the form `agentflowSummary` in `tools/oracle/summaries/agentflow.mjs` writes it.
fn summary(db: &AgentflowDb) -> String {
    let mut out: Vec<String> = Vec::new();
    for node in &db.nodes {
        if db.subgraph(&node.id).is_some() {
            continue;
        }
        if node.is_connector {
            continue;
        }
        let shape = node.shape.as_deref().map(resolve_shape_alias);
        out.push(format!(
            "vertex:{}|{}|{}|{}",
            q(Some(node.id.as_str())),
            q(node.text.as_deref()),
            q(shape.as_deref()),
            q(Some(db.vertex_kind(node))),
        ));
    }
    for node in &db.nodes {
        if db.subgraph(&node.id).is_some() || !node.is_connector {
            continue;
        }
        let title = node.text.as_deref().filter(|t| *t != node.id);
        out.push(format!(
            "connector:{}|{}",
            q(Some(node.id.as_str())),
            q(title),
        ));
    }
    for edge in &db.edges {
        out.push(format!(
            "edge:{}|{}|{}|{}|{}|{}|{}|{}",
            q(Some(edge.start.as_str())),
            q(Some(edge.end.as_str())),
            q(edge.id.as_deref()),
            q(if edge.text.is_empty() {
                None
            } else {
                Some(edge.text.as_str())
            }),
            q(edge.kind.map(|k| k.name())),
            q(edge.stroke.map(|s| s.name())),
            q(edge.semantic.map(|s| s.name())),
            edge.length.map_or("-".to_string(), |l| l.to_string()),
        ));
    }
    for sg in &db.subgraphs {
        out.push(format!(
            "subgraph:{}|{}|{}|{}|{}",
            q(Some(sg.id.as_str())),
            list(&sg.nodes),
            q(Some(sg.kind.as_str())),
            q(sg.title.as_deref()),
            q(sg.dir.as_deref()),
        ));
    }
    if let Some(dir) = &db.direction {
        out.push(format!("direction:{}", q(Some(dir.as_str()))));
    }
    out.join("\n")
}

/// The model of a source, or the error the parser gives.
fn parsed(src: &str) -> String {
    // Mermaid encodes the entities of the whole diagram before any parser sees it, and adds a
    // line break to the text it parses.
    let mut db = AgentflowDb::new(usize::MAX, false);
    match parse_agentflow(
        &(crate::text::entities::encode_entities(src) + "\n"),
        &mut db,
    ) {
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
    let skipped = skipped(SKIPPED);
    for name in &skipped {
        assert!(
            CASES.iter().any(|c| c.name == *name),
            "SKIPPED.md lists {name}, which is not a case"
        );
    }
    let mut without_a_model = Vec::new();
    for (case, (_, expected)) in CASES.iter().zip(EXPECTED) {
        if skipped.contains(&case.name) {
            assert!(
                *expected == "template" || expected.starts_with("error:"),
                "{} is listed in SKIPPED.md, but the golden file has a model for it: {expected}",
                case.name
            );
        }
        if *expected == "template" {
            without_a_model.push(case.name);
        }
    }
    without_a_model.sort_unstable();
    for name in &without_a_model {
        assert!(
            skipped.contains(name),
            "{name} has no model to compare, but it is not listed in SKIPPED.md"
        );
    }
}

/// Writes pele-rs's model of the inputs of a differential run; see `tools/oracle/diff.mjs`.
#[test]
#[ignore = "run by tools/oracle/diff.mjs"]
fn oracle_dump() {
    dump("agentflow", parsed);
}
