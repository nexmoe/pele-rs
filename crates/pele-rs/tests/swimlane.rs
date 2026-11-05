//! Swimlane rendering, security, limits and speed.
#![cfg(feature = "swimlane")]

mod support;

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
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

include!("compat/swimlane/cases.rs");

const LANES: &str = "swimlane-beta LR
  subgraph Customer
    Browse[Browse catalogue]
    Pay[Pay]
  end
  subgraph Warehouse
    Pick[Pick items]
    Ship[Ship order]
  end
  Browse --> Pay
  Pay --> Pick
  Pick --> Ship
";

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

/// The position of the node with the given id.
fn at_of(svg: &str, id: &str) -> (f64, f64) {
    let at = svg
        .find(&format!("data-id=\"{id}\""))
        .expect("the node is drawn");
    let rest = &svg[at..];
    let start = rest.find("translate(").expect("the node is placed") + "translate(".len();
    let mut parts = rest[start..].split_terminator([',', ')']);
    let x = parts.next().expect("an x").parse().expect("a number");
    let y = parts.next().expect("a y").parse().expect("a number");
    (x, y)
}

/// The box of the lane with the given id: its left, top, width and height.
fn lane_of(svg: &str, id: &str) -> (f64, f64, f64, f64) {
    let at = svg
        .find(&format!("data-id=\"{id}\""))
        .expect("the lane is drawn");
    let rest = &svg[at..];
    let start = rest.find("<rect").expect("the lane has a box");
    // x, y, width, height.
    let read = |name: &str| {
        let at = start
            + rest[start..]
                .find(&format!("{name}=\""))
                .unwrap_or_else(|| panic!("the lane has {name}"))
            + name.len()
            + 2;
        let end = rest[at..].find('"').expect("the number ends");
        rest[at..at + end].parse().expect("a number")
    };
    (read("x"), read("y"), read("width"), read("height"))
}

#[test]
fn draws_the_lanes() {
    let out = render(LANES);
    assert_eq!(out.kind, DiagramKind::Swimlane);
    // Two lanes, each with its title in a band of its own.
    assert_eq!(count(&out.svg, "class=\"pele-rs-cluster pele-rs-lane"), 2);
    assert_eq!(count(&out.svg, "class=\"pele-rs-cluster-label\""), 2);
    // The nodes of the lanes and the edges between them.
    assert_eq!(count(&out.svg, "class=\"pele-rs-node "), 4);
    assert_eq!(count(&out.svg, "class=\"pele-rs-edge\""), 3);
    // The title of a lane is written out.
    assert!(out.svg.contains(">Customer<"), "{}", out.svg);
    assert!(out.svg.contains(">Warehouse<"), "{}", out.svg);
    // The lane runs the length of the diagram along the flow.
    // The lane runs the length of the diagram along the flow, so every lane is as long as the
    // others; each is as broad across it as its own items need.
    let (_, _, width, _) = lane_of(&out.svg, "Customer");
    let (_, _, other_width, _) = lane_of(&out.svg, "Warehouse");
    assert_eq!(width, other_width);
}

#[test]
fn keeps_the_nodes_inside_their_lane() {
    let out = render(LANES);
    let (left, top, width, height) = lane_of(&out.svg, "Customer");
    let (_, warehouse_top, _, warehouse_height) = lane_of(&out.svg, "Warehouse");
    for (id, lane) in [("Browse", 0), ("Pay", 0), ("Pick", 1), ("Ship", 1)] {
        let (x, y) = at_of(&out.svg, id);
        assert!(x >= left && x <= left + width, "{id} is outside its lane");
        let (top, height) = if lane == 0 {
            (top, height)
        } else {
            (warehouse_top, warehouse_height)
        };
        assert!(
            y >= top && y <= top + height,
            "{id} is outside its lane: y={y} top={top} height={height}"
        );
    }
    // The lanes are side by side across the flow, and the first one written comes first.
    assert!(top < warehouse_top);
}

#[test]
fn shares_one_unnamed_lane_between_the_nodes_that_have_none() {
    let out = render("swimlane-beta LR\n A[One] --> B[Two]\n");
    // One lane, with no title of its own.
    assert_eq!(count(&out.svg, "class=\"pele-rs-cluster pele-rs-lane"), 1);
    assert_eq!(count(&out.svg, "class=\"pele-rs-cluster-label\""), 0);
    assert_eq!(count(&out.svg, "class=\"pele-rs-node "), 2);
}

