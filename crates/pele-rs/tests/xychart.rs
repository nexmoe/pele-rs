//! XY chart rendering, security, limits and speed.
#![cfg(feature = "xychart")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "xychart-beta
  title \"An Example Chart\"
  x-axis [\"90d\", \"60d\", \"30d\", \"7d\", \"1d\", \"Current\"]
  y-axis \"Seconds\" 0 --> 198.2
  line \"avg\" [48.1, 41.5, 45.7, 72.8, 67.7, 59.9]
  line \"p50\" [38.2, 36.8, 39.7, 54.5, 49.0, 38.4]
  line \"p95\" [112.2, 75.3, 103.0, 177.0, 180.2, 109.4]
";

const BARS: &str = "xychart
  x-axis [Q1, Q2, Q3, Q4]
  y-axis \"Revenue\" 0 --> 100
  bar [12, 2, 20, 25]
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
fn draws_lines_and_bars() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::XyChart);
    assert_eq!(count(&out.svg, "pele-series pele-line"), 3);
    assert_eq!(count(&out.svg, "<circle "), 18);
    // The title and the axis titles are drawn.
    assert!(out.svg.contains(">An Example Chart<"), "{}", out.svg);
    assert!(out.svg.contains(">Seconds<"), "{}", out.svg);
    // The categories stand under the axis.
    assert!(out.svg.contains(">90d<"), "{}", out.svg);
    // The legend names every series.
    assert!(out.svg.contains("data-id=\"avg\""), "{}", out.svg);
    assert!(out.svg.contains("data-id=\"p95\""), "{}", out.svg);
}

#[test]
fn draws_bars() {
    let out = render(BARS);
    assert_eq!(count(&out.svg, "pele-bars"), 1);
    assert_eq!(count(&out.svg, "<rect "), 4);
    assert!(out.svg.contains(">Q1<"), "{}", out.svg);
    assert!(out.svg.contains(">Revenue<"), "{}", out.svg);
}

#[test]
fn reads_the_orientation() {
    // The orientation stands right after the keyword, as Mermaid's grammar has it.
    let vertical = svg("xychart vertical\nx-axis [a]\nline [1]\n");
    let horizontal = svg("xychart horizontal\nx-axis [a]\nline [1]\n");
    assert_ne!(vertical, horizontal);
    // The horizontal chart draws its grid upright, the upright one across.
    assert!(grid_of(&horizontal).contains('V'), "{horizontal}");
    assert!(grid_of(&vertical).contains('H'), "{vertical}");
}

/// The path of the grid of a chart.
fn grid_of(svg: &str) -> String {
    svg.split("pele-grid\" d=\"")
        .nth(1)
        .unwrap_or("")
        .split('"')
        .next()
        .unwrap_or("")
        .to_string()
}

#[test]
fn reads_the_config() {
    let plain = svg(BARS);
    assert!(!plain.contains("pele-data-labels"), "{plain}");

    let values = svg(&format!(
        "---\nconfig:\n  xyChart:\n    showDataLabel: true\n---\n{BARS}"
    ));
    assert!(values.contains("pele-data-labels"), "{values}");
    assert!(values.contains(">25<"), "{values}");

    // The size of the chart is read from the configuration.
    let sized = render(&format!(
        "---\nconfig:\n  xyChart:\n    width: 900\n    height: 600\n---\n{BARS}"
    ));
    assert_eq!(sized.width, 900.0);
    assert_eq!(sized.height, 600.0);

    // A size that is out of range is cut down to it.
    let clamped = render(&format!(
        "---\nconfig:\n  xyChart:\n    width: 9000\n---\n{BARS}"
    ));
    assert_eq!(clamped.width, 4000.0);

    // The legend can be turned off.
    let no_legend = svg(&format!(
        "---\nconfig:\n  xyChart:\n    showLegend: false\n---\n{BARS}"
    ));
    assert!(!no_legend.contains("pele-legend"), "{no_legend}");

    // The orientation can come from the configuration.
    let turned = svg(&format!(
        "---\nconfig:\n  xyChart:\n    chartOrientation: horizontal\n---\n{BARS}"
    ));
    assert_ne!(turned, plain);
}

#[test]
fn draws_a_title() {
    let out = render("---\ntitle: Energy use\n---\nxychart\nx-axis [a]\nline [1]\n");
    assert!(out.svg.contains(">Energy use<"), "{}", out.svg);
    assert!(out.svg.contains("pele-title"));
}

#[test]
fn reads_the_accessible_text() {
    let out =
        render("xychart\naccTitle: Sales\naccDescr: Revenue for Q1 to Q4\nx-axis [a]\nline [1]\n");
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
    let err = pele_rs::render("xychart\nx-axis [a\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::XyChart));
    assert_eq!(err.line, 2);
    assert!(err.message.contains("SQUARE_BRACES_END"), "{}", err.message);

    // A chart without data is nothing to draw.
    let err = pele_rs::render("xychart\nx-axis [a]\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert!(err.message.contains("No Plot to render"), "{}", err.message);

    // The keyword is needed.
    let err = pele_rs::render("hello\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported);
    assert!(
        err.message.contains("No diagram type detected"),
        "{}",
        err.message
    );
}

#[test]
fn fits_a_narrow_host() {
    let wide = render(FULL);
    let narrow = narrow(FULL);
    assert!(narrow.width <= 360.0, "{}", narrow.width);
    assert!(narrow.height > 0.0);
    // The chart is still drawn.
    assert_eq!(count(&narrow.svg, "pele-series pele-line"), 3);
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
            format!("xychart\nx-axis [a]\nline [1]\ntitle \"{p}\"\n"),
            format!("xychart\nx-axis [\"{p}\"]\nline [1]\n"),
            format!("xychart\nx-axis [a]\nline [1 \"{p}\"]\n"),
            format!("xychart\naccTitle: {p}\nx-axis [a]\nline [1]\n"),
            format!("xychart\naccDescr {{ {p} }}\nx-axis [a]\nline [1]\n"),
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
        "xychart\n",
        "xychart-beta\n",
        "x-axis ",
        "y-axis ",
        "line ",
        "bar ",
        "[",
        "]",
        ",",
        "\"",
        "-->",
        "1",
        "-1",
        "1.5",
        ".5",
        "\n",
        " ",
        "title ",
        "accTitle: ",
        "accDescr {",
        "}",
    ]);
    let corpus = [FULL, BARS];
    let mut rnd = Random::new(13);
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
    let head = "xychart\n";
    let cases = [
        format!("{head}x-axis [{}]\nline [1]\n", "a".repeat(N)),
        format!("{head}x-axis [a]\nline [{}1]\n", "1,".repeat(N / 4)),
        format!("{head}x-axis [a]\nline [1]\ntitle {}\n", "x".repeat(N)),
        format!("{head}x-axis [a]\nline [1 \"{}\"]\n", "x".repeat(N)),
        format!("{head}x-axis [a]\nline [1]\n{}", "line [1]\n".repeat(N / 8)),
        format!("{head}x-axis [{}]\nline [1]\n", "a,".repeat(N / 2)),
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
        ("bars", BARS.to_string()),
        (
            "horizontal",
            "xychart horizontal\nx-axis [a, b]\ny-axis \"v\" 0 --> 10\nline [1, 2]\n".to_string(),
        ),
        (
            "values",
            format!("---\nconfig:\n  xyChart:\n    showDataLabel: true\n---\n{BARS}"),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
