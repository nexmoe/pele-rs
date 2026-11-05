//! Wardley map rendering, security, limits and speed.
#![cfg(feature = "wardley")]

mod support;

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "wardley-beta
evolution Genesis -> Custom Built -> Product -> Commodity
component Cup of Tea [0.79, 0.61]
component Bag of Flour [0.79, 0.85]
component Milk [0.62, 0.35]
anchor User [0.75, 0.63]
Cup of Tea +<> Bag of Flour
Bag of Flour -> Milk
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
fn draws_the_components_and_the_anchors() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Wardley);
    assert_eq!(
        count(&out.svg, "<g class=\"pele-node pele-wardley-component\""),
        3
    );
    assert_eq!(
        count(&out.svg, "<g class=\"pele-node pele-wardley-anchor\""),
        1
    );
    // The name of a component is its label.
    assert!(out.svg.contains(">Cup of Tea<"), "{}", out.svg);
    assert!(out.svg.contains(">User<"), "{}", out.svg);
}

#[test]
fn draws_the_links_between_the_components() {
    let out = render(FULL);
    assert_eq!(count(&out.svg, "<g class=\"pele-edge\""), 2);
    // The link that names both of its ends is drawn between them.
    assert!(
        out.svg.contains("data-id=\"Cup of Tea-Bag of Flour\""),
        "{}",
        out.svg
    );
}

#[test]
fn draws_the_arrowheads_of_the_flows() {
    let out = render("wardley-beta\ncomponent A [0.2, 0.8]\ncomponent B [0.8, 0.2]\nA +<> B\n");
    // The link is in both directions, so it has two arrowheads.
    assert_eq!(count(&out.svg, "pele-marker"), 2);
}

#[test]
fn draws_the_stages_of_the_evolution_axis() {
    let out = render(FULL);
    assert!(out.svg.contains(">Genesis<"), "{}", out.svg);
    assert!(out.svg.contains(">Commodity<"), "{}", out.svg);
    // The stages are written under the map.
    assert!(out.svg.contains("pele-wardley-stage-label"), "{}", out.svg);
}

#[test]
fn draws_the_default_stages_when_the_map_names_none() {
    let out = render("wardley-beta\ncomponent A [0.5, 0.5]\n");
    assert!(out.svg.contains(">Genesis<"), "{}", out.svg);
    assert!(out.svg.contains(">Commodity<"), "{}", out.svg);
}

#[test]
fn draws_a_pipeline() {
    let out = render(
        "wardley-beta\ncomponent Kettle [0.45, 0.57]\npipeline Kettle {\n  component Campfire [0.35]\n  component Electric [0.53]\n}\n",
    );
    assert_eq!(count(&out.svg, "<g class=\"pele-wardley-pipeline\""), 1);
    assert_eq!(
        count(
            &out.svg,
            "<g class=\"pele-node pele-wardley-pipeline-component\""
        ),
        2
    );
    assert_eq!(
        count(
            &out.svg,
            "<g class=\"pele-node pele-wardley-pipeline-parent\""
        ),
        1
    );
}

#[test]
fn draws_the_notes_and_the_annotations() {
    let out = render(
        "wardley-beta\ncomponent A [0.5, 0.5]\nnote \"a note\" [0.3, 0.7]\nannotations [0.1, 0.9]\nannotation 1, [0.6, 0.65] \"first\"\n",
    );
    assert!(out.svg.contains(">a note<"), "{}", out.svg);
    assert_eq!(count(&out.svg, "<g class=\"pele-wardley-annotation\""), 1);
    assert!(out.svg.contains(">1. first<"), "{}", out.svg);
}

#[test]
fn draws_the_accelerators_and_the_deaccelerators() {
    let out = render("wardley-beta\naccelerator Trend [0.3, 0.7]\ndeaccelerator Drag [0.6, 0.3]\n");
    assert_eq!(count(&out.svg, "<g class=\"pele-wardley-accelerator\""), 1);
    assert_eq!(
        count(&out.svg, "<g class=\"pele-wardley-deaccelerator\""),
        1
    );
}

