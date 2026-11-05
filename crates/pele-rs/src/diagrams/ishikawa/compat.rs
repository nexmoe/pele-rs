//! Mermaid's Ishikawa specs, as pele ports them. The cases are generated into
//! `tests/compat/ishikawa/cases.rs` by `cargo xtask compat`, and what the parser Mermaid
//! generates from its own grammar makes of each into `expected.rs` by
//! `tools/oracle/golden.mjs`: Mermaid passes these specs, so a case on which pele-rs's model
//! matches Mermaid's passes as well. Every case is either checked or listed in
//! `tests/compat/ishikawa/SKIPPED.md`.

use super::db::{IshikawaModel, build_ishikawa};
use super::parser::parse_ishikawa;
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
    /// Mermaid's own `it.todo` spec. It is compared with the reference like any other case; the
    /// flag only records that Mermaid does not test it either.
    #[allow(dead_code)]
    todo: bool,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/ishikawa/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/ishikawa/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/ishikawa/SKIPPED.md"
));

/// The model in the form `ishikawaSummary` in `tools/oracle/summaries/ishikawa.mjs` writes it.
fn summary(model: &IshikawaModel) -> String {
    let mut out: Vec<String> = Vec::new();
    if let Some(root) = model.root {
        // The path of a node, as the position of every cause under it.
        let mut stack = vec![(root, "0".to_string())];
        while let Some((node, path)) = stack.pop() {
            out.push(format!("node:{path}|{}", json(model.text(node))));
            let children = &model.nodes[node].children;
            for (i, &child) in children.iter().enumerate().rev() {
                stack.push((child, format!("{path}.{i}")));
            }
        }
    }
    out.push(format!("title:{}", q(model.title.as_deref())));
    out.join("\n")
}

fn parsed(src: &str) -> String {
    // Mermaid encodes the entities of the whole diagram before any parser sees it, and adds a
    // line break to the text it parses.
    let text = encode_entities(src) + "\n";
    match parse_ishikawa(&text).and_then(|ast| build_ishikawa(&ast, None, 64)) {
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
    dump("ishikawa", parsed);
}
