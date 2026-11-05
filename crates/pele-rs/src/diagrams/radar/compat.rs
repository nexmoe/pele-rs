//! Mermaid's radar specs, as pele ports them. The cases are generated into
//! `tests/compat/radar/cases.rs` by `cargo xtask compat`, and what Mermaid makes of each into
//! `expected.rs` by `tools/oracle/golden.mjs`: the reference parses the text with Mermaid's own
//! Langium parser and fills the model with pele's port of Mermaid's radar database. Every case
//! is either checked or listed in `tests/compat/radar/SKIPPED.md`.

use super::db::{RadarModel, build_radar};
use super::parser::parse_radar;
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
    /// Mermaid's own `it.todo` spec. It is compared with the reference like any other case; the
    /// flag only records that Mermaid does not test it either.
    #[allow(dead_code)]
    todo: bool,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/radar/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/radar/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/radar/SKIPPED.md"
));

/// A list of texts, as `list` in `tools/oracle/lib.mjs` writes it.
fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json(&items.join("\u{1}"))
    }
}

/// The model in the form `radarSummary` in `tools/oracle/summaries/radar.mjs` writes it.
fn summary(model: &RadarModel) -> String {
    let mut out: Vec<String> = Vec::new();
    let axes: Vec<String> = model
        .axes
        .iter()
        .map(|axis| format!("{}|{}", q(Some(&axis.name)), q(Some(&axis.label))))
        .collect();
    out.push(format!("axes:{}", list(&axes)));
    let curves: Vec<String> = model
        .curves
        .iter()
        .map(|curve| {
            let entries: Vec<String> = curve
                .entries
                .iter()
                .map(|value| js_number(*value))
                .collect();
            format!(
                "{}|{}|{}",
                q(Some(&curve.name)),
                q(Some(&curve.label)),
                list(&entries)
            )
        })
        .collect();
    out.push(format!("curves:{}", list(&curves)));
    out.push(format!(
        "options:showLegend={}|ticks={}|max={}|min={}|graticule={}",
        model.options.show_legend,
        js_number(model.options.ticks),
        model
            .options
            .max
            .map_or_else(|| "null".to_string(), js_number),
        js_number(model.options.min),
        match model.options.graticule {
            super::db::Graticule::Circle => "circle",
            super::db::Graticule::Polygon => "polygon",
        }
    ));
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
    match parse_radar(&text).and_then(|ast| build_radar(&ast, None)) {
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
    dump("radar", parsed);
}
