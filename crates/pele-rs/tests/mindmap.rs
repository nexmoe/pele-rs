//! Mind map rendering, security, limits and speed.
#![cfg(feature = "mindmap")]

mod support;

use std::fmt::Write;
use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "mindmap
  root((mindmap))
    Origins
      Long history
      ::icon(fa fa-book)
      Popularisation
        British popular psychology author Tony Buzan
    Research
      On effectiveness<br/>and features
      On Automatic creation
        Uses
            Creative techniques
            Strategic planning
    Tools
      Pen and paper
      Mermaid
      :::urgent
    id1[Square]
    id2(Rounded)
    id4)Cloud(
    id5))Bang((
    id6{{Hexagon}}
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
fn draws_branches_and_shapes() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Mindmap);
    assert_eq!(count(&out.svg, "pele-node "), 19);
    assert_eq!(count(&out.svg, "pele-edge\""), 18);
    assert_eq!(count(&out.svg, "pele-branch pele-section-"), 8);
    assert_eq!(count(&out.svg, "pele-shape-branch"), 6);
    assert_eq!(count(&out.svg, " pele-root"), 1);
    for needle in [
        "pele-shape-circle pele-root",
        "pele-shape-cloud",
        "pele-shape-bang",
        "pele-shape-hex",
        "pele-shape-rect",
        "pele-shape-rounded",
        " urgent\"",
        ">Origins<",
    ] {
        assert!(out.svg.contains(needle), "{needle}");
    }
    assert!(out.links.is_empty());
}

#[test]
fn runs_down_in_a_narrow_host() {
    let wide = render(FULL);
    let down = narrow(FULL);
    assert!(down.width <= 360.0);
    assert!(down.height > wide.height);
    // The root becomes a pill heading the outline.
    assert!(down.svg.contains("pele-shape-stadium pele-root"));
}

#[test]
fn colors_wrap_after_eleven_branches() {
    let mut src = String::from("mindmap\n root\n");
    for i in 0..13 {
        writeln!(src, "  b{i}").unwrap();
    }
    let svg = svg(&src);
    assert!(svg.contains("pele-section-10\""));
    assert!(!svg.contains("pele-section-11\""));
    assert_eq!(count(&svg, "pele-section-0\""), 2);
}

/// The `stroke` of every link, in the order the links are drawn.
fn link_colors(svg: &str) -> Vec<&str> {
    svg.split("<path class=\"pele-edge\"")
        .skip(1)
        .map(|p| match p.strip_prefix(" stroke=\"") {
            Some(rest) => rest.split('"').next().unwrap(),
            None => "",
        })
        .collect()
}

#[test]
fn colors_by_depth_across_branches() {
    // Mermaid 12.2.0: `mindmap.colorBy: depth` gives one color to a level in every branch.
    let body = "mindmap
  root
    a
      a1
        a2
    b
      b1
        b2
";
    let by_depth = svg(&format!(
        "---\nconfig:\n  mindmap:\n    colorBy: depth\n---\n{body}"
    ));
    let colors = link_colors(&by_depth);
    // a, a1, a2, b, b1, b2: the same level has the same color, and the levels differ.
    assert_eq!(colors.len(), 6);
    assert_eq!(colors[0], colors[3]);
    assert_eq!(colors[1], colors[4]);
    assert_eq!(colors[2], colors[5]);
    assert_ne!(colors[0], colors[1]);
    assert_ne!(colors[1], colors[2]);

    // The default, and `branch`, keep a color per branch, and write no color on a link.
    let by_branch = svg(body);
    assert!(link_colors(&by_branch).iter().all(|c| c.is_empty()));
    assert_eq!(
        by_branch,
        svg(&format!(
            "---\nconfig:\n  mindmap:\n    colorBy: branch\n---\n{body}"
        ))
    );
    assert_ne!(by_branch, by_depth);
}

