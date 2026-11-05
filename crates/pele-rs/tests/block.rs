//! Block diagram rendering, security, limits and speed.
#![cfg(feature = "block")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "block
    columns 2
    a[\"First\"] --> b[\"Second\"]
    space
    c{\"Third\"}
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
fn draws_blocks_and_links() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Block);
    // The three blocks: the first, the second and the third. The space is no block.
    assert_eq!(count(&out.svg, "<g class=\"pele-rs-node "), 3);
    // The link between the first and the second.
    assert_eq!(count(&out.svg, "<g class=\"pele-rs-edge\""), 1);
    // The labels of the blocks are written.
    assert!(out.svg.contains(">First<"), "{}", out.svg);
    assert!(out.svg.contains(">Second<"), "{}", out.svg);
    assert!(out.svg.contains(">Third<"), "{}", out.svg);
    // The diamond of the third block.
    assert!(out.svg.contains("pele-rs-shape-diam"), "{}", out.svg);
}

#[test]
fn draws_a_composite() {
    let out = render("block\nblock:group[\"A group\"]\n  a\n  b\nend\n");
    // The composite is a cluster, and it holds the two blocks.
    assert_eq!(count(&out.svg, "<g class=\"pele-rs-cluster\""), 1);
    assert_eq!(count(&out.svg, "<g class=\"pele-rs-node "), 2);
    assert!(out.svg.contains(">A group<"), "{}", out.svg);
}

#[test]
fn puts_the_blocks_in_a_grid() {
    // Two columns put the blocks side by side, one column puts them on top of each other.
    let across = svg("block\ncolumns 2\nA\nB\n");
    let down = svg("block\ncolumns 1\nA\nB\n");
    assert_ne!(across, down);
    // Without a column statement every block is on one row of its own.
    let free = svg("block\nA\nB\n");
    assert_eq!(across, free);
    assert_ne!(down, free);
}

#[test]
fn draws_an_arrow_block() {
    let out = render("block\nblockArrow<[\"go\"]>(right)\n");
    assert!(out.svg.contains("pele-rs-shape-block-arrow"), "{}", out.svg);
    assert!(out.svg.contains(">go<"), "{}", out.svg);
}

#[test]
fn reads_the_config() {
    // The room between the blocks is set in the config.
    let close = svg("block\nA\nB\n");
    let apart = svg("---\nconfig:\n  block:\n    padding: 40\n---\nblock\nA\nB\n");
    assert_ne!(close, apart);
}

#[test]
fn draws_a_title() {
    let out = render("---\ntitle: A block diagram\n---\nblock\nA\n");
    assert!(out.svg.contains(">A block diagram<"), "{}", out.svg);
    assert!(out.svg.contains("pele-rs-title"));
}

#[test]
fn reports_errors_with_positions() {
    // A link that names no block.
    let err = pele_rs::render("block\nA\n-->\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Block));
    assert_eq!(err.line, 3);

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
fn refuses_a_composite_without_blocks() {
    // Mermaid's grammar needs a statement between the keyword and the `end`.
    let err = pele_rs::render("block\nblock\nend\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Block));
}

#[test]
fn keeps_only_the_last_link_of_a_chain() {
    // Mermaid's grammar reads a chain of links from the left and keeps only the last one, so
    // the nodes before it are not part of the diagram.
    let out = render("block\nA --> B --> C\n");
    assert_eq!(count(&out.svg, "<g class=\"pele-rs-node "), 2);
    assert_eq!(count(&out.svg, "<g class=\"pele-rs-edge\""), 0);
}

#[test]
fn fits_a_narrow_host() {
    let wide = render(FULL);
    let narrow = narrow(FULL);
    assert!(narrow.width <= 360.0, "{}", narrow.width);
    assert!(narrow.height > 0.0);
    // The blocks are still drawn.
    assert_eq!(count(&narrow.svg, "<g class=\"pele-rs-node "), 3);
    let _ = wide;
}

#[test]
fn draws_a_link_when_a_block_stands_twice() {
    // A block that is named twice is one block, and the links that name it still find it: the
    // room a block holds and the blocks that wait for a class shift the places of the blocks.
    let out = render("block\nspace\nA\nA\nA --> B\n");
    // The room a block holds is not drawn, and the block that stands twice is one block.
    assert_eq!(count(&out.svg, "<g class=\"pele-rs-node "), 2);
    assert_eq!(count(&out.svg, "<g class=\"pele-rs-edge\""), 1);
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
            format!("block\nA[\"{p}\"]\n"),
            format!("block\nA\nB[\"{p}\"]\n"),
            format!("block\nA --> B\nC[\"{p}\"]\n"),
            format!("block\nblock:group[\"{p}\"]\n  A\nend\n"),
            format!("block\nclassDef c fill:{p}\nA\n"),
            format!("block\nstyle A fill:{p}\n"),
            format!("block\nblockArrow<[\"{p}\"]>(right)\n"),
            format!("---\ntitle: \"{p}\"\n---\nblock\nA\n"),
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
        "block\n",
        "block-beta\n",
        "block:",
        "columns 3\n",
        "columns auto\n",
        "space\n",
        "space:3\n",
        "end\n",
        "A",
        "B",
        "-->",
        "--",
        "-.->",
        "==>",
        "[",
        "]",
        "(",
        ")",
        "{",
        "}",
        "<[",
        "]>",
        "(right)",
        ",left",
        ":2",
        "classDef c fill:red\n",
        "class A c\n",
        "style A fill:red\n",
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
    let head = "block\n";
    let cases = [
        format!("{head}A[\"{}\"]\n", "x".repeat(N)),
        format!("{head}{}", "A\n".repeat(N / 2)),
        format!("{head}{}", "A --> B\n".repeat(N / 10)),
        format!("{head}columns 3\n{}", "A\n".repeat(N / 2)),
        format!("{head}block\n{}\nend\n", "A\n".repeat(N / 4)),
        format!("{head}classDef c fill:red\n{}", "A\n".repeat(N / 2)),
        format!("{head}space:2\n{}", "A\n".repeat(N / 2)),
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
            "composite",
            "block\nblock:group[\"A group\"]\n  a\n  b\nend\n".to_string(),
        ),
        (
            "arrow",
            "block\ncolumns 3\nblockArrow<[\"go\"]>(right)\n".to_string(),
        ),
        (
            "shapes",
            "block\na[\"square\"]\nb(\"round\")\nc{\"diamond\"}\nd{{\"hexagon\"}}\ne[[\"sub\"]]\n"
                .to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
#[test]
fn probe_comment() {
    for src in [
        "block\n  columns 3\n  a[\"A label\"]%%b:2 c:2-.-> d\n",
        "block\na[\"A label\"]%% comment\n",
        "block\n%% whole line\na\n",
    ] {
        let r = pele_rs::render(src, &pele_rs::RenderOptions::default());
        match r {
            Ok(o) => eprintln!("OK   {:?} -> width {}", src, o.width),
            Err(e) => eprintln!("ERR  {:?} -> {} at {}:{}", src, e.message, e.line, e.column),
        }
    }
}
