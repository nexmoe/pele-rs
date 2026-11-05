//! Flowchart rendering, security, limits and speed.
#![cfg(feature = "flowchart")]

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

include!("compat/flowchart/cases.rs");

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

/// The drawing of the node with the given id, without the group that holds it.
fn shape_of(svg: &str, id: &str) -> String {
    let at = svg
        .find(&format!("data-id=\"{id}\""))
        .expect("the node is drawn");
    let open = svg[at..].find('>').expect("the group is closed") + at + 1;
    let end = svg[open..].find("</g>").expect("the group ends") + open;
    svg[open..end].to_string()
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

/// Whether a drawing is the plain rectangle a shape falls back to when it has no drawing of
/// its own: one rectangle with nothing but the usual fill, stroke and corner radius.
fn is_plain_rect(drawing: &str) -> bool {
    let drawing = drawing.trim();
    drawing.starts_with("<rect ")
        && drawing.ends_with("/>")
        && drawing.matches("<rect").count() == 1
        && drawing.contains("style=\"rx:var(--_r)\"")
        && !drawing.contains("rx=\"")
        && !drawing.contains("<path")
        && !drawing.contains("<circle")
        && !drawing.contains("<ellipse")
        && !drawing.contains("<polygon")
}

#[test]
fn draws_nodes_edges_and_labels() {
    let out =
        render("flowchart TD\n A[Start] -->|yes| B{Choose}\n B --> C((End))\n B -.-> D>Flag]\n");
    assert_eq!(out.kind, DiagramKind::Flowchart);
    assert_eq!(count(&out.svg, "class=\"pele-node "), 4);
    assert_eq!(count(&out.svg, "class=\"pele-edge\""), 3);
    assert_eq!(count(&out.svg, "class=\"pele-edge-label\""), 1);
    assert!(out.svg.contains("pele-shape-diam"));
    assert!(out.svg.contains("pele-shape-circle"));
    assert!(out.svg.contains("pele-shape-odd"));
    assert!(out.svg.contains("stroke-dasharray=\"3 4\""));
    assert!(out.svg.contains(">yes<"));
}

#[test]
fn draws_every_documented_shape() {
    for shape in [
        "rect",
        "rounded",
        "stadium",
        "fr-rect",
        "cyl",
        "circle",
        "dbl-circ",
        "diam",
        "hex",
        "lean-r",
        "lean-l",
        "trap-b",
        "trap-t",
        "text",
        "sm-circ",
        "f-circ",
        "fr-circ",
        "fork",
        "notch-rect",
        "lin-rect",
        "div-rect",
        "win-pane",
        "tri",
        "flip-tri",
        "sl-rect",
        "notch-pent",
        "hourglass",
        "delay",
        "h-cyl",
        "cross-circ",
        "st-rect",
        "doc",
        "flag",
        "datastore",
        "folder",
        "bucket",
        "console",
        "browser",
        "person",
        "bang",
        "cloud",
        "brace",
        "brace-r",
        "braces",
        "bolt",
        "lin-cyl",
        "curv-trap",
        "docs",
        "bow-rect",
        "tag-rect",
        "tag-doc",
        "lin-doc",
        "odd",
    ] {
        let svg = svg(&format!("flowchart LR\n A@{{ shape: {shape} }} --> B\n"));
        assert!(svg.contains(&format!("pele-shape-{shape}")), "{shape}");
        // The class says which shape it is; the drawing has to be that shape and not the
        // rectangle a shape without a drawing of its own falls back to.
        assert!(!is_plain_rect(&shape_of(&svg, "A")), "{shape}");
        assert_inert(&svg, shape);
    }
    let err = pele_rs::render(
        "flowchart LR\n A@{ shape: nope }",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
}

#[test]
fn resolves_the_shape_aliases() {
    for (alias, canonical) in [
        ("decision", "diam"),
        ("db", "cyl"),
        ("subprocess", "fr-rect"),
        ("terminal", "stadium"),
        ("start", "sm-circ"),
        ("stop", "fr-circ"),
        ("junction", "f-circ"),
        ("document", "doc"),
        ("disk", "lin-cyl"),
    ] {
        let alias_svg = svg(&format!("flowchart LR\n A@{{ shape: {alias} }} --> B\n"));
        let canonical_svg = svg(&format!(
            "flowchart LR\n A@{{ shape: {canonical} }} --> B\n"
        ));
        assert!(
            shape_of(&alias_svg, "A") == shape_of(&canonical_svg, "A"),
            "{alias} is not drawn as {canonical}"
        );
        assert!(
            alias_svg.contains(&format!("pele-shape-{canonical}")),
            "{alias}"
        );
    }
}

#[test]
fn draws_subgraphs_with_their_own_direction() {
    let svg = svg(
        "flowchart TB\n c1-->a2\n subgraph one\n a1-->a2\n end\n subgraph two\n direction LR\n b1-->b2\n end\n one --> two\n",
    );
    assert_eq!(count(&svg, "class=\"pele-cluster\""), 2);
    assert!(svg.contains(">one<") && svg.contains(">two<"));
    // The subgraph that sets a direction lays its own nodes out in it: the two nodes of `two`
    // are in one row, the two nodes of `one` are not.
    let (b1, b2) = (at_of(&svg, "b1"), at_of(&svg, "b2"));
    assert_eq!(b1.1, b2.1, "the nodes of `two` are in one row");
    assert_ne!(b1.0, b2.0, "the nodes of `two` are side by side");
    let (a1, a2) = (at_of(&svg, "a1"), at_of(&svg, "a2"));
    assert_ne!(a1.1, a2.1, "the nodes of `one` are stacked");
}

#[test]
fn applies_classes_and_styles_safely() {
    let svg = svg(
        "flowchart LR\n A:::hot --> B\n classDef hot fill:#f96,stroke:#333,color:#fff\n style B fill:javascript:alert,stroke:red\n linkStyle 0 stroke:#0a0\n",
    );
    assert!(svg.contains("fill:#f96;stroke:#333;"));
    assert!(svg.contains("pele-node pele-shape-rect hot"));
    assert!(!svg.contains("javascript"));
    assert!(svg.contains("stroke:red;"));
    // The colour belongs to the edge, not to the arrowhead that happens to carry it as well.
    assert!(
        svg.contains("<g class=\"pele-edge\" data-id=\"L_A_B_0\" style=\"stroke:#0a0;\">"),
        "{svg}"
    );
}

#[test]
fn keeps_only_allowed_links() {
    let src = "flowchart LR\n A --> B --> C --> D\n click A \"https://example.com\" \"Tip\"\n click B \"javascript:alert(1)\"\n click C \"file:///etc/passwd\"\n click D \"/docs\" _blank\n";
    let out = render(src);
    assert!(
        out.svg
            .contains("<a href=\"https://example.com/\" rel=\"noopener\">")
    );
    assert!(out.svg.contains("<title>Tip</title>"));
    assert!(
        out.svg
            .contains("<a href=\"/docs\" target=\"_blank\" rel=\"noopener\">")
    );
    assert!(!out.svg.contains("javascript:") && !out.svg.contains("file:"));
    let hrefs: Vec<&str> = out.links.iter().map(|l| l.href.as_str()).collect();
    assert_eq!(hrefs, ["https://example.com/", "/docs"]);

    let options = RenderOptions {
        link_schemes: Some(vec![]),
        link_rel: Some(String::new()),
        ..RenderOptions::default()
    };
    let out = pele_rs::render(src, &options).unwrap();
    assert_eq!(out.links.len(), 1);
    assert!(out.svg.contains("<a href=\"/docs\" target=\"_blank\">"));
}

#[test]
fn reports_errors_with_positions() {
    let err = pele_rs::render(
        "flowchart TD\n A --> B\n B --> )\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Flowchart));
    assert_eq!((err.line, err.column), (3, 8));
    assert_eq!(err.snippet, " B --> )\n-------^");
    let err = pele_rs::render(
        "flowchart TD\n A --> B\n linkStyle 5 stroke:red\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
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
    let wide = "flowchart TD\n a & b & c & d --> e & f & g\n";
    assert_eq!(
        pele_rs::render(wide, &options).unwrap_err().code,
        ErrorCode::Limit
    );
    let deep = format!(
        "flowchart TD\n{}x\n{}",
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
        pele_rs::render("flowchart TD\n A --> B --> C\n", &options)
            .unwrap_err()
            .code,
        ErrorCode::Limit
    );
}

#[test]
fn turns_down_to_fit_a_narrow_host() {
    let src =
        "flowchart LR\n A[First step] --> B[Second step] --> C[Third step] --> D[Fourth step]\n";
    let wide = render(src);
    let options = RenderOptions {
        max_width: Some(320.0),
        ..RenderOptions::default()
    };
    let narrow = pele_rs::render(src, &options).unwrap();
    assert!(narrow.width < wide.width);
    assert!(narrow.height > wide.height);
    let fixed = RenderOptions {
        max_width: Some(320.0),
        auto_direction: false,
        ..RenderOptions::default()
    };
    assert!(pele_rs::render(src, &fixed).unwrap().width > narrow.width);
}

#[test]
fn output_is_deterministic() {
    let src = "flowchart TD\n subgraph s\n a-->b & c\n end\n c-->d-->a\n d-.->|x| b\n";
    assert_eq!(svg(src), svg(src));
}

#[test]
fn is_inert_for_corpus_hostile_and_mutated_inputs() {
    let corpus: Vec<&str> = CASES
        .iter()
        .map(|c| c.src)
        .filter(|s| s.contains("graph") || s.contains("flowchart"))
        .collect();
    for src in &corpus {
        if let Some(svg) = try_render(src) {
            assert_inert(&svg, src);
        }
    }
    let mut rendered = 0;
    for payload in PAYLOADS {
        for src in [
            format!("flowchart TD\n A[\"{payload}\"] --> B"),
            format!("flowchart TD\n A -->|\"{payload}\"| B"),
            format!("flowchart TD\n subgraph s [\"{payload}\"]\n A\n end"),
            format!("flowchart TD\n A --> B\n click A \"{payload}\" \"{payload}\""),
            format!("flowchart TD\n A --> B\n click A href \"{payload}\""),
            format!("flowchart TD\n A --> B\n style A fill:{payload}"),
            format!("flowchart TD\n A:::c --> B\n classDef c color:{payload}"),
            format!("flowchart TD\n A@{{ img: \"{payload}\", label: \"x\" }}"),
            format!("flowchart TD\n A@{{ icon: \"{payload}\" }}"),
            format!("flowchart TD\n A[\"`**{payload}**`\"]"),
            format!(
                "---\ntitle: \"{payload}\"\n---\nflowchart TD\n accTitle: {payload}\n accDescr: {payload}\n A"
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
    assert!(rendered > 300, "only {rendered} hostile inputs rendered");
    let mut fragments: Vec<&str> = PAYLOADS.to_vec();
    fragments.extend([
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

fn graph_with(edges: usize, nodes: usize, seed: u32) -> String {
    let mut rnd = Random::new(seed);
    let mut src = String::from("flowchart TD\n");
    for _ in 0..edges {
        let a = (rnd.next() * nodes as f64) as usize;
        let b = (rnd.next() * nodes as f64) as usize;
        let _ = writeln!(src, "n{a}-->n{b}");
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
        (graph_with(5000, 1000, 1), true),
        (
            format!(
                "flowchart LR\n{}",
                (0..2000).fold(String::new(), |mut s, i| {
                    let _ = writeln!(s, "a{i}-->a{}", i + 1);
                    s
                })
            ),
            true,
        ),
        (
            format!(
                "flowchart TD\n{} --> {}\n",
                vec!["a"; 70].join(" & "),
                vec!["b"; 70].join(" & ")
            ),
            true,
        ),
        (format!("flowchart TD\nA[\"{}\"]", "x ".repeat(25000)), true),
        (format!("flowchart TD\n{}", "A-->".repeat(12000)), false),
        (graph_with(12000, 1000, 2), true),
        (format!("flowchart TD\n{}", "%%{\n".repeat(12000)), false),
        (format!("flowchart TD\n{}", "direction".repeat(5000)), true),
        (format!("flowchart TD\n{}", "@".repeat(50000)), false),
        (format!("flowchart TD\nA{}", " ".repeat(50000)), true),
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
fn draws_the_arrowhead_each_edge_kind_asks_for() {
    let point = svg("flowchart LR\n A --> B\n");
    assert!(point.contains("<path class=\"pele-marker\""), "{point}");
    let cross = svg("flowchart LR\n A --x B\n");
    assert!(
        cross.contains("<path class=\"pele-marker\" d=\"M")
            && !cross.contains("fill=\"var(--_l)\""),
        "{cross}"
    );
    let circle = svg("flowchart LR\n A o--o B\n");
    assert_eq!(
        count(&circle, "<circle class=\"pele-marker\""),
        2,
        "{circle}"
    );
    let double = svg("flowchart LR\n A <--> B\n");
    assert_eq!(count(&double, "<path class=\"pele-marker\""), 2, "{double}");
    assert!(svg("flowchart LR\n A ==> B\n").contains("stroke-width=\"2.5\""));
}

#[test]
fn an_invisible_link_takes_room_but_draws_nothing() {
    let plain = render("flowchart LR\n A --> B\n");
    let hidden = render("flowchart LR\n A ~~~ B\n");
    assert!(
        !hidden.svg.contains("class=\"pele-edge\""),
        "{}",
        hidden.svg
    );
    assert_eq!(hidden.width, plain.width, "the room is still taken");
}

#[test]
fn styles_every_edge_with_linkstyle_default_and_keeps_only_known_properties() {
    let styled = svg("flowchart LR\n A --> B\n B --> C\n linkStyle default stroke:#0a0\n");
    assert_eq!(count(&styled, "style=\"stroke:#0a0;\""), 2, "{styled}");
    assert!(svg("flowchart LR\n A --> B\n style A fill:red\n").contains("fill:red;"));
    assert!(!svg("flowchart LR\n A --> B\n style A background:red\n").contains("background"));
    assert!(!svg("flowchart LR\n A --> B\n style A fill:image-set\n").contains("image-set"));
}

#[test]
fn applies_a_class_to_several_nodes() {
    let svg = svg("flowchart LR\n A --> B\n classDef hot fill:#f96\n class A,B hot\n");
    assert_eq!(count(&svg, "pele-node pele-shape-rect hot"), 2, "{svg}");
}

#[test]
fn names_edges_and_marks_callback_clicks_without_a_link() {
    let out = render("flowchart LR\n A e1@--> B\n click A callback\n");
    assert!(out.svg.contains("data-id=\"e1\""), "{}", out.svg);
    assert!(out.links.is_empty());
    assert!(
        out.svg.contains("pele-node pele-shape-rect clickable"),
        "{}",
        out.svg
    );
}

#[test]
fn draws_markdown_and_decodes_entities_in_labels() {
    let bold = svg("flowchart LR\n A[\"`**bold**`\"] --> B\n");
    assert!(bold.contains("font-weight=\"var(--_w)\""), "{bold}");
    let entity = svg("flowchart LR\n A[\"a &amp; b\"] --> B\n");
    assert!(entity.contains(">a &amp; b<"), "{entity}");
}

#[test]
fn reads_the_flowchart_config_from_an_init_directive() {
    let plain = render("flowchart LR\n A --> B\n");
    let spaced =
        render("%%{init: {\"flowchart\": {\"rankSpacing\": 200}}}%%\nflowchart LR\n A --> B\n");
    assert!(
        spaced.width > plain.width,
        "{} vs {}",
        spaced.width,
        plain.width
    );
    let wide = render("flowchart TD\n A[This is a very long label indeed] --> B\n");
    let narrow = render(
        "%%{init: {\"flowchart\": {\"wrappingWidth\": 40}}}%%\nflowchart TD\n A[This is a very long label indeed] --> B\n",
    );
    assert!(
        narrow.width < wide.width,
        "{} vs {}",
        narrow.width,
        wide.width
    );
}

#[test]
fn writes_the_accessible_title_and_description() {
    let svg = svg("flowchart LR\n accTitle: the title\n accDescr: the description\n A --> B\n");
    assert!(
        svg.contains("<title id=\"pele-title\">the title</title>"),
        "{svg}"
    );
    assert!(
        svg.contains("<desc id=\"pele-desc\">the description</desc>"),
        "{svg}"
    );
    assert!(svg.contains("aria-labelledby=\"pele-title\""), "{svg}");
    assert!(svg.contains("aria-describedby=\"pele-desc\""), "{svg}");
}

#[test]
fn draws_nested_subgraphs_and_stops_at_the_source_limit() {
    let svg = svg("flowchart TB\n subgraph outer\n subgraph inner\n a-->b\n end\n end\n");
    assert_eq!(count(&svg, "class=\"pele-cluster\""), 2, "{svg}");
    let err = pele_rs::render(
        &format!("flowchart TD\n{}", "A".repeat(60_000)),
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Limit);
}

#[test]
fn snapshots() {
    let cases = [
        (
            "decision",
            "flowchart TD\n A[Christmas] -->|Get money| B(Go shopping)\n B --> C{Let me think}\n C -->|One| D[Laptop]\n C -->|Two| E[iPhone]\n C -->|Three| F[Car]\n",
        ),
        (
            "subgraphs",
            "flowchart TB\n c1-->a2\n subgraph one\n a1-->a2\n end\n subgraph two\n b1-->b2\n end\n subgraph three\n c1-->c2\n end\n one --> two\n three --> two\n two --> c2\n",
        ),
        (
            "styles_and_links",
            "---\ntitle: Styled\n---\nflowchart LR\n A:::hot -- text --> B((Circle))\n B ==> C[(Database)]\n C -.-> D{{Hexagon}}\n D --o E>Flag] --x A\n classDef hot fill:#f96\n click B \"https://example.com\"\n",
        ),
        (
            "self_loop",
            "flowchart LR\n A --> A\n A -->|again| A\n A --> B\n",
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(src));
    }
}
