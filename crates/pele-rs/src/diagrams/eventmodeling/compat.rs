//! Mermaid's event modeling specs, as pele ports them. The cases are generated into
//! `tests/compat/eventmodeling/cases.rs` by `cargo xtask compat`, and what the reference makes
//! of each into `expected.rs` by `tools/oracle/golden.mjs`: the reference parses the source with
//! Mermaid's own Langium grammar and fills the model with pele's port of Mermaid's event
//! modeling database, so a case on which pele-rs's model matches the reference's passes as
//! well. Every case is either checked or listed in `tests/compat/eventmodeling/SKIPPED.md`.

use super::db::build_event_model;
use super::parser::parse_event_modeling;
use super::summary::{dump, error_code, summary};
use crate::text::entities::encode_entities;

struct Case {
    #[allow(
        dead_code,
        reason = "names the spec file a case comes from, for a reader"
    )]
    file: &'static str,
    name: &'static str,
    src: &'static str,
    /// Mermaid's own `it.todo` spec. It is compared with the reference like any other case;
    /// the flag only records that Mermaid does not test it either.
    #[allow(dead_code)]
    todo: bool,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/eventmodeling/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/eventmodeling/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/eventmodeling/SKIPPED.md"
));

/// The test names a `SKIPPED.md` lists, as `` - `name`: reason `` lines under "Skipped tests".
fn skipped(text: &'static str) -> Vec<&'static str> {
    text.lines()
        .skip_while(|line| *line != "## Skipped tests")
        .take_while(|line| !line.starts_with("## Spec files"))
        .filter_map(|line| {
            line.strip_prefix("- `")?
                .split_once("`:")
                .map(|(name, _)| name)
        })
        .collect()
}

/// The model of a source, in the form the golden file holds.
fn parsed(src: &str) -> String {
    // Mermaid's Diagram.fromText encodes the entities of the whole diagram before any parser
    // sees it, and adds a line break to the text it parses.
    let text = encode_entities(src) + "\n";
    match parse_event_modeling(&text).and_then(|ast| build_event_model(&ast, None, &text)) {
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
            CASES.iter().any(|case| case.name == name),
            "SKIPPED.md lists {name}, which is not a case"
        );
    }
    // A case the golden file has no model for has to be listed, or a difference would be
    // hidden.
    for (case, (_, expected)) in CASES.iter().zip(EXPECTED) {
        if *expected == "template" {
            assert!(
                skipped(SKIPPED).contains(&case.name),
                "{} has no model to compare, but it is not listed in SKIPPED.md",
                case.name
            );
        }
    }
}

/// Writes pele-rs's model of the inputs of a differential run; see `tools/oracle/diff.mjs`.
#[test]
#[ignore = "run by tools/oracle/diff.mjs"]
fn oracle_dump() {
    dump("eventmodeling", parsed);
}
