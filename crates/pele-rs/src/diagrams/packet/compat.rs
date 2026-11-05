//! Mermaid's packet specs, as pele ports them. The cases are generated into
//! `tests/compat/packet/cases.rs` by `cargo xtask compat`, and what the reference makes of each
//! into `expected.rs` by `tools/oracle/golden.mjs`: the reference is Mermaid's own Langium
//! grammar with pele's port of Mermaid's packet database, so a case on which pele-rs's model
//! matches the reference's passes the spec as well. Every case is either checked or listed in
//! `tests/compat/packet/SKIPPED.md`.

use std::fmt::Write;

use super::db::{PacketModel, build_packet};
use super::parser::parse_packet;
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
    /// Mermaid's own `it.todo` spec. It is compared with the reference's like any other case;
    /// the flag only records that Mermaid does not test it either.
    #[allow(dead_code)]
    todo: bool,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/packet/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/packet/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/packet/SKIPPED.md"
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

/// The name of an error code, as the reference's errors carry it.
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

/// The model in the form `packetSummary` in `tools/oracle/summaries/packet.mjs` writes it.
fn summary(model: &PacketModel) -> String {
    let mut out: Vec<String> = Vec::new();
    out.push(format!(
        "title:{}|acc:{}|{}",
        q(model.title.as_deref()),
        q(model.acc_title.as_deref()),
        q(model.acc_descr.as_deref())
    ));
    out.push(format!("bits:{}", js_number(model.bits_per_row)));
    for (row, fields) in model.rows.iter().enumerate() {
        for (place, field) in fields.iter().enumerate() {
            out.push(format!(
                "block:{}|{}|{}|{}|{}|{}",
                row,
                place,
                js_number(field.start),
                js_number(field.end),
                js_number(field.bits),
                q(Some(field.label.as_str()))
            ));
        }
    }
    out.join("\n")
}

/// Reads a source the way Mermaid does: the front matter, the directives and the comments are
/// taken out first, the entities of the whole diagram are encoded, and a line break is added.
fn parsed(src: &str) -> String {
    let pre = match crate::preprocess::preprocess(src) {
        Ok(pre) => pre,
        Err(e) => return format!("error:{}", error_code(e.code)),
    };
    let text = encode_entities(&pre.text) + "\n";
    match parse_packet(&text).and_then(|ast| build_packet(&ast, &pre.config, pre.title.as_deref()))
    {
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
    use crate::preprocess::{Value, parse_json};
    let Some(dir) = std::env::var_os("PELE_RS_ORACLE") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let Ok(text) = std::fs::read_to_string(dir.join("packet.json")) else {
        return;
    };
    let Some(Value::Seq(inputs)) = parse_json(&text) else {
        panic!("packet.json is not an array");
    };
    let results: Vec<String> = inputs
        .iter()
        .map(|input| match input {
            Value::String(src) => json(&parsed(src)),
            _ => panic!("packet.json holds a value that is not a string"),
        })
        .collect();
    std::fs::write(
        dir.join("packet.out.json"),
        format!("[{}]", results.join(",\n")),
    )
    .expect("results");
}
