//! Mermaid's Wardley specs, as pele ports them. The cases are generated into
//! `tests/compat/wardley/cases.rs` by `cargo xtask compat`, and what Mermaid makes of each into
//! `expected.rs` by `tools/oracle/golden.mjs`: the diagram is parsed with the grammar Mermaid
//! generates from its own, so a case on which pele-rs's model matches passes as well. Every case
//! is either checked or listed in `tests/compat/wardley/SKIPPED.md`.

use super::db::{Flow, WardleyModel};
use super::parser::parse_wardley;
use crate::diagrams::common::golden::{
    assert_skips_match_the_templates, dump, error_code, json, q, skipped,
};
use crate::text::entities::encode_entities;
use crate::util::js_number;

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
    "/tests/compat/wardley/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/wardley/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/wardley/SKIPPED.md"
));

/// A list of texts, as `list` in `tools/oracle/lib.mjs` writes it.
fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json(&items.join("\u{1}"))
    }
}

/// A number, as JavaScript writes it, or `-` when there is none.
fn num(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_string(), js_number)
}

/// A flag, as JavaScript writes it, or `-` when Mermaid leaves it unset.
fn flag(value: Option<bool>) -> String {
    value.map_or_else(|| "-".to_string(), |value| value.to_string())
}

/// The name of a flow, as Mermaid writes it.
fn flow(flow: Option<Flow>) -> String {
    match flow {
        Some(Flow::Forward) => q(Some("forward")),
        Some(Flow::Backward) => q(Some("backward")),
        Some(Flow::Bidirectional) => q(Some("bidirectional")),
        None => "-".to_string(),
    }
}

/// The model in the form `wardleySummary` in `tools/oracle/summaries/wardley.mjs` writes it.
fn summary(model: &WardleyModel) -> String {
    let mut out: Vec<String> = Vec::new();
    for (_, node) in &model.nodes.items {
        out.push(format!(
            "node:{}|{}|x={}|y={}|class={}|lx={}|ly={}|pipe={}|parent={}|inertia={}|strategy={}",
            q(Some(node.id.as_str())),
            q(Some(node.label.as_str())),
            num(Some(node.x)),
            num(Some(node.y)),
            q(Some(node.class_name)),
            num(node.label_offset_x),
            num(node.label_offset_y),
            flag(node.in_pipeline),
            flag(node.is_pipeline_parent),
            flag(node.inertia),
            q(node.source_strategy.as_deref()),
        ));
    }
    for link in &model.links {
        out.push(format!(
            "link:{}|{}|dashed={}|label={}|flow={}",
            q(Some(link.source.as_str())),
            q(Some(link.target.as_str())),
            link.dashed,
            q(link.label.as_deref()),
            flow(link.flow),
        ));
    }
    for trend in &model.trends {
        out.push(format!(
            "trend:{}|x={}|y={}",
            q(Some(trend.node_id.as_str())),
            num(Some(trend.target_x)),
            num(Some(trend.target_y)),
        ));
    }
    for pipeline in &model.pipelines {
        out.push(format!(
            "pipeline:{}|members={}",
            q(Some(pipeline.node_id.as_str())),
            list(&pipeline.component_ids),
        ));
    }
    for annotation in &model.annotations {
        let at = annotation
            .coordinates
            .iter()
            .map(|(x, y)| format!("{},{}", js_number(*x), js_number(*y)))
            .collect::<Vec<_>>()
            .join(";");
        out.push(format!(
            "annotation:{}|at={}|text={}",
            js_number(annotation.number),
            at,
            q(annotation.text.as_deref()),
        ));
    }
    for note in &model.notes {
        out.push(format!(
            "note:{}|x={}|y={}",
            q(Some(note.text.as_str())),
            num(Some(note.x)),
            num(Some(note.y)),
        ));
    }
    for accelerator in &model.accelerators {
        out.push(format!(
            "accelerator:{}|x={}|y={}",
            q(Some(accelerator.name.as_str())),
            num(Some(accelerator.x)),
            num(Some(accelerator.y)),
        ));
    }
    for deaccelerator in &model.deaccelerators {
        out.push(format!(
            "deaccelerator:{}|x={}|y={}",
            q(Some(deaccelerator.name.as_str())),
            num(Some(deaccelerator.x)),
            num(Some(deaccelerator.y)),
        ));
    }
    out.push(match model.annotations_box {
        Some((x, y)) => format!("box:{},{}", js_number(x), js_number(y)),
        None => "box:-".to_string(),
    });
    out.push(format!(
        "axes:{}|{}",
        list(&model.axes.stages.clone().unwrap_or_default()),
        list(
            &model
                .axes
                .stage_boundaries
                .clone()
                .unwrap_or_default()
                .iter()
                .map(|boundary| js_number(*boundary))
                .collect::<Vec<_>>()
        ),
    ));
    out.push(match model.size {
        Some(size) => format!("size:{},{}", js_number(size.width), js_number(size.height)),
        None => "size:-".to_string(),
    });
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
    match parse_wardley(&text).and_then(|ast| super::db::build_wardley(&ast, None)) {
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
    dump("wardley", parsed);
}
