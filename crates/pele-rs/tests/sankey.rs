//! Sankey chart rendering, security, limits and speed.
#![cfg(feature = "sankey")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "sankey-beta
Agricultural waste,Bio-conversion,124.729
Bio-conversion,Liquid,0.597
Bio-conversion,Losses,26.862
Bio-conversion,Solid,280.322
Bio-conversion,Gas,81.144
Coal,Solid,75.571
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
fn draws_nodes_and_bands() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Sankey);
    assert_eq!(count(&out.svg, "<rect class=\"pele-sankey-node\""), 7);
    assert_eq!(count(&out.svg, "<path class=\"pele-sankey-link\""), 6);
    assert_eq!(count(&out.svg, "<g class=\"pele-label\""), 7);
    // The values are shown next to the names.
    assert!(
        out.svg.contains(">Agricultural waste <tspan"),
        "{}",
        out.svg
    );
    assert!(out.svg.contains(">124.73</tspan>"), "{}", out.svg);
    // Every band carries the flow it stands for.
    assert!(
        out.svg
            .contains("<title>Bio-conversion → Liquid: 0.6</title>")
    );
}

#[test]
fn reads_quoted_fields() {
    let out = render("sankey\n\"a, b\",\"c\"\"d\",1\n");
    assert_eq!(count(&out.svg, "<rect class=\"pele-sankey-node\""), 2);
    assert!(out.svg.contains(">a, b <tspan"), "{}", out.svg);
    // The doubled quote of a quoted field is one quote.
    assert!(out.svg.contains("data-id=\"c&quot;d\""), "{}", out.svg);
}

#[test]
fn reads_the_config() {
    let plain = svg("sankey\na,b,1\n");
    let no_values = svg("---\nconfig:\n  sankey:\n    showValues: false\n---\nsankey\na,b,1\n");
    assert!(plain.contains("pele-sankey-value"));
    assert!(!no_values.contains("pele-sankey-value"));

    let prefixed =
        svg("---\nconfig:\n  sankey:\n    prefix: '$'\n    suffix: ' kWh'\n---\nsankey\na,b,1\n");
    assert!(prefixed.contains(">$1 kWh<"), "{prefixed}");

    // The alignment puts the nodes in different columns.
    let justify = svg("sankey\na,b,1\nb,c,2\nc,d,3\ne,f,4\n");
    let left = svg(
        "---\nconfig:\n  sankey:\n    nodeAlignment: left\n---\nsankey\na,b,1\nb,c,2\nc,d,3\ne,f,4\n",
    );
    assert_ne!(justify, left);

    // A colour for a node overrides the series colour.
    let colored =
        svg("---\nconfig:\n  sankey:\n    nodeColors:\n      a: \"#123456\"\n---\nsankey\na,b,1\n");
    assert!(colored.contains("#123456"), "{colored}");

    // The links can be painted in one colour.
    let fixed = svg("---\nconfig:\n  sankey:\n    linkColor: \"#abcdef\"\n---\nsankey\na,b,1\n");
    assert!(fixed.contains("#abcdef"), "{fixed}");
}

#[test]
fn draws_a_title() {
    let out = render("---\ntitle: Energy flows\n---\nsankey\na,b,1\n");
    assert!(out.svg.contains(">Energy flows<"), "{}", out.svg);
    assert!(out.svg.contains("pele-title"));
}

#[test]
fn reports_errors_with_positions() {
    let err = pele_rs::render("sankey\na,b\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Sankey));
    assert_eq!(err.line, 2);
    assert!(err.message.contains("COMMA"), "{}", err.message);

    let err = pele_rs::render("sankey\na,,1\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert!(err.message.contains("DQUOTE"), "{}", err.message);

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
fn reads_unicode_names_like_pele() {
    let out = render("sankey\nCafé,Thé,1\n");
    assert!(out.svg.contains("Café"));
    assert!(out.svg.contains("Thé"));
}

#[test]
fn refuses_circular_links() {
    let err = pele_rs::render("sankey\na,b,1\nb,a,2\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert!(err.message.contains("circular links"), "{}", err.message);
}

#[test]
fn fits_a_narrow_host() {
    let wide = render(FULL);
    let narrow = narrow(FULL);
    assert!(narrow.width <= 360.0, "{}", narrow.width);
    assert!(narrow.height > 0.0);
    // The chart is still drawn.
    assert_eq!(count(&narrow.svg, "<rect class=\"pele-sankey-node\""), 7);
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
            format!("sankey\n{p},b,1\n"),
            format!("sankey\na,{p},1\n"),
            format!("sankey\na,b,{p}\n"),
            format!("sankey\n\"{p}\",b,1\n"),
            format!("---\ntitle: \"{p}\"\n---\nsankey\na,b,1\n"),
            format!(
                "---\nconfig:\n  sankey:\n    prefix: \"{p}\"\n    nodeColors:\n      a: \"{p}\"\n---\nsankey\na,b,1\n"
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
    fragments.extend([
        "sankey\n",
        "sankey-beta\n",
        ",",
        "\"",
        "\"\"",
        "\n",
        " ",
        "1",
        "-1",
        "1.5",
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
    assert!(rendered > 400, "only {rendered} of 1500 mutations rendered");
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
    let head = "sankey\n";
    let cases = [
        format!("{head}{},b,1\n", "x".repeat(N)),
        format!("{head}{}", "a,b,1\n".repeat(N / 8)),
        format!("{head}a,b,{}\n", "9".repeat(N)),
        format!("{head}\"{}\",b,1\n", "x".repeat(N)),
        format!("{head}a,b,1\n{}", "a,b,1\n".repeat(N / 16)),
        format!("{head}{}", "x,b,1\n".repeat(N / 8)),
        format!("{head}{}", "a,b,1\n".repeat(N / 4)),
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
        ("quoted", "sankey\n\"a, b\",\"c\"\"d\",1\n".to_string()),
        (
            "values_off",
            "---\nconfig:\n  sankey:\n    showValues: false\n---\nsankey\na,b,1\n".to_string(),
        ),
        (
            "outlined",
            "---\nconfig:\n  sankey:\n    labelStyle: outlined\n---\nsankey\na,b,1\nb,c,2\n"
                .to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
