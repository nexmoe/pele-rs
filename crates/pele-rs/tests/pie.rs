//! Pie chart rendering, security and speed. Ported from pele's `tests/render/pie.test.ts`.
#![cfg(feature = "pie")]

mod support;

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use pele_rs::{
    DiagramKind, ErrorCode, Limits, MetricsMeasurer, RenderOptions, Style, TextMeasurer,
};
use support::{PAYLOADS, Random, assert_inert, mutate};

struct Case {
    #[allow(dead_code)]
    file: &'static str,
    #[allow(dead_code)]
    name: &'static str,
    src: &'static str,
    #[allow(dead_code)]
    todo: bool,
}

include!("compat/pie/cases.rs");

fn svg(src: &str) -> String {
    render(src).svg
}

fn render(src: &str) -> pele_rs::Rendered {
    pele_rs::render(src, &RenderOptions::default()).unwrap_or_else(|e| panic!("{src:?}: {e}"))
}

fn count(svg: &str, needle: &str) -> usize {
    svg.matches(needle).count()
}

#[test]
fn draws_a_slice_a_percentage_and_a_legend_entry_per_section() {
    let out = render("pie title Pets\n \"Dogs\" : 386\n \"Cats\" : 85\n \"Rats\" : 15");
    assert_eq!(out.kind, DiagramKind::Pie);
    assert_eq!(
        pele_rs::detect_type("pie\n \"a\": 1"),
        Some(DiagramKind::Pie)
    );
    assert_eq!(count(&out.svg, "class=\"pele-rs-slice\""), 3);
    assert_eq!(count(&out.svg, "class=\"pele-rs-legend-item\""), 3);
    assert!(out.svg.contains(">79%<"));
    assert!(out.svg.contains(">Pets<"));
    assert!(out.svg.contains("var(--pele-rs-series-1,"));
}

#[test]
fn shows_values_in_the_legend_with_show_data() {
    assert!(svg("pie showData\n \"A\" : 42.96\n \"B\" : 50").contains("A [42.96]"));
}

#[test]
fn leaves_out_slices_under_one_percent_but_keeps_them_in_the_legend() {
    let svg = svg("pie\n \"Big\" : 1000\n \"Tiny\" : 1");
    assert_eq!(count(&svg, "class=\"pele-rs-slice\""), 1);
    assert_eq!(count(&svg, "class=\"pele-rs-legend-item\""), 2);
}

#[test]
fn draws_a_single_full_slice_and_an_empty_chart() {
    assert!(svg("pie\n \"Only\" : 5").contains(">100%<"));
    assert!(!svg("pie\n \"Zero\" : 0").contains("NaN"));
    assert!(svg("pie").starts_with("<svg"));
}

#[test]
fn honours_donut_hole_text_position_and_legend_position() {
    let plain = render("pie\n \"A\" : 1\n \"B\" : 1");
    let donut =
        render("---\nconfig:\n  pie:\n    donutHole: 0.5\n---\npie\n \"A\" : 1\n \"B\" : 1");
    let below = render(
        "---\nconfig:\n  pie:\n    legendPosition: bottom\n---\npie\n \"A\" : 1\n \"B\" : 1",
    );
    let inside =
        render("%%{init: {\"pie\": {\"textPosition\": 0.3}}}%%\npie\n \"A\" : 1\n \"B\" : 1");
    assert_ne!(donut.svg, plain.svg);
    assert_ne!(inside.svg, plain.svg);
    assert!(below.height > plain.height);
    assert!(below.width < plain.width);
}

