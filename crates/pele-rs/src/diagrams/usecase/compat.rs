//! Mermaid's use case specs, as pele ports them. The cases are generated into
//! `tests/compat/usecase/cases.rs` by `cargo xtask compat`, and what Mermaid makes of each into
//! `expected.rs` by `tools/oracle/golden.mjs`: Mermaid has no grammar file for use case
//! diagrams, so pele's port of Mermaid's Chevrotain parser is the reference. Every case is
//! either checked or listed in `tests/compat/usecase/SKIPPED.md`.
//!
//! The helpers of the golden tests live in `diagrams/common/golden.rs`, which is not compiled
//! for this type yet, so they are here.

use std::fmt::Write;

use super::db::{ActorType, Animation, BoundaryType, LabelType, StatementKind, UsecaseModel};
use super::graph::{EdgeKind, Pattern};
use super::parser::parse_usecase;
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
    "/tests/compat/usecase/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/usecase/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/usecase/SKIPPED.md"
));

/// `JSON.stringify` of a string.
fn json(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A quoted string, or `-` for none.
fn q(s: Option<&str>) -> String {
    s.map_or("-".to_string(), json)
}

/// A list of texts, as `list` in `tools/oracle/lib.mjs` writes it.
fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json(&items.join("\u{1}"))
    }
}

/// The test names a `SKIPPED.md` lists, as `` - `name`: reason `` lines under "Skipped tests".
fn skipped(text: &'static str) -> Vec<&'static str> {
    text.lines()
        .skip_while(|line| *line != "## Skipped tests")
        .take_while(|line| !line.starts_with("## Spec files"))
        .filter_map(|line| line.strip_prefix("- `")?.split_once("`:").map(|(n, _)| n))
        .collect()
}

/// Checks that the cases a `SKIPPED.md` lists are exactly the ones there is nothing to compare
/// with.
fn assert_skips_match_the_templates(
    pairs: impl Iterator<Item = (&'static str, &'static str)>,
    skipped: &[&'static str],
) {
    let mut listed: Vec<&str> = skipped.to_vec();
    listed.sort_unstable();
    listed.dedup();
    let mut without_a_model: Vec<&str> = Vec::new();
    for (name, expected) in pairs {
        if skipped.contains(&name) {
            assert!(
                expected == "template" || expected.starts_with("error:"),
                "{name} is listed in SKIPPED.md, but the golden file has a model for it: {expected}"
            );
        }
        if expected == "template" {
            without_a_model.push(name);
        }
    }
    without_a_model.sort_unstable();
    without_a_model.dedup();
    for name in &without_a_model {
        assert!(
            listed.contains(name),
            "{name} has no model to compare, but it is not listed in SKIPPED.md"
        );
    }
}

/// The name of an error code, as pele's errors carry it.
fn error_code(code: crate::error::ErrorCode) -> &'static str {
    use crate::error::ErrorCode;
    match code {
        ErrorCode::Syntax => "syntax",
        ErrorCode::Semantic => "semantic",
        ErrorCode::Limit => "limit",
        ErrorCode::Unsupported => "unsupported-diagram",
    }
}

/// Feeds the inputs `tools/oracle/diff.mjs` wrote to `$PELE_RS_ORACLE/<name>.json` (an array
/// of strings) through `outcome`, and writes the results to `<name>.out.json` next to it.
fn dump(name: &str, outcome: impl Fn(&str) -> String) {
    use crate::preprocess::{Value, parse_json};
    let Some(dir) = std::env::var_os("PELE_RS_ORACLE") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.json"))) else {
        return;
    };
    let Some(Value::Seq(inputs)) = parse_json(&text) else {
        panic!("{name}.json is not an array");
    };
    let results: Vec<String> = inputs
        .iter()
        .map(|input| match input {
            Value::String(src) => json(&outcome(src)),
            _ => panic!("{name}.json holds a value that is not a string"),
        })
        .collect();
    std::fs::write(
        dir.join(format!("{name}.out.json")),
        format!("[{}]", results.join(",\n")),
    )
    .expect("results");
}

fn label_type(kind: LabelType) -> &'static str {
    match kind {
        LabelType::Text => "text",
        LabelType::Markdown => "markdown",
    }
}

