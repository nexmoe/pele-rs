//! Agent flow diagram rendering, security, limits and speed.
#![cfg(feature = "agentflow")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "agentflow-beta TB
  flow reviewer[\"Review Agent\"]
    changes[\"Gather changes\"]@{ shape: input }
    analyse[\"Analyse diff\"]@{ shape: task }
    ok[\"Clean?\"]@{ shape: decision }
  end
  flow writer[\"Drafting Agent\"]
    draft[\"Draft the notes\"]@{ shape: task }
  end
  connector github[\"GitHub\"]
  reviewer --> writer
  writer --> github
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
fn draws_agents_flows_and_edges() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Agentflow);
    // The four agents of the flows and the connector.
    assert_eq!(count(&out.svg, "<g class=\"pele-node "), 5);
    // The two flows.
    assert_eq!(count(&out.svg, "<g class=\"pele-cluster "), 2);
    // The two edges.
    assert_eq!(count(&out.svg, "<g class=\"pele-edge pele-edge-"), 2);
    // The labels are written.
    assert!(out.svg.contains(">Gather changes<"), "{}", out.svg);
    assert!(out.svg.contains(">Review Agent<"), "{}", out.svg);
    assert!(out.svg.contains(">GitHub<"), "{}", out.svg);
}

#[test]
fn draws_the_shapes_of_the_agents() {
    let out = render(
        "agentflow-beta TB\n  a[\"square\"]\n  b(\"round\")\n  c{\"diamond\"}\n  d{{\"hexagon\"}}\n  e[[\"sub\"]]\n  f>\"odd\"]\n",
    );
    assert!(out.svg.contains("pele-shape-rounded"), "{}", out.svg);
    assert!(out.svg.contains("pele-shape-diam"), "{}", out.svg);
    assert!(out.svg.contains("pele-shape-hex"), "{}", out.svg);
    assert!(out.svg.contains("pele-shape-fr-rect"), "{}", out.svg);
}

#[test]
fn draws_the_shapes_the_metadata_asks_for() {
    let out = render(
        "agentflow-beta TB\n  a[\"A\"]@{ shape: task }\n  b[\"B\"]@{ shape: tool }\n  c[\"C\"]@{ shape: input }\n  d[\"D\"]@{ shape: decision }\n  e[\"E\"]@{ shape: refdoc }\n  f[\"F\"]@{ shape: action }\n",
    );
    assert!(out.svg.contains("pele-shape-rounded"), "{}", out.svg);
    assert!(out.svg.contains("pele-shape-fr-rect"), "{}", out.svg);
    assert!(out.svg.contains("pele-shape-lean-r"), "{}", out.svg);
    assert!(out.svg.contains("pele-shape-diam"), "{}", out.svg);
    assert!(out.svg.contains("pele-shape-lin-doc"), "{}", out.svg);
    assert!(out.svg.contains("pele-shape-hex"), "{}", out.svg);
}

#[test]
fn draws_a_collapsed_flow_as_one_box() {
    let out = render(
        "agentflow-beta TB\n  flow f[\"F\"]@{ view: collapsed }\n    a[\"A\"]\n    b[\"B\"]\n  end\n",
    );
    // The flow is one box, and the agents inside it are not drawn.
    assert_eq!(count(&out.svg, "<g class=\"pele-node "), 1);
    assert!(out.svg.contains("pele-collapsed-mark"), "{}", out.svg);
}

#[test]
fn draws_a_collapsed_flow_as_one_box_when_the_metadata_comes_later() {
    let out = render(
        "agentflow-beta TB\n  flow f[\"F\"]\n    a[\"A\"]\n    b[\"B\"]\n  end\n  f@{ view: collapsed }\n",
    );
    assert_eq!(count(&out.svg, "<g class=\"pele-node "), 1);
}

#[test]
fn draws_the_kinds_of_edge() {
    let out = render("agentflow-beta TB\n  a --> b\n  a --x c\n  a -.- d\n");
    assert_eq!(count(&out.svg, "<g class=\"pele-edge pele-edge-"), 3);
    // The arrow of the first edge and the cross of the second are drawn; the reference edge
    // has no arrowhead.
    assert_eq!(count(&out.svg, "pele-marker"), 2);
    // The reference edge is drawn with a dotted line.
    assert!(out.svg.contains("stroke-dasharray=\"3 4\""), "{}", out.svg);
}

#[test]
fn reads_the_direction() {
    let down = svg("agentflow-beta TB\n  a --> b\n");
    let across = svg("agentflow-beta LR\n  a --> b\n");
    assert_ne!(down, across);
}

#[test]
fn draws_a_title() {
    let out = render("---\ntitle: An agent flow\n---\nagentflow-beta TB\n  a --> b\n");
    assert!(out.svg.contains(">An agent flow<"), "{}", out.svg);
    assert!(out.svg.contains("pele-title"));
}

#[test]
fn reports_errors_with_positions() {
    // A link that names no agent.
    let err =
        pele_rs::render("agentflow-beta TB\n  a -->\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Agentflow));
    assert_eq!(err.line, 2);

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
fn refuses_a_shape_it_does_not_have() {
    // Mermaid reads an unknown shape in a metadata block as an error.
    let err = pele_rs::render(
        "agentflow-beta TB\n  a[\"A\"]@{ shape: no-such-shape }\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert!(err.message.contains("No such shape"), "{}", err.message);
}

#[test]
fn fits_a_narrow_host() {
    let src = "agentflow-beta TB\n  a[\"A\"] --> b[\"B\"]\n";
    let narrow = narrow(src);
    assert!(narrow.width <= 360.0, "{}", narrow.width);
    assert!(narrow.height > 0.0);
    // The agents are still drawn.
    assert_eq!(count(&narrow.svg, "<g class=\"pele-node "), 2);
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
            format!("agentflow-beta TB\n  a[\"{p}\"]\n"),
            format!("agentflow-beta TB\n  a --> b[\"{p}\"]\n"),
            format!("agentflow-beta TB\n  flow f[\"{p}\"]\n    a\n  end\n"),
            format!("agentflow-beta TB\n  connector c[\"{p}\"]\n"),
            format!("agentflow-beta TB\n  classDef c fill:{p}\n  a\n"),
            format!("agentflow-beta TB\n  style a fill:{p}\n"),
            format!("agentflow-beta TB\n  click a \"{p}\"\n"),
            format!("agentflow-beta TB\n  a[\"A\"]@{{ shape: task, x: \"{p}\" }}\n"),
            format!("---\ntitle: \"{p}\"\n---\nagentflow-beta TB\n  a\n"),
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
        "agentflow-beta TB\n",
        "agentflow-beta LR\n",
        "flow f\n",
        "end\n",
        "connector c\n",
        "global g\n",
        "direction TB\n",
        "a",
        "b",
        "-->",
        "--x",
        "--",
        "-.-",
        "|",
        "[",
        "]",
        "(",
        ")",
        "{",
        "}",
        "{{",
        "}}",
        ">",
        "classDef c fill:red\n",
        "class a c\n",
        "style a fill:red\n",
        "linkStyle 0 stroke:#f00\n",
        "click a callback\n",
        "@{ shape: task }\n",
        "@{ view: collapsed }\n",
        "\"",
        "\n",
        " ",
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
    let head = "agentflow-beta TB\n";
    let cases = [
        format!("{head}a[\"{}\"]\n", "x".repeat(N)),
        format!("{head}{}", "a\n".repeat(N / 2)),
        format!("{head}{}", "a --> b\n".repeat(N / 10)),
        format!("{head}flow f\n{}\nend\n", "a\n".repeat(N / 4)),
        format!("{head}classDef c fill:red\n{}", "a\n".repeat(N / 2)),
        format!("{head}{}", "a[\"A\"]@{ shape: task }\n".repeat(N / 2)),
        format!("{head}connector c[\"C\"]\n{}", "a\n".repeat(N / 2)),
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
            "agentflow-beta TB\n  a[\"square\"]\n  b(\"round\")\n  c{\"diamond\"}\n  d{{\"hexagon\"}}\n  e[[\"sub\"]]\n  f>\"odd\"]\n"
                .to_string(),
        ),
        (
            "flows",
            "agentflow-beta LR\n  flow f[\"F\"]\n    a[\"A\"]\n    b[\"B\"]\n  end\n  a --> b\n"
                .to_string(),
        ),
        (
            "edges",
            "agentflow-beta TB\n  a --> b\n  a --x c\n  a -.- d\n  a -- yes --> e\n".to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
