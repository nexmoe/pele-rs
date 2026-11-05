//! Mermaid's sequence specs, as pele ports them. The cases are generated into
//! `tests/compat/sequence/cases.rs` by `cargo xtask compat`, and what pele makes of each into
//! `expected.rs` by `tools/oracle/golden.mjs`: pele passes these specs, so a case on which
//! pele-rs's model matches pele's passes as well. Every case is either checked or listed in
//! `tests/compat/sequence/SKIPPED.md`.

use std::fmt::Write;

use super::db::{MessageText, SeqDb};
use super::parser::parse_sequence;
use crate::diagrams::common::golden::{
    assert_skips_match_the_templates, dump, error_code, json, q, skipped,
};
use crate::preprocess::Value;
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
    /// Mermaid's own `it.todo` spec. It is compared with pele's like any other case; the flag
    /// only records that Mermaid does not test it either.
    #[allow(dead_code)]
    todo: bool,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/sequence/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/sequence/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/sequence/SKIPPED.md"
));

fn utf16_sorted<'a, T>(items: impl Iterator<Item = (&'a String, T)>) -> Vec<(&'a String, T)> {
    let mut items: Vec<_> = items.collect();
    items.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    items
}

/// JSON with the keys of every object sorted, as `canon` in `golden.mjs` writes it.
fn canon(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => {
            let _ = write!(out, "{b}");
        }
        Value::Number(n) if n.is_finite() => out.push_str(&js_number(*n)),
        Value::Number(_) => out.push_str("null"),
        Value::String(s) => out.push_str(&json(s)),
        Value::Seq(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canon(item, out);
            }
            out.push(']');
        }
        Value::Map(map) => object(map.iter(), out),
    }
}

fn object<'a>(entries: impl Iterator<Item = (&'a String, &'a Value)>, out: &mut String) {
    out.push('{');
    for (i, (k, v)) in utf16_sorted(entries).into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&json(k));
        out.push(':');
        canon(v, out);
    }
    out.push('}');
}

fn pairs(entries: &[(String, Value)]) -> String {
    let mut out = String::new();
    object(entries.iter().map(|(k, v)| (k, v)), &mut out);
    out
}

fn counts(map: &std::collections::HashMap<String, usize>) -> String {
    utf16_sorted(map.iter())
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn opt(n: Option<impl ToString>) -> String {
    n.map_or("-".to_string(), |n| n.to_string())
}

/// The model in the form `golden.mjs` writes pele's.
fn summary(db: &SeqDb) -> String {
    let mut out = Vec::new();
    for (id, a) in &db.actors {
        out.push(format!(
            "actor:{}|{}|{}|{}|{}|b={}|l={}|p={}|d={}",
            json(id),
            json(&a.name),
            json(&a.description),
            a.wrap,
            json(&a.kind),
            opt(a.boxed),
            pairs(&a.links),
            pairs(&a.properties),
            q(a.details.as_deref())
        ));
    }
    out.push(format!("created:{}", counts(&db.created)));
    out.push(format!("destroyed:{}", counts(&db.destroyed)));
    for b in &db.boxes {
        let keys = if b.actor_keys.is_empty() {
            "-".to_string()
        } else {
            json(&b.actor_keys.join("\u{1}"))
        };
        out.push(format!(
            "box:{}|{}|{}|{keys}",
            q(b.name.as_deref()),
            b.wrap,
            json(&b.fill)
        ));
    }
    for m in &db.messages {
        let text = match &m.message {
            MessageText::Text(t) => json(t),
            MessageText::Numbering {
                start,
                step,
                visible,
            } => {
                format!(
                    "n={},{},{visible}",
                    opt(start.map(js_number)),
                    opt(step.map(js_number))
                )
            }
        };
        out.push(format!(
            "msg:{}|{}|{text}|{}|{}|{}|{}",
            q(m.from.as_deref()),
            q(m.to.as_deref()),
            m.wrap,
            opt(m.kind),
            opt(m.placement),
            m.central
        ));
    }
    out.push(format!(
        "title:{}|acc:{}|{}",
        q(db.title.as_deref()),
        q(db.acc_title.as_deref()),
        q(db.acc_descr.as_deref())
    ));
    out.join("\n")
}

fn outcome(src: &str) -> String {
    let mut db = SeqDb::new(None, false);
    match parse_sequence(&(encode_entities(src) + "\n"), &mut db, 64) {
        Ok(()) => summary(&db),
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
                let got = outcome(case.src);
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
        let got = outcome(case.src);
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
    dump("sequence", outcome);
}
