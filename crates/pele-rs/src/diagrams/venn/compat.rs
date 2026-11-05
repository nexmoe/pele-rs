//! Mermaid's venn specs, as pele ports them. The cases are generated into
//! `tests/compat/venn/cases.rs` by `cargo xtask compat`, and what the parser Mermaid generates
//! from its own grammar makes of each into `expected.rs` by `tools/oracle/golden.mjs`: pele
//! passes these specs, so a case on which pele-rs's model matches passes as well. Every case is
//! either checked or listed in `tests/compat/venn/SKIPPED.md`.

use super::db::{VennModel, json_string};
use super::parser::parse_venn;
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
    "/tests/compat/venn/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/venn/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/venn/SKIPPED.md"
));

/// A quoted string, or `-` for none.
fn q(s: Option<&str>) -> String {
    s.map_or_else(|| "-".to_string(), json_string)
}

/// The items of a list, as `list` in `tools/oracle/lib.mjs` writes them.
fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json_string(&items.join("\u{1}"))
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

/// The test names a `SKIPPED.md` lists, as `` - `name`: reason `` lines under "Skipped tests".
fn skipped(text: &'static str) -> Vec<&'static str> {
    text.lines()
        .skip_while(|line| *line != "## Skipped tests")
        .take_while(|line| !line.starts_with("## Spec files"))
        .filter_map(|line| line.strip_prefix("- `")?.split_once("`:").map(|(n, _)| n))
        .collect()
}

/// Checks that the cases a `SKIPPED.md` lists are exactly the ones there is nothing to compare
/// with. A case is listed because the spec builds it from values in the test code, so the golden
/// file has no model for it, or because its source is not a diagram and both sides reject it. A
/// case the golden file does have a model for is compared, and listing it would hide a difference
/// that has to be fixed instead.
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

/// Feeds the inputs `tools/oracle/diff.mjs` wrote to `$PELE_RS_ORACLE/<name>.json` (an array
/// of strings) through `outcome`, and writes the results to `<name>.out.json` next to it. Does
/// nothing when the variable is not set or the run has no inputs of this kind.
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
            Value::String(src) => json_string(&outcome(src)),
            _ => panic!("{name}.json holds a value that is not a string"),
        })
        .collect();
    std::fs::write(
        dir.join(format!("{name}.out.json")),
        format!("[{}]", results.join(",\n")),
    )
    .expect("results");
}

/// The model in the form `vennSummary` in `tools/oracle/summaries/venn.mjs` writes it.
fn summary(model: &VennModel) -> String {
    let mut out: Vec<String> = Vec::new();
    out.push(format!("title:{}", q(model.title.as_deref())));
    for subset in &model.subsets {
        out.push(format!(
            "subset:{}|size={}|label={}",
            list(&subset.sets),
            crate::util::js_number(subset.size),
            q(subset.label.as_deref())
        ));
    }
    for text in &model.text_nodes {
        out.push(format!(
            "text:{}|id={}|label={}",
            list(&text.sets),
            q(Some(text.id.as_str())),
            q(text.label.as_deref())
        ));
    }
    for style in &model.styles {
        let properties: Vec<String> = style
            .styles
            .iter()
            .map(|(key, value)| format!("{key}={}", q(Some(value.as_str()))))
            .collect();
        out.push(format!(
            "style:{}|{}",
            list(&style.targets),
            properties.join(",")
        ));
    }
    out.join("\n")
}

fn parsed(src: &str) -> String {
    // Mermaid encodes the entities of the whole diagram before any parser sees it, and adds a
    // line break to the text it parses.
    let text = encode_entities(src) + "\n";
    let mut db = super::db::VennDb::default();
    match parse_venn(&text, &mut db) {
        Ok(()) => summary(&db.build(None)),
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
    dump("venn", parsed);
}
