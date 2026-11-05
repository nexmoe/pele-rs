//! Requirement diagram rendering, security, limits and speed.
#![cfg(feature = "requirement")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "requirementDiagram\n requirement test_req {\n id: 1\n text: the test text.\n risk: high\n verifymethod: test\n }\n functionalRequirement test_req2 {\n id: 1.1\n text: the second **test** text.\n risk: low\n verifymethod: inspection\n }\n designConstraint test_req3:::hot {\n id: 1.2\n }\n element test_entity {\n type: simulation\n }\n element \"test entity2\" {\n type: word doc\n docRef: reqs/test_entity\n }\n test_entity - satisfies -> test_req2\n test_req - traces -> test_req2\n test_req - contains -> test_req3\n test_req <- copies - \"test entity2\"\n test_req - verifies -> test_req\n classDef hot fill:#f96,color:white\n style test_entity stroke:red\n";

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
fn draws_requirements_elements_and_relationships() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Requirement);
    assert_eq!(count(&out.svg, "pele-node pele-requirement"), 3);
    assert_eq!(count(&out.svg, "pele-node pele-element"), 2);
    assert_eq!(count(&out.svg, "pele-relationship-contains"), 1);
    assert_eq!(count(&out.svg, "pele-relationship-arrow"), 4);
    for needle in [
        "«Functional Requirement»",
        "«Element»",
        "«satisfies»",
        ">Verification<",
        ">Inspection<",
        ">reqs/test_entity<",
        "pele-requirement hot",
        "fill:#f96",
        "stroke:red",
    ] {
        assert!(out.svg.contains(needle), "{needle}");
    }
    assert!(out.links.is_empty());
    assert_inert(&out.svg, FULL);
}

#[test]
fn keeps_only_allowed_styles() {
    let svg = svg(
        "requirementDiagram\n requirement a {\n }\n element b {\n }\n classDef c fill:javascript:alert,stroke:red\n class a c\n style b fill:blue,color:javascript:x\n",
    );
    assert!(!svg.contains("javascript") && svg.contains("fill:blue"));
    assert!(svg.contains("stroke:red"));
}

