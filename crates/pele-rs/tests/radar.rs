//! Radar chart rendering, security, limits and speed.
#![cfg(feature = "radar")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "radar-beta
    axis A[\"Axis A\"], B[\"Axis B\"], C[\"Axis C\"]
    curve first{1,2,3}
    curve second[\"Second\"]{3,2,1}
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
fn draws_a_radar_chart() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Radar);
    // The grid of the chart, the spokes of the axes and the two curves.
    assert_eq!(count(&out.svg, "<g class=\"pele-graticule\""), 1);
    assert_eq!(count(&out.svg, "class=\"pele-axes\""), 1);
    assert_eq!(count(&out.svg, "class=\"pele-curve\""), 2);
    // The labels of the axes.
    assert_eq!(count(&out.svg, "class=\"pele-axis-label\""), 3);
    assert!(out.svg.contains(">Axis A<"), "{}", out.svg);
    assert!(out.svg.contains(">Axis C<"), "{}", out.svg);
    // The legend holds the two curves.
    assert_eq!(count(&out.svg, "class=\"pele-legend-item\""), 2);
}

#[test]
fn reads_the_options() {
    // The number of rings of the grid.
    let few = svg("radar-beta\naxis A\ncurve c{1}\nticks 2\n");
    let many = svg("radar-beta\naxis A\ncurve c{1}\nticks 6\n");
    assert_ne!(few, many);
    assert_eq!(count(&few, "<circle r="), 2);
    assert_eq!(count(&many, "<circle r="), 6);

    // The grid may be a polygon rather than circles.
    let circles = svg("radar-beta\naxis A,B\ncurve c{1,2}\n");
    let polygon = svg("radar-beta\naxis A,B\ncurve c{1,2}\ngraticule polygon\n");
    assert!(circles.contains("<circle r="), "{}", circles);
    assert!(polygon.contains("<polygon points="), "{}", polygon);

    // The legend may be turned off.
    let with = svg("radar-beta\naxis A\ncurve c{1}\n");
    let without = svg("radar-beta\naxis A\ncurve c{1}\nshowLegend false\n");
    assert!(with.contains("pele-legend-item"), "{}", with);
    assert!(!without.contains("pele-legend-item"), "{}", without);
}

#[test]
fn reads_the_values_of_the_axes() {
    // The values of a curve are either in the order of the axes, or named.
    let plain = svg("radar-beta\naxis A,B\ncurve c{1,2}\n");
    let named = svg("radar-beta\naxis A,B\ncurve c{B: 2, A: 1}\n");
    // The named values are put in the order of the axes, so both charts are the same.
    assert_eq!(plain, named);

    // The range of the chart is what the values are drawn against.
    let wide = svg("radar-beta\naxis A\ncurve c{50}\nmax 100\n");
    let narrow = svg("radar-beta\naxis A\ncurve c{50}\nmax 10\n");
    assert_ne!(wide, narrow);
}

#[test]
fn draws_a_title_and_the_accessible_text() {
    let out = render("---\ntitle: A radar chart\n---\nradar-beta\naxis A\ncurve c{1}\n");
    assert!(out.svg.contains(">A radar chart<"), "{}", out.svg);
    assert!(out.svg.contains("pele-title"));

    let out = render(
        "radar-beta\naccTitle: A radar chart\naccDescr: A chart of one axis\naxis A\ncurve c{1}\n",
    );
    assert!(
        out.svg
            .contains("<title id=\"pele-title\">A radar chart</title>"),
        "{}",
        out.svg
    );
    assert!(
        out.svg
            .contains("<desc id=\"pele-desc\">A chart of one axis</desc>"),
        "{}",
        out.svg
    );
}

#[test]
fn reports_errors_with_positions() {
    // A curve without values.
    let err =
        pele_rs::render("radar-beta\naxis A\ncurve c\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Radar));
    assert_eq!(err.line, 3);

    // A curve that names an axis that is not there.
    let err = pele_rs::render(
        "radar-beta\naxis A\ncurve c{B: 1}\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert_eq!(err.kind, Some(DiagramKind::Radar));

    // A curve that names its axes before there are any.
    let err =
        pele_rs::render("radar-beta\ncurve c{A: 1}\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);

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
fn fits_a_narrow_host() {
    let wide = render(FULL);
    let narrow = narrow(FULL);
    assert!(narrow.width <= 360.0, "{}", narrow.width);
    assert!(narrow.height > 0.0);
    // The chart is still drawn.
    assert_eq!(count(&narrow.svg, "class=\"pele-curve\""), 2);
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
            format!("radar-beta\naxis A[\"{p}\"]\ncurve c{{1}}\n"),
            format!("radar-beta\naxis A\ncurve c[\"{p}\"]{{1}}\n"),
            format!("radar-beta\ntitle {p}\naxis A\ncurve c{{1}}\n"),
            format!("radar-beta\naccDescr: {p}\naxis A\ncurve c{{1}}\n"),
            format!("radar-beta\naxis A\ncurve c{{1}}\nshowLegend {p}\n"),
            format!("radar-beta\naxis A\ncurve c{{1}}\ngraticule {p}\n"),
            format!("radar-beta\naxis A\ncurve c{{1}}\nticks {p}\n"),
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
        "radar-beta\n",
        "radar-beta:",
        "radar-beta :",
        "axis",
        "axis A",
        "axis A,B",
        "axis A[\"L\"]",
        "curve",
        "curve c",
        "curve c{1}",
        "curve c{1,2}",
        "curve c{A: 1}",
        "curve c{A: 1, B: 2}",
        "ticks",
        "ticks 5",
        "min 0",
        "max 10",
        "showLegend",
        "showLegend true",
        "graticule",
        "graticule circle",
        "graticule polygon",
        "{",
        "}",
        "[",
        "]",
        ",",
        ":",
        "\"",
        "'",
        "1",
        "1.5",
        "A",
        "\n",
        " ",
        "%%",
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
    let head = "radar-beta\n";
    let cases = [
        format!("{head}axis A\ncurve c{{{}}}\n", "1,".repeat(N / 2)),
        format!(
            "{head}axis A\ncurve c{{1}}\n{}",
            "curve c{1}\n".repeat(N / 10)
        ),
        format!("{head}{}", "axis A\n".repeat(N / 10)),
        format!("{head}axis A\ncurve c{{{}}}\n", "0".repeat(N)),
        format!("{head}axis A\ncurve c{{1}}\nticks {}\n", "9".repeat(N)),
        format!("{head}accDescr: {}\naxis A\ncurve c{{1}}\n", "x".repeat(N)),
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
            "options",
            "radar-beta\naxis A,B,C\ncurve c{1,2,3}\nticks 3\ngraticule polygon\nmax 5\n"
                .to_string(),
        ),
        (
            "named",
            "radar-beta\naxis A[\"Axis A\"], B\ncurve c{B: 2, A: 1}\n".to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
