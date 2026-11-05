//! Use case diagram rendering, security, limits and speed.
#![cfg(feature = "usecase")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "usecase-beta
actor Customer(\"Customer\")
actor Staff
systemBoundary Store[\"Store\"]@{ type: package }
  Browse(\"Browse products\")
  Checkout(\"Checkout\")
end
Customer --> Browse
Browse -- buys --> Checkout
Staff -- approves --> Checkout
Checkout ..> : include Browse
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
fn draws_actors_use_cases_and_relations() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Usecase);
    // The two actors, the two use cases and the boundary.
    assert_eq!(count(&out.svg, "<g class=\"pele-rs-node "), 4);
    assert_eq!(count(&out.svg, "<g class=\"pele-rs-cluster "), 1);
    // The four relationships.
    assert_eq!(count(&out.svg, "<g class=\"pele-rs-edge "), 4);
    assert!(out.svg.contains(">Customer<"), "{}", out.svg);
    assert!(out.svg.contains(">Browse products<"), "{}", out.svg);
    assert!(out.svg.contains(">Store<"), "{}", out.svg);
    // The label of a relationship.
    assert!(out.svg.contains(">buys<"), "{}", out.svg);
}

#[test]
fn draws_the_actors_as_figures() {
    let out = render("usecase-beta\nactor Customer\nactor Staff\n");
    assert_eq!(count(&out.svg, "pele-rs-actor-figure"), 2);
    // The stick figure of an actor is drawn.
    assert!(out.svg.contains("usecase-actor-normal"), "{}", out.svg);
    assert!(
        out.svg.contains("pele-rs-shape-usecaseActor"),
        "{}",
        out.svg
    );
}

#[test]
fn draws_the_shapes_of_the_elements() {
    // A use case in brackets is a box, one in parentheses an oval.
    let out = render("usecase-beta\nA[\"A box\"]\nB(\"An oval\")\n");
    assert!(out.svg.contains("<rect"), "{}", out.svg);
    assert!(out.svg.contains("<ellipse"), "{}", out.svg);
}

#[test]
fn draws_a_boundary_around_its_members() {
    let out = render("usecase-beta\nsystemBoundary Store\n  A\n  B\nend\n");
    assert_eq!(count(&out.svg, "<g class=\"pele-rs-cluster "), 1);
    assert_eq!(count(&out.svg, "<g class=\"pele-rs-node "), 2);
}

#[test]
fn draws_a_note_about_an_element() {
    let out = render("usecase-beta\nA\nnote for A \"Remember\"\n");
    assert_eq!(count(&out.svg, "usecase-note"), 1);
    assert!(out.svg.contains(">Remember<"), "{}", out.svg);
}

#[test]
fn draws_a_json_table() {
    let out = render("usecase-beta\njson Payload@{ \"a\": 1, \"b\": [\"x\"] }\n");
    assert!(
        out.svg.contains("pele-rs-shape-usecaseJsonTable"),
        "{}",
        out.svg
    );
    assert!(out.svg.contains("pele-rs-json-key"), "{}", out.svg);
}

#[test]
fn reads_the_config() {
    let close = svg("usecase-beta\nA\nB\n");
    let apart = svg("---\nconfig:\n  usecase:\n    nodeSpacing: 80\n---\nusecase-beta\nA\nB\n");
    assert_ne!(close, apart);
}

#[test]
fn draws_a_title() {
    let out = render("---\ntitle: A use case diagram\n---\nusecase-beta\nA\n");
    assert!(out.svg.contains(">A use case diagram<"), "{}", out.svg);
    assert!(out.svg.contains("pele-rs-title"));
}

#[test]
fn reports_errors_with_positions() {
    // The keyword of the diagram is needed.
    let err = pele_rs::render("usecase-beta\nA -->\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Usecase));
    assert_eq!(err.line, 2);

    // A diagram of another type is not a use case diagram.
    let err = pele_rs::render("hello\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported);
}

#[test]
fn refuses_a_json_node_that_is_not_json() {
    let err = pele_rs::render(
        "usecase-beta\njson A@{ not json }\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
}

#[test]
fn draws_an_edge_whose_route_is_short() {
    // The route of the edge holds fewer points than the drawing expects, so the edge has no
    // end to put on the outline of a node. It is still drawn.
    let rendered = try_render("usecase-beta\nA--A");
    assert!(rendered.is_some());
}

#[test]
fn output_is_deterministic() {
    assert_eq!(svg(FULL), svg(FULL));
}

#[test]
fn fits_a_narrow_host() {
    let wide = render(FULL);
    let narrow = narrow(FULL);
    assert!(narrow.width <= 360.0, "{}", narrow.width);
    assert!(narrow.height > 0.0);
    assert_eq!(count(&narrow.svg, "<g class=\"pele-rs-node "), 4);
    let _ = wide;
}

#[test]
fn is_inert_for_hostile_and_mutated_inputs() {
    let mut rendered = 0;
    for payload in PAYLOADS {
        let p = *payload;
        for src in [
            format!("usecase-beta\nactor A(\"{p}\")\n"),
            format!("usecase-beta\nA(\"{p}\")\n"),
            format!("usecase-beta\nsystemBoundary S[\"{p}\"]\n  A\nend\n"),
            format!("usecase-beta\nnote for A \"{p}\"\n"),
            format!("usecase-beta\nclassDef c fill:{p}\nA\n"),
            format!("usecase-beta\nstyle A fill:{p}\n"),
            format!("usecase-beta\njson A@{{ \"k\": \"{p}\" }}\n"),
            format!("usecase-beta\nA --> B : {p}\n"),
            format!("---\ntitle: \"{p}\"\n---\nusecase-beta\nA\n"),
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
        "usecase-beta\n",
        "usecase-beta:",
        "actor\n",
        "systemBoundary\n",
        "end\n",
        "A",
        "B",
        "-->",
        "--",
        "<--",
        "..>",
        ": include",
        ": extend",
        "--|>",
        "<<",
        ">>",
        "@{",
        "}",
        ":::",
        "classDef c fill:red\n",
        "class A c\n",
        "style A fill:red\n",
        "note for A: x\n",
        "direction LR\n",
        "accTitle: x\n",
        "accDescr: x\n",
        "json A@{}\n",
        "(",
        ")",
        "[",
        "]",
        "\"",
        "\n",
        " ",
    ]);
    let corpus = [FULL];
    let mut rnd = Random::new(23);
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
    const N: usize = 20000;
    let head = "usecase-beta\n";
    let cases = [
        format!("{head}actor A(\"{}\")\n", "x".repeat(N)),
        format!("{head}{}", "A\n".repeat(N)),
        format!("{head}{}", "A --> B\n".repeat(N / 4)),
        format!("{head}systemBoundary S\n{}end\n", "A\n".repeat(N / 2)),
        format!("{head}classDef c fill:red\n{}", "A\n".repeat(N / 2)),
        format!("{head}json A@{{ \"k\": \"{}\" }}\n", "x".repeat(N / 2)),
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
            "actors",
            "usecase-beta\nactor Customer(\"Customer\")\nactor Staff\n".to_string(),
        ),
        (
            "boundary",
            "usecase-beta\nsystemBoundary Store[\"Store\"]@{ type: package }\n  A(\"A use case\")\n  B(\"Another\")\nend\n"
                .to_string(),
        ),
        (
            "relations",
            "usecase-beta\nA --> B\nB -- approves --> C\nC ..> : include D\nE --|> F\n"
                .to_string(),
        ),
        (
            "notes_and_json",
            "usecase-beta\nA\nnote for A \"Remember\"\njson Payload@{ \"a\": 1 }\n"
                .to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
