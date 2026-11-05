//! Tree view rendering, security, limits and speed.
#![cfg(feature = "treeview")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "treeView-beta
title My project
src/
    index.js
    components/
        App.tsx
        Button.tsx
    utils/
        format.ts
package.json
README.md
";

fn render(src: &str) -> pele_rs::Rendered {
    pele_rs::render(src, &RenderOptions::default()).unwrap_or_else(|e| panic!("{src:?}: {e}"))
}

fn narrow(src: &str) -> pele_rs::Rendered {
    let options = RenderOptions {
        max_width: Some(120.0),
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
fn draws_the_nodes_of_the_tree() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::TreeView);
    // Seven nodes: `src`, `index.js`, `components`, `App.tsx`, `Button.tsx`, `utils`,
    // `format.ts`, `package.json` and `README.md` are nine.
    assert_eq!(count(&out.svg, "pele-node "), 9);
    assert_eq!(count(&out.svg, "pele-tree-dir"), 3);
    assert_eq!(count(&out.svg, "pele-tree-file"), 6);
    assert_eq!(count(&out.svg, "pele-tree-guides"), 1);
    for needle in [
        "data-id=\"src\"",
        "data-id=\"index.js\"",
        "data-id=\"components\"",
        "data-id=\"App.tsx\"",
        ">My project<",
    ] {
        assert!(out.svg.contains(needle), "{needle}");
    }
    assert!(out.links.is_empty());
}

#[test]
fn draws_the_lines_that_join_a_node_to_the_ones_below_it() {
    let out = render(FULL);
    // The lines of `src`, `components` and `utils` join their children.
    assert_eq!(count(&out.svg, "<path d=\"M"), 3);
    assert!(out.svg.contains("pele-tree-guides"));
}

#[test]
fn reads_a_tree_that_is_drawn_with_box_drawing_characters() {
    let out = render("treeView-beta\n├── src\n│   └── index.js\n└── package.json\n");
    assert_eq!(count(&out.svg, "pele-node "), 3);
    assert!(out.svg.contains("data-id=\"src\""));
    assert!(out.svg.contains("data-id=\"index.js\""));
    assert!(out.svg.contains("data-id=\"package.json\""));
    // The tree of the box-drawing text is the tree of the indented text.
    let indented = render("treeView-beta\n    src\n        index.js\n    package.json\n");
    assert_eq!(out.svg, indented.svg);
}

#[test]
fn reads_the_annotations_of_a_node() {
    let out = render(
        "treeView-beta\n    a.txt :::highlight\n    b.txt icon(logos:react)\n    c.txt ## a note\n",
    );
    // The class of a node is a class of the node.
    assert!(out.svg.contains(" highlight"), "{}", out.svg);
    // Icons retain their resolver name and use a built-in vector fallback.
    assert!(
        out.svg
            .contains("<svg class=\"pele-icon\" data-icon=\"logos:react\""),
        "{}",
        out.svg
    );
    assert!(out.svg.contains(">a note<"), "{}", out.svg);
}

#[test]
fn draws_no_icon_when_none_is_named() {
    let out = render("treeView-beta\n    a.txt\n");
    assert!(!out.svg.contains("pele-icon"), "{}", out.svg);
}

#[test]
fn runs_down_in_a_narrow_host() {
    let wide = render(FULL);
    let down = narrow(FULL);
    assert!(down.width < wide.width);
    // A name that does not fit is broken over several lines.
    let long = narrow("treeView-beta\n    averyveryverylongfilename-indeed.txt\n");
    assert!(
        count(&long.svg, "<text class=\"pele-label\"") > 1,
        "{}",
        long.svg
    );
    assert!(long.height > render("treeView-beta\n    a\n").height);
}

