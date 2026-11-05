//! Event modeling diagram rendering, security, limits and speed.
#![cfg(feature = "eventmodeling")]

mod support;

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "eventmodeling
    tf 01 ui CartUI
    tf 02 cmd Inventory.AddItem
    tf 03 evt Inventory.ItemAdded
    rf 04 evt Other.Thing
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
fn draws_the_lanes_and_the_frames() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::EventModeling);
    // Four lanes: the user interface, the commands, the events, and the namespace of the
    // event that is not a command.
    assert_eq!(count(&out.svg, "<g class=\"pele-cluster pele-em-lane\""), 4);
    // Four frames.
    assert_eq!(count(&out.svg, "<g class=\"pele-node pele-em-"), 4);
    // The lane of each frame is written as its label.
    assert!(out.svg.contains(">UI/Automation<"), "{}", out.svg);
    assert!(out.svg.contains(">C/RM: Inventory<"), "{}", out.svg);
    assert!(out.svg.contains(">Stream: Inventory<"), "{}", out.svg);
    // The name of a frame is the name of its entity, without the namespace.
    assert!(out.svg.contains(">AddItem<"), "{}", out.svg);
    assert!(out.svg.contains(">ItemAdded<"), "{}", out.svg);
    // A reset frame says so.
    assert_eq!(count(&out.svg, "pele-em-reset"), 1);
}

#[test]
fn draws_the_relations() {
    // Every frame but the first follows the one before it, unless it is a reset frame.
    let out = render(FULL);
    assert_eq!(count(&out.svg, "<g class=\"pele-edge\""), 2);
    // A frame that names its source is connected to that one.
    let out = render("eventmodeling\nrf 02 evt A\ntf 01 rmo C ->> 02\n");
    assert_eq!(count(&out.svg, "<g class=\"pele-edge\""), 1);
    // A reset frame without a source ends the chain.
    let out = render("eventmodeling\ntf 01 evt A\nrf 02 ui B\ntf 03 cmd C\n");
    assert_eq!(count(&out.svg, "<g class=\"pele-edge\""), 1);
}

#[test]
fn draws_the_data_of_a_frame() {
    let out = render("eventmodeling\ntf 01 cmd AddItem { productId: 7 }\n");
    assert!(out.svg.contains(">productId: 7<"), "{}", out.svg);
    // A data block is drawn under the frame that refers to it.
    let out = render("eventmodeling\ndata D {\n  productId: 7\n}\ntf 01 cmd AddItem [[D]]\n");
    assert!(out.svg.contains(">productId: 7<"), "{}", out.svg);
}

#[test]
fn draws_the_notes_and_the_specifications() {
    // Pele 1.13 draws notes and given/when/then cards alongside the frames.
    let out = render(
        "eventmodeling\nentity Cart\ntf 01 evt A\nnote 01 {\n  a note\n}\ngwt 01 given cmd Cart then evt Cart\n",
    );
    assert!(out.svg.contains("a note"), "{}", out.svg);
    assert!(out.svg.contains("Given"), "{}", out.svg);
    assert!(out.svg.contains(">A<"), "{}", out.svg);
}

#[test]
fn reports_errors_with_positions() {
    // A statement that is not one of the grammar's.
    let err = pele_rs::render("eventmodeling\nnonsense\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::EventModeling));
    assert_eq!(err.line, 2);

    // A number that is used twice.
    let err = pele_rs::render(
        "eventmodeling\ntf 01 evt A\n%% a comment takes a line of its own\nrf 01 evt B\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert_eq!(err.line, 4);
    assert!(
        err.message
            .contains("Duplicate event modeling frame ID \"01\""),
        "{}",
        err.message
    );
}

#[test]
fn refuses_what_meraid_cannot_read() {
    // Mermaid's grammar has a terminal for a title, but its name token comes first, so the
    // line is a name and the diagram does not parse. pele reads a title; pele-rs follows
    // Mermaid.
    let err = pele_rs::render(
        "eventmodeling\ntitle T\ntf 01 evt A\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::EventModeling));

    // The same for the accessible text.
    let err = pele_rs::render(
        "eventmodeling\naccTitle: T\ntf 01 evt A\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
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
            format!("eventmodeling\ntf 01 cmd {p}\n"),
            format!("eventmodeling\ntf 01 cmd A {p}\n"),
            format!("eventmodeling\ndata D {{\n{p}\n}}\ntf 01 cmd A [[D]]\n"),
            format!("eventmodeling\ntf 01 evt A\nnote 01 {{\n{p}\n}}\n"),
            format!("eventmodeling\nentity {p}\ntf 01 evt A\n"),
            format!("eventmodeling\ntf 01 evt A ->> {p}\n"),
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
        "eventmodeling\n",
        "tf ",
        "rf ",
        "ui ",
        "cmd ",
        "evt ",
        "rmo ",
        "pcr ",
        "entity ",
        "data ",
        "note ",
        "gwt ",
        "given ",
        "when ",
        "then ",
        "->> ",
        "[[",
        "]]",
        "`json`",
        "{ a: 1 }",
        "01 ",
        "CartUI",
        "AddItem",
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
    let head = "eventmodeling\n";
    // A number of frames, each with its own number.
    let frames = |kind: &str| {
        (0..500).fold(String::new(), |mut s, i| {
            let _ = writeln!(s, "tf {i:03} {kind} A");
            s
        })
    };
    let cases = [
        format!("{head}tf 01 cmd {}\n", "x".repeat(N)),
        format!("{head}{}", frames("cmd")),
        format!("{head}{}", frames("evt")),
        format!("{head}data D {{\n{}\n}}\n", "x".repeat(N)),
        format!(
            "{head}tf 01 evt A\n{}",
            "note 01 {{\n  a note\n}}\n".repeat(N / 40)
        ),
        format!("{head}{}", "entity A\n".repeat(N / 9)),
        format!("{head}tf 01 evt A {}\n", "{".repeat(N / 2)),
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
            "data",
            "eventmodeling\ndata D {\n  productId: 7\n}\ntf 01 cmd AddItem [[D]]\n".to_string(),
        ),
        (
            "notes",
            "eventmodeling\nentity Cart\ntf 01 evt A\nnote 01 {\n  a note\n}\ngwt 01 given cmd Cart then evt Cart\n"
                .to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
