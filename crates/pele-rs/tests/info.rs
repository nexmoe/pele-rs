//! Information diagram rendering, security, limits and speed.
#![cfg(feature = "info")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "info
title An information diagram
accTitle: An information diagram
accDescr: It shows the version of the library.
";

fn render(src: &str) -> pele_rs::Rendered {
    pele_rs::render(src, &RenderOptions::default()).unwrap_or_else(|e| panic!("{src:?}: {e}"))
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
fn draws_the_version() {
    let out = render("info\n");
    assert_eq!(out.kind, DiagramKind::Info);
    // The version of the library, under the title.
    assert_eq!(count(&out.svg, "class=\"pele-rs-version\""), 1);
    assert!(out.svg.contains(">pele-rs v"), "{}", out.svg);
    // There is no title to draw.
    assert_eq!(count(&out.svg, "class=\"pele-rs-title\""), 0);
}

#[test]
fn draws_a_title() {
    let out = render("info\ntitle An information diagram\n");
    assert_eq!(count(&out.svg, "class=\"pele-rs-title\""), 1);
    assert!(out.svg.contains(">An information diagram<"), "{}", out.svg);

    // The title of the front matter is the title of the diagram.
    let out = render("---\ntitle: From the front matter\n---\ninfo\n");
    assert!(out.svg.contains(">From the front matter<"), "{}", out.svg);

    // A title in the diagram is the title of the diagram.
    let out = render("---\ntitle: From the front matter\n---\ninfo\ntitle In the diagram\n");
    assert!(out.svg.contains(">In the diagram<"), "{}", out.svg);
    assert!(!out.svg.contains("From the front matter"), "{}", out.svg);
}

#[test]
fn draws_the_accessible_text() {
    let out = render("info\naccTitle: An information diagram\naccDescr: A description\n");
    assert!(
        out.svg
            .contains("<title id=\"pele-rs-title\">An information diagram</title>"),
        "{}",
        out.svg
    );
    assert!(
        out.svg
            .contains("<desc id=\"pele-rs-desc\">A description</desc>"),
        "{}",
        out.svg
    );
}

#[test]
fn reports_errors_with_positions() {
    // A word that Mermaid's grammar does not know.
    let err = pele_rs::render("info unsupported\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Info));
    assert_eq!(err.line, 1);
    assert_eq!(err.column, 6);

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
fn output_is_deterministic() {
    assert_eq!(svg(FULL), svg(FULL));
}

#[test]
fn is_inert_for_hostile_and_mutated_inputs() {
    let mut rendered = 0;
    for payload in PAYLOADS {
        let p = *payload;
        for src in [
            format!("info\ntitle {p}\n"),
            format!("info\naccTitle: {p}\n"),
            format!("info\naccDescr: {p}\n"),
            format!("info\naccDescr {{\n{p}\n}}\n"),
            format!("info showInfo\ntitle {p}\n"),
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
        "info\n",
        "info ",
        "info showInfo\n",
        "showInfo",
        "title ",
        "title",
        "accTitle: ",
        "accTitle:",
        "accDescr: ",
        "accDescr:",
        "accDescr {",
        "}",
        "{",
        ":",
        "\n",
        " ",
        "\t",
        "%%",
        "x",
        "é",
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
    let cases = [
        format!("info\ntitle {}\n", "x".repeat(N)),
        format!("info\ntitle A\n{}\n", "title B\n".repeat(N / 10)),
        format!("info\naccDescr: {}\n", "x".repeat(N)),
        format!("info\naccDescr {{\n{}\n}}\n", "x\n".repeat(N / 2)),
        format!("info\n{}", "accTitle: A\n".repeat(N / 12)),
        format!("info\n{}", "%% a comment\n".repeat(N / 14)),
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
        rendered >= 4,
        "only {rendered} of {} cases were drawn",
        cases.len()
    );
}

#[test]
fn snapshots() {
    let cases = [
        ("full", FULL.to_string()),
        ("title", "info\ntitle An information diagram\n".to_string()),
        (
            "accessible",
            "info\naccTitle: An information diagram\naccDescr: It shows the version of the library.\n"
                .to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
