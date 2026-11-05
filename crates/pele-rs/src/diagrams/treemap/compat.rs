//! Mermaid's treemap specs, as pele ports them. The cases are generated into
//! `tests/compat/treemap/cases.rs` by `cargo xtask compat`, and what Mermaid makes of each into
//! `tests/compat/treemap/expected.rs` by `tools/oracle/golden.mjs`: Mermaid parses a treemap
//! with its own parser, so a case on which pele-rs's model matches Mermaid's passes the spec as
//! well. Every case is either checked or listed in `tests/compat/treemap/SKIPPED.md`.

use super::db::{TreemapDb, build_db};
use super::layout::{Insets, layout_treemap};
use super::parser::parse_treemap;
use crate::diagrams::common::golden::{
    assert_skips_match_the_templates, dump, error_code, json, q, skipped,
};
use crate::text::entities::encode_entities;
use crate::util::js_number;

/// The size the diagram is drawn at: Mermaid sizes it as ten times `nodeWidth` by ten times
/// `nodeHeight`, which are 64 and 40 by default. The type reads no configuration.
const WIDTH: f64 = 640.0;
const HEIGHT: f64 = 400.0;
/// The space between two boxes, the space a section keeps for its header, and the space it keeps
/// around its children on the other three sides.
const GAP: f64 = 4.0;
const HEADER: f64 = 28.0;
const SIDE: f64 = 6.0;

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
    "/tests/compat/treemap/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/treemap/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/treemap/SKIPPED.md"
));

/// A list of styles, as `list` in `tools/oracle/lib.mjs` writes it.
fn list(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        json(&values.join("\u{1}"))
    }
}

/// The model in the form `treemapSummary` in `tools/oracle/summaries/treemap.mjs` writes it.
fn summary(db: &TreemapDb) -> String {
    let mut out: Vec<String> = Vec::new();
    out.push(format!(
        "title:{}|accTitle:{}|accDescr:{}",
        q(db.title.as_deref()),
        q(db.acc_title.as_deref()),
        q(db.acc_descr.as_deref())
    ));
    for &root in &db.tree.roots {
        walk(db, root, 0, &mut out);
    }
    for (name, styles) in &db.classes.items {
        out.push(format!("class:{}|{}", q(Some(name.as_str())), list(styles)));
    }
    for b in layout_treemap(
        &db.tree,
        WIDTH,
        HEIGHT,
        Insets {
            gap: GAP,
            header: HEADER,
            short: HEADER,
            side: SIDE,
        },
    ) {
        out.push(format!(
            "box:{}|{}|{}|{}|{}",
            b.series,
            js_number(b.x),
            js_number(b.y),
            js_number(b.w),
            js_number(b.h)
        ));
    }
    out.join("\n")
}

/// Writes an item and the items under it.
fn walk(db: &TreemapDb, index: usize, depth: usize, out: &mut Vec<String>) {
    let node = &db.tree.nodes[index];
    let value = match node.value {
        Some(value) => js_number(value),
        None => "-".to_string(),
    };
    out.push(format!(
        "node:{}|{}|{}|{}|{}|{}|{}",
        depth,
        q(Some(node.name.as_str())),
        node.kind.as_str(),
        value,
        js_number(db.tree.total(index)),
        q(node.class_selector.as_deref()),
        list(&node.styles)
    ));
    for &child in &node.children {
        walk(db, child, depth + 1, out);
    }
}

fn parsed(src: &str) -> String {
    // Mermaid encodes the entities of the whole diagram before any parser sees it, and adds a
    // line break to the text it parses.
    match parse_treemap(&(encode_entities(src) + "\n")) {
        Ok(ast) => summary(&build_db(&ast)),
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
    dump("treemap", parsed);
}
