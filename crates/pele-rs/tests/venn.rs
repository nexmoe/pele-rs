//! Venn diagram rendering, security, limits and speed.
#![cfg(feature = "venn")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "venn-beta
  title What makes a good feature
  set Desirable
  set Feasible
  set Viable
  union Desirable,Feasible[\"Buildable\"]
  union Feasible,Viable[\"Sustainable\"]
  union Desirable,Viable[\"Marketable\"]
  union Desirable,Feasible,Viable[\"Ship it\"]
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
fn draws_a_venn_diagram() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Venn);
    // One circle for every set.
    assert_eq!(count(&out.svg, "class=\"pele-rs-venn-set\""), 3);
    assert!(out.svg.contains("data-id=\"Desirable\""), "{}", out.svg);
    // The name of every set, and the label of every overlap.
    assert!(out.svg.contains(">Desirable<"), "{}", out.svg);
    assert!(out.svg.contains(">Feasible<"), "{}", out.svg);
    assert!(out.svg.contains(">Viable<"), "{}", out.svg);
    assert!(out.svg.contains(">Buildable<"), "{}", out.svg);
    assert!(out.svg.contains(">Ship it<"), "{}", out.svg);
    // The title of the diagram.
    assert!(
        out.svg.contains(">What makes a good feature<"),
        "{}",
        out.svg
    );
    assert!(out.svg.contains("pele-rs-title"), "{}", out.svg);
}

#[test]
fn draws_the_texts_of_the_regions() {
    let out = render(
        "venn-beta
  set A[\"Frontend\"]
    text A1[\"React\"]
    text A2[\"Design Systems\"]
  set B[\"Backend\"]
    text B1[\"API\"]
  union A,B[\"Shared\"]
    text AB1[\"OpenAPI\"]
",
    );
    assert_eq!(count(&out.svg, "class=\"pele-rs-venn-text\""), 4);
    assert!(out.svg.contains(">React<"), "{}", out.svg);
    assert!(out.svg.contains(">OpenAPI<"), "{}", out.svg);
    // A text carries its own id, and the region it is written in carries the sets.
    assert!(out.svg.contains("data-id=\"A1\""), "{}", out.svg);
    assert!(out.svg.contains("data-id=\"A|B\""), "{}", out.svg);
}

#[test]
fn draws_the_sizes_of_the_sets() {
    // The radius of a circle grows with the square root of the size of its set, and the largest
    // circle always has the same radius.
    let radii = |svg: &str| -> Vec<f64> {
        svg.match_indices(" r=\"")
            .map(|(at, _)| {
                let rest = &svg[at + 4..];
                let end = rest.find('"').expect("the end of the radius");
                rest[..end].parse().expect("a number")
            })
            .collect()
    };
    let even = radii(&svg("venn-beta\nset A:40\nset B:40\n"));
    let uneven = radii(&svg("venn-beta\nset A:10\nset B:40\n"));
    assert_eq!(even.len(), 2);
    assert_eq!(uneven.len(), 2);
    assert_eq!(even[0], even[1]);
    // The bigger set has the bigger circle, and twice the size is about a quarter bigger.
    assert!(uneven[1] > uneven[0], "{uneven:?}");
    assert!((uneven[1] / uneven[0] - 2.0).abs() < 0.01, "{uneven:?}");
}

#[test]
fn draws_the_styles() {
    let out = render("venn-beta\n  set A\n  style A fill:#ff6b6b\n");
    assert!(out.svg.contains("fill:#ff6b6b"), "{}", out.svg);
    // A style of an overlap draws the outline of the overlap.
    let out = render("venn-beta\n  set A\n  set B\n  union A,B\n  style A,B fill:#333\n");
    assert_eq!(count(&out.svg, "class=\"pele-rs-venn-overlap\""), 1);
    assert!(out.svg.contains("fill:#333"), "{}", out.svg);
    // A style of a text colors the text.
    let out = render("venn-beta\n  set A\n    text A1\n  style A1 color:red\n");
    assert!(out.svg.contains("fill:red"), "{}", out.svg);
}

#[test]
fn reads_the_configuration() {
    // The canvas of the configuration, with the diagram centred in it.
    let small = render("venn-beta\nset A\nset B\n");
    let big = render(
        "---\nconfig:\n  venn:\n    width: 900\n    height: 700\n---\nvenn-beta\nset A\nset B\n",
    );
    assert_eq!(big.width, 900.0);
    assert_eq!(big.height, 700.0);
    assert!(big.width > small.width);
    // The padding of the configuration.
    let tight = render("venn-beta\nset A\nset B\n");
    let loose = render("---\nconfig:\n  venn:\n    padding: 40\n---\nvenn-beta\nset A\nset B\n");
    assert!(
        loose.width > tight.width,
        "{} vs {}",
        loose.width,
        tight.width
    );
}

#[test]
fn reports_errors_with_positions() {
    // A union needs more than one set.
    let err =
        pele_rs::render("venn-beta\n  set A\n  union A\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert_eq!(err.kind, Some(DiagramKind::Venn));

    // A union of sets that are not there.
    let err = pele_rs::render("venn-beta\n  union A,B\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);

    // A text needs a set to belong to.
    let err = pele_rs::render("venn-beta\n  text A1\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Venn));

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
            format!("venn-beta\nset A[\"{p}\"]\n"),
            format!("venn-beta\nset A\nunion A,B[\"{p}\"]\n"),
            format!("venn-beta\ntitle {p}\nset A\n"),
            format!("venn-beta\nset A\n  text A1[\"{p}\"]\n"),
            format!("venn-beta\nset A\nstyle A fill:{p}\n"),
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
        "venn-beta\n",
        "venn-beta:",
        "venn-beta :",
        "title",
        "title T",
        "set",
        "set A",
        "set A[\"L\"]",
        "set A:20",
        "union",
        "union A,B",
        "union A,B,C",
        "text",
        "text A1",
        "style",
        "style A fill:red",
        "A",
        "B",
        "[",
        "]",
        ",",
        ":",
        "\"",
        "'",
        "1",
        "-1",
        "1.5",
        ".5",
        "\n",
        " ",
        "\t",
        "%%",
    ]);
    let corpus = [FULL];
    let mut rnd = Random::new(19);
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
    let head = "venn-beta\n";
    let cases = [
        format!("{head}set A:{}", "1".repeat(N)),
        format!("{head}{}", "set A\n".repeat(N / 10)),
        format!("{head}set A\nstyle A fill:{}", "x".repeat(N)),
        format!("{head}set A\nunion A,B[{}]\n", "x".repeat(N)),
        format!("{head}title {}\nset A\n", "x".repeat(N)),
        format!("{head}set A\n{}\n", "text A1\n".repeat(N / 10)),
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
        (
            "texts",
            "venn-beta\n  set A[\"Frontend\"]\n    text A1[\"React\"]\n  set B[\"Backend\"]\n    text B1[\"API\"]\n  union A,B[\"Shared\"]\n"
                .to_string(),
        ),
        (
            "styled",
            "venn-beta\n  set A:20\n  set B:12\n  union A,B[\"AB\"]:3\n  style A fill:#ff6b6b\n"
                .to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