#[test]
fn draws_the_evolution_of_a_component() {
    let out = render("wardley-beta\ncomponent Kettle [0.35, 0.43]\nevolve Kettle 0.62\n");
    assert_eq!(count(&out.svg, "<g class=\"pele-wardley-trend\""), 1);
}

#[test]
fn reads_the_config() {
    let small = svg("wardley-beta\ncomponent A [0.5, 0.5]\n");
    let large = svg(
        "---\nconfig:\n  wardley-beta:\n    width: 1200\n    height: 800\n---\nwardley-beta\ncomponent A [0.5, 0.5]\n",
    );
    assert_ne!(small, large);
    // The grid is drawn when the config asks for it.
    let plain = svg("wardley-beta\ncomponent A [0.5, 0.5]\n");
    let grid = svg(
        "---\nconfig:\n  wardley-beta:\n    showGrid: true\n---\nwardley-beta\ncomponent A [0.5, 0.5]\n",
    );
    assert!(!plain.contains("pele-wardley-grid"));
    assert!(grid.contains("pele-wardley-grid"), "{grid}");
}

#[test]
fn draws_a_title() {
    let out = render("---\ntitle: A strategy map\n---\nwardley-beta\ncomponent A [0.5, 0.5]\n");
    assert!(out.svg.contains(">A strategy map<"), "{}", out.svg);
    assert!(out.svg.contains("pele-title"));
}

#[test]
fn draws_the_accessibility_text() {
    let out = render(
        "wardley-beta\naccTitle: The map\naccDescr: Two components and a link\ncomponent A [0.5, 0.5]\n",
    );
    assert!(
        out.svg.contains("<title id=\"pele-title\">The map</title>"),
        "{}",
        out.svg
    );
    assert!(
        out.svg
            .contains("<desc id=\"pele-desc\">Two components and a link</desc>"),
        "{}",
        out.svg
    );
}