#[test]
fn a_direction_inside_a_lane_has_no_effect() {
    // Mermaid's lanes all run the same way, so a `direction` inside a lane is of no effect.
    let plain = svg("swimlane-beta LR\n subgraph s\n a[One] --> b[Two]\n end\n");
    let directed = svg("swimlane-beta LR\n subgraph s\n direction TB\n a[One] --> b[Two]\n end\n");
    assert_eq!(plain, directed);
}

#[test]
fn reads_the_direction() {
    let across = svg(LANES);
    let down = svg(&LANES.replace("swimlane-beta LR", "swimlane-beta TB"));
    assert_ne!(across, down);
    // The lanes run the length of the diagram: across means the lanes are side by side.
    let (_, top, _, height) = lane_of(&across, "Customer");
    assert!(height > top);
}

#[test]
fn reports_errors_with_positions() {
    let err = pele_rs::render(
        "swimlane-beta TD\n A --> B\n B --> )\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Swimlane));
    assert_eq!((err.line, err.column), (3, 8));
}

#[test]
fn enforces_limits() {
    let options = RenderOptions {
        limits: Limits {
            max_edges: 10,
            ..Limits::default()
        },
        ..RenderOptions::default()
    };
    let wide = "swimlane-beta TD\n a & b & c & d --> e & f & g\n";
    assert_eq!(
        pele_rs::render(wide, &options).unwrap_err().code,
        ErrorCode::Limit
    );
    let deep = format!(
        "swimlane-beta TD\n{}x\n{}",
        "subgraph s\n".repeat(100),
        "end\n".repeat(100)
    );
    assert_eq!(
        pele_rs::render(&deep, &RenderOptions::default())
            .unwrap_err()
            .code,
        ErrorCode::Limit
    );
    let options = RenderOptions {
        limits: Limits {
            output: 1000,
            ..Limits::default()
        },
        ..RenderOptions::default()
    };
    assert_eq!(
        pele_rs::render(LANES, &options).unwrap_err().code,
        ErrorCode::Limit
    );
}

#[test]
fn turns_down_to_fit_a_narrow_host() {
    let wide = render(LANES);
    let options = RenderOptions {
        max_width: Some(320.0),
        ..RenderOptions::default()
    };
    let narrow = pele_rs::render(LANES, &options).unwrap();
    assert!(narrow.width < wide.width);
    assert!(narrow.height > wide.height);
    let fixed = RenderOptions {
        max_width: Some(320.0),
        auto_direction: false,
        ..RenderOptions::default()
    };
    assert!(pele_rs::render(LANES, &fixed).unwrap().width > narrow.width);
}

#[test]
fn output_is_deterministic() {
    assert_eq!(svg(LANES), svg(LANES));
}

#[test]
fn is_inert_for_corpus_hostile_and_mutated_inputs() {
    let corpus: Vec<&str> = CASES.iter().map(|c| c.src).collect();
    for src in &corpus {
        if let Some(svg) = try_render(src) {
            assert_inert(&svg, src);
        }
    }
    let mut rendered = 0;
    for payload in PAYLOADS {
        for src in [
            format!("swimlane-beta TD\n A[\"{payload}\"] --> B"),
            format!("swimlane-beta TD\n A -->|\"{payload}\"| B"),
            format!("swimlane-beta TD\n subgraph s [\"{payload}\"]\n A\n end"),
            format!("swimlane-beta TD\n A --> B\n click A \"{payload}\" \"{payload}\""),
            format!("swimlane-beta TD\n A --> B\n click A href \"{payload}\""),
            format!("swimlane-beta TD\n A --> B\n style A fill:{payload}"),
            format!("swimlane-beta TD\n A:::c --> B\n classDef c color:{payload}"),
            format!("swimlane-beta TD\n A@{{ img: \"{payload}\", label: \"x\" }}"),
            format!("swimlane-beta TD\n A[\"`**{payload}**`\"]"),
            format!(
                "---\ntitle: \"{payload}\"\n---\nswimlane-beta TD\n accTitle: {payload}\n accDescr: {payload}\n A"
            ),
        ] {
            let Some(svg) = try_render(&src) else {
                continue;
            };
            assert_inert(&svg, &src);
            assert!(!svg.contains("<script"), "{src:?}");
            rendered += 1;
        }
    }
    assert!(rendered > 200, "only {rendered} hostile inputs rendered");
    let mut fragments: Vec<&str> = PAYLOADS.to_vec();
    fragments.extend([
        "swimlane-beta",
        "-->",
        "---",
        "-.->",
        "==>",
        "[",
        "]",
        "(",
        ")",
        "{",
        "}",
        "|",
        "\n",
        "subgraph x\n",
        "end\n",
        ":::c",
        "&",
    ]);
    let mut rnd = Random::new(5);
    let mut rendered = 0;
    for _ in 0..1500 {
        let src = mutate(&corpus, &fragments, &mut rnd);
        if let Some(svg) = try_render(&src) {
            assert_inert(&svg, &src);
            rendered += 1;
        }
    }
    // A parser that refused everything would make the loop above assert nothing at all.
    assert!(rendered > 100, "only {rendered} of 1500 mutations rendered");
}

fn lanes_with(edges: usize, nodes: usize, seed: u32) -> String {
    let mut rnd = Random::new(seed);
    let mut src = String::from("swimlane-beta TD\n");
    for lane in 0..4 {
        let _ = writeln!(src, "subgraph lane{lane}");
        for i in 0..nodes {
            let _ = writeln!(src, "l{lane}n{i}");
        }
        let _ = writeln!(src, "end");
    }
    for _ in 0..edges {
        let a = (rnd.next() * nodes as f64) as usize;
        let b = (rnd.next() * nodes as f64) as usize;
        let lane = (rnd.next() * 4.0) as usize;
        let _ = writeln!(src, "l{lane}n{a} --> l{lane}n{b}");
    }
    src
}

#[test]
fn stays_fast_on_large_and_hostile_input() {
    // Debug builds are several times slower than release, where 5,000 edges take about 0.1 s.
    let budget = Duration::from_secs(if cfg!(debug_assertions) { 20 } else { 2 });
    let options = RenderOptions {
        limits: Limits {
            source: usize::MAX,
            output: usize::MAX,
            // The edge limit is a limit of its own; this test is about how long the layout takes.
            max_edges: 100_000,
            ..Limits::default()
        },
        ..RenderOptions::default()
    };
    // Whether each case is drawn: a case that is refused answers at once, so the timing
    // assertion says nothing about it.
    let cases: Vec<(String, bool)> = vec![
        (lanes_with(2000, 50, 1), true),
        (
            format!(
                "swimlane-beta LR\n{}",
                (0..2000).fold(String::new(), |mut s, i| {
                    let _ = writeln!(s, "a{i}-->a{}", i + 1);
                    s
                })
            ),
            true,
        ),
        (
            format!("swimlane-beta TD\nA[\"{}\"]", "x ".repeat(25000)),
            true,
        ),
        (format!("swimlane-beta TD\n{}", "A-->".repeat(12000)), false),
        (lanes_with(8000, 40, 2), true),
        (
            format!("swimlane-beta TD\n{}", "%%{\n".repeat(12000)),
            false,
        ),
        (format!("swimlane-beta TD\n{}", "@".repeat(50000)), false),
        (format!("swimlane-beta TD\nA{}", " ".repeat(50000)), true),
    ];
    for (src, draws) in &cases {
        let started = Instant::now();
        let out = pele_rs::render(src, &options);
        let took = started.elapsed();
        assert!(
            took < budget,
            "{:?} took {took:?}",
            &src[..src.len().min(40)]
        );
        assert_eq!(out.is_ok(), *draws, "{:?}", &src[..src.len().min(40)]);
    }
}

#[test]
fn draws_a_nested_lane_as_a_group() {
    // A group inside a lane is a group of the drawing, not a lane of its own.
    let out = render(
        "swimlane-beta LR\n subgraph lane\n  subgraph inner\n   a[One] --> b[Two]\n  end\n end\n",
    );
    assert_eq!(count(&out.svg, "class=\"pele-rs-cluster pele-rs-lane"), 1);
    assert_eq!(count(&out.svg, "class=\"pele-rs-cluster\""), 1);
}

#[test]
fn snapshots() {
    let cases = [
        ("lanes", LANES),
        (
            "decision",
            "swimlane-beta LR\n  subgraph Intake\n    start([Start])\n    task[Do work]\n  end\n\n  subgraph Review\n    decision{Ready?}\n  end\n\n  subgraph Complete\n    done((Done))\n  end\n\n  start --> task --> decision\n  decision -->|Yes| done\n  decision -->|No| task\n",
        ),
        (
            "styles_and_titles",
            "---\ntitle: Styled\n---\nswimlane-beta LR\n  subgraph sales [Sales team]\n    lead[Qualify lead]\n  end\n\n  subgraph finance [Finance team]\n    review[Review terms]\n  end\n\n  lead --> review\n  classDef hot fill:#f96\n  class review hot\n",
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(src));
    }
}
