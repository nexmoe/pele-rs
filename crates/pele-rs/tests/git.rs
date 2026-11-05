//! Git graph rendering, security, limits and speed.
#![cfg(feature = "git")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "gitGraph
    commit id:\"init\" msg:\"the first commit\"
    branch develop
    commit id:\"work\" msg:\"work in progress\" tag:\"wip\"
    checkout main
    merge develop id:\"merge\" tag:\"v1.0\"
    commit id:\"ship\" msg:\"ship it\" type:HIGHLIGHT
";

fn render(src: &str) -> pele_rs::Rendered {
    pele_rs::render(src, &RenderOptions::default()).unwrap_or_else(|e| panic!("{src:?}: {e}"))
}

fn narrow(src: &str) -> pele_rs::Rendered {
    let options = RenderOptions {
        max_width: Some(360.0),
        ..RenderOptions::default()
    };
    pele_rs::render(src, &options).unwrap_or_else(|e| panic!("{src:?}: {e}"))
}

fn svg(src: &str) -> String {
    render(src).svg
}

fn count(svg: &str, needle: &str) -> usize {
    svg.matches(needle).count()
}

fn try_render(src: &str) -> Option<String> {
    pele_rs::render(src, &RenderOptions::default())
        .ok()
        .map(|r| r.svg)
}

#[test]
fn draws_commits_and_branches() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::GitGraph);
    // Four commits: the first, the one on the branch, the merge and the last.
    assert_eq!(count(&out.svg, "<g class=\"pele-commit "), 4);
    // Two lanes: the main branch and the branch that grows from it.
    assert_eq!(count(&out.svg, "<g class=\"pele-branch\""), 2);
    // The lane of each branch is a dashed line.
    assert_eq!(count(&out.svg, "<path class=\"pele-lane\""), 2);
    // The merge joins two commits.
    assert_eq!(count(&out.svg, "<path class=\"pele-edge\""), 3);
    // The tag of a commit is drawn next to it.
    assert_eq!(count(&out.svg, "<g class=\"pele-tag\">"), 2);
    // The id of a commit is written beside it.
    assert!(out.svg.contains(">init<"), "{}", out.svg);
    // The message of a commit is its tooltip.
    assert!(out.svg.contains("<title>ship it</title>"), "{}", out.svg);
    // A highlighted commit is a square.
    assert!(out.svg.contains("rx=\"2\""), "{}", out.svg);
}

#[test]
fn reads_the_direction() {
    // The history runs from left to right by default.
    let across = svg("gitGraph\ncommit\ncommit\n");
    let down = svg("gitGraph TB:\ncommit\ncommit\n");
    assert_ne!(across, down);
    let up = svg("gitGraph BT:\ncommit\ncommit\n");
    assert_ne!(down, up);
    assert_ne!(across, up);
}

#[test]
fn reads_the_config() {
    // The branches can be hidden.
    let shown = svg("gitGraph\nbranch other\ncommit\n");
    assert!(shown.contains("pele-branch"), "{shown}");
    let hidden = svg(
        "---\nconfig:\n  gitGraph:\n    showBranches: false\n---\ngitGraph\nbranch other\ncommit\n",
    );
    assert!(!hidden.contains("pele-branch"), "{hidden}");

    // The ids of the commits can be hidden.
    let labelled = svg("gitGraph\ncommit id:\"abc\"\n");
    assert!(labelled.contains(">abc<"), "{labelled}");
    let unlabelled = svg(
        "---\nconfig:\n  gitGraph:\n    showCommitLabel: false\n---\ngitGraph\ncommit id:\"abc\"\n",
    );
    assert!(!unlabelled.contains(">abc<"), "{unlabelled}");

    // The ids can be written flat instead of turned.
    let turned = svg("gitGraph\ncommit id:\"abc\"\n");
    assert!(turned.contains("rotate(-45"), "{turned}");
    let flat = svg(
        "---\nconfig:\n  gitGraph:\n    rotateCommitLabel: false\n---\ngitGraph\ncommit id:\"abc\"\n",
    );
    assert!(!flat.contains("rotate(-45"), "{flat}");

    // The name of the main branch can be changed.
    let main = svg("gitGraph\ncommit\n");
    assert!(main.contains(">main<"), "{main}");
    let trunk =
        svg("---\nconfig:\n  gitGraph:\n    mainBranchName: trunk\n---\ngitGraph\ncommit\n");
    assert!(trunk.contains(">trunk<"), "{trunk}");

    // The commits can be placed side by side: a commit then stands one step after its own
    // parent, not after the commit before it.
    let history = "gitGraph\ncommit\nbranch a\ncommit\ncheckout main\ncommit\n";
    let serial = svg(history);
    let parallel = svg(&format!(
        "---\nconfig:\n  gitGraph:\n    parallelCommits: true\n---\n{history}"
    ));
    assert_ne!(serial, parallel);
}