fn actor_type(kind: ActorType) -> &'static str {
    match kind {
        ActorType::Normal => "normal",
        ActorType::Hollow => "hollow",
        ActorType::Awesome => "awesome",
        ActorType::Icon => "icon",
    }
}

fn boundary_type(kind: BoundaryType) -> &'static str {
    match kind {
        BoundaryType::Rect => "rect",
        BoundaryType::Package => "package",
    }
}

fn animation(kind: Animation) -> &'static str {
    match kind {
        Animation::Fast => "fast",
        Animation::Slow => "slow",
    }
}

fn edge_kind(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Association => "association",
        EdgeKind::Include => "include",
        EdgeKind::Extend => "extend",
        EdgeKind::Generalization => "generalization",
        EdgeKind::Note => "note",
    }
}

fn pattern(kind: Pattern) -> &'static str {
    match kind {
        Pattern::Solid => "solid",
        Pattern::Dotted => "dotted",
    }
}

fn statement_kind(kind: StatementKind) -> &'static str {
    kind.as_str()
}

/// The model in the form `usecaseSummary` in `tools/oracle/summaries/usecase.mjs` writes it.
fn summary(model: &UsecaseModel) -> String {
    let graph = super::graph::build_usecase_graph(model);
    let mut out: Vec<String> = Vec::new();
    for node in &graph.nodes {
        let rows: Vec<String> = node
            .json_rows
            .as_ref()
            .map(|rows| {
                rows.iter()
                    .map(|row| format!("{}={}", row.key, row.value))
                    .collect()
            })
            .unwrap_or_default();
        let business = node.business.map(|business| business.to_string());
        out.push(format!(
            "node:{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            q(Some(node.id.as_str())),
            q(Some(node.label.as_str())),
            q(Some(label_type(node.label_type))),
            q(Some(node.shape.as_str())),
            node.is_group,
            crate::util::js_number(node.padding),
            q(Some(node.css_classes.as_str())),
            list(&node.css_styles),
            list(&node.css_compiled_styles),
            q(node.actor_type.map(actor_type)),
            q(business.as_deref()),
            q(node.icon.as_deref()),
            q(node.stereotype.as_deref()),
            q(node.parent_id.as_deref()),
            q(node.note_target.as_deref()),
            list(&rows),
            q(node.boundary_type.map(boundary_type)),
        ));
    }
    for edge in &graph.edges {
        out.push(format!(
            "edge:{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            q(Some(edge.id.as_str())),
            q(Some(edge.start.as_str())),
            q(Some(edge.end.as_str())),
            q(Some(edge_kind(edge.relationship_type))),
            q(Some(pattern(edge.pattern))),
            q(Some(edge.arrow_type_start.as_str())),
            q(Some(edge.arrow_type_end.as_str())),
            q(edge.label.as_deref()),
            q(edge.label_type.map(label_type)),
            q(Some(edge.classes.as_str())),
            list(&edge.style),
            list(&edge.css_compiled_styles),
            edge.animate,
            q(edge.animation.map(animation)),
            edge.minlen,
            edge.explicit_id,
            edge.internal,
        ));
    }
    for statement in &model.statements {
        let nodes: Vec<String> = statement.nodes.iter().map(|node| node.id.clone()).collect();
        let edges: Vec<String> = statement.edges.iter().map(|edge| edge.id.clone()).collect();
        out.push(format!(
            "stmt:{}|{}|{}|{}|{}",
            q(Some(statement_kind(statement.kind))),
            q(statement.group.as_deref()),
            q(statement.reference.as_deref()),
            list(&nodes),
            list(&edges),
        ));
    }
    out.push(format!(
        "model:{}|{}|{}|{}",
        q(model.title.as_deref()),
        q(Some(model.acc_title.as_str())),
        q(Some(model.acc_descr.as_str())),
        q(Some(model.direction.as_str())),
    ));
    out.join("\n")
}

fn parsed(src: &str) -> String {
    // Mermaid encodes the entities of the whole diagram before any parser sees it, and adds a
    // line break to the text it parses.
    let text = encode_entities(src) + "\n";
    match parse_usecase(&text, None, 64) {
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
                "{}\n  src: {:?}\n  want:\n{}\n  got:\n{}",
                case.name, case.src, expected, got
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
    dump("usecase", parsed);
}
