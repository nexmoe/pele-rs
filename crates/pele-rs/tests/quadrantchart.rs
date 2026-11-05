//! Quadrant chart rendering, security, limits and speed.
#![cfg(feature = "quadrantchart")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "quadrantChart
    title Reach and engagement of campaigns
    x-axis Low Reach --> High Reach
    y-axis Low Engagement --> High Engagement
    quadrant-1 We should expand
    quadrant-2 Need to promote
    quadrant-3 Re-evaluate
    quadrant-4 May be improved
    Campaign A: [0.3, 0.6]
    Campaign B: [0.45, 0.23]
    Campaign C: [0.57, 0.69]
    Campaign D: [0.78, 0.34]
    Campaign E: [0.40, 0.34]
    Campaign F: [0.35, 0.78]
";

const STYLED: &str = "quadrantChart
    title Styled
    x-axis Low Reach --> High Reach
    y-axis Low Engagement --> High Engagement
    quadrant-1 We should expand
    quadrant-2 Need to promote
    quadrant-3 Re-evaluate
    quadrant-4 May be improved
    Campaign A: [0.9, 0.0] radius: 12
    Campaign B:::class1: [0.8, 0.1] color: #ff3300, radius: 10
    Campaign C: [0.7, 0.2] radius: 25, color: #00ff33, stroke-color: #10f0f0
    Campaign D: [0.6, 0.3] radius: 15, stroke-color: #00ff0f, stroke-width: 5px ,color: #ff33f0
    Campaign E:::class2: [0.5, 0.4]
    Campaign F:::class3: [0.4, 0.5] color: #0000ff
    classDef class1 color: #109060
    classDef class2 color: #908342, radius : 10, stroke-color: #310085, stroke-width: 10px
    classDef class3 color: #f00fff, radius : 10
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
fn draws_the_boxes_and_the_axes() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::QuadrantChart);
    assert_eq!(count(&out.svg, "pele-quadrant\""), 4);
    assert_eq!(count(&out.svg, "<rect "), 4);
    assert_eq!(count(&out.svg, "<circle "), 6);
    assert!(out.svg.contains(">We should expand<"), "{}", out.svg);
    assert!(out.svg.contains(">Low Reach<"), "{}", out.svg);
    assert!(out.svg.contains(">High Engagement<"), "{}", out.svg);
    assert!(
        out.svg.contains(">Reach and engagement of campaigns<"),
        "{}",
        out.svg
    );
    // The first quadrant is the top right one.
    let first = out.svg.find("data-id=\"1\"").unwrap();
    let second = out.svg.find("data-id=\"2\"").unwrap();
    let x_of = |at: usize| -> f64 {
        let rest = &out.svg[at..];
        let x = rest.find("x=\"").unwrap() + 3;
        rest[x..].split('"').next().unwrap().parse().unwrap()
    };
    assert!(
        x_of(first) > x_of(second),
        "the first quadrant is on the right"
    );
}

#[test]
fn reads_the_styles_of_the_points() {
    let out = render(STYLED);
    assert_eq!(count(&out.svg, "<circle "), 6);
    assert!(out.svg.contains("r=\"12\""), "{}", out.svg);
    assert!(out.svg.contains("fill:#ff3300"), "{}", out.svg);
    // A stroke color is only drawn when there is a stroke width to draw it with.
    assert!(out.svg.contains("stroke:#00ff0f"), "{}", out.svg);
    assert!(out.svg.contains("stroke-width:5px"), "{}", out.svg);
    assert!(!out.svg.contains("stroke:#10f0f0"), "{}", out.svg);
    // The class of a point is written in its class list.
    assert!(out.svg.contains("pele-point class1"), "{}", out.svg);
}