#[test]
fn draws_a_title() {
    let out = render("---\ntitle: The history\n---\ngitGraph\ncommit\n");
    assert!(out.svg.contains(">The history<"), "{}", out.svg);
    assert!(out.svg.contains("pele-title"));
}

#[test]
fn reads_the_accessible_text() {
    let out = render(
        "---\nconfig:\n  gitGraph:\n    showCommitLabel: true\n---\ngitGraph\naccTitle: The history\naccDescr: How the project grew\ncommit\n",
    );
    assert!(out.svg.contains("The history"), "{}", out.svg);
    assert!(out.svg.contains("How the project grew"), "{}", out.svg);
}

#[test]
fn reports_errors_with_positions() {
    // A statement that is not one of the grammar's.
    let err = pele_rs::render("gitGraph\nnonsense\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::GitGraph));
    assert_eq!(err.line, 2);

    // The keyword of the diagram is needed.
    let err = pele_rs::render("hello\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported);
    assert!(
        err.message.contains("No diagram type detected"),
        "{}",
        err.message
    );
}

#[test]
fn refuses_a_history_that_does_not_hold() {
    // A branch that is checked out but never made.
    let err = pele_rs::render("gitGraph\ncheckout other\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert!(
        err.message.contains("Trying to checkout branch"),
        "{}",
        err.message
    );

    // A branch that is made twice.
    let err = pele_rs::render(
        "gitGraph\nbranch other\nbranch other\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert!(
        err.message.contains("Trying to create an existing branch"),
        "{}",
        err.message
    );

    // A branch that is merged into itself.
    let err = pele_rs::render(
        "gitGraph\nbranch other\ncheckout other\nmerge other\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert!(
        err.message.contains("Cannot merge a branch to itself"),
        "{}",
        err.message
    );
}

#[test]
fn refuses_a_name_mermaid_cannot_read() {
    // Mermaid's grammar reads a branch name as a name of ASCII letters, digits, underscores,
    // dashes, dots and slashes, so a name with a letter outside that range is a parse error
    // there. pele reads it; pele-rs follows Mermaid.
    let err = pele_rs::render("gitGraph\nbranch Café\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::GitGraph));
}

#[test]
fn fits_a_narrow_host() {
    let wide = render(FULL);
    let narrow = narrow(FULL);
    assert!(narrow.width <= 360.0, "{}", narrow.width);
    assert!(narrow.height > 0.0);
    // The history is still drawn.
    assert_eq!(count(&narrow.svg, "<g class=\"pele-commit "), 4);
    let _ = wide;
}

#[test]
fn output_is_deterministic() {
    assert_eq!(svg(FULL), svg(FULL));
}

#[test]
fn is_inert_for_hostile_and_mutated_inputs() {
    let mut rendered = 0;
    for payload in PAYLOADS {
        let p = *payload;
        for src in [
            format!("gitGraph\ncommit msg:\"{p}\"\n"),
            format!("gitGraph\ncommit id:\"{p}\"\n"),
            format!("gitGraph\ncommit tag:\"{p}\"\n"),
            format!("gitGraph\nbranch {p}\n"),
            format!("gitGraph\nbranch \"{p}\"\n"),
            format!("gitGraph\ncommit\ncheckout {p}\n"),
            format!("gitGraph\ncommit\nmerge {p}\n"),
            format!("gitGraph\ncherry-pick id:\"{p}\"\n"),
            format!("---\ntitle: \"{p}\"\n---\ngitGraph\ncommit\n"),
            format!(
                "---\nconfig:\n  gitGraph:\n    mainBranchName: \"{p}\"\n---\ngitGraph\ncommit\n"
            ),
        ] {
            let Some(svg) = try_render(&src) else {
                continue;
            };
            assert_inert(&svg, &src);
            rendered += 1;
        }
    }
    assert!(rendered > 60, "only {rendered} hostile inputs rendered");
    let mut fragments: Vec<&str> = PAYLOADS.to_vec();
    fragments.extend([
        "gitGraph\n",
        "gitGraph:\n",
        "commit",
        "branch",
        "checkout",
        "merge",
        "cherry-pick",
        "id:",
        "msg:",
        "tag:",
        "type:",
        "order:",
        "parent:",
        "NORMAL",
        "REVERSE",
        "HIGHLIGHT",
        "\"",
        "\n",
        " ",
    ]);
    let corpus = [FULL];
    let mut rnd = Random::new(11);
    let mut rendered = 0;
    for _ in 0..1500 {
        let src = mutate(&corpus, &fragments, &mut rnd);
        if let Some(svg) = try_render(&src) {
            assert_inert(&svg, &src);
            rendered += 1;
        }
    }
    // A parser that refused every mutation would make the loop above assert nothing at all.
    assert!(rendered > 150, "only {rendered} of 1500 mutations rendered");
}

#[test]
fn stays_fast_on_large_and_hostile_input() {
    // Debug builds are several times slower than release.
    let budget = Duration::from_secs(if cfg!(debug_assertions) { 30 } else { 3 });
    let options = RenderOptions {
        limits: Limits {
            source: usize::MAX,
            output: usize::MAX,
            ..Limits::default()
        },
        ..RenderOptions::default()
    };
    const N: usize = 50000;
    let head = "gitGraph\n";
    let cases = [
        format!("{head}commit msg:\"{}\"\n", "x".repeat(N)),
        format!("{head}{}", "commit\n".repeat(N / 8)),
        format!("{head}branch {}\ncommit\n", "x".repeat(N)),
        format!("{head}commit tag:\"{}\"\n", "x".repeat(N)),
        format!("{head}commit\n{}", "commit\n".repeat(N / 16)),
        format!("{head}{}", "branch a\ncommit\n".repeat(N / 16)),
        format!(
            "{head}{}",
            "branch a\ncommit\nbranch b\ncommit\n".repeat(N / 32)
        ),
    ];
    let mut rendered = 0;
    for src in &cases {
        let started = Instant::now();
        let out = pele_rs::render(src, &options);
        let took = started.elapsed();
        assert!(
            took < budget,
            "{:?} took {took:?}",
            &src[..src.len().min(40)]
        );
        if out.is_ok() {
            rendered += 1;
        }
    }
    // A renderer that refused everything would answer at once and say nothing about speed.
    assert!(
        rendered >= 5,
        "only {rendered} of {} cases were drawn",
        cases.len()
    );
}

#[test]
fn snapshots() {
    let cases = [
        ("full", FULL.to_string()),
        ("down", "gitGraph TB:\ncommit\nbranch other\ncommit\n".to_string()),
        (
            "branches_off",
            "---\nconfig:\n  gitGraph:\n    showBranches: false\n---\ngitGraph\nbranch other\ncommit\n"
                .to_string(),
        ),
        (
            "labels_off",
            "---\nconfig:\n  gitGraph:\n    showCommitLabel: false\n---\ngitGraph\ncommit id:\"abc\"\n"
                .to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
