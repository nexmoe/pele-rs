//! Mermaid's architecture specs, as pele ports them. The cases are generated into
//! `tests/compat/architecture/cases.rs` by `cargo xtask compat`, and what Mermaid makes of each
//! into `expected.rs` by `tools/oracle/golden.mjs`: the diagram is parsed with the grammar
//! Mermaid generates from its own, so a case on which pele-rs's model matches passes as well.
//! Every case is either checked or listed in `tests/compat/architecture/SKIPPED.md`.

use super::db::{ArchitectureModel, NodeKind};
use super::parser::{Side, parse_architecture};
use crate::diagrams::common::golden::{
    assert_skips_match_the_templates, dump, error_code, json, q, skipped,
};
use crate::text::entities::encode_entities;

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
    "/tests/compat/architecture/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/architecture/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/architecture/SKIPPED.md"
));

/// A list of texts, as `list` in `tools/oracle/lib.mjs` writes it.
fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json(&items.join("\u{1}"))
    }
}

/// The name of a side, as Mermaid writes it.
fn side(side: Side) -> &'static str {
    match side {
        Side::Left => "L",
        Side::Right => "R",
        Side::Top => "T",
        Side::Bottom => "B",
    }
}

/// The model in the form `architectureSummary` in `tools/oracle/summaries/architecture.mjs` writes
/// it.
fn summary(model: &ArchitectureModel) -> String {
    let mut out: Vec<String> = Vec::new();
    for group in model.groups.values() {
        out.push(format!(
            "group:{}|icon={}|title={}|in={}",
            q(Some(group.id.as_str())),
            q(group.icon.as_deref()),
            q(group.title.as_deref()),
            q(group.parent.as_deref()),
        ));
    }
    for node in model.nodes.values() {
        match node.kind {
            NodeKind::Service => out.push(format!(
                "service:{}|icon={}|iconText={}|title={}|in={}",
                q(Some(node.id.as_str())),
                q(node.icon.as_deref()),
                q(node.icon_text.as_deref()),
                q(node.title.as_deref()),
                q(node.parent.as_deref()),
            )),
            NodeKind::Junction => out.push(format!(
                "junction:{}|in={}",
                q(Some(node.id.as_str())),
                q(node.parent.as_deref()),
            )),
        }
    }
    for edge in &model.edges {
        out.push(format!(
            "edge:{}|{}|{}|{}|{}|{}|{}|{}|{}",
            q(Some(edge.lhs_id.as_str())),
            side(edge.lhs_dir),
            edge.lhs_into,
            edge.lhs_group,
            q(Some(edge.rhs_id.as_str())),
            side(edge.rhs_dir),
            edge.rhs_into,
            edge.rhs_group,
            q(edge.title.as_deref()),
        ));
    }
    for hint in &model.layout_hints {
        out.push(format!(
            "align:{}|members={}",
            hint.direction.as_str(),
            list(&hint.members),
        ));
    }
    out.push(format!(
        "title:{}|acc:{}|{}",
        q(model.title.as_deref()),
        q(model.acc_title.as_deref()),
        q(model.acc_descr.as_deref())
    ));
    out.join("\n")
}

fn parsed(src: &str) -> String {
    // Mermaid encodes the entities of the whole diagram before any parser sees it, and adds a
    // line break to the text it parses.
    let text = encode_entities(src) + "\n";
    match parse_architecture(&text).and_then(|ast| super::db::build_architecture(&ast, None)) {
        Ok(model) => summary(&model),
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
    dump("architecture", parsed);
}