#[test]
fn depth_colors_wrap_after_eleven_levels() {
    let mut src =
        String::from("---\nconfig:\n  mindmap:\n    colorBy: depth\n---\nmindmap\n  root\n");
    for d in 1..=13 {
        let _ = writeln!(src, "{}n{d}", "  ".repeat(d + 1));
    }
    let out = svg(&src);
    let colors = link_colors(&out);
    assert_eq!(colors.len(), 13);
    // Level 12 is level 1 again, as the thirteen levels of Mermaid's own spec.
    assert_eq!(colors[0], colors[11]);
    assert_eq!(colors[1], colors[12]);
}

#[test]
fn reports_errors_with_positions() {
    let err = pele_rs::render("mindmap\n  root\n a\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert_eq!(err.kind, Some(DiagramKind::Mindmap));
    assert!(err.message.contains("only one root"));

    // The bracket is still open at the end of the text.
    let err = pele_rs::render("mindmap\n  root[a\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.line, 2);
}

#[test]
fn output_is_deterministic() {
    assert_eq!(svg(FULL), svg(FULL));
    assert_eq!(narrow(FULL), narrow(FULL));
}

#[test]
fn is_inert_for_hostile_and_mutated_inputs() {
    let mut rendered = 0;
    for payload in PAYLOADS {
        let p = payload.replace('\n', " ");
        for src in [
            format!("mindmap\n  r[{p}]\n    a[{p}]"),
            format!("mindmap\n  r\n    a\n    :::{p}"),
            format!("mindmap\n  r\n    a\n    ::icon({p})"),
            format!("mindmap\n  r\n    a(\"`{p}`\")"),
            format!("---\ntitle: \"{p}\"\n---\nmindmap\n  r"),
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
    fragments.extend(["\n", "  ", ":::", "::icon(", "[", "]", "((", "))", "<br>"]);
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
    let head = "mindmap\n";
    let cases = [
        format!("{head}  r[{}]", "x".repeat(N)),
        format!("{head}  r[{}]", "x ".repeat(N / 2)),
        format!("{head}  r\n{}", repeat(4000, |i| format!("    a{i}\n"))),
        // A chain of nodes, each set in one further than the last.
        format!(
            "{head}{}",
            repeat(2000, |i| format!("{}a{i}\n", " ".repeat(i + 1)))
        ),
        format!(
            "{head}  r\n{}",
            repeat(2000, |i| format!("   a{i}\n    b{i}\n"))
        ),
        format!("{head}{}", " ".repeat(N)),
        format!("{head}{}", "%%\n".repeat(N / 3)),
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
                &src[..src.len().min(40)]
            );
            if out.is_ok() {
                rendered += 1;
            }
        }
    }
    // A renderer that refused everything would answer at once and say nothing about speed.
    assert!(
        rendered >= 10,
        "only {rendered} of {} cases were drawn",
        cases.len() * 2
    );
}

#[test]
fn keeps_the_node_an_icon_belongs_to() {
    let out = render("mindmap\n  root\n    a\n    ::icon(fa fa-book)\n");
    assert!(out.svg.contains("data-id=\"a\""), "{}", out.svg);
    assert!(out.svg.contains(">a<"), "{}", out.svg);
}

#[test]
fn an_accessibility_statement_becomes_a_node() {
    // Mermaid's mindmap grammar has no accessibility statements, so a line that looks like one
    // is a node. Mermaid does the same, so this is ported as it is.
    let out = render("mindmap\n  root\n    accTitle: M\n");
    assert_eq!(count(&out.svg, "pele-node "), 2, "{}", out.svg);
    assert!(out.svg.contains(">accTitle: M<"), "{}", out.svg);
    assert!(!out.svg.contains("<title id="), "{}", out.svg);
}

#[test]
fn snapshots() {
    insta::assert_snapshot!("across", svg(FULL));
    insta::assert_snapshot!("down", narrow(FULL).svg);
}
