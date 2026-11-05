//! Mermaid's swimlane specs. The cases are in `tests/compat/swimlane/cases.rs`, and what Mermaid
//! makes of each into `tests/compat/swimlane/expected.rs` by `tools/oracle/golden.mjs`. A swimlane
//! diagram is a flowchart, so the model here is the flowchart's, and the golden file holds what
//! Mermaid's own flowchart grammar makes of the same source. Every case is either checked or
//! listed in `tests/compat/swimlane/SKIPPED.md`.

use crate::diagrams::common::golden::{
    assert_skips_match_the_templates, dump, error_code, json, q, skipped,
};
use crate::diagrams::flowchart::db::{FlowDb, LabelType};
use crate::diagrams::flowchart::parser::parse_flowchart;

struct Case {
    #[allow(
        dead_code,
        reason = "names the spec file a case comes from, for a reader"
    )]
    file: &'static str,
    name: &'static str,
    src: &'static str,
    /// Mermaid's own `it.todo` spec. It is compared with Mermaid's like any other case; the flag
    /// only records that Mermaid does not test it either.
    #[allow(dead_code)]
    todo: bool,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/swimlane/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/swimlane/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/swimlane/SKIPPED.md"
));

fn list(items: Option<&[String]>) -> String {
    match items {
        Some(items) if !items.is_empty() => json(&items.join("\u{1}")),
        _ => "-".to_string(),
    }
}

fn label_type(t: LabelType) -> &'static str {
    match t {
        LabelType::Text => "text",
        LabelType::String => "string",
        LabelType::Markdown => "markdown",
    }
}

/// The model in the form `golden.mjs` writes Mermaid's. It is the flowchart's summary: a swimlane
/// diagram is a flowchart, and Mermaid fills the flowchart's own model.
fn summary(db: &FlowDb) -> String {
    let mut out = vec![format!("dir:{}", q(db.direction.as_deref()))];
    for n in &db.nodes {
        out.push(format!(
            "node:{}|{}|{}|{}|c={}|s={}|l={}|t={}",
            json(&n.id),
            q(n.text.as_deref()),
            label_type(n.label_type),
            q(n.kind.as_deref()),
            list(Some(&n.classes)),
            list(Some(&n.styles)),
            q(n.link.as_deref()),
            q(n.link_target.as_deref())
        ));
    }
    for e in &db.edges {
        out.push(format!(
            "edge:{}|{}|{}|{}|{}|{}|{}|{}|i={}|s={}|c={}",
            json(&e.start),
            json(&e.end),
            q(e.kind.as_deref()),
            q(e.stroke.as_deref()),
            e.length.map_or("-".to_string(), |l| l.to_string()),
            json(&e.text),
            label_type(e.label_type),
            json(&e.id),
            q(e.interpolate.as_deref()),
            list(e.style.as_deref()),
            list(Some(&e.classes))
        ));
    }
    for s in &db.subgraphs {
        out.push(format!(
            "sub:{}|{}|{}|n={}|d={}|c={}",
            json(&s.id),
            json(&s.title),
            label_type(s.label_type),
            list(Some(&s.nodes)),
            q(s.dir.as_deref()),
            list(Some(&s.classes))
        ));
    }
    let mut classes: Vec<_> = db.classes.iter().collect();
    classes.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    for (id, c) in classes {
        out.push(format!(
            "class:{}|s={}|t={}",
            json(id),
            list(Some(&c.styles)),
            list(Some(&c.text_styles))
        ));
    }
    let mut tips: Vec<_> = db.tooltips.iter().collect();
    tips.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    for (id, tip) in tips {
        out.push(format!("tip:{}|{}", json(id), json(tip)));
    }
    out.push(format!(
        "default:s={}|i={}",
        list(db.default_edge_style.as_deref()),
        q(db.default_interpolate.as_deref())
    ));
    out.push(format!(
        "acc:{}|{}",
        q(db.acc_title.as_deref()),
        q(db.acc_descr.as_deref())
    ));
    out.join("\n")
}

fn outcome(src: &str) -> String {
    // Mermaid prepares the text before any parser sees it: the front matter and the comments are
    // taken off first.
    let pre = match crate::preprocess::preprocess(src) {
        Ok(pre) => pre,
        Err(e) => return format!("error:{}", error_code(e.code)),
    };
    let mut db = FlowDb::new(500, false);
    if let Some(title) = pre.title.filter(|t| !t.is_empty()) {
        db.title = Some(title.to_string());
    }
    match parse_flowchart(&pre.text, &mut db, 64) {
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
    dump("swimlane", outcome);
}
