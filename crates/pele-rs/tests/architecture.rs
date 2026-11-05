//! Architecture diagram rendering, security, limits and speed.
#![cfg(feature = "architecture")]

mod support;

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "architecture-beta
    group api(cloud)[API]
    service db(database)[Database] in api
    service server(server)[Server] in api
    db:L -- R:server
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
fn draws_services_and_groups() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Architecture);
    // The two services and the one group.
    assert_eq!(count(&out.svg, "<g class=\"pele-node pele-service\""), 2);
    assert_eq!(count(&out.svg, "<g class=\"pele-cluster\""), 1);
    // The edge between the two services.
    assert_eq!(count(&out.svg, "<g class=\"pele-edge\""), 1);
    // The name of a service is the label of its icon, and the name of the group is its title.
    assert!(out.svg.contains(">Database<"), "{}", out.svg);
    assert!(out.svg.contains(">Server<"), "{}", out.svg);
    assert!(out.svg.contains(">API<"), "{}", out.svg);
    // The icon a service names is kept.
    assert!(out.svg.contains("data-icon=\"database\""), "{}", out.svg);
}

#[test]
fn draws_a_junction_as_a_dot() {
    let out = render(
        "architecture-beta\nservice a(server)[A]\njunction mid\nservice b(server)[B]\na:R -- L:mid\n",
    );
    assert_eq!(count(&out.svg, "<g class=\"pele-node pele-junction\""), 1);
    assert_eq!(count(&out.svg, "<g class=\"pele-node pele-service\""), 2);
}

#[test]
fn draws_the_arrowheads_of_the_edges() {
    // The arrow of an edge is written at the end it points to.
    let out = render(FULL);
    assert_eq!(count(&out.svg, "pele-marker"), 0);
    let into =
        render("architecture-beta\nservice a(server)[A]\nservice b(server)[B]\na:R --> L:b\n");
    assert_eq!(count(&into.svg, "pele-marker"), 1);
}

#[test]
fn draws_the_label_of_an_edge() {
    let out = render(
        "architecture-beta\nservice a(server)[A]\nservice b(server)[B]\na:R - [reads] - L:b\n",
    );
    assert!(out.svg.contains(">reads<"), "{}", out.svg);
    assert_eq!(count(&out.svg, "<g class=\"pele-edge-label\""), 1);
}

#[test]
fn puts_the_services_side_by_side() {
    // An edge says which end is on the left, so the services end up on that side.
    let across = svg("architecture-beta\nservice a(server)[A]\nservice b(server)[B]\na:R -- L:b\n");
    let down = svg("architecture-beta\nservice a(server)[A]\nservice b(server)[B]\na:B -- T:b\n");
    assert_ne!(across, down);
}

#[test]
fn reads_the_config() {
    // The size of the icon of a service is set in the config.
    let small = svg("architecture-beta\nservice a(server)[A]\n");
    let large = svg(
        "---\nconfig:\n  architecture:\n    iconSize: 120\n---\narchitecture-beta\nservice a(server)[A]\n",
    );
    assert_ne!(small, large);
}

#[test]
fn draws_a_title() {
    let out = render(
        "---\ntitle: An architecture diagram\n---\narchitecture-beta\nservice a(server)[A]\n",
    );
    assert!(out.svg.contains(">An architecture diagram<"), "{}", out.svg);
    assert!(out.svg.contains("pele-title"));
}

#[test]
fn draws_the_accessibility_text() {
    let out = render(
        "architecture-beta\naccTitle: The services\naccDescr: Two services and an edge\nservice a(server)[A]\n",
    );
    assert!(
        out.svg
            .contains("<title id=\"pele-title\">The services</title>"),
        "{}",
        out.svg
    );
    assert!(
        out.svg
            .contains("<desc id=\"pele-desc\">Two services and an edge</desc>"),
        "{}",
        out.svg
    );
}

