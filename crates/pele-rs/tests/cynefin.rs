//! Cynefin diagram rendering, security, limits and speed.
#![cfg(feature = "cynefin")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "cynefin-beta
  title Incident Response

  complex
    \"Investigate root cause\"
    \"Run chaos experiment\"

  complicated
    \"Analyze performance data\"

  clear
    \"Restart service\"

  chaotic
    \"Page on-call immediately\"

  confusion
    \"Unknown failure mode\"
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
fn draws_the_five_domains() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Cynefin);
    // The four regions of the frame and the ellipse in the middle.
    assert_eq!(count(&out.svg, "class=\"pele-rs-cynefin-region\""), 5);
    assert_eq!(count(&out.svg, "class=\"pele-rs-cynefin-boundaries\""), 1);
    assert_eq!(count(&out.svg, "class=\"pele-rs-cynefin-cliff\""), 1);
    // Every domain is drawn, with its name.
    assert_eq!(count(&out.svg, "class=\"pele-rs-cynefin-domain\""), 5);
    for name in ["Complex", "Complicated", "Clear", "Chaotic", "Confusion"] {
        assert!(out.svg.contains(&format!(">{name}<")), "{}", out.svg);
    }
    // The items are in the badges.
    assert!(out.svg.contains(">Investigate root cause<"), "{}", out.svg);
    assert!(out.svg.contains(">Unknown failure mode<"), "{}", out.svg);
    assert_eq!(count(&out.svg, "class=\"pele-rs-node\""), 6);
}

#[test]
fn puts_the_items_in_the_domain_they_name() {
    let out = render("cynefin-beta\n  clear\n    \"A\"\n  chaotic\n    \"B\"\n");
    // The clear domain stands in the right half of the frame, the chaotic one in the left half.
    let clear = out
        .svg
        .find("class=\"pele-rs-cynefin-domain\" data-id=\"clear\"")
        .expect("the clear domain");
    let chaotic = out
        .svg
        .find("class=\"pele-rs-cynefin-domain\" data-id=\"chaotic\"")
        .expect("the chaotic domain");
    // The domains are drawn in the order of the frame.
    assert!(chaotic < clear, "{}", out.svg);
    // The clear domain stands in the right half of the frame, the chaotic one in the left half.
    let chaotic_text = &out.svg[chaotic..chaotic + 300];
    assert!(chaotic_text.contains("x=\"168\""), "{chaotic_text}");
    let clear_text = &out.svg[clear..clear + 300];
    assert!(clear_text.contains("x=\"488\""), "{clear_text}");
    // Every item is in the group of the domain it was named after.
    let badge_a = out.svg.find("data-id=\"A\"").expect("the item A");
    let badge_b = out.svg.find("data-id=\"B\"").expect("the item B");
    assert!(chaotic < badge_b && badge_b < clear, "{}", out.svg);
    assert!(clear < badge_a, "{}", out.svg);
    // The name of a domain is in its group.
    assert!(out.svg[clear..].contains(">Clear<"), "{}", out.svg);
    assert!(out.svg[chaotic..].contains(">Chaotic<"), "{}", out.svg);
}

#[test]
fn draws_the_transitions() {
    let out =
        render("cynefin-beta\n  complex\n  complicated\n  complex --> complicated : \"Pattern\"\n");
    assert_eq!(count(&out.svg, "class=\"pele-rs-edge\""), 1);
    assert!(
        out.svg.contains("data-id=\"complex-complicated\""),
        "{}",
        out.svg
    );
    // The label of the transition is drawn.
    assert!(out.svg.contains(">Pattern<"), "{}", out.svg);
    // The arrowhead of the transition.
    assert_eq!(count(&out.svg, "class=\"pele-rs-marker\""), 1);
}

#[test]
fn drops_a_self_loop() {
    // Mermaid's database skips a transition that starts and ends in the same domain.
    let out = render("cynefin-beta\n  complex\n  complex --> complex : \"x\"\n");
    assert_eq!(count(&out.svg, "class=\"pele-rs-edge\""), 0);
}

#[test]
fn reads_the_config() {
    // The frame is wider and taller when the config says so.
    let small = render("cynefin-beta\n  complex\n");
    let big = render(
        "---\nconfig:\n  cynefin:\n    width: 1200\n    height: 900\n---\ncynefin-beta\n  complex\n",
    );
    assert!(big.width > small.width, "{} {}", big.width, small.width);
    assert!(big.height > small.height, "{} {}", big.height, small.height);
    // The descriptions of the domains are left out when the config says so.
    let described = svg("cynefin-beta\n  complex\n");
    let plain = svg(
        "---\nconfig:\n  cynefin:\n    showDomainDescriptions: false\n---\ncynefin-beta\n  complex\n",
    );
    assert!(described.contains("Probe"));
    assert!(!plain.contains("Probe"));
}

