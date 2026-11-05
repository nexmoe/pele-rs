//! Kanban board rendering, security, limits and speed.
#![cfg(feature = "kanban")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "kanban
  Todo
    [Create Documentation]
    docs[Create Blog about the new diagram]@{ priority: 'Very Low', ticket: MC-2037 }
  id7[In progress]
    id6[Create renderer so that it works in all cases, with some extra text for wrapping]
  id9[Ready for deploy]
    Supercalifragilisticexpialidocious_is_a_very_long_word_indeed
    id10[Ready for test]@{ ticket: MC-2036, assigned: 'knsv', priority: 'Very High' }
  id11[Done]
    id5[define getData]@{ priority: low }
    id2[**Bold** and *italic*]
    :::done
    id3[Update DB function]@{ ticket: MC-2034, priority: 'High' }
";

const LINKED: &str =
    "---\nconfig:\n  kanban:\n    ticketBaseUrl: 'https://example.com/browse/#TICKET#'\n---\n";

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
fn draws_columns_and_cards() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Kanban);
    assert_eq!(count(&out.svg, "pele-cluster pele-column"), 4);
    assert_eq!(count(&out.svg, "pele-node pele-card"), 8);
    assert_eq!(count(&out.svg, "pele-priority-very-high"), 1);
    assert_eq!(count(&out.svg, "pele-priority-very-low"), 1);
    assert_eq!(count(&out.svg, "class=\"pele-priority\""), 4);
    for needle in [">Todo<", ">MC-2036<", ">knsv<", " done\"", ">Bold<"] {
        assert!(out.svg.contains(needle), "{needle}");
    }
    assert!(out.links.is_empty());
}

#[test]
fn links_tickets_to_safe_urls() {
    let out = render(&format!("{LINKED}{FULL}"));
    assert_eq!(out.links.len(), 3);
    assert_eq!(out.links[0].id, "docs");
    assert_eq!(out.links[0].href, "https://example.com/browse/MC-2037");
    assert!(
        out.svg
            .contains("href=\"https://example.com/browse/MC-2036\"")
    );

    let hostile = LINKED.replace("https://example.com/browse/", "javascript:alert(1)//");
    let out = render(&format!("{hostile}{FULL}"));
    assert!(out.links.is_empty());
    assert!(!out.svg.contains("javascript"));
}

#[test]
fn reports_errors_with_positions() {
    // The bracket is still open at the end of the text.
    let err = pele_rs::render("kanban\n  a\n    b[c\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Kanban));
    assert_eq!(err.line, 3);

    let err = pele_rs::render("kanban\n  a\n b\n  c\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert!(
        err.message.contains("Items without section"),
        "{}",
        err.message
    );
    // A semantic error carries no position of its own.
    assert_eq!((err.line, err.column), (0, 0));
}

#[test]
fn marks_only_the_outermost_priorities() {
    // A priority of medium gets no stripe: the stripe is for the two outermost ones.
    let medium = svg("kanban\n Todo\n  c1@{ priority: 'Medium' }\n");
    assert_eq!(count(&medium, "class=\"pele-priority\""), 0, "{medium}");
}

#[test]
fn puts_a_class_on_the_column_or_on_the_card() {
    // Where the class is written decides what it lands on.
    let column = svg("kanban\n Todo\n :::done\n  card\n");
    assert!(column.contains("pele-cluster pele-column done"), "{column}");
    let card = svg("kanban\n Todo\n  card\n  :::done\n");
    assert!(card.contains("pele-node pele-card done"), "{card}");
}

#[test]
fn folds_columns_into_rows_for_a_narrow_host() {
    let wide = render(FULL);
    let options = RenderOptions {
        max_width: Some(480.0),
        ..RenderOptions::default()
    };
    let narrow = pele_rs::render(FULL, &options).unwrap();
    assert!(narrow.width <= 480.0);
    assert!(narrow.height > wide.height);
}

#[test]
fn output_is_deterministic() {
    assert_eq!(svg(FULL), svg(FULL));
}

#[test]
fn is_inert_for_hostile_and_mutated_inputs() {
    let mut rendered = 0;
    for payload in PAYLOADS {
        let p = payload.replace('\n', " ");
        for src in [
            format!("kanban\n  c[{p}]\n    a[{p}]"),
            format!("kanban\n  c\n    a@{{ assigned: \"{p}\", ticket: \"{p}\" }}"),
            format!("kanban\n  c\n    a@{{ label: \"{p}\", priority: \"{p}\" }}"),
            format!("{LINKED}kanban\n  c\n    a@{{ ticket: \"{p}\" }}"),
            format!("---\ntitle: \"{p}\"\n---\nkanban\n  c"),
        ] {
            let Some(svg) = try_render(&src) else {
                continue;
            };
            assert_inert(&svg, &src);
            rendered += 1;
        }
    }
    assert!(rendered > 50, "only {rendered} hostile inputs rendered");
    let mut fragments: Vec<&str> = PAYLOADS.to_vec();
    fragments.extend([
        "\n",
        "  ",
        "@{ ",
        "}",
        ":::",
        "[",
        "]",
        "priority: high",
        "<br>",
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
    assert!(rendered > 500, "only {rendered} of 1500 mutations rendered");
}

fn repeat(n: usize, line: impl Fn(usize) -> String) -> String {
    (0..n).map(line).collect()
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
    let head = "kanban\n";
    let cases = [
        format!("{head}  c[{}]", "x".repeat(N)),
        format!("{head}  c\n    a[{}]", "x ".repeat(N / 2)),
        format!("{head}  c\n    a[{}]", "日".repeat(N / 3)),
        format!("{head}{}", repeat(4000, |i| format!("  c{i}\n"))),
        format!("{head}  c\n{}", repeat(4000, |i| format!("    a{i}\n"))),
        format!("{head}{}", " ".repeat(N)),
        format!("{head}  c\n    a@{{ {}", "x: 1, ".repeat(N / 6)),
        format!("{head}{}", "%%\n".repeat(N / 3)),
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
fn an_accessibility_statement_becomes_a_column() {
    // Mermaid's kanban grammar has no accessibility statements, so a line that looks like one
    // is a column. Mermaid does the same, so this is ported as it is.
    let out = render("kanban\naccTitle: B\naccDescr: D\n Todo\n  card\n");
    assert!(out.svg.contains("pele-cluster pele-column"), "{}", out.svg);
    assert!(out.svg.contains(">accTitle: B<"), "{}", out.svg);
    assert!(out.svg.contains(">accDescr: D<"), "{}", out.svg);
    assert!(!out.svg.contains("<title id="), "{}", out.svg);
}

#[test]
fn snapshots() {
    let cases = [
        ("full", FULL.to_string()),
        ("linked", format!("{LINKED}{FULL}")),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
