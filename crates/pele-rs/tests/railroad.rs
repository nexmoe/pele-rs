//! Railroad diagram rendering, security, limits and speed.
#![cfg(feature = "railroad")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "railroad-beta
title A grammar
rule = sequence(terminal(\"a\"), nonterminal(\"b\"), optional(terminal(\"c\")));
other = choice(terminal(\"x\"), special(\"y\"));
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
fn draws_a_railroad_diagram() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Railroad);
    // The two rules, each with its own group.
    assert_eq!(count(&out.svg, "<g class=\"pele-rule\""), 2);
    // The terminals, the non-terminal and the special sequence.
    assert_eq!(count(&out.svg, "pele-terminal"), 3);
    assert_eq!(count(&out.svg, "pele-nonterminal"), 1);
    assert_eq!(count(&out.svg, "pele-special"), 1);
    // The text of the parts is written.
    assert!(out.svg.contains(">a<"), "{}", out.svg);
    assert!(out.svg.contains(">b<"), "{}", out.svg);
    assert!(out.svg.contains(">c<"), "{}", out.svg);
}

#[test]
fn draws_the_four_notations() {
    // Mermaid's own notation.
    let own = svg("railroad-beta\nr = sequence(terminal(\"a\"), optional(terminal(\"b\")));\n");
    assert!(own.contains("pele-terminal"), "{own}");
    // EBNF.
    let ebnf = svg("railroad-ebnf-beta\nr = \"a\" (\"b\" | \"c\")* ;\n");
    assert!(ebnf.contains("pele-terminal"), "{ebnf}");
    // ABNF.
    let abnf = svg("railroad-abnf-beta\nr = 1*3\"a\" / \"b\" ;\n");
    assert!(abnf.contains("pele-terminal"), "{abnf}");
    // PEG.
    let peg = svg("railroad-peg-beta\nr <- \"a\" / (\"b\" \"c\")* ;\n");
    assert!(peg.contains("pele-terminal"), "{peg}");
    // The four are drawn differently.
    assert_ne!(own, ebnf);
    assert_ne!(ebnf, abnf);
    assert_ne!(abnf, peg);
}

#[test]
fn draws_a_repetition_with_its_count() {
    // A repetition of an exact number of times shows the number.
    let exact = render("railroad-abnf-beta\nr = 2\"a\" ;\n");
    assert!(exact.svg.contains("2×"), "{}", exact.svg);
    // A repetition of a range of times shows the range.
    let range = render("railroad-abnf-beta\nr = 1*3\"a\" ;\n");
    assert!(range.svg.contains("1–3×"), "{}", range.svg);
    // A repetition of any number of times shows no number, as in Mermaid.
    let any = render("railroad-beta\nr = oneOrMore(terminal(\"a\"));\n");
    assert!(!any.svg.contains("×"), "{}", any.svg);
}

#[test]
fn reads_the_config() {
    // The room between the parts is set in the config.
    let close = svg("railroad-beta\nr = sequence(terminal(\"a\"), terminal(\"b\"));\n");
    let apart = svg(
        "---\nconfig:\n  railroad:\n    horizontalSeparation: 60\n---\nrailroad-beta\nr = sequence(terminal(\"a\"), terminal(\"b\"));\n",
    );
    assert_ne!(close, apart);

    // The markers at the ends of the track can be turned off.
    let with = svg("railroad-beta\nr = terminal(\"a\");\n");
    let without = svg(
        "---\nconfig:\n  railroad:\n    showMarkers: false\n---\nrailroad-beta\nr = terminal(\"a\");\n",
    );
    assert!(with.contains("pele-marker"));
    assert!(!without.contains("pele-marker"));
}

#[test]
fn draws_a_title() {
    let out = render("---\ntitle: A grammar\n---\nrailroad-beta\nr = terminal(\"a\");\n");
    assert!(out.svg.contains(">A grammar<"), "{}", out.svg);
    assert!(out.svg.contains("pele-title"));
}

#[test]
fn takes_the_quotes_off_a_title_that_holds_only_one() {
    // Mermaid takes the quotes off with a slice, so a title that is a single quote leaves
    // nothing behind.
    let out = render("railroad-beta\ntitle '\nr = terminal(\"a\");\n");
    assert!(!out.svg.contains(">'<"), "{}", out.svg);
}

#[test]
fn reads_the_accessibility_statements() {
    let out = render(
        "railroad-beta\naccTitle: The title\naccDescr: The description\nr = terminal(\"a\");\n",
    );
    assert!(out.svg.contains(">The title</title>"), "{}", out.svg);
    assert!(out.svg.contains(">The description</desc>"), "{}", out.svg);
}

#[test]
fn reports_errors_with_positions() {
    // A rule without a definition.
    let err = pele_rs::render("railroad-beta\nrule\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Railroad));

    // The keyword of the diagram is needed.
    let err = pele_rs::render("hello\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported);
    assert!(
        err.message.contains("No diagram type detected"),
        "{}",
        err.message
    );

    // The keyword is written in lower case: Mermaid's detector does not care, but its parser
    // does.
    let err = pele_rs::render(
        "RAILROAD-BETA\nr = terminal(\"a\");\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
}

#[test]
fn refuses_a_diagram_that_is_too_deep() {
    // The nesting of the expressions is limited, so that a diagram cannot use the whole stack.
    let deep = format!(
        "railroad-beta\nr = {};\n",
        "sequence(".repeat(200) + &format!("terminal(\"a\"){}", ")".repeat(200))
    );
    let err = pele_rs::render(&deep, &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Limit);
}

#[test]
fn fits_a_narrow_host() {
    let wide = render(FULL);
    let narrow = narrow(FULL);
    assert!(narrow.width <= 360.0, "{}", narrow.width);
    assert!(narrow.height > 0.0);
    // The parts are still drawn.
    assert_eq!(count(&narrow.svg, "<g class=\"pele-rule\""), 2);
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
            format!("railroad-beta\nr = terminal(\"{p}\");\n"),
            format!("railroad-beta\nr = nonterminal(\"{p}\");\n"),
            format!("railroad-beta\nr = special(\"{p}\");\n"),
            format!("railroad-ebnf-beta\nr = \"{p}\" ;\n"),
            format!("railroad-abnf-beta\nr = \"{p}\" ;\n"),
            format!("railroad-peg-beta\nr <- \"{p}\" ;\n"),
            format!("---\ntitle: \"{p}\"\n---\nrailroad-beta\nr = terminal(\"a\");\n"),
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
        "railroad-beta\n",
        "railroad-ebnf-beta\n",
        "railroad-abnf-beta\n",
        "railroad-peg-beta\n",
        "r",
        "=",
        "::=",
        "<-",
        ";",
        "(",
        ")",
        "[",
        "]",
        "{",
        "}",
        ",",
        "|",
        "/",
        "&",
        "!",
        "?",
        "*",
        "+",
        "-",
        ".",
        "terminal",
        "nonterminal",
        "special",
        "sequence",
        "choice",
        "optional",
        "oneOrMore",
        "zeroOrMore",
        "\"",
        "\n",
        " ",
    ]);
    let corpus = [
        FULL,
        "railroad-beta\nr = sequence(terminal(\"a\"), optional(terminal(\"b\")));\n",
        "railroad-beta\nr = choice(terminal(\"a\"), terminal(\"b\"));\n",
        "railroad-ebnf-beta\nr = \"a\" (\"b\" | \"c\")* ;\n",
        "railroad-abnf-beta\nr = 1*3\"a\" / \"b\" ;\n",
        "railroad-peg-beta\nr <- \"a\" / (\"b\" \"c\")* ;\n",
    ];
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
    let _ = rendered;
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
    let cases = [
        format!("railroad-beta\nr = terminal(\"{}\");\n", "x".repeat(N)),
        format!("railroad-beta\n{}", "r = terminal(\"a\");\n".repeat(N / 25)),
        format!(
            "railroad-beta\nr = sequence({});\n",
            "terminal(\"a\"),".repeat(N / 50)
        ),
        format!("railroad-ebnf-beta\nr = {} ;\n", "\"a\" ".repeat(N / 25)),
        format!("railroad-abnf-beta\nr = {} ;\n", "\"a\" ".repeat(N / 25)),
        format!("railroad-peg-beta\nr <- {} ;\n", "\"a\" ".repeat(N / 25)),
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
            "choice",
            "railroad-beta\nr = choice(terminal(\"a\"), terminal(\"bb\"), special(\"x\"));\n"
                .to_string(),
        ),
        (
            "loop",
            "railroad-beta\nr = oneOrMore(terminal(\"a\"));\ns = zeroOrMore(terminal(\"b\"));\n"
                .to_string(),
        ),
        (
            "ebnf",
            "railroad-ebnf-beta\nr = \"a\" (\"b\" | \"c\")* \"d\" ;\n".to_string(),
        ),
        (
            "abnf",
            "railroad-abnf-beta\nr = 1*3\"a\" / \"b\" ;\n".to_string(),
        ),
        (
            "peg",
            "railroad-peg-beta\nr <- \"a\" / (\"b\" \"c\")* ;\n".to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