#[test]
fn draws_a_title() {
    let out = render("---\ntitle: A Cynefin map\n---\ncynefin-beta\n  complex\n");
    assert!(out.svg.contains(">A Cynefin map<"), "{}", out.svg);
    assert!(out.svg.contains("pele-rs-title"));
    // The title of the source wins over the one of the front matter.
    let out = render("---\ntitle: A Cynefin map\n---\ncynefin-beta\n  title Another\n  complex\n");
    assert!(out.svg.contains(">Another<"), "{}", out.svg);
    assert!(!out.svg.contains(">A Cynefin map<"), "{}", out.svg);
}

#[test]
fn draws_the_accessible_text() {
    let out = render(
        "cynefin-beta\n  accTitle: The framework\n  accDescr: A map of the five domains\n  complex\n",
    );
    assert!(
        out.svg
            .contains("<title id=\"pele-rs-title\">The framework</title>"),
        "{}",
        out.svg
    );
    assert!(
        out.svg
            .contains("<desc id=\"pele-rs-desc\">A map of the five domains</desc>"),
        "{}",
        out.svg
    );
}

#[test]
fn reports_errors_with_positions() {
    // A word that is not a domain.
    let err = pele_rs::render(
        "cynefin-beta\n  complex\n  foo\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Cynefin));
    assert_eq!(err.line, 3);

    // A transition that names no domain.
    let err =
        pele_rs::render("cynefin-beta\n  complex -->\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
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
fn keeps_the_comments_out_of_the_way() {
    // A comment is a comment, and the domains around it are still read.
    let out = render("cynefin-beta\n  %% a comment\n  complex\n    \"An item\"\n");
    assert!(out.svg.contains(">An item<"), "{}", out.svg);
    assert!(!out.svg.contains("a comment"), "{}", out.svg);
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
            format!("cynefin-beta\n  complex\n    \"{p}\"\n"),
            format!("cynefin-beta\n  complex\n    \"A\"\n    \"{p}\"\n"),
            format!("cynefin-beta\n  complex --> complicated : \"{p}\"\n"),
            format!("cynefin-beta\n  title {p}\n  complex\n"),
            format!("cynefin-beta\n  accTitle: {p}\n  complex\n"),
            format!("cynefin-beta\n  accDescr: {p}\n  complex\n"),
            format!("---\ntitle: \"{p}\"\n---\ncynefin-beta\n  complex\n"),
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
        "cynefin-beta\n",
        "cynefin-beta:\n",
        "complex\n",
        "complicated\n",
        "clear\n",
        "chaotic\n",
        "confusion\n",
        "title T\n",
        "accTitle: t\n",
        "accDescr: d\n",
        "accDescr {\n}\n",
        "-->\n",
        ": \"x\"\n",
        "\"item\"\n",
        "'item'\n",
        "%%\n",
        "\n",
        " ",
        "complex",
    ]);
    let corpus = [FULL];
    let mut rnd = Random::new(23);
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
    let head = "cynefin-beta\n";
    let cases = [
        format!("{head}complex\n  \"{}\"\n", "x".repeat(N)),
        format!("{head}{}", "complex\n".repeat(N / 10)),
        format!("{head}{}", "\"item\"\n".repeat(N / 10)),
        format!("{head}complex --> complicated : \"{}\"\n", "x".repeat(N)),
        format!("{head}{}", "complex --> complicated\n".repeat(N / 20)),
        format!("{head}title {}\ncomplex\n", "x".repeat(N)),
        format!("{head}accDescr: {}\ncomplex\n", "x".repeat(N)),
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
        (
            "transitions",
            "cynefin-beta\n  title A map\n  complex\n    \"A\"\n  complicated\n    \"B\"\n  \
             complex --> complicated : \"Pattern\"\n  complicated --> clear : \"Practice\"\n"
                .to_string(),
        ),
        (
            "empty",
            "cynefin-beta\n  complex\n  complicated\n  clear\n  chaotic\n  confusion\n".to_string(),
        ),
        (
            "accessible",
            "cynefin-beta\n  accTitle: The framework\n  accDescr: A map\n  confusion\n    \"A\"\n"
                .to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
