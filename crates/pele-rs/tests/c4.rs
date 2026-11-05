//! C4 diagram rendering, security, limits and speed.
#![cfg(feature = "c4")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "C4Context
title System Context diagram
Enterprise_Boundary(b0, \"BankBoundary\") {
  Person(customerA, \"Banking Customer A\", \"A customer of the bank\")
  System(SystemAA, \"Internet Banking System\", \"Allows customers to view information\")
  System_Boundary(b1, \"BankBoundary2\") {
    System(SystemA, \"Banking System A\")
  }
}
Rel(customerA, SystemAA, \"Uses\")
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
fn draws_shapes_and_relations() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::C4);
    // The three shapes: the customer, the system and the system in the inner boundary.
    assert_eq!(count(&out.svg, "<g class=\"pele-node "), 3);
    // The two boundaries.
    assert_eq!(count(&out.svg, "<g class=\"pele-cluster "), 2);
    // The relation between the customer and the system.
    assert_eq!(count(&out.svg, "<g class=\"pele-edge\""), 1);
    // The labels of the shapes are written.
    assert!(out.svg.contains(">Banking Customer A<"), "{}", out.svg);
    assert!(out.svg.contains(">Internet Banking System<"), "{}", out.svg);
    assert!(out.svg.contains(">Uses<"), "{}", out.svg);
}

#[test]
fn gives_each_keyword_its_shape() {
    // Mermaid draws a person as a person, a system and a container as a box, a component as a
    // box with a folded corner, a database as a cylinder and a queue as a cylinder on its side.
    let out = render(
        "C4Context\nPerson(p, \"P\")\nSystem(s, \"S\")\nContainer(c, \"C\")\n\
         Component(k, \"K\")\nSystemDb(d, \"D\")\nSystemQueue(q, \"Q\")\n",
    );
    assert!(out.svg.contains("pele-shape-person"), "{}", out.svg);
    assert!(out.svg.contains("pele-shape-rect"), "{}", out.svg);
    assert!(out.svg.contains("pele-shape-rect"), "{}", out.svg);
    assert!(out.svg.contains("pele-shape-cyl"), "{}", out.svg);
    assert!(out.svg.contains("pele-shape-h-cyl"), "{}", out.svg);
    // The kind of the shape is written above its name.
    assert!(out.svg.contains(">«system»<"), "{}", out.svg);
    assert!(out.svg.contains(">«component»<"), "{}", out.svg);
}

#[test]
fn draws_a_title_and_a_description() {
    // The title is drawn, and the description is kept for a screen reader.
    let out = render(
        "C4Context\ntitle A context diagram\naccDescr: What the diagram shows\nPerson(p, \"P\")\n",
    );
    assert!(out.svg.contains(">A context diagram<"), "{}", out.svg);
    assert!(out.svg.contains("pele-title"));
    assert!(out.svg.contains("What the diagram shows"), "{}", out.svg);
    assert!(out.svg.contains("<desc"), "{}", out.svg);
}

#[test]
fn reads_the_title_from_acc_title() {
    // Mermaid's grammar hands `accTitle:` to the title of the diagram.
    let out = render("C4Context\naccTitle: A named diagram\nPerson(p, \"P\")\n");
    assert!(out.svg.contains(">A named diagram<"), "{}", out.svg);
}

#[test]
fn draws_a_deployment() {
    let out = render(
        "C4Deployment\nDeployment_Node(node1, \"Node 1\", \"Node\") {\n  \
         Person(p, \"A person\")\n}\n",
    );
    assert_eq!(count(&out.svg, "<g class=\"pele-cluster "), 1);
    assert!(out.svg.contains("pele-c4-node"), "{}", out.svg);
    assert!(out.svg.contains(">Node 1<"), "{}", out.svg);
}

#[test]
fn draws_a_technology_and_a_description() {
    let out = render("C4Context\nContainer(c, \"C\", \"Node.js\", \"What it does\")\n");
    assert!(out.svg.contains(">[Node.js]<"), "{}", out.svg);
    assert!(out.svg.contains(">What it does<"), "{}", out.svg);
}

#[test]
fn reports_errors_with_positions() {
    // The keyword of the diagram is needed.
    let err = pele_rs::render("C4Contextx\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::C4));
    assert_eq!(err.line, 1);

    // A statement that is not one the grammar knows.
    let err = pele_rs::render("C4Context\nHello\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::C4));
    assert_eq!(err.line, 2);

    // A shape without arguments.
    let err = pele_rs::render("C4Context\nPerson\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.line, 2);
}

#[test]
fn refuses_a_relation_to_an_unknown_shape() {
    let err = pele_rs::render(
        "C4Context\nPerson(p, \"P\")\nRel(p, q, \"Uses\")\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert_eq!(err.kind, Some(DiagramKind::C4));
    assert!(err.message.contains("unknown shape"), "{}", err.message);
}

#[test]
fn fits_a_narrow_host() {
    let narrow = narrow(FULL);
    assert!(narrow.width <= 360.0, "{}", narrow.width);
    assert!(narrow.height > 0.0);
    // The shapes are still drawn.
    assert_eq!(count(&narrow.svg, "<g class=\"pele-node "), 3);
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
            format!("C4Context\nPerson(a, \"{p}\")\n"),
            format!("C4Context\ntitle {p}\nPerson(a, \"A\")\n"),
            format!("C4Context\naccDescr: {p}\nPerson(a, \"A\")\n"),
            format!("C4Context\nPerson(a, \"A\", $link=\"{p}\")\n"),
            format!("C4Context\nPerson(a, \"A\", $bgColor=\"{p}\")\n"),
            format!("C4Context\nSystem(a, \"A\", $tags=\"{p}\")\n"),
            format!("C4Context\nRel(a, b, \"{p}\")\n"),
            format!("C4Context\nUpdateElementStyle(a, $bgColor=\"{p}\")\n"),
            format!("C4Context\nUpdateRelStyle(a, b, $lineColor=\"{p}\")\n"),
            format!("C4Context\nBoundary(b, \"{p}\") {{\n  Person(a, \"A\")\n}}\n"),
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
        "C4Context\n",
        "C4Container\n",
        "C4Component\n",
        "C4Dynamic\n",
        "C4Deployment\n",
        "Person",
        "System",
        "Container",
        "Component",
        "SystemDb",
        "SystemQueue",
        "Person_Ext",
        "Enterprise_Boundary",
        "System_Boundary",
        "Container_Boundary",
        "Deployment_Node",
        "Rel",
        "BiRel",
        "RelIndex",
        "UpdateElementStyle",
        "UpdateRelStyle",
        "UpdateLayoutConfig",
        "title ",
        "accTitle: ",
        "accDescr: ",
        "accDescr {",
        "}",
        "(",
        ")",
        "{",
        "}",
        ",",
        "\"",
        "$link=\"https://x\"",
        "$tags=\"a,b\"",
        "$sprite=\"users\"",
        "$descr=\"d\"",
        "$techn=\"t\"",
        "$type=\"t\"",
        "$shape=\"box\"",
        "$bgColor=\"red\"",
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
    let head = "C4Context\n";
    let cases = [
        format!("{head}Person(a, \"{}\")\n", "x".repeat(N)),
        format!("{head}{}", "Person(a, \"A\")\n".repeat(N / 2)),
        format!(
            "{head}Person(a, \"A\")\nPerson(b, \"B\")\n{}",
            "Rel(a, b, \"r\")\n".repeat(N / 10)
        ),
        format!(
            "{head}Boundary(b, \"B\") {{\n{}\n}}\n",
            "Person(a, \"A\")\n".repeat(N / 2)
        ),
        format!("{head}{}", "System(a, \"A\", $tags=\"x\")\n".repeat(N / 2)),
        format!(
            "{head}UpdateElementStyle(a, $bgColor=\"red\")\n{}",
            "Person(a, \"A\")\n".repeat(N / 2)
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
            "shapes",
            "C4Context\nPerson(p, \"A person\")\nSystem(s, \"A system\")\n\
             Container(c, \"A container\", \"Node.js\")\nComponent(k, \"A component\")\n\
             SystemDb(d, \"A database\")\nSystemQueue(q, \"A queue\")\n"
                .to_string(),
        ),
        (
            "deployment",
            "C4Deployment\nDeployment_Node(node1, \"Node 1\", \"Node\") {\n  \
             Person(p, \"A person\")\n}\n"
                .to_string(),
        ),
        (
            "relations",
            "C4Context\nPerson(a, \"A\")\nSystem(b, \"B\")\nSystem(c, \"C\")\n\
             Rel(a, b, \"uses\")\nRel(b, c, \"uses\", \"HTTPS\")\nBiRel(a, c, \"knows\")\n"
                .to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
