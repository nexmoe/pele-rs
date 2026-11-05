//! Helpers of the compat tests that compare a model with pele's, as `tools/oracle/golden.mjs`
//! summarizes it.

use std::fmt::Write;

/// `JSON.stringify` of a string.
pub(crate) fn json(s: &str) -> String {
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
pub(crate) fn q(s: Option<&str>) -> String {
    s.map_or("-".to_string(), json)
}

/// The test names a `SKIPPED.md` lists, as `` - `name`: reason `` lines under "Skipped tests".
pub(crate) fn skipped(text: &'static str) -> Vec<&'static str> {
    text.lines()
        .skip_while(|line| *line != "## Skipped tests")
        .take_while(|line| !line.starts_with("## Spec files"))
        .filter_map(|line| line.strip_prefix("- `")?.split_once("`:").map(|(n, _)| n))
        .collect()
}

/// Checks that the cases a `SKIPPED.md` lists are exactly the ones there is nothing to compare
/// with. A case is listed because the spec builds it from values in the test code, so the golden
/// file has no model for it, or because its source is not a diagram and both sides reject it. A
/// case the golden file does have a model for is compared, and listing it would hide a
/// difference that has to be fixed instead.
#[allow(
    dead_code,
    reason = "used by every type but pie, whose compat test reads no golden file"
)]
pub(crate) fn assert_skips_match_the_templates(
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
#[allow(
    dead_code,
    reason = "used by every type but pie, whose compat test reads no errors"
)]
pub(crate) fn error_code(code: crate::error::ErrorCode) -> &'static str {
    use crate::error::ErrorCode;
    match code {
        ErrorCode::Syntax => "syntax",
        ErrorCode::Semantic => "semantic",
        ErrorCode::Limit => "limit",
        ErrorCode::Unsupported => "unsupported-diagram",
    }
}

/// Feeds the inputs `tools/oracle/diff.mjs` wrote to `$PELE_RS_ORACLE/<name>.json` (an array
/// of strings) through `outcome`, and writes the results to `<name>.out.json` next to it. Does
/// nothing when the variable is not set or the run has no inputs of this kind.
pub(crate) fn dump(name: &str, outcome: impl Fn(&str) -> String) {
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
