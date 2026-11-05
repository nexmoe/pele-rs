//! Class diagram rendering, security, limits and speed.
#![cfg(feature = "class")]

mod support;

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

include!("compat/class/cases.rs");

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

fn corpus() -> Vec<&'static str> {
    CASES
        .iter()
        .map(|c| c.src)
        .filter(|s| s.contains("classDiagram"))
        .collect()
}

#[test]
fn draws_classes_members_and_relations() {
    let out = render(
        "classDiagram\n class Animal {\n <<abstract>>\n +String name\n +int age$\n +makeSound()* void\n }\n Animal <|-- Duck\n Animal <|-- Fish\n Duck \"1\" *-- \"many\" Egg : lays\n Fish o-- Fin\n Duck ..> Water\n Duck -- Pond\n",
    );
    assert_eq!(out.kind, DiagramKind::Class);
    assert_eq!(count(&out.svg, "pele-node pele-class"), 7);
    assert_eq!(count(&out.svg, "pele-relation"), 6);
    assert_eq!(count(&out.svg, "pele-cardinality\""), 2);
    for needle in [
        "«abstract»",
        "+String name",
        "pele-static",
        "pele-abstract",
        "+makeSound() : void",
        ">lays<",
    ] {
        assert!(out.svg.contains(needle), "{needle}");
    }
}

#[test]
fn draws_namespaces_notes_generics_and_interfaces() {
    let svg = svg(
        "classDiagram\n namespace Shapes {\n class Square~Shape~ {\n List~int~ position\n }\n class Circle\n }\n note for Circle \"round\"\n note \"loose\"\n Square --() Drawable\n Circle --> Circle : self\n",
    );
    for needle in [
        "pele-namespace",
        "class=\"pele-node pele-note\"",
        "pele-note-edge",
        "pele-interface",
        "Square&lt;Shape&gt;",
        "List&lt;int&gt; position",
        ">round<",
        ">self<",
    ] {
        assert!(svg.contains(needle), "{needle}");
    }
    // The note that names no class is drawn on its own; the one that names a class is joined to it.
    assert_eq!(count(&svg, "class=\"pele-node pele-note\""), 2, "{svg}");
    assert!(svg.contains(">loose<"), "the loose note is drawn");
    assert_eq!(count(&svg, "pele-note-edge"), 1, "{svg}");
    assert_inert(&svg, "namespaces");
}

#[test]
fn keeps_only_allowed_links_and_styles() {
    for url in [
        "javascript:alert(1)",
        " JaVaScRiPt:alert(1)",
        "data:text/html,x",
        "vbscript:x",
        "jav#9;ascript:alert(1)",
        "#106;avascript#colon;alert(1)",
    ] {
        let out = render(&format!(
            "classDiagram\n class A\n link A \"{url}\" \"t\"\n"
        ));
        assert!(!out.svg.contains("<a "), "{url}");
        assert!(out.links.is_empty());
    }
    let out =
        render("classDiagram\n class A\n click A href \"https://example.com\" \"Docs\" _self\n");
    assert_eq!(out.links.len(), 1);
    assert!(out.svg.contains("<title>Docs</title>"));
    assert!(out.svg.contains("target=\"_self\""));
    let svg = svg(
        "classDiagram\n class A\n class B\n classDef c fill:url(javascript:alert(1)),stroke:red\n cssClass \"A\" c\n style B fill:expression(x)\n callback A \"alert\"\n",
    );
    assert!(!svg.contains("javascript") && !svg.contains("expression") && !svg.contains("alert"));
    assert!(svg.contains("stroke:red"));
}

