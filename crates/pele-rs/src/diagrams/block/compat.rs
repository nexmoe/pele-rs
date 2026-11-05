//! Mermaid's block specs, as pele ports them. The cases are generated into
//! `tests/compat/block/cases.rs` by `cargo xtask compat`, and what the parser Mermaid generates
//! from its own grammar makes of each into `expected.rs` by `tools/oracle/golden.mjs`: pele
//! passes these specs, so a case on which pele-rs's model matches passes as well. Every case is
//! either checked or listed in `tests/compat/block/SKIPPED.md`.

use super::db::BlockModel;
use super::parser::parse_block;
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
    "/tests/compat/block/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/block/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/block/SKIPPED.md"
));

/// The items of a list, as `list` in `tools/oracle/lib.mjs` writes them.
fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json(&items.join("\u{1}"))
    }
}

/// The model in the form `blockSummary` in `tools/oracle/lib.mjs` writes it.
fn summary(model: &BlockModel) -> String {
    let mut out: Vec<String> = Vec::new();
    walk(model, model.root, 0, &mut out);
    for edge in &model.edges {
        out.push(format!(
            "edge:{}|{}|{}|{}|{}|{}|{}|{}",
            q(Some(edge.id.as_str())),
            q(Some(edge.start.as_deref().unwrap_or(""))),
            q(Some(edge.end.as_deref().unwrap_or(""))),
            q(Some(edge.label.as_str())),
            q(Some(edge.arrow_type_start.as_str())),
            q(Some(edge.arrow_type_end.as_str())),
            q(Some(edge.thickness.as_str())),
            q(Some(edge.pattern.as_str())),
        ));
    }
    for (name, def) in &model.classes.items {
        out.push(format!(
            "class:{}|s={}|t={}",
            q(Some(name.as_str())),
            list(&def.styles),
            list(&def.text_styles),
        ));
    }
    out.join("\n")
}

/// The blocks of the diagram, from the one that holds them all down.
fn walk(model: &BlockModel, at: usize, depth: usize, out: &mut Vec<String>) {
    let block = &model.arena[at];
    out.push(format!(
        "block:{}|{}|{}|{}|w={}|c={}|cls={}|s={}",
        depth,
        q(Some(block.id.as_str())),
        q(Some(block.kind.as_str())),
        q(Some(block.label.as_deref().unwrap_or(""))),
        block
            .width_in_columns
            .map_or("-".to_string(), |n| n.to_string()),
        block.columns.map_or("-".to_string(), |n| n.to_string()),
        list(&block.classes),
        list(&block.styles),
    ));
    for child in &block.children {
        walk(model, *child, depth + 1, out);
    }
}

fn parsed(src: &str) -> String {
    // Mermaid encodes the entities of the whole diagram before any parser sees it, and adds a
    // line break to the text it parses.
    match parse_block(&(encode_entities(src) + "\n"), 64) {
        Ok(parsed) => {
            let model = super::db::build_block(&parsed, None).expect("the model is built");
            summary(&model)
        }
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
    dump("block", parsed);
}

/// The blocks of a diagram, in the order they are read.
#[test]
fn keeps_the_order_of_the_blocks() {
    let parsed = parse_block("block\nA\nB\nC\n", 64).unwrap();
    let model = super::db::build_block(&parsed, None).unwrap();
    let ids: Vec<&str> = model.arena[model.root]
        .children
        .iter()
        .map(|at| model.arena[*at].id.as_str())
        .collect();
    assert_eq!(ids, ["A", "B", "C"]);
}

/// Mermaid keeps only the last link of a chain of three or more nodes.
#[test]
fn keeps_only_the_last_link_of_a_chain() {
    let parsed = parse_block("block\nA --> B --> C\n", 64).unwrap();
    let model = super::db::build_block(&parsed, None).unwrap();
    let ids: Vec<&str> = model.arena[model.root]
        .children
        .iter()
        .map(|at| model.arena[*at].id.as_str())
        .collect();
    assert_eq!(ids, ["id-1", "C"]);
    assert_eq!(model.edges.len(), 1);
    assert_eq!(model.edges[0].start, None);
    assert_eq!(model.edges[0].end.as_deref(), Some("C"));
}
