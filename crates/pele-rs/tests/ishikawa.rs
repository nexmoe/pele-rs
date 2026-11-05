//! Ishikawa rendering, security, limits and speed.
#![cfg(feature = "ishikawa")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "ishikawa
    Blurry Photo
        Process
            Out of focus
            Shutter speed too slow
        User
            Shaky hands
        Equipment
            LENS
                Inappropriate lens
            SENSOR
                Damaged sensor
        Environment
            Subject moved too quickly
";

const FLAT: &str = "ishikawa
Effect
Cause one
Cause two
Cause three
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
fn draws_the_effect_and_the_causes() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Ishikawa);
    // The effect is the head of the fishbone, every category of causes a box on a bone.
    assert_eq!(count(&out.svg, "pele-rs-ishikawa-head"), 1);
    assert_eq!(count(&out.svg, "pele-rs-ishikawa-category"), 4);
    // The spine and the bones of the categories.
    assert_eq!(count(&out.svg, "pele-rs-ishikawa-spine"), 1);
    assert_eq!(count(&out.svg, "pele-rs-ishikawa-bones"), 4);
    // The causes of a category are written along its bones.
    assert_eq!(count(&out.svg, "pele-rs-ishikawa-causes"), 4);
    for text in [
        "Blurry Photo",
        "Process",
        "Out of focus",
        "Shutter speed too slow",
        "User",
        "Shaky hands",
        "Equipment",
        "LENS",
        "Inappropriate lens",
        "SENSOR",
        "Damaged sensor",
        "Environment",
        "Subject moved too quickly",
    ] {
        assert!(out.svg.contains(&format!(">{text}<")), "{text} is missing");
    }
}

#[test]
fn the_beta_keyword_works_too() {
    let out = render("ishikawa-beta\nEffect\nCause\n");
    assert_eq!(out.kind, DiagramKind::Ishikawa);
    assert!(out.svg.contains(">Effect<"), "{}", out.svg);
}

#[test]
fn draws_the_title_of_the_front_matter() {
    let out = render(&format!("---\ntitle: Root causes\n---\n{FULL}"));
    assert!(out.svg.contains(">Root causes<"), "{}", out.svg);
    assert!(out.svg.contains("pele-rs-title"), "{}", out.svg);
}

#[test]
fn reads_the_comments_and_the_blank_lines() {
    let out = render("%% a comment\nishikawa\nEffect\n\n%% another comment\nCause\n");
    assert!(out.svg.contains(">Effect<"), "{}", out.svg);
    assert!(out.svg.contains(">Cause<"), "{}", out.svg);
    assert_eq!(count(&out.svg, "pele-rs-ishikawa-category"), 1);
}

#[test]
fn reports_errors_with_positions() {
    // The keyword is needed.
    let err = pele_rs::render("hello\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported);

    // A blank line between the keyword and the effect is not a diagram Mermaid reads.
    let err = pele_rs::render("ishikawa\n\nEffect\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Ishikawa));
    assert_eq!(err.line, 3, "{}", err.snippet);
    assert!(err.snippet.contains("Effect"), "{}", err.snippet);
}

#[test]
fn fits_a_narrow_host() {
    let narrow = narrow(FULL);
    assert!(narrow.width <= 360.0, "{}", narrow.width);
    assert!(narrow.height > 0.0);
    // The causes are drawn one under another instead of as a fishbone.
    assert_eq!(count(&narrow.svg, "pele-rs-ishikawa-cause"), 8);
    assert_eq!(count(&narrow.svg, "pele-rs-ishikawa-spine"), 0);
}

#[test]
fn refuses_a_diagram_that_nests_too_deeply() {
    let mut src = String::from("ishikawa\nEffect\n");
    for depth in 1..80 {
        src.push_str(&" ".repeat(depth));
        src.push_str("Cause\n");
    }
    let err = pele_rs::render(&src, &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Limit);
    assert!(err.message.contains("nests deeper"), "{}", err.message);
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
            format!("ishikawa\n{p}\n"),
            format!("ishikawa\nEffect\n{p}\n"),
            format!("ishikawa\nEffect\n  {p}\n"),
            format!("ishikawa\n{p}\nCause\n"),
        ] {
            let Some(svg) = try_render(&src) else {
                continue;
            };
            assert_inert(&svg, &src);
            rendered += 1;
        }
    }
    assert!(rendered > 40, "only {rendered} hostile inputs rendered");
    let mut fragments: Vec<&str> = PAYLOADS.to_vec();
    fragments.extend([
        "ishikawa\n",
        "ishikawa-beta\n",
        "Effect\n",
        "Cause\n",
        "  ",
        "    ",
        "\n",
        " ",
        "%%",
        "Process",
        "User",
        "é",
    ]);
    let corpus = [FULL, FLAT];
    let mut rnd = Random::new(31);
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
    let head = "ishikawa\n";
    let cases = [
        format!("{head}{}\n", "a".repeat(N)),
        format!("{head}Effect\n{}\n", "Cause\n".repeat(N / 5)),
        format!("{head}Effect\n  {}\n", "b".repeat(N)),
        format!("{head}Effect\n{}\n", "  Cause\n".repeat(N / 7)),
        format!("{head}{}", "Effect\nCause\n".repeat(N / 12)),
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
        ("flat", FLAT.to_string()),
        ("narrow", FULL.to_string()),
    ];
    for (name, src) in cases {
        let svg = if name == "narrow" {
            narrow(&src).svg
        } else {
            svg(&src)
        };
        insta::assert_snapshot!(name, svg);
    }
}
