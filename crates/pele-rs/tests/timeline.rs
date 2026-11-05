//! Timeline rendering, security, limits and speed.
#![cfg(feature = "timeline")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "timeline\n title History of Social Media Platform\n 2002 : LinkedIn\n section Early days\n 2004 : Facebook : Google\n 2005 : YouTube\n section Later\n 2006 : Twitter\n : A second event that is long enough to wrap onto more lines\n 2010 : Supercalifragilisticexpialidocious_is_a_very_long_word\n";

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
fn draws_periods_events_and_sections() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Timeline);
    assert_eq!(count(&out.svg, "pele-node pele-period"), 5);
    assert_eq!(count(&out.svg, "pele-node pele-event"), 7);
    assert_eq!(count(&out.svg, "pele-cluster pele-section"), 2);
    for needle in [">LinkedIn<", ">Early days<", "pele-axis", "pele-title"] {
        assert!(out.svg.contains(needle), "{needle}");
    }
    assert!(out.links.is_empty());
}

#[test]
fn lays_the_events_out_in_a_row_or_in_a_column() {
    // `TD` puts the events under one another, so they share an x and differ in their y.
    let across = svg("timeline\n 2001 : a\n 2002 : b\n 2003 : c\n");
    assert!(across.contains("<rect x=\"8\""), "{across}");
    assert!(across.contains("<rect x=\"400\""), "{across}");
    let down = svg("timeline TD\n 2001 : a\n 2002 : b\n 2003 : c\n");
    assert_eq!(down.matches("<rect x=\"232\"").count(), 3, "{down}");
}

#[test]
fn writes_the_accessible_title_and_description() {
    let out = render("timeline\naccTitle: the title\naccDescr: the description\n 2001 : a\n");
    assert!(
        out.svg
            .contains("<title id=\"pele-title\">the title</title>"),
        "{}",
        out.svg
    );
    assert!(
        out.svg
            .contains("<desc id=\"pele-desc\">the description</desc>"),
        "{}",
        out.svg
    );
}

#[test]
fn shows_markup_as_text() {
    let svg = svg("timeline\n 2001 : <b>bold</b> :fa-car <script>x</script>\n");
    assert!(svg.contains("&lt;b&gt;bold&lt;/b&gt;"));
    assert!(!svg.contains("<script"));
    assert!(!svg.contains("<b>"));
}

#[test]
fn reports_errors_with_positions() {
    let err = pele_rs::render("timeline\n : orphan\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert_eq!(err.kind, Some(DiagramKind::Timeline));
    let err = pele_rs::render("timeline\n 2001 : a\n :x\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.line, 3);
}

#[test]
fn fits_a_narrow_host() {
    let src = "timeline\n 2001 : a\n 2002 : b\n 2003 : c\n 2004 : d\n 2005 : e\n";
    let wide = render(src);
    let options = RenderOptions {
        max_width: Some(320.0),
        ..RenderOptions::default()
    };
    let narrow = pele_rs::render(src, &options).unwrap();
    assert!(narrow.width < wide.width);
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
            format!("timeline\n title {p}\n 2001 : x"),
            format!("timeline\n {p} : {p}"),
            format!("timeline\n section {p}\n 2001 : {p} : {p}"),
            format!("timeline\n accTitle: {p}\n accDescr: {p}\n 2001"),
            format!("---\ntitle: \"{p}\"\n---\ntimeline\n 2001 : x"),
        ] {
            let Some(svg) = try_render(&src) else {
                continue;
            };
            assert_inert(&svg, &src);
            rendered += 1;
        }
    }
    assert!(rendered > 50, "only {rendered} hostile inputs rendered");
    let mut fragments: Vec<&str> = PAYLOADS.to_vec();
    fragments.extend([
        "section ",
        "title ",
        " : ",
        ":",
        "timeline TD",
        "<br>",
        "\n",
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
    assert!(rendered > 800, "only {rendered} of 1500 mutations rendered");
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
    let head = "timeline\n";
    let cases = [
        format!("{head}{}", "x".repeat(N)),
        format!("{head}{}", "x ".repeat(N / 2)),
        format!("{head}2001 {}", ": x ".repeat(N / 4)),
        format!("{head}{}", "2001 : x\n".repeat(N / 9)),
        format!("{head}{}", "section s\n".repeat(N / 10)),
        format!("{head}{}", "section s\n2001\n".repeat(N / 15)),
        format!("{head}{}", "%%".repeat(N / 2)),
        format!("{head}accDescr {{{}", "x ".repeat(N / 2)),
        format!("{head}title {}", "x".repeat(N)),
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
        rendered >= 6,
        "only {rendered} of {} cases were drawn",
        cases.len()
    );
}

#[test]
fn snapshots() {
    let cases = [
        ("across", FULL.to_string()),
        ("down", FULL.replace("timeline\n", "timeline TD\n")),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