#[test]
fn reports_errors_with_positions() {
    // A word of the grammar is not a name.
    let err = pele_rs::render(
        "architecture-beta\nservice align(server)[x]\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Architecture));

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
fn refuses_a_service_in_a_group_that_is_not_there() {
    let err = pele_rs::render(
        "architecture-beta\nservice a(server)[A] in g\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert_eq!(err.kind, Some(DiagramKind::Architecture));
    assert!(
        err.message.contains("parent does not exist"),
        "{}",
        err.message
    );
}

#[test]
fn refuses_a_name_that_is_in_use_twice() {
    let err = pele_rs::render(
        "architecture-beta\nservice a(server)[A]\nservice a(server)[B]\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert!(err.message.contains("already in use"), "{}", err.message);
}

#[test]
fn refuses_an_alignment_of_a_name_that_is_not_there() {
    let err = pele_rs::render(
        "architecture-beta\nservice a(server)[A]\nalign row a b\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert!(
        err.message.contains("not a service or junction"),
        "{}",
        err.message
    );
}

#[test]
fn fits_a_narrow_host() {
    let wide = render(FULL);
    let narrow = narrow(FULL);
    assert!(narrow.width <= 360.0, "{}", narrow.width);
    assert!(narrow.height > 0.0);
    // The services are still drawn.
    assert_eq!(count(&narrow.svg, "<g class=\"pele-node pele-service\""), 2);
    let _ = wide;
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
            format!("architecture-beta\nservice a(server)[{p}]\n"),
            format!("architecture-beta\ngroup g(cloud)[{p}]\n"),
            format!(
                "architecture-beta\nservice a(server)[A]\nservice b(server)[B]\na:R - [{p}] - L:b\n"
            ),
            "architecture-beta\nservice a(server)[A]\nalign row a a\n".to_string(),
            format!("architecture-beta\ntitle {p}\nservice a(server)[A]\n"),
            format!("architecture-beta\naccDescr: {p}\nservice a(server)[A]\n"),
            format!("architecture-beta\nservice a(\"{p}\")[A]\n"),
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
        "architecture-beta\n",
        "service ",
        "group ",
        "junction ",
        "align row ",
        " in ",
        "(database)",
        "[Name]",
        "{group}",
        ":L",
        ":R",
        ":T",
        ":B",
        " -- ",
        " --> ",
        " - [label] - ",
        "\n",
        " ",
        "%%",
        "é",
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
    // A parser that refused every mutation would make the loop above assert nothing at all. The
    // grammar of the diagram is strict, so most mutations leave a diagram that is not one.
    assert!(rendered > 50, "only {rendered} of 1500 mutations rendered");
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
    const N: usize = 5000;
    let head = "architecture-beta\n";
    let cases = [
        format!("{head}service a(server)[{}]\n", "x".repeat(N * 10)),
        format!(
            "{head}{}",
            (0..N).fold(String::new(), |mut s, i| {
                let _ = writeln!(s, "service a{i}(server)[A]");
                s
            })
        ),
        format!(
            "{head}{}",
            (0..N / 3).fold(String::new(), |mut s, i| {
                let _ = writeln!(s, "service a{i}(server)[A]");
                let _ = writeln!(s, "service b{i}(server)[B]");
                let _ = writeln!(s, "a{i}:R -- L:b{i}");
                s
            })
        ),
        format!(
            "{head}group g(cloud)[G]\n{}",
            (0..N / 2).fold(String::new(), |mut s, i| {
                let _ = writeln!(s, "service a{i}(server)[A] in g");
                s
            })
        ),
        format!(
            "{head}{}",
            (0..N / 4).fold(String::new(), |mut s, i| {
                let _ = writeln!(s, "service a{i}(server)[A]");
                let _ = writeln!(s, "service b{i}(server)[B]");
                let _ = writeln!(s, "align row a{i} b{i}");
                s
            })
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
        rendered >= 4,
        "only {rendered} of {} cases were drawn",
        cases.len()
    );
}

#[test]
fn snapshots() {
    let cases = [
        ("full", FULL.to_string()),
        (
            "groups",
            "architecture-beta\ngroup sub1(cloud)[Subscription A]\ngroup vnet1(cloud)[VNet A] in sub1\nservice vm1(server)[VM] in vnet1\nservice reg(database)[Registry] in vnet1\nreg:B --> T:vm1\n"
                .to_string(),
        ),
        (
            "junction",
            "architecture-beta\nservice a(server)[A]\njunction mid\nservice b(server)[B]\na:R -- L:mid\n"
                .to_string(),
        ),
        (
            "edge_label",
            "architecture-beta\nservice a(server)[A]\nservice b(server)[B]\na:R - [reads] - L:b\n"
                .to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
