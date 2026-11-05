//! Treemap rendering, security, limits and speed.
#![cfg(feature = "treemap")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "treemap
\"Category A\"
    \"Item A1\": 10
    \"Item A2\": 20
\"Category B\"
    \"Item B1\": 15
    \"Item B2\": 25
";

const NESTED: &str = "treemap
\"Products\"
    \"Electronics\"
        \"Phones\": 50
        \"Computers\": 30
    \"Clothing\"
        \"Men's\": 40
        \"Women's\": 40
";

const STYLED: &str = "treemap
\"Section 1\"
    \"Leaf 1.1\": 12
    \"Section 1.2\":::class1
      \"Leaf 1.2.1\": 12
\"Section 2\"
    \"Leaf 2.1\": 20:::class1
    \"Leaf 2.2\": 25

classDef class1 fill:red,color:blue,stroke:#FFD600;
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
fn draws_the_sections_and_the_leaves() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Treemap);
    assert_eq!(count(&out.svg, "pele-node pele-section"), 2);
    assert_eq!(count(&out.svg, "pele-node pele-leaf"), 4);
    assert_eq!(count(&out.svg, "<rect "), 6);
    assert!(out.svg.contains(">Item A1<"), "{}", out.svg);
    assert!(out.svg.contains(">Category B<"), "{}", out.svg);
    // The values are written as they are.
    assert!(out.svg.contains(">10<"), "{}", out.svg);
    assert!(out.svg.contains(">25<"), "{}", out.svg);
}

#[test]
fn draws_the_tree_of_the_sections() {
    let out = render(NESTED);
    // Three sections and four leaves.
    assert_eq!(count(&out.svg, "pele-node pele-section"), 3);
    assert_eq!(count(&out.svg, "pele-node pele-leaf"), 4);
    // The leaves of a section are drawn inside it.
    let products = out.svg.find("data-id=\"Products\"").unwrap();
    let phones = out.svg.find("data-id=\"Phones\"").unwrap();
    assert!(products < phones, "the section comes before its leaves");
}

#[test]
fn reads_the_styles_of_a_class() {
    let out = render(STYLED);
    assert!(out.svg.contains("fill:red"), "{}", out.svg);
    assert!(out.svg.contains("stroke:#FFD600"), "{}", out.svg);
    // The class of an item is written in its class list.
    assert!(out.svg.contains("pele-section class1"), "{}", out.svg);
    assert!(out.svg.contains("pele-leaf class1"), "{}", out.svg);
}

#[test]
fn reads_the_title_and_the_accessible_text() {
    let out = render(
        "treemap\ntitle My Treemap\naccTitle: Sales\naccDescr: Revenue by category\n\
         \"Category\"\n    \"Item\": 10\n",
    );
    assert!(out.svg.contains(">My Treemap<"), "{}", out.svg);
    assert!(
        out.svg.contains("<title id=\"pele-title\">Sales</title>"),
        "{}",
        out.svg
    );
    assert!(
        out.svg
            .contains("<desc id=\"pele-desc\">Revenue by category</desc>"),
        "{}",
        out.svg
    );
}

#[test]
fn reports_errors_with_positions() {
    let err = pele_rs::render(
        "treemap\n\"A\"\n    \"B\" : abc\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Treemap));
    assert_eq!(err.line, 3);

    // A class of one letter is not a class.
    let err =
        pele_rs::render("treemap\nclassDef a fill:red\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);

    // The keyword is needed.
    let err = pele_rs::render("hello\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported);
}

#[test]
fn fits_a_narrow_host() {
    let narrow = narrow(FULL);
    assert!(narrow.width <= 360.0, "{}", narrow.width);
    assert!(narrow.height > 0.0);
    assert_eq!(count(&narrow.svg, "pele-leaf"), 4);
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
            format!("treemap\ntitle {p}\n\"A\": 10\n"),
            format!("treemap\n\"A\":::{p}\n"),
            format!("treemap\naccTitle: {p}\n\"A\": 10\n"),
            format!("treemap\naccDescr {{ {p} }}\n\"A\": 10\n"),
            format!("treemap\nclassDef cls fill:{p}\n\"A\":::cls\n"),
            format!("treemap\n\"A\": {p}\n"),
        ] {
            let Some(svg) = try_render(&src) else {
                continue;
            };
            assert_inert(&svg, &src);
            rendered += 1;
        }
    }
    assert!(rendered > 30, "only {rendered} hostile inputs rendered");
    let mut fragments: Vec<&str> = PAYLOADS.to_vec();
    fragments.extend([
        "treemap\n",
        "treemap-beta\n",
        "\"",
        "'",
        ":",
        ",",
        ":::",
        "classDef ",
        "class ",
        "title ",
        "accTitle: ",
        "accDescr: ",
        "\n",
        " ",
        "%%",
        "A",
        "é",
        "0",
        "1",
        "100",
        "1,000",
    ]);
    let corpus = [FULL, NESTED, STYLED];
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
    let head = "treemap\n";
    let cases = [
        format!("{head}\"{}\": 10\n", "a".repeat(N)),
        format!("{head}{}", "\"A\": 10\n".repeat(N / 8)),
        format!("{head}title {}\n\"A\": 10\n", "x".repeat(N)),
        format!("{head}\"A\":::{}\n", "c".repeat(N)),
        format!("{head}\"A\": {}\n", "5".repeat(N / 2)),
        format!("{head}\"{}\": 1\n", "b".repeat(N / 2)),
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
        ("nested", NESTED.to_string()),
        ("styled", STYLED.to_string()),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