#[test]
fn reports_errors_with_positions() {
    // The word of the grammar is not a name.
    let err = pele_rs::render(
        "wardley-beta\ncomponent sizes [0.5, 0.5]\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Wardley));

    // The keyword of the diagram is needed.
    let err = pele_rs::render("hello\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported);
}

#[test]
fn refuses_a_coordinate_that_is_out_of_range() {
    let err = pele_rs::render(
        "wardley-beta\ncomponent A [0.5, 150.5]\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert_eq!(err.kind, Some(DiagramKind::Wardley));
    assert!(
        err.message.contains("must be between 0-1"),
        "{}",
        err.message
    );
}

#[test]
fn refuses_a_pipeline_whose_parent_is_not_there() {
    let err = pele_rs::render(
        "wardley-beta\ncomponent A [0.5, 0.5]\npipeline B {\n  component C [0.3]\n}\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert!(
        err.message.contains("must reference an existing component"),
        "{}",
        err.message
    );
}

#[test]
fn fits_a_narrow_host() {
    let wide = render(FULL);
    let narrow = narrow(FULL);
    // The map gives up the room it can; the labels around the plot keep a little of their own.
    assert!(narrow.width <= 400.0, "{}", narrow.width);
    assert!(
        narrow.width < wide.width,
        "{} vs {}",
        narrow.width,
        wide.width
    );
    assert!(narrow.height > 0.0);
    // The components are still drawn.
    assert_eq!(
        count(&narrow.svg, "<g class=\"pele-node pele-wardley-component\""),
        3
    );
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
            format!("wardley-beta\ncomponent {p} [0.5, 0.5]\n"),
            format!("wardley-beta\ncomponent \"{p}\" [0.5, 0.5]\n"),
            format!("wardley-beta\nanchor {p} [0.5, 0.5]\n"),
            format!("wardley-beta\nnote \"{p}\" [0.5, 0.5]\n"),
            format!("wardley-beta\ntitle {p}\ncomponent A [0.5, 0.5]\n"),
            format!("wardley-beta\naccDescr: {p}\ncomponent A [0.5, 0.5]\n"),
            format!("wardley-beta\ncomponent A [0.5, 0.5]\ncomponent B [0.6, 0.6]\nA -> B ; {p}\n"),
            format!("wardley-beta\ncomponent A [0.5, 0.5]\nannotation 1, [0.6, 0.6] \"{p}\"\n"),
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
        "wardley-beta\n",
        "component ",
        "anchor ",
        "note ",
        "evolution ",
        "evolve ",
        "pipeline ",
        "annotations ",
        "annotation ",
        "accelerator ",
        "deaccelerator ",
        "size ",
        "label ",
        "inertia",
        " [0.5, 0.5]",
        " -> ",
        " -.-> ",
        " +<> ",
        " ; a label",
        " {",
        "}",
        "\n",
        " ",
        "%%",
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
    assert!(rendered > 50, "only {rendered} of 1500 mutations rendered");
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
    const N: usize = 5000;
    let head = "wardley-beta\n";
    let cases = [
        format!("{head}component {} [0.5, 0.5]\n", "x".repeat(N * 10)),
        format!(
            "{head}{}",
            (0..N).fold(String::new(), |mut s, i| {
                let _ = writeln!(s, "component a{i} [0.5, 0.5]");
                s
            })
        ),
        format!(
            "{head}{}",
            (0..N / 3).fold(String::new(), |mut s, i| {
                let _ = writeln!(s, "component a{i} [0.5, 0.5]");
                let _ = writeln!(s, "component b{i} [0.6, 0.6]");
                let _ = writeln!(s, "a{i} -> b{i}");
                s
            })
        ),
        format!(
            "{head}{}",
            (0..N / 4).fold(String::new(), |mut s, i| {
                let _ = writeln!(s, "evolution A{i} -> B{i}");
                let _ = writeln!(s, "component a{i} [0.5, 0.5]");
                s
            })
        ),
        format!(
            "{head}{}",
            (0..N / 2).fold(String::new(), |mut s, i| {
                let _ = writeln!(s, "note \"n{i}\" [0.5, 0.5]");
                s
            })
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
            "stages",
            "wardley-beta\ntitle Strategy\nevolution Genesis -> Custom Built -> Product -> Commodity\ncomponent Cup of Tea [0.79, 0.61]\ncomponent Bag of Flour [0.79, 0.85]\ncomponent Milk [0.62, 0.35]\nanchor User [0.75, 0.63]\nCup of Tea +<> Bag of Flour\nBag of Flour -> Milk\nevolve Milk 0.8\n"
                .to_string(),
        ),
        (
            "pipeline",
            "wardley-beta\ncomponent Kettle [0.45, 0.57]\npipeline Kettle {\n  component Campfire Kettle [0.35] label [-60, 35]\n  component Electric Kettle [0.53]\n}\n"
                .to_string(),
        ),
        (
            "strategies",
            "wardley-beta\ncomponent A [0.5, 0.5] (build)\ncomponent B [0.6, 0.4] (buy)\ncomponent C [0.7, 0.3] (outsource)\ncomponent D [0.8, 0.2] (market)\ncomponent E [0.4, 0.6] inertia\n"
                .to_string(),
        ),
        (
            "notes_and_annotations",
            "wardley-beta\ncomponent A [0.5, 0.5]\nnote \"a note\" [0.3, 0.7]\nannotations [0.1, 0.9]\nannotation 1, [0.6, 0.65] \"first\"\nannotation 2, [0.5, 0.5] \"second\"\naccelerator Trend [0.2, 0.85]\ndeaccelerator Drag [0.4, 0.35]\n"
                .to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