#[test]
fn reports_errors_with_positions() {
    let err = pele_rs::render(
        "classDiagram\n class A\n A <|--\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Class));
    assert_eq!(err.line, 3);
}

#[test]
fn enforces_limits() {
    for deep in [
        format!(
            "classDiagram\n{}class A\n{}",
            "namespace n {\n".repeat(100),
            "}\n".repeat(100)
        ),
        format!(
            "classDiagram\nnamespace {} {{\nclass A\n}}\n",
            vec!["n"; 100].join(".")
        ),
    ] {
        assert_eq!(
            pele_rs::render(&deep, &RenderOptions::default())
                .unwrap_err()
                .code,
            ErrorCode::Limit
        );
    }
}

#[test]
fn fits_a_narrow_host() {
    let src = "classDiagram\n direction LR\n A --> B\n B --> C\n C --> D\n D --> E\n";
    let wide = render(src);
    let options = RenderOptions {
        max_width: Some(320.0),
        ..RenderOptions::default()
    };
    let narrow = pele_rs::render(src, &options).unwrap();
    assert!(narrow.width < wide.width);
    let parts = "classDiagram\n A --> B\n C --> D\n E --> F\n G --> H\n I --> J\n";
    let wrapped = pele_rs::render(parts, &options).unwrap();
    assert!(wrapped.width < render(parts).width);
}

#[test]
fn output_is_deterministic() {
    let src = "classDiagram\n namespace N {\n class A\n class B\n }\n A <|-- B\n A \"1\" --> \"*\" C\n note for A \"n\"\n";
    assert_eq!(svg(src), svg(src));
}

#[test]
fn is_inert_for_corpus_hostile_and_mutated_inputs() {
    let corpus = corpus();
    assert!(corpus.len() > 50);
    for src in &corpus {
        if let Some(svg) = try_render(src) {
            assert_inert(&svg, src);
        }
    }
    let mut rendered = 0;
    for payload in PAYLOADS {
        let p = *payload;
        for src in [
            format!("classDiagram\n class `{p}`"),
            format!("classDiagram\n class A[\"{p}\"]"),
            format!("classDiagram\n class A~{p}~"),
            format!("classDiagram\n class A {{\n {p}\n {p}()\n +x({p}) {p}$\n }}"),
            format!("classDiagram\n A : {p}"),
            format!("classDiagram\n A : {p}(x)*"),
            format!("classDiagram\n class A {{\n <<{p}>>\n }}"),
            format!("classDiagram\n A --> B : {p}"),
            format!("classDiagram\n A \"{p}\" --> \"{p}\" B"),
            format!("classDiagram\n A --> A : {p}"),
            format!("classDiagram\n A --() `{p}`"),
            format!("classDiagram\n namespace `{p}` {{\n class A\n }}"),
            format!("classDiagram\n namespace N[\"{p}\"] {{\n class A\n }}"),
            format!("classDiagram\n note \"{p}\""),
            format!("classDiagram\n class A\n note for A \"{p}\""),
            format!("classDiagram\n class A:::{p}"),
            format!("classDiagram\n class A\n classDef c fill:{p}\n cssClass \"A\" c"),
            format!(
                "classDiagram\n class A\n classDef c {p}:red,color:{p},font-family:{p},stroke:{p}\n cssClass \"A\" c"
            ),
            format!("classDiagram\n class A\n style A color:{p},stroke:{p},font-size:{p}"),
            format!("classDiagram\n class A\n link A \"{p}\" \"{p}\""),
            format!("classDiagram\n class A\n click A href \"{p}\" \"{p}\" {p}"),
            format!("classDiagram\n class A\n callback A \"{p}\" \"{p}\""),
            format!("classDiagram\n accTitle: {p}\n accDescr: {p}\n class A"),
            format!(
                "---\ntitle: \"{p}\"\nconfig:\n  class:\n    nodeSpacing: \"{p}\"\n---\nclassDiagram\n A --> B"
            ),
        ] {
            let Some(svg) = try_render(&src) else {
                continue;
            };
            assert_inert(&svg, &src);
            rendered += 1;
        }
    }
    assert!(rendered > 700, "only {rendered} hostile inputs rendered");
    let mut fragments: Vec<&str> = PAYLOADS.to_vec();
    fragments.extend([
        "class ",
        "namespace ",
        "{",
        "}",
        "<|--",
        "*--",
        "o--",
        "..>",
        "--()",
        " : ",
        "~T~",
        "<<interface>>",
        "note for A \"x\"",
        "\"1\"",
        "classDef c fill:red",
        ":::c",
        "+x()$",
        "direction LR",
        "\n",
    ]);
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
    assert!(rendered > 300, "only {rendered} of 1500 mutations rendered");
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
    let head = "classDiagram\n";
    let cases = [
        format!("{head}{}", "a direction XX ".repeat(N / 15)),
        format!("{head}{}", "direction TB direction lr ".repeat(N / 26)),
        format!("{head}class A {{\n{}", "x\n".repeat(N / 2)),
        format!("{head}class A {{\n{}}}", "+f(".repeat(N / 3)),
        format!("{head}class A {{\n{}}}", ")(".repeat(N / 2)),
        format!("{head}A : {}", "(".repeat(N)),
        format!("{head}A : {}", "~".repeat(N)),
        format!("{head}A : {}", "~,".repeat(N / 2)),
        format!("{head}class A~{}~", "~".repeat(N)),
        format!("{head}{}", "`".repeat(N)),
        format!("{head}{}", "\"".repeat(N)),
        format!("{head}{}", "<<".repeat(N / 2)),
        format!("{head}{}", "class ".repeat(N / 6)),
        format!("{head}{}", "namespace ".repeat(N / 10)),
        format!("{head}{}", "a.".repeat(N / 2)),
        format!("{head}note for {}", "a ".repeat(N / 2)),
        format!("{head}{}", "a%%{".repeat(N / 4)),
        format!("{head}accDescr {{{}", "x ".repeat(N / 2)),
        format!("{head}{}", repeat(5000, |i| format!("n{i}-->n{}\n", i + 1))),
        format!("{head}{}", repeat(5000, |i| format!("c-->n{i}\n"))),
        format!(
            "{head}{}",
            repeat(3000, |i| format!("c \"1\" --> \"{i}\" n{i}\n"))
        ),
        format!("{head}{}{}", "a-->b\n".repeat(3000), "a-->a\n".repeat(3000)),
        format!("{head}{}", repeat(5000, |i| format!("class c{i}\n"))),
        format!(
            "{head}namespace N {{\n{}}}\n",
            repeat(5000, |i| format!("class c{i}\n"))
        ),
        format!(
            "{head}{}",
            repeat(3000, |i| format!("namespace N{i} {{\nclass c{i}\n}}\n"))
        ),
        format!(
            "{head}{}",
            repeat(3000, |i| format!("namespace N.n{i} {{\nclass c{i}\n}}\n"))
        ),
        format!(
            "{head}{}",
            repeat(3000, |i| format!("note for c \"n{i}\"\n"))
        ),
        format!("{head}{}", repeat(3000, |i| format!("c --() i{i}\n"))),
        format!(
            "{head}class c\n{}",
            repeat(5000, |i| format!("cssClass \"c\" k{i}\n"))
        ),
        format!(
            "{head}class c\n{}",
            repeat(3000, |i| format!(
                "classDef k{i} fill:red\ncssClass \"c\" k{i}\n"
            ))
        ),
        format!(
            "{head}cssClass \"{}z\" k\n",
            repeat(8000, |i| format!("s{i},"))
        ),
        format!(
            "{head}class c\n{}",
            "style c fill:red,stroke:blue\n".repeat(3000)
        ),
        format!("{head}class c\n{}", "link c \"u\" \"t\"\n".repeat(3000)),
        format!("{head}class c\nlink c \"&{}x\"\n", "#38;".repeat(N / 4)),
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
        rendered >= 20,
        "only {rendered} of {} cases were drawn",
        cases.len()
    );
}

#[test]
fn draws_an_end_marker_for_every_relation_end() {
    let drawn = svg(
        "classDiagram\n Animal <|-- Duck\n Duck \"1\" *-- \"many\" Egg\n Fish o-- Fin\n Duck ..> Water\n Duck -- Pond\n",
    );
    assert_eq!(count(&drawn, "pele-relation"), 5, "{drawn}");
    assert_eq!(count(&drawn, "pele-marker"), 4, "{drawn}");
    let lollipop = svg("classDiagram\n A --() B\n");
    assert!(
        lollipop.contains("<circle class=\"pele-marker\""),
        "{lollipop}"
    );
}

#[test]
fn draws_a_relation_without_an_end_marker() {
    let svg = svg("classDiagram\n A -- B\n");
    assert_eq!(count(&svg, "pele-relation"), 1, "{svg}");
    assert_eq!(count(&svg, "pele-marker"), 0, "{svg}");
}

#[test]
fn draws_class_labels_and_annotations() {
    let svg = svg("classDiagram\n class A[\"My Label\"]\n <<interface>> B\n <<enumeration>> C\n");
    assert!(svg.contains(">My Label<"), "{svg}");
    assert!(svg.contains("«interface»"), "{svg}");
    assert!(svg.contains("«enumeration»"), "{svg}");
    assert_eq!(count(&svg, "pele-annotation"), 2, "{svg}");
}

#[test]
fn draws_member_visibility_classifiers_and_generics() {
    let svg = svg(
        "classDiagram\n class A~T~ {\n -private\n #protected\n ~package\n +public\n +int age$\n +abs()* void\n +get() List~int~\n }\n",
    );
    for needle in [">-private<", ">#protected<", ">~package<", ">+public<"] {
        assert!(svg.contains(needle), "{needle}");
    }
    assert_eq!(count(&svg, "pele-static"), 1, "{svg}");
    assert_eq!(count(&svg, "pele-abstract"), 1, "{svg}");
    assert!(svg.contains("List&lt;int&gt;"), "{svg}");
}

#[test]
fn nests_namespaces() {
    let svg = svg("classDiagram\n namespace A {\n namespace B {\n class C\n }\n }\n");
    assert_eq!(count(&svg, "pele-namespace"), 2, "{svg}");
    assert!(svg.contains("pele-node pele-class"), "{svg}");
}

#[test]
fn applies_a_css_class_to_several_classes() {
    let svg = svg("classDiagram\n class A\n class B\n cssClass \"A,B\" hot\n");
    assert_eq!(count(&svg, "pele-node pele-class hot"), 2, "{svg}");
}

#[test]
fn draws_a_link_with_its_tooltip_and_never_a_callback() {
    let out = render("classDiagram\n class A\n link A \"https://example.com\" \"Docs\" _blank\n");
    assert_eq!(out.links.len(), 1);
    assert!(out.svg.contains("<title>Docs</title>"), "{}", out.svg);
    assert!(out.svg.contains("target=\"_blank\""), "{}", out.svg);
    // The callback is never run: its name is dropped and only its tooltip is drawn.
    let cb = svg("classDiagram\n class A\n callback A \"alertFn\" \"the tooltip\"\n");
    assert!(!cb.contains("alertFn"), "{cb}");
    assert!(cb.contains("<title>the tooltip</title>"), "{cb}");
    assert!(cb.contains("pele-node pele-class clickable"), "{cb}");
}

#[test]
fn renders_accessibility_text() {
    let svg = svg("classDiagram\n accTitle: My Title\n accDescr: My Description\n A --> B\n");
    assert!(
        svg.contains("<title id=\"pele-title\">My Title</title>"),
        "{svg}"
    );
    assert!(
        svg.contains("<desc id=\"pele-desc\">My Description</desc>"),
        "{svg}"
    );
    assert!(svg.contains("aria-labelledby=\"pele-title\""), "{svg}");
    assert!(svg.contains("aria-describedby=\"pele-desc\""), "{svg}");
}

#[test]
fn snapshots() {
    let cases = [
        (
            "basic",
            "classDiagram\n Animal <|-- Duck\n Animal <|-- Zebra\n Animal : +int age\n Animal : +isMammal()\n class Duck {\n +String beakColor\n +swim()\n }\n",
        ),
        (
            "namespaces",
            "classDiagram\n namespace Shop {\n class Order\n class Item\n }\n Order \"1\" *-- \"many\" Item : contains\n Customer --> Order\n note for Customer \"buys\"\n",
        ),
        (
            "styled",
            "---\ntitle: Styled\n---\nclassDiagram\n direction LR\n class Shape~T~ {\n <<interface>>\n +draw()* void\n +count$ int\n }\n Shape <|.. Square\n Square --() Printable\n classDef hot fill:#f96,color:white\n cssClass \"Square\" hot\n",
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(src));
    }
}