#[test]
fn reads_the_config() {
    let plain = svg(FULL);
    assert!(!plain.is_empty());

    // The size of the chart is read from the configuration.
    let sized = render(&format!(
        "---\nconfig:\n  quadrantChart:\n    chartWidth: 900\n    chartHeight: 600\n---\n{FULL}"
    ));
    assert_eq!(sized.width, 900.0);
    assert_eq!(sized.height, 600.0);

    // A size that is out of range is cut down to it.
    let clamped = render(&format!(
        "---\nconfig:\n  quadrantChart:\n    chartWidth: 9000\n---\n{FULL}"
    ));
    assert_eq!(clamped.width, 4000.0);

    // The axes may be put on other sides.
    let moved = svg(&format!(
        "---\nconfig:\n  quadrantChart:\n    xAxisPosition: bottom\n    yAxisPosition: right\n---\n{FULL}"
    ));
    assert_ne!(moved, plain);

    // The radius of the points is read from the configuration.
    let big = svg(&format!(
        "---\nconfig:\n  quadrantChart:\n    pointRadius: 20\n---\n{FULL}"
    ));
    assert!(big.contains("r=\"20\""), "{big}");
}

#[test]
fn reads_the_accessible_text() {
    let out =
        render("quadrantChart\naccTitle: Sales\naccDescr: Revenue for Q1 to Q4\nquadrant-1 A\n");
    assert!(
        out.svg.contains("<title id=\"pele-title\">Sales</title>"),
        "{}",
        out.svg
    );
    assert!(
        out.svg
            .contains("<desc id=\"pele-desc\">Revenue for Q1 to Q4</desc>"),
        "{}",
        out.svg
    );
}

#[test]
fn reports_errors_with_positions() {
    let err = pele_rs::render(
        "quadrantChart\nquadrant-1 A\n???\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::QuadrantChart));

    // A style that is not known is refused.
    let err = pele_rs::render(
        "quadrantChart\nclassDef a fill: #ff0000\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert!(err.message.contains("style named fill"), "{}", err.message);

    // A color that is not a hex code is refused.
    let err = pele_rs::render(
        "quadrantChart\nCampaign A: [0.5, 0.5] color: red\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert!(err.message.contains("valid hex code"), "{}", err.message);

    // The keyword is needed.
    let err = pele_rs::render("hello\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported);
}

#[test]
fn fits_a_narrow_host() {
    let narrow = narrow(FULL);
    assert!(narrow.width <= 360.0, "{}", narrow.width);
    assert!(narrow.height > 0.0);
    assert_eq!(count(&narrow.svg, "pele-quadrant\""), 4);
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
            format!("quadrantChart\ntitle \"{p}\"\nquadrant-1 A\n"),
            format!("quadrantChart\nx-axis {p} --> High\nquadrant-1 A\n"),
            format!("quadrantChart\nquadrant-1 {p}\n"),
            format!("quadrantChart\n{p}: [0.5, 0.5]\n"),
            format!("quadrantChart\naccTitle: {p}\nquadrant-1 A\n"),
            format!("quadrantChart\naccDescr {{ {p} }}\nquadrant-1 A\n"),
            format!("quadrantChart\nclassDef a color: {p}\n"),
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
        "quadrantChart\n",
        "quadrant-1 ",
        "quadrant-2 ",
        "x-axis ",
        "y-axis ",
        "-->",
        "[",
        "]",
        ",",
        ":",
        "\"",
        "0",
        "1",
        "0.5",
        "title ",
        "classDef ",
        ":::",
        "radius: 10",
        "color: #ff0000",
        "\n",
        " ",
        ";",
    ]);
    let corpus = [FULL, STYLED];
    let mut rnd = Random::new(29);
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
    let head = "quadrantChart\n";
    let cases = [
        format!("{head}quadrant-1 {}\n", "a".repeat(N)),
        format!(
            "{head}Campaign A: [0.5, 0.5]\n{}",
            "Campaign B: [0.5, 0.5]\n".repeat(N / 25)
        ),
        format!("{head}title {}\nquadrant-1 A\n", "x".repeat(N)),
        format!("{head}Campaign A:::{}: [0.5, 0.5]\n", "c".repeat(N)),
        format!("{head}{}", "quadrant-1 A\n".repeat(N / 14)),
        format!("{head}Campaign A: [0.{}, 0.5]\n", "5".repeat(N / 2)),
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
        ("styled", STYLED.to_string()),
        (
            "moved",
            format!(
                "---\nconfig:\n  quadrantChart:\n    xAxisPosition: bottom\n    \
                 yAxisPosition: right\n---\n{FULL}"
            ),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
