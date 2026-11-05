//! Mermaid's pie specs, as pele ports them. The cases are generated into
//! `tests/compat/pie/cases.rs`; the expectations are written here, by test name. Every case is
//! either checked or listed in `tests/compat/pie/SKIPPED.md`.

use super::model::{PieModel, build_pie};
use super::parser::{PieAst, parse_pie};
use crate::diagrams::common::golden::{dump, q, skipped};
use crate::error::Error;
use crate::util::js_number;

struct Case {
    #[allow(
        dead_code,
        reason = "names the spec file a case comes from, for a reader"
    )]
    file: &'static str,
    name: &'static str,
    src: &'static str,
    todo: bool,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/pie/cases.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/pie/SKIPPED.md"
));

fn model(src: &str) -> Result<PieModel, Error> {
    build_pie(parse_pie(src)?, None)
}

fn sections(m: &PieModel) -> Vec<(&str, f64)> {
    m.sections.iter().map(|(l, v)| (l.as_str(), *v)).collect()
}

fn ast(src: &str) -> PieAst {
    parse_pie(src).unwrap_or_else(|e| panic!("{src:?}: {}", e.message))
}

fn ok(src: &str) -> PieModel {
    model(src).unwrap_or_else(|e| panic!("{src:?}: {}", e.message))
}

fn github_gitlab(a: &PieAst) {
    assert_eq!(
        a.sections,
        [("GitHub".to_string(), 100.0), ("GitLab".to_string(), 50.0)]
    );
}

/// Checks a case the way its Mermaid test does. Returns false for a name it does not know.
fn check(name: &str, src: &str) -> bool {
    let base = name.rsplit_once(" [").map_or(name, |(base, _)| base);
    let base = base.strip_prefix("pie > ").unwrap_or(base);
    match base {
        "parse > should handle very simple pie" => {
            assert_eq!(sections(&ok(src)), [("ash", 100.0)]);
        }
        "parse > should handle simple pie" | "parse > should handle simple pie with comments" => {
            assert_eq!(sections(&ok(src)), [("ash", 60.0), ("bat", 40.0)]);
        }
        "parse > should handle simple pie with showData" => {
            let m = ok(src);
            assert!(m.show_data);
            assert_eq!(sections(&m), [("ash", 60.0), ("bat", 40.0)]);
        }
        "parse > should handle simple pie with a title" => {
            let m = ok(src);
            assert_eq!(m.title.as_deref(), Some("a 60/40 pie"));
            assert_eq!(sections(&m), [("ash", 60.0), ("bat", 40.0)]);
        }
        "parse > should handle simple pie with an acc title (accTitle)" => {
            let m = ok(src);
            assert_eq!(m.title.as_deref(), Some("a neat chart"));
            assert_eq!(m.acc.title.as_deref(), Some("a neat acc title"));
            assert_eq!(sections(&m), [("ash", 60.0), ("bat", 40.0)]);
        }
        "parse > should handle simple pie with an acc description (accDescr)" => {
            let m = ok(src);
            assert_eq!(m.title.as_deref(), Some("a neat chart"));
            assert_eq!(m.acc.descr.as_deref(), Some("a neat description"));
            assert_eq!(sections(&m), [("ash", 60.0), ("bat", 40.0)]);
        }
        "parse > should handle simple pie with a multiline acc description (accDescr)" => {
            let m = ok(src);
            assert_eq!(m.title.as_deref(), Some("a neat chart"));
            assert_eq!(
                m.acc.descr.as_deref(),
                Some("a neat description\non multiple lines")
            );
            assert_eq!(sections(&m), [("ash", 60.0), ("bat", 40.0)]);
        }
        "parse > should handle simple pie with positive decimal" => {
            assert_eq!(sections(&ok(src)), [("ash", 60.67), ("bat", 40.0)]);
        }
        "parse > should handle simple pie with negative decimal" => {
            assert!(model(src).is_err());
        }
        "parse > should handle simple pie with zero slice value" => {
            assert_eq!(sections(&ok(src)), [("dogs", 0.0), ("rats", 40.12)]);
        }
        "parse > should handle simple pie with negative slice value" => {
            assert_eq!(
                model(src).unwrap_err().message,
                "\"dogs\" has invalid value: -60.67. Negative values are not allowed in pie \
                 charts. All slice values must be >= 0."
            );
        }
        "parse > should handle unsafe properties" => {
            let m = ok(src);
            let labels: Vec<&str> = m.sections.iter().map(|(l, _)| l.as_str()).collect();
            assert_eq!(labels, ["__proto__", "constructor", "prototype"]);
        }
        "should handle pie definition with or without showData > should handle regular pie" => {
            ast(src);
        }
        "should handle pie definition with or without showData > should handle regular showData" => {
            assert!(ast(src).show_data);
        }
        "should handle TitleAndAccessibilities > should handle TitleAndAccessibilities without showData > should handle regular pie + title in same line"
        | "should handle TitleAndAccessibilities > should handle TitleAndAccessibilities without showData > should handle regular pie + title in different line" =>
        {
            assert_eq!(ast(src).title.as_deref(), Some("sample title"));
        }
        "should handle TitleAndAccessibilities > should handle TitleAndAccessibilities with showData > should handle regular pie + showData + title"
        | "should handle TitleAndAccessibilities > should handle TitleAndAccessibilities with showData > should handle regular showData + title in different line" =>
        {
            let a = ast(src);
            assert!(a.show_data);
            assert_eq!(a.title.as_deref(), Some("sample title"));
        }
        "should handle sections > should handle regular sections" => github_gitlab(&ast(src)),
        "should handle sections > should handle sections with showData" => {
            let a = ast(src);
            assert!(a.show_data);
            github_gitlab(&a);
        }
        "should handle sections > should handle sections with title" => {
            let a = ast(src);
            assert_eq!(a.title.as_deref(), Some("sample wow"));
            github_gitlab(&a);
        }
        "should handle sections > should handle value with positive decimal" => {
            assert_eq!(
                ast(src).sections,
                [("ash".to_string(), 60.67), ("bat".to_string(), 40.0)]
            );
        }
        "should handle sections > should handle sections with accTitle" => {
            let a = ast(src);
            assert_eq!(a.acc_title.as_deref(), Some("sample wow"));
            github_gitlab(&a);
        }
        "should handle sections > should handle sections with single line accDescr"
        | "should handle sections > should handle sections with multi line accDescr" => {
            let a = ast(src);
            assert_eq!(a.acc_descr.as_deref(), Some("sample wow"));
            github_gitlab(&a);
        }
        _ => return false,
    }
    true
}

