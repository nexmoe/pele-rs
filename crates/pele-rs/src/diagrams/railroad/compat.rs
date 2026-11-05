//! Mermaid's railroad specs, as pele ports them. The cases are generated into
//! `tests/compat/railroad/cases.rs` by `cargo xtask compat`, and what Mermaid's own parser
//! makes of each into `expected.rs` by `tools/oracle/golden.mjs`: Mermaid's own specs, so a
//! case on which pele-rs's model matches Mermaid's passes as well. Every case is either
//! checked or listed in `tests/compat/railroad/SKIPPED.md`.

use super::db::{Node, RailroadModel};
use super::parser::parse_railroad;
use crate::Limits;
use crate::diagrams::common::golden::{
    assert_skips_match_the_templates, dump, error_code, q, skipped,
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
    /// Mermaid's own `it.todo` spec. It is compared with Mermaid's like any other case; the
    /// flag only records that Mermaid does not test it either.
    #[allow(dead_code)]
    todo: bool,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/railroad/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/railroad/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/railroad/SKIPPED.md"
));

/// A node of a rule, as `railroadSummary` in `tools/oracle/summaries/railroad.mjs` writes it.
fn node(part: &Node) -> String {
    match part {
        Node::Terminal(value) => format!("t({})", q(Some(value))),
        Node::NonTerminal(name) => format!("n({})", q(Some(name))),
        Node::Special(text) => format!("s({})", q(Some(text))),
        Node::Sequence(elements) => format!(
            "seq({})",
            elements.iter().map(node).collect::<Vec<_>>().join(",")
        ),
        Node::Choice(alternatives) => format!(
            "alt({})",
            alternatives.iter().map(node).collect::<Vec<_>>().join(",")
        ),
        Node::Optional(element) => format!("opt({})", node(element)),
        Node::Repetition { element, min, max } => format!(
            "rep({},{},{})",
            min,
            max.map_or_else(|| "inf".to_string(), |max| max.to_string()),
            node(element)
        ),
    }
}

/// The model in the form `railroadSummary` in `tools/oracle/summaries/railroad.mjs` writes it.
fn summary(model: &RailroadModel) -> String {
    let mut out: Vec<String> = vec![format!(
        "title:{}|accTitle:{}|accDescr:{}",
        q(model.title.as_deref()),
        q(model.acc_title.as_deref()),
        q(model.acc_descr.as_deref())
    )];
    for rule in &model.rules {
        out.push(format!(
            "rule:{}|{}",
            q(Some(&rule.name)),
            node(&rule.definition)
        ));
    }
    out.join("\n")
}

fn parsed(src: &str) -> String {
    // Mermaid encodes the entities of the whole diagram before any parser sees it, and adds a
    // line break to the text it parses.
    let text = encode_entities(src) + "\n";
    match parse_railroad(&text, Limits::default().max_depth, None) {
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
    dump("railroad", parsed);
}