#[test]
fn moves_the_labels_of_thin_slices_outside_the_rim_apart() {
    let svg =
        svg("pie\n \"Calcium\" : 2\n \"Potassium\" : 2.4\n \"Magnesium\" : 46\n \"Test\" : 50");
    let group = svg
        .split("<g class=\"pele-rs-slice-labels\"")
        .skip(1)
        .find(|g| !g.split('>').next().unwrap_or("").contains("fill="))
        .expect("labels outside the rim");
    let size: f64 = group
        .split("font-size=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .and_then(|s| s.parse().ok())
        .unwrap();
    let body = &group[..group.find("</g>").unwrap()];
    let boxes: Vec<(f64, f64, f64)> = body
        .split("<text x=\"")
        .skip(1)
        .map(|t| {
            let (x, rest) = t.split_once("\" y=\"").unwrap();
            let (y, rest) = rest.split_once("\">").unwrap();
            let text = &rest[..rest.find('<').unwrap()];
            (
                x.parse().unwrap(),
                y.parse().unwrap(),
                MetricsMeasurer.width(text, size, Style::NONE),
            )
        })
        .collect();
    let [a, b] = boxes[..] else {
        panic!("expected two labels outside, found {boxes:?}");
    };
    assert!((a.0 - b.0).abs() >= (a.2 + b.2) / 2.0 || (a.1 - b.1).abs() >= size);
}

#[test]
fn takes_the_title_from_front_matter_unless_the_chart_sets_one() {
    assert!(
        svg("---\ntitle: From front matter\n---\npie\n \"A\" : 1").contains("From front matter")
    );
    assert!(svg("---\ntitle: From front matter\n---\npie title Own\n \"A\" : 1").contains(">Own<"));
}

#[test]
fn rejects_negative_values_with_mermaids_message() {
    let err = pele_rs::render("pie\n \"dogs\" : -60.67", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert_eq!(err.kind, Some(DiagramKind::Pie));
    assert_eq!(
        err.message,
        "\"dogs\" has invalid value: -60.67. Negative values are not allowed in pie charts. \
         All slice values must be >= 0."
    );
}

#[test]
fn keeps_the_first_value_of_a_repeated_label() {
    let svg = svg("pie showData\n \"A\" : 1\n \"A\" : 2\n \"B\" : 3");
    assert_eq!(count(&svg, "class=\"pele-rs-legend-item\""), 2);
    assert!(svg.contains(">A [1]<"));
}

#[test]
fn reports_syntax_errors_with_a_position() {
    let err = pele_rs::render("pie\n \"a\" 1\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!((err.line, err.column), (2, 6));
    assert_eq!(err.snippet, " \"a\" 1\n-----^");
}

#[test]
fn fits_a_narrow_host() {
    let src = "pie title Pets\n  \"Dogs\" : 386\n  \"Cats\" : 85\n  \"Rats\" : 15";
    let wide = render(src);
    let options = RenderOptions {
        max_width: Some(320.0),
        ..RenderOptions::default()
    };
    let narrow = pele_rs::render(src, &options).unwrap();
    assert!(wide.width > 320.0);
    assert!(narrow.width <= 320.0);
    assert!(
        narrow.height > wide.height,
        "the legend moves below the chart"
    );
}

#[test]
fn output_is_deterministic() {
    let src = "pie showData title T\n \"a\" : 1\n \"b\" : 2.5\n \"c\" : 0.1";
    assert_eq!(svg(src), svg(src));
}

/// `n` lines of `"s{i}" : {value(i)}`.
fn sections(n: usize, value: impl Fn(usize) -> usize) -> String {
    let mut out = String::new();
    for i in 0..n {
        let _ = writeln!(out, "\"s{i}\" : {}", value(i));
    }
    out
}

fn try_render(src: &str) -> Option<String> {
    pele_rs::render(src, &RenderOptions::default())
        .ok()
        .map(|r| r.svg)
}

#[test]
fn is_inert_for_corpus_hostile_and_mutated_inputs() {
    let corpus: Vec<&str> = CASES
        .iter()
        .map(|c| c.src)
        .filter(|s| s.contains("pie"))
        .collect();
    for src in &corpus {
        if let Some(svg) = try_render(src) {
            assert_inert(&svg, src);
        }
    }
    let mut rendered = 0;
    for payload in PAYLOADS {
        for src in [
            format!("pie title {payload}\n \"a\" : 1"),
            format!("pie\n \"{payload}\" : 1\n \"b\" : 2"),
            format!("pie\n accTitle: {payload}\n accDescr: {payload}\n \"a\" : 1"),
            format!("pie\n accDescr {{\n {payload}\n }}\n \"a\" : 1"),
            format!(
                "---\ntitle: \"{payload}\"\nconfig:\n  pie:\n    legendPosition: \"{payload}\"\n    \
                 textPosition: \"{payload}\"\n---\npie\n \"a\" : 1"
            ),
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
    fragments.extend(["\"", ":", "\n", "title ", "showData"]);
    let mut rnd = Random::new(5);
    for _ in 0..2000 {
        let src = mutate(&corpus, &fragments, &mut rnd);
        if let Some(svg) = try_render(&src) {
            assert_inert(&svg, &src);
        }
    }
}

#[test]
fn escapes_text_everywhere_it_is_written() {
    let svg = svg(
        "pie title <b>T</b> & \"q\"\n accTitle: <t>\n accDescr: a&b\n \"<s>\" : 1\n \"x\\\"y\" : 2",
    );
    assert!(!svg.contains("<s>") && !svg.contains("<t>") && !svg.contains("<b>"));
    assert!(svg.contains("data-id=\"&lt;s&gt;\""));
    assert!(svg.contains("data-id=\"x&quot;y\""));
    assert!(svg.contains("<title id=\"pele-rs-title\">&lt;t&gt;</title>"));
    assert!(svg.contains("<desc id=\"pele-rs-desc\">a&amp;b</desc>"));
}

#[test]
fn enforces_the_output_limit() {
    let src = format!("pie\n{}", sections(200, |i| i + 1));
    let options = RenderOptions {
        limits: Limits {
            output: 10_000,
            ..Limits::default()
        },
        ..RenderOptions::default()
    };
    assert_eq!(
        pele_rs::render(&src, &options).unwrap_err().code,
        ErrorCode::Limit
    );
}

fn assert_fast(src: &str, options: &RenderOptions, budget: Duration) {
    let started = Instant::now();
    let _ = pele_rs::render(src, options);
    let took = started.elapsed();
    assert!(
        took < budget,
        "{:?} took {took:?}",
        &src[..src.len().min(30)]
    );
}

#[test]
fn stays_fast_on_oversized_input() {
    let options = RenderOptions {
        limits: Limits {
            source: usize::MAX,
            ..Limits::default()
        },
        ..RenderOptions::default()
    };
    let cases = [
        format!("pie\n{}", sections(5000, |i| i)),
        format!("pie title {}", "x".repeat(50000)),
        format!("pie\n\"{}", "a".repeat(50000)),
        format!("pie\n{}", "%%{\n".repeat(12000)),
        format!("pie\n{}", "x ---\n".repeat(8000)),
        format!("pie\naccDescr {{{}", " a\n".repeat(15000)),
        format!("pie\n{}\"a\":1", " ".repeat(50000)),
        format!("pie\n{}", "---\n".repeat(12000)),
        format!("pie\n{}", "accDescr\n".repeat(5000)),
    ];
    for src in &cases {
        assert_fast(src, &options, Duration::from_secs(2));
    }
}

#[test]
fn reads_a_long_run_of_digits_in_linear_time() {
    for tail in [
        "0".repeat(49000),
        "0".repeat(49000) + ".",
        "9".repeat(49000) + "..",
        format!("-{}", "1.".repeat(24000)),
    ] {
        let src = format!("pie\n\"a\": {tail}");
        let started = Instant::now();
        let err = pele_rs::render(&src, &RenderOptions::default()).unwrap_err();
        assert_eq!(err.code, ErrorCode::Syntax);
        assert!(started.elapsed() < Duration::from_millis(500));
    }
}

#[test]
fn snapshots() {
    let cases = [
        (
            "pets",
            "pie title Pets adopted by volunteers\n \"Dogs\" : 386\n \"Cats\" : 85\n \"Rats\" : 15",
        ),
        (
            "show_data_and_accessibility",
            "pie showData\n accTitle: Key elements\n accDescr: Elements in the Earth's crust\n \
             \"Calcium\" : 42.96\n \"Potassium\" : 50.05\n \"Magnesium\" : 10.01\n \"Iron\" : 5",
        ),
        (
            "donut_legend_bottom",
            "---\nconfig:\n  pie:\n    donutHole: 0.5\n    legendPosition: bottom\n---\npie\n \
             \"A\" : 1\n \"B\" : 2\n \"C\" : 3",
        ),
        ("empty", "pie"),
        ("single", "pie title Only one\n \"Only\" : 5"),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(src));
    }
}