#[test]
fn reports_errors_with_positions() {
    // A line that is indented but holds no name.
    let err =
        pele_rs::render("treeView-beta\n    a\n    \n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::TreeView));

    // A branch without a name.
    let err = pele_rs::render("treeView-beta\n├──\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.line, 2);

    // A line that is indented in a tree that is drawn with box-drawing characters.
    let err =
        pele_rs::render("treeView-beta\n├── a\n    b\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.line, 3);
}

#[test]
fn output_is_deterministic() {
    assert_eq!(svg(FULL), svg(FULL));
    assert_eq!(narrow(FULL).svg, narrow(FULL).svg);
}

#[test]
fn is_inert_for_hostile_and_mutated_inputs() {
    let mut rendered = 0;
    for payload in PAYLOADS {
        let p = payload.replace('\n', " ");
        for src in [
            format!("treeView-beta\n    {p}\n"),
            format!("treeView-beta\n    a {p}\n"),
            format!("treeView-beta\n    a :::{p}\n"),
            format!("treeView-beta\n    a icon({p})\n"),
            format!("treeView-beta\n    a ## {p}\n"),
            format!("treeView-beta\n    \"{p}\"\n"),
            format!("---\ntitle: \"{p}\"\n---\ntreeView-beta\n    a"),
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
    fragments.extend(["\n", "    ", ":::", "icon(", "##", "\"", "'", "├──", "└──"]);
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

fn repeat(n: usize, line: impl Fn(usize) -> String) -> String {
    (0..n).map(line).collect()
}

#[test]
fn stays_fast_on_large_and_hostile_input() {
    // Debug builds are several times slower than release.
    let budget = Duration::from_secs(if cfg!(debug_assertions) { 30 } else { 3 });
    let options = |max_width| RenderOptions {
        max_width,
        limits: Limits {
            source: usize::MAX,
            output: usize::MAX,
            ..Limits::default()
        },
        ..RenderOptions::default()
    };
    const N: usize = 50000;
    let head = "treeView-beta\n";
    let cases = [
        format!("{head}    {}\n", "x".repeat(N)),
        format!("{head}    {}\n", "x ".repeat(N / 2)),
        format!("{head}{}", repeat(4000, |i| format!("    a{i}\n"))),
        // A chain of nodes, each set in one further than the last.
        format!(
            "{head}{}",
            repeat(2000, |i| format!("{}a{i}\n", " ".repeat(i + 1)))
        ),
        format!("{head}    {}\n", " ".repeat(N)),
        format!("{head}{}", "%%\n".repeat(N / 3)),
        format!(
            "{head}├── a\n{}",
            repeat(2000, |i| format!("{}b{i}\n", "│   ".repeat(i)))
        ),
    ];
    let mut rendered = 0;
    for src in &cases {
        for max_width in [None, Some(300.0)] {
            let started = Instant::now();
            let out = pele_rs::render(src, &options(max_width));
            let took = started.elapsed();
            assert!(
                took < budget,
                "{:?} took {took:?}",
                src.chars().take(40).collect::<String>()
            );
            if out.is_ok() {
                rendered += 1;
            }
        }
    }
    // A renderer that refused everything would answer at once and say nothing about speed.
    assert!(
        rendered >= 12,
        "only {rendered} of {} cases were drawn",
        cases.len() * 2
    );
}

#[test]
fn keeps_the_accessibility_statements() {
    let out = render("treeView-beta\naccTitle: The tree\naccDescr: A tree of files\n    a\n");
    assert!(out.svg.contains("<title id="), "{}", out.svg);
    assert!(out.svg.contains("The tree"), "{}", out.svg);
}

#[test]
fn snapshots() {
    insta::assert_snapshot!("across", svg(FULL));
    insta::assert_snapshot!("down", narrow(FULL).svg);
}

#[test]
fn reads_the_settings_of_the_diagram() {
    // The settings of the diagram come from the front matter.
    let src =
        "---\nconfig:\n  treeView:\n    showIcons: true\n---\ntreeView-beta\n    a.txt\n    b/\n";
    let out = render(src);
    assert!(out.svg.contains("pele-icon"), "{}", out.svg);
    assert!(
        out.svg.contains("data-icon=\"mermaid-treeview:file\""),
        "{}",
        out.svg
    );
    assert!(
        out.svg.contains("data-icon=\"mermaid-treeview:folder\""),
        "{}",
        out.svg
    );
}
