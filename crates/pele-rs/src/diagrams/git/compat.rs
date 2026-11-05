//! Mermaid's git graph specs, as pele ports them. The cases are generated into
//! `tests/compat/git/cases.rs` by `cargo xtask compat`, and what pele makes of each into
//! `expected.rs` by `tools/oracle/golden.mjs`: pele passes these specs, so a case on which
//! pele-rs's model matches pele's passes as well. Every case is either checked or listed in
//! `tests/compat/git/SKIPPED.md`.

use super::db::{GitCommit, GitModel, build_git};
use super::parser::{Direction, parse_git};
use crate::diagrams::common::golden::{
    assert_skips_match_the_templates, dump, error_code, json, q, skipped,
};
use crate::preprocess::Config;
use crate::text::entities::encode_entities;

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
    "/tests/compat/git/cases.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/git/expected.rs"
));

const SKIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/compat/git/SKIPPED.md"
));

/// A list of texts, as `list` in `tools/oracle/lib.mjs` writes it.
fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json(&items.join("\u{1}"))
    }
}

/// The name of a direction, as Mermaid writes it.
fn direction(direction: Direction) -> &'static str {
    match direction {
        Direction::LeftToRight => "LR",
        Direction::TopToBottom => "TB",
        Direction::BottomToTop => "BT",
    }
}

/// The number of a commit, as Mermaid counts the commits.
fn seq(commit: Option<&GitCommit>) -> String {
    commit.map_or_else(|| "-".to_string(), |commit| commit.seq.to_string())
}

/// The model in the form `gitSummary` in `tools/oracle/lib.mjs` writes pele's.
fn summary(model: &GitModel) -> String {
    let mut out: Vec<String> = Vec::new();
    out.push(format!("dir:{}", q(Some(direction(model.direction)))));

    let mut commits: Vec<&GitCommit> = model.commits.values().collect();
    commits.sort_by_key(|commit| commit.seq);
    for commit in commits {
        // Mermaid ends the id of a commit it made up with seven random digits; pele-rs derives
        // them, so only the number of the commit is compared.
        let id = if commit.id.starts_with(&format!("{}-", commit.seq)) {
            format!("{}-*", commit.seq)
        } else {
            commit.id.clone()
        };
        let parents = commit
            .parents
            .iter()
            .map(|parent| match model.commits.get(parent) {
                Some(parent) => parent.seq.to_string(),
                None if parent.is_empty() => "-".to_string(),
                None => "?".to_string(),
            })
            .collect::<Vec<_>>()
            .join(",");
        out.push(format!(
            "commit:{}|{}|{}|{}|{}|p={}|b={}|ct={}|ci={}",
            commit.seq,
            json(&id),
            json(&commit.message),
            commit.kind.index(),
            list(&commit.tags),
            parents,
            json(&commit.branch),
            commit
                .custom_type
                .map_or_else(|| "-".to_string(), |kind| kind.index().to_string()),
            commit.custom_id
        ));
    }
    for (name, head) in &model.branches.items {
        let head = head.as_deref().and_then(|id| model.commits.get(id));
        out.push(format!("branch:{}|{}", json(name), seq(head)));
    }
    out.push(format!("lanes:{}", list(&model.lanes)));
    out.push(format!("current:{}", json(&model.current_branch)));
    out.push(format!(
        "head:{}",
        seq(model.head.as_deref().and_then(|id| model.commits.get(id)))
    ));
    out.push(format!(
        "title:{}|acc:{}|{}",
        q(model.title.as_deref()),
        q(model.acc_title.as_deref()),
        q(model.acc_descr.as_deref())
    ));
    out.push(format!("warnings:{}", model.warnings.len()));
    out.join("\n")
}

fn parsed(src: &str) -> String {
    // Mermaid encodes the entities of the whole diagram before any parser sees it, and adds a
    // line break to the text it parses.
    let text = encode_entities(src) + "\n";
    match parse_git(&text).and_then(|ast| build_git(&ast, &Config::new(), None)) {
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
    dump("git", parsed);
}