#[test]
fn passes_mermaid_specs() {
    let skipped = skipped(SKIPPED);
    let mut unknown = Vec::new();
    for case in CASES {
        let listed = skipped.contains(&case.name);
        if case.todo || listed {
            assert!(
                listed,
                "{} is a todo in Mermaid but not in SKIPPED.md",
                case.name
            );
            continue;
        }
        if !check(case.name, case.src) {
            unknown.push(case.name);
        }
        // What parses also renders.
        if model(case.src).is_ok() {
            crate::render(case.src, &crate::RenderOptions::default())
                .unwrap_or_else(|e| panic!("{}: {}", case.name, e.message));
        }
    }
    assert!(
        unknown.is_empty(),
        "cases with no expectation: {unknown:#?}"
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
}

/// The fields of the tree, as `pieSummary` in `tools/oracle/lib.mjs` writes those of
/// Mermaid's.
fn summary(ast: &PieAst) -> String {
    let mut out = vec![format!(
        "show:{}|t:{}|at:{}|ad:{}",
        ast.show_data,
        q(ast.title.as_deref()),
        q(ast.acc_title.as_deref()),
        q(ast.acc_descr.as_deref())
    )];
    for (label, value) in &ast.sections {
        out.push(format!("section:{}|{}", q(Some(label)), js_number(*value)));
    }
    out.join("\n")
}

/// Writes pele-rs's tree of the inputs of a differential run; see `tools/oracle/diff.mjs`.
#[test]
#[ignore = "run by tools/oracle/diff.mjs"]
fn oracle_dump() {
    dump("pie", |src| match parse_pie(src) {
        Ok(ast) => summary(&ast),
        Err(_) => "error".to_string(),
    });
}
