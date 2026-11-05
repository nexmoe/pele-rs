//! Mermaid's C4 specs, as pele ports them. The cases are generated into
//! `tests/compat/c4/cases.rs` by `cargo xtask compat`, and what the parser Mermaid generates
//! from its own grammar makes of each into `expected.rs` by `tools/oracle/golden.mjs`: pele
//! passes these specs, so a case on which pele-rs's model matches passes as well. Every case is
//! either checked or listed in `tests/compat/c4/SKIPPED.md`.

use super::db::{Element, Model, build};
use super::parser::parse_c4;
use crate::diagrams::common::golden::{
    assert_skips_match_the_templates, dump, error_code, json, q, skipped,
};
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
    "/tests/compat/c4/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/c4/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/c4/SKIPPED.md"
));

/// The items of a list, as `list` in `tools/oracle/lib.mjs` writes them.
fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json(&items.join("\u{1}"))
    }
}

/// The alias of the boundary an element sits in.
fn parent_alias(model: &Model, parent: Option<usize>) -> Option<&str> {
    parent
        .and_then(|at| model.boundaries.get(at))
        .map(|boundary| boundary.element.alias.as_str())
}

/// The key of a named argument, when there is one.
fn key(attr: Option<&super::db::Named>) -> Option<&str> {
    attr.map(|named| named.key.as_str())
}

/// The fields a shape and a boundary have in common.
fn element_fields(model: &Model, element: &Element) -> Vec<String> {
    vec![
        q(Some(element.alias.as_str())),
        q(parent_alias(model, element.parent)),
        q(Some(element.label.as_str())),
        q(key(element.label_attr.as_ref())),
        q(key(element.alias_attr.as_ref())),
        q(element.type_.as_deref()),
        q(element.descr.as_deref()),
        q(element.techn.as_deref()),
        q(element.sprite.as_deref()),
        q(element.tags.as_deref()),
        q(element.link.as_deref()),
        q(element.bg_color.as_deref()),
        q(element.font_color.as_deref()),
        q(element.border_color.as_deref()),
        q(element.shadowing.as_deref()),
        q(element.shape.as_deref()),
        q(element.legend_text.as_deref()),
        q(element.legend_sprite.as_deref()),
        list(
            &element
                .extra
                .items
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>(),
        ),
    ]
}

/// A number a relation asks for, as JavaScript writes it.
fn number(value: Option<f64>) -> String {
    match value {
        None => "-".to_string(),
        Some(value) if value.is_nan() => "NaN".to_string(),
        Some(value) => value.to_string(),
    }
}

/// The model in the form `c4Summary` in `tools/oracle/summaries/c4.mjs` writes it.
fn summary(model: &Model) -> String {
    let mut out: Vec<String> = Vec::new();
    out.push(format!("c4Type:{}", q(model.c4_type.as_deref())));
    out.push(format!("title:{}", q(model.title.as_deref())));
    out.push(format!("accTitle:{}", q(model.acc_title.as_deref())));
    out.push(format!("accDescr:{}", q(model.acc_descr.as_deref())));
    out.push(format!("shapeInRow:{}", model.shape_in_row));
    out.push(format!("boundaryInRow:{}", model.boundary_in_row));
    for shape in &model.shapes {
        let mut fields = vec![q(Some(shape.kind.as_str()))];
        fields.extend(element_fields(model, &shape.element));
        out.push(format!("shape:{}", fields.join("|")));
    }
    for boundary in &model.boundaries {
        let mut fields = vec![q(boundary.node_type.as_deref())];
        fields.extend(element_fields(model, &boundary.element));
        out.push(format!("boundary:{}", fields.join("|")));
    }
    for rel in &model.rels {
        out.push(format!(
            "rel:{}",
            [
                q(Some(rel.kind.as_str())),
                q(Some(rel.from.as_str())),
                q(Some(rel.to.as_str())),
                q(Some(rel.label.as_str())),
                q(key(rel.label_attr.as_ref())),
                q(key(rel.from_attr.as_ref())),
                q(key(rel.to_attr.as_ref())),
                q(rel.techn.as_deref()),
                q(rel.descr.as_deref()),
                q(rel.sprite.as_deref()),
                q(rel.tags.as_deref()),
                q(rel.link.as_deref()),
                q(rel.text_color.as_deref()),
                q(rel.line_color.as_deref()),
                number(rel.offset_x),
                number(rel.offset_y),
                list(
                    &rel.extra
                        .items
                        .iter()
                        .map(|(name, value)| format!("{name}={value}"))
                        .collect::<Vec<_>>(),
                ),
            ]
            .join("|")
        ));
    }
    out.join("\n")
}

fn parsed(src: &str) -> String {
    // Mermaid encodes the entities of the whole diagram before any parser sees it, and adds a
    // line break to the text it parses.
    match parse_c4(&(encode_entities(src) + "\n")) {
        Ok(actions) => summary(&build(actions)),
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
    dump("c4", parsed);
}
