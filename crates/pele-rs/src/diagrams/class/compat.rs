//! Mermaid's class specs, as pele ports them. The cases are generated into
//! `tests/compat/class/cases.rs` by `cargo xtask compat`, and what pele makes of each into
//! `expected.rs` by `tools/oracle/golden.mjs`: pele passes these specs, so a case on which
//! pele-rs's model matches pele's passes as well. Every case is either checked or listed in
//! `tests/compat/class/SKIPPED.md`.

use super::db::{ClassDb, ClassMember};
use super::graph::{EndMarker, Pattern, Shape, build_class_graph};
use super::parser::parse_class;
use crate::diagrams::common::golden::{
    assert_skips_match_the_templates, dump, error_code, json, q, skipped,
};

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
    "/tests/compat/class/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/class/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/class/SKIPPED.md"
));

fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json(&items.join("\u{1}"))
    }
}

fn char_text(c: Option<char>) -> String {
    json(&c.map(String::from).unwrap_or_default())
}

fn member(m: &ClassMember) -> String {
    format!(
        "{}:{}|{}|{}|{}|{}|{}",
        if m.method { "method" } else { "attribute" },
        char_text(m.visibility),
        json(&m.id),
        char_text(m.classifier),
        json(&m.parameters),
        json(&m.return_type),
        json(&m.display_text())
    )
}

fn end(e: Option<u8>) -> String {
    e.map_or("none".to_string(), |t| t.to_string())
}

fn marker(m: EndMarker) -> &'static str {
    match m {
        EndMarker::Aggregation => "aggregation",
        EndMarker::Extension => "extension",
        EndMarker::Composition => "composition",
        EndMarker::Dependency => "dependency",
        EndMarker::Lollipop => "lollipop",
        EndMarker::None => "none",
    }
}

/// The model in the form `classSummary` in `tools/oracle/lib.mjs` writes pele's.
fn summary(db: &ClassDb) -> String {
    let mut out = vec![format!("dir:{}", json(&db.direction))];
    for (id, c) in &db.classes.items {
        out.push(format!(
            "class:{}|{}|{}|{}|a={}|s={}|p={}|l={}|{}|cb={}|t={}",
            json(id),
            json(&c.generic),
            json(&c.label),
            json(&c.css_classes),
            list(&c.annotations),
            list(&c.styles),
            q(c.parent.as_deref()),
            q(c.link.as_deref()),
            q(c.link_target.as_deref()),
            c.have_callback,
            q(c.tooltip.as_deref())
        ));
        for m in &c.members {
            out.push(format!("  member:{}", member(m)));
        }
        for m in &c.methods {
            out.push(format!("  method:{}", member(m)));
        }
    }
    for r in &db.relations {
        out.push(format!(
            "rel:{}|{}|{}|{}|{}|{}|{}|{}",
            json(&r.id1),
            json(&r.id2),
            json(&r.title1),
            json(&r.title2),
            q(r.title.as_deref()),
            end(r.type1),
            end(r.type2),
            u8::from(r.dotted)
        ));
    }
    for (id, n) in &db.notes.items {
        out.push(format!(
            "note:{}|{}|{}|{}|p={}",
            json(id),
            q(n.class.as_deref()),
            json(&n.text),
            n.index,
            q(n.parent.as_deref())
        ));
    }
    for (id, ns) in &db.namespaces.items {
        out.push(format!(
            "ns:{}|{}|c={}|n={}|k={}|p={}|{}",
            json(id),
            json(&ns.label),
            list(&ns.classes),
            list(&ns.notes),
            list(&ns.children),
            q(ns.parent.as_deref()),
            ns.explicit
        ));
    }
    for i in &db.interfaces {
        out.push(format!(
            "iface:{}|{}|{}",
            json(&i.id),
            json(&i.label),
            json(&i.class_id)
        ));
    }
    for (id, sc) in &db.style_classes.items {
        out.push(format!(
            "style:{}|s={}|t={}",
            json(id),
            list(&sc.styles),
            list(&sc.text_styles)
        ));
    }
    for hierarchical in [true, false] {
        let graph = build_class_graph(db, hierarchical);
        for n in &graph.nodes {
            let shape = match n.shape {
                Shape::Namespace => "rect",
                Shape::Class => "classBox",
                Shape::Note => "note",
                Shape::Interface => "interface",
            };
            out.push(format!(
                "node:{hierarchical}|{}|{}|{shape}|p={}|{}|{}",
                json(&n.id),
                json(&n.label),
                q(n.parent_id.as_deref()),
                n.parent,
                n.class.map_or("-".to_string(), |k| k.to_string())
            ));
        }
        for e in &graph.edges {
            let pattern = match e.pattern {
                Pattern::Solid => "solid",
                Pattern::Dashed => "dashed",
                Pattern::Dotted => "dotted",
            };
            out.push(format!(
                "edge:{hierarchical}|{}|{}|{}|{}|{}|{}|{}|{}|{pattern}",
                json(&e.id),
                e.source,
                e.target,
                q(e.label.as_deref()),
                marker(e.start),
                marker(e.end),
                json(&e.start_label),
                json(&e.end_label)
            ));
        }
    }
    out.push(format!(
        "title:{}|acc:{}|{}",
        q(db.title.as_deref()),
        q(db.acc_title.as_deref()),
        q(db.acc_descr.as_deref())
    ));
    out.join("\n")
}

fn parsed(src: &str) -> String {
    let mut db = ClassDb::new(64);
    match parse_class(src, &mut db) {
        Ok(()) => summary(&db),
        Err(e) => format!("error:{}", error_code(e.code)),
    }
}

fn outcome(src: &str) -> String {
    // Many class specs test members on their own, so each source is also read as one.
    format!(
        "{}\n{}\n{}",
        parsed(src),
        member(&ClassMember::new(src, true)),
        member(&ClassMember::new(src, false))
    )
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
    dump("class", parsed);
}