#[test]
fn reports_errors_with_positions() {
    let err = pele_rs::render(
        "requirementDiagram\n requirement a {\n risk: huge\n }\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Requirement));
    assert_eq!(err.line, 3);
}

#[test]
fn enforces_limits() {
    let mut src = String::from("requirementDiagram\n");
    for i in 0..1000 {
        src += &format!("requirement r{i} {{\n}}\n");
    }
    src += &format!("classDef c {}\n", vec!["fill:red"; 1100].join(","));
    src += &format!(
        "class {} c\n",
        (0..1000)
            .map(|i| format!("r{i}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let options = RenderOptions {
        limits: Limits {
            source: usize::MAX,
            ..Limits::default()
        },
        ..RenderOptions::default()
    };
    assert_eq!(
        pele_rs::render(&src, &options).unwrap_err().code,
        ErrorCode::Limit
    );
}

#[test]
fn fits_a_narrow_host() {
    let src = "requirementDiagram\n direction LR\n requirement a {\n }\n requirement b {\n }\n requirement c {\n }\n requirement d {\n }\n a - traces -> b\n b - traces -> c\n c - traces -> d\n";
    let wide = render(src);
    let options = RenderOptions {
        max_width: Some(320.0),
        ..RenderOptions::default()
    };
    let narrow = pele_rs::render(src, &options).unwrap();
    assert!(
        wide.width > 320.0,
        "the wide drawing is wider than the host"
    );
    assert!(
        narrow.width <= 320.0,
        "the narrow drawing fits: {}",
        narrow.width
    );
    assert!(
        narrow.height > wide.height,
        "the narrow drawing is turned down"
    );
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
            format!("requirementDiagram\n requirement \"{p}\" {{\n }}"),
            format!("requirementDiagram\n requirement a {{\n text: {p}\n }}"),
            format!("requirementDiagram\n requirement a {{\n text: \"{p}\"\n id: \"{p}\"\n }}"),
            format!("requirementDiagram\n element a {{\n type: \"{p}\"\n docRef: \"{p}\"\n }}"),
            format!("requirementDiagram\n element \"{p}\" {{\n }}\n \"{p}\" - traces -> \"{p}\""),
            format!("requirementDiagram\n element a {{\n }}\n classDef c fill:{p}\n class a c"),
            format!(
                "requirementDiagram\n element a {{\n }}\n style a color:{p},stroke:{p},font-size:{p}"
            ),
            format!("requirementDiagram\n accTitle: {p}\n accDescr: {p}\n"),
            format!("---\ntitle: \"{p}\"\n---\nrequirementDiagram\n element a {{\n }}"),
        ] {
            let Some(svg) = try_render(&src) else {
                continue;
            };
            assert_inert(&svg, &src);
            rendered += 1;
        }
    }
    assert!(rendered > 100, "only {rendered} hostile inputs rendered");
    let mut fragments: Vec<&str> = PAYLOADS.to_vec();
    fragments.extend([
        "requirement ",
        "element ",
        "{",
        "}",
        "id: 1",
        "text: t",
        "risk: high",
        "type: x",
        " - contains -> ",
        " <- copies - ",
        ":::c",
        "classDef c fill:red",
        "style a fill:blue",
        "direction LR",
        "\n",
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
    assert!(rendered > 50, "only {rendered} of 1500 mutations rendered");
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
    let head = "requirementDiagram\n";
    let cases = [
        format!("{head}{}", "a direction XX ".repeat(N / 15)),
        format!("{head}{}", "direction TB direction lr ".repeat(N / 26)),
        format!("{head}requirement a {{\n{}", "text: x\n".repeat(N / 8)),
        format!("{head}{}", "\"".repeat(N)),
        format!("{head}{}", "x ".repeat(N / 2)),
        format!("{head}accDescr {{{}", "x ".repeat(N / 2)),
        format!(
            "{head}{}",
            repeat(3000, |i| format!(
                "requirement n{i} {{\n}}\nn{i} - traces -> n{}\n",
                i + 1
            ))
        ),
        format!(
            "{head}element c {{\n}}\n{}",
            repeat(3000, |i| format!(
                "element n{i} {{\n}}\nc - contains -> n{i}\n"
            ))
        ),
        format!(
            "{head}element a {{\n}}\n{}",
            "a - traces -> a\n".repeat(2000)
        ),
        format!(
            "{head}element c {{\n}}\n{}",
            repeat(3000, |i| format!("classDef k{i} fill:red\nclass c k{i}\n"))
        ),
        format!("{head}class {}z k\n", repeat(8000, |i| format!("s{i},"))),
        format!(
            "{head}element c {{\n}}\n{}",
            "style c fill:red,stroke:blue\n".repeat(3000)
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
        rendered >= 8,
        "only {rendered} of {} cases were drawn",
        cases.len()
    );
}

#[test]
fn draws_every_relationship_kind() {
    for kind in [
        "contains",
        "copies",
        "derives",
        "satisfies",
        "verifies",
        "refines",
        "traces",
    ] {
        let src = format!(
            "requirementDiagram\n requirement a {{\n }}\n requirement b {{\n }}\n a - {kind} -> b\n"
        );
        let out = render(&src);
        assert!(out.svg.contains(&format!("\u{ab}{kind}\u{bb}")), "{kind}");
        assert_eq!(
            count(&out.svg, "pele-relationship-contains"),
            usize::from(kind == "contains"),
            "{kind}"
        );
        assert_eq!(
            count(&out.svg, "pele-relationship-arrow"),
            usize::from(kind != "contains"),
            "{kind}"
        );
    }
}

#[test]
fn draws_every_requirement_type() {
    for (ty, stereotype) in [
        ("requirement", "\u{ab}Requirement\u{bb}"),
        (
            "functionalRequirement",
            "\u{ab}Functional Requirement\u{bb}",
        ),
        ("interfaceRequirement", "\u{ab}Interface Requirement\u{bb}"),
        (
            "performanceRequirement",
            "\u{ab}Performance Requirement\u{bb}",
        ),
        ("physicalRequirement", "\u{ab}Physical Requirement\u{bb}"),
        ("designConstraint", "\u{ab}Design Constraint\u{bb}"),
    ] {
        let out = render(&format!("requirementDiagram\n {ty} a {{\n id: 1\n }}\n"));
        assert!(out.svg.contains(stereotype), "{ty}");
    }
}

#[test]
fn draws_every_risk_and_verification_method() {
    for vm in ["analysis", "demonstration", "inspection", "test"] {
        let out = render(&format!(
            "requirementDiagram\n requirement a {{\n id: 1\n verifymethod: {vm}\n }}\n"
        ));
        let cap = format!(">{}<", vm[..1].to_uppercase() + &vm[1..]);
        assert!(out.svg.contains(&cap), "{vm}");
    }
    for (risk, cap) in [("low", ">Low<"), ("medium", ">Medium<"), ("high", ">High<")] {
        let out = render(&format!(
            "requirementDiagram\n requirement a {{\n id: 1\n risk: {risk}\n }}\n"
        ));
        assert!(out.svg.contains(cap), "{risk}");
    }
}

#[test]
fn puts_a_class_on_an_element() {
    let out = render(
        "requirementDiagram\n element a:::hot {\n type: simulation\n }\n classDef hot fill:#f96\n",
    );
    assert!(out.svg.contains("pele-element hot"), "{}", out.svg);
    assert!(out.svg.contains("fill:#f96"), "{}", out.svg);
}

#[test]
fn draws_the_contains_marker_and_dashes_the_other_relationships() {
    let out = render(
        "requirementDiagram\n requirement a {\n }\n requirement b {\n }\n a - contains -> b\n a - traces -> b\n",
    );
    assert_eq!(
        count(&out.svg, "<circle class=\"pele-marker\""),
        1,
        "{}",
        out.svg
    );
    assert!(out.svg.contains("stroke-dasharray=\"5 4\""), "{}", out.svg);
}

#[test]
fn draws_the_accessible_title_and_description() {
    let out = render(
        "requirementDiagram\n accTitle: the title\n accDescr: the description\n requirement a {\n }\n",
    );
    assert!(
        out.svg
            .contains("<title id=\"pele-title\">the title</title>"),
        "{}",
        out.svg
    );
    assert!(
        out.svg
            .contains("<desc id=\"pele-desc\">the description</desc>"),
        "{}",
        out.svg
    );
}

#[test]
fn snapshots() {
    let cases = [
        ("full", FULL.to_string()),
        (
            "sideways",
            format!(
                "---\ntitle: Sideways\n---\n{}",
                FULL.replace(
                    "requirementDiagram\n",
                    "requirementDiagram\n direction LR\n"
                )
            ),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
