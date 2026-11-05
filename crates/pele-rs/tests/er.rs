//! ER diagram rendering, security, limits and speed.
#![cfg(feature = "er")]

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

include!("compat/er/cases.rs");

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
        .filter(|s| s.contains("erDiagram"))
        .collect()
}

#[test]
fn draws_entities_attributes_and_relationships() {
    let out = render(
        "erDiagram\n CUSTOMER ||--o{ ORDER : places\n ORDER ||--|{ LINE-ITEM : contains\n CUSTOMER }|..|{ DELIVERY-ADDRESS : uses\n CUSTOMER {\n string name PK \"the name\"\n int age\n List~int~ ids FK, UK\n }\n PERSON u--o| PERSON : knows\n",
    );
    assert_eq!(out.kind, DiagramKind::Er);
    assert_eq!(count(&out.svg, "pele-rs-node pele-rs-entity"), 5);
    assert_eq!(count(&out.svg, "pele-rs-er-identifying"), 3);
    assert_eq!(count(&out.svg, "pele-rs-er-non-identifying"), 1);
    for needle in [
        ">places<",
        ">string<",
        ">PK<",
        ">the name<",
        "List&lt;int&gt;",
        ">FK, UK<",
        "pele-rs-er-stripes",
        "<circle class=\"pele-rs-marker\"",
    ] {
        assert!(out.svg.contains(needle), "{needle}");
    }
}

#[test]
fn draws_aliases_subgraphs_and_classes() {
    let svg = svg(
        "erDiagram\n subgraph Shop [The shop]\n direction LR\n p[\"Person\"]:::hot\n a[Address]\n end\n p ||--o{ a : has\n classDef hot fill:#f96,color:white\n style a stroke:red\n",
    );
    for needle in [
        "pele-rs-cluster",
        ">The shop<",
        ">Person<",
        ">Address<",
        "pele-rs-entity hot",
        "fill:#f96",
        "stroke:red",
    ] {
        assert!(svg.contains(needle), "{needle}");
    }
    assert_inert(&svg, "subgraphs");
}

#[test]
fn keeps_only_allowed_styles_and_links() {
    let svg = svg(
        "erDiagram\n A\n B\n classDef c fill:javascript:alert,stroke:red\n class A c\n style B fill:blue,color:javascript:x\n",
    );
    assert!(!svg.contains("javascript") && svg.contains("fill:blue"));
    assert!(svg.contains("stroke:red"));
    let out =
        render("erDiagram\n \"javascript:alert(1)\":::internal-link\n Page:::internal-link\n");
    assert_eq!(out.links.len(), 2);
    assert!(out.links.iter().all(|l| l.internal));
    assert!(!out.svg.contains(" href=\"javascript"));
    assert!(out.svg.contains("href=\"Page\""));
    // The drawing is inert even though the link is reported.
    assert_inert(&out.svg, "internal links");
}

#[test]
fn reports_errors_with_positions() {
    let err = pele_rs::render("erDiagram\n A ||--o{ B\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Er));
    assert_eq!(err.line, 2);
}

#[test]
fn enforces_limits() {
    let deep = format!(
        "erDiagram\n{}A\n{}",
        "subgraph s\n".repeat(100),
        "end\n".repeat(100)
    );
    assert_eq!(
        pele_rs::render(&deep, &RenderOptions::default())
            .unwrap_err()
            .code,
        ErrorCode::Limit
    );
}

#[test]
fn fits_a_narrow_host() {
    let src = "erDiagram\n direction LR\n A ||--o{ B : x\n B ||--o{ C : y\n C ||--o{ D : z\n D ||--o{ E : w\n";
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
    let src = "erDiagram\n subgraph S\n A\n B\n end\n A ||--o{ B : x\n A |o--|| A : self\n C {\n int id PK\n }\n";
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
            format!("erDiagram\n \"{p}\""),
            format!("erDiagram\n A[\"{p}\"]"),
            format!("erDiagram\n A {{\n string `{p}`\n }}"),
            format!("erDiagram\n A {{\n string x PK \"{p}\"\n }}"),
            format!("erDiagram\n A ||--o{{ B : \"{p}\""),
            format!("erDiagram\n A ||--o{{ A : \"{p}\""),
            format!("erDiagram\n subgraph S [\"{p}\"]\n A\n end"),
            format!("erDiagram\n \"{p}\":::internal-link"),
            format!("erDiagram\n A\n classDef c fill:{p}\n class A c"),
            format!(
                "erDiagram\n A\n classDef c {p}:red,color:{p},font-family:{p},stroke:{p}\n class A c"
            ),
            format!("erDiagram\n A\n style A color:{p},stroke:{p},font-size:{p}"),
            format!("erDiagram\n accTitle: {p}\n accDescr: {p}\n A"),
            format!(
                "---\ntitle: \"{p}\"\nconfig:\n  er:\n    nodeSpacing: \"{p}\"\n---\nerDiagram\n A ||--o{{ B : x"
            ),
        ] {
            let Some(svg) = try_render(&src) else {
                continue;
            };
            assert_inert(&svg, &src);
            rendered += 1;
        }
    }
    assert!(rendered > 150, "only {rendered} hostile inputs rendered");
    let mut fragments: Vec<&str> = PAYLOADS.to_vec();
    fragments.extend([
        "||--o{",
        "}|..|{",
        "|o--||",
        "u--o{",
        " : ",
        "{",
        "}",
        "string name PK \"c\"",
        "int~T~ x",
        "[alias]",
        ":::c",
        "classDef c fill:red",
        "subgraph s\n",
        "end\n",
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
    let head = "erDiagram\n";
    let cases = [
        format!("{head}{}", "a direction XX ".repeat(N / 15)),
        format!("{head}{}", "direction TB direction lr ".repeat(N / 26)),
        format!("{head}A {{\n{}", "x ".repeat(N / 2)),
        format!("{head}A {{\n{}}}", "x~".repeat(N / 2)),
        format!("{head}A {{\n{}}}", "~ ".repeat(N / 2)),
        format!("{head}A {{\n{}}}", "int x\n".repeat(N / 6)),
        format!("{head}A {{\nint x {}}}", "PK,".repeat(N / 3)),
        format!("{head}{}", "\"".repeat(N)),
        format!("{head}{}", "`".repeat(N)),
        format!("{head}{}", "1 ".repeat(N / 2)),
        format!("{head}{}", "subgraph ".repeat(N / 9)),
        format!("{head}{}", "subgraph s\n".repeat(40)),
        format!("{head}accDescr {{{}", "x ".repeat(N / 2)),
        format!(
            "{head}{}",
            repeat(5000, |i| format!("n{i} ||--o{{ n{} : r\n", i + 1))
        ),
        format!(
            "{head}{}",
            repeat(5000, |i| format!("c ||--o{{ n{i} : r\n"))
        ),
        format!("{head}{}", "a ||--|| a : r\n".repeat(3000)),
        format!("{head}{}", repeat(5000, |i| format!("e{i}\n"))),
        format!(
            "{head}subgraph S\n{}end\n",
            repeat(5000, |i| format!("e{i}\n"))
        ),
        format!(
            "{head}{}",
            repeat(2000, |i| format!("subgraph s{i}\ns{}\nend\n", i + 1))
        ),
        format!(
            "{head}{}",
            repeat(3000, |i| format!("subgraph s{i}\ne{i}\nend\n"))
        ),
        format!("{head}c\n{}", repeat(5000, |i| format!("class c k{i}\n"))),
        format!(
            "{head}c\n{}",
            repeat(3000, |i| format!("classDef k{i} fill:red\nclass c k{i}\n"))
        ),
        format!("{head}class {}z k\n", repeat(8000, |i| format!("s{i},"))),
        format!("{head}c\n{}", "style c fill:red,stroke:blue\n".repeat(3000)),
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
        rendered >= 14,
        "only {rendered} of {} cases were drawn",
        cases.len()
    );
}

#[test]
fn draws_a_marker_for_every_cardinality() {
    for (src, rings) in [
        ("erDiagram\n A ||--|| B : x\n", 0),
        ("erDiagram\n A |o--|| B : x\n", 1),
        ("erDiagram\n A ||--|{ B : x\n", 0),
        ("erDiagram\n A ||--o{ B : x\n", 1),
        ("erDiagram\n A u--o{ B : x\n", 1),
    ] {
        let svg = svg(src);
        assert_eq!(
            count(&svg, "<circle class=\"pele-rs-marker\""),
            rings,
            "{src}"
        );
        assert_eq!(count(&svg, "<path class=\"pele-rs-marker\""), 2, "{src}");
    }
}

#[test]
fn draws_every_attribute_key() {
    let out = render(
        "erDiagram\n A {\n string a PK\n string b FK\n string c UK\n string d PK, FK\n string e FK, UK\n }\n",
    );
    for needle in [">PK<", ">FK<", ">UK<", ">PK, FK<", ">FK, UK<"] {
        assert!(out.svg.contains(needle), "{needle}");
    }
}

#[test]
fn accepts_the_cardinality_aliases() {
    for alias in [
        "only one",
        "1",
        "one or zero",
        "zero or one",
        "one or more",
        "one or many",
        "many(1)",
        "1+",
        "zero or more",
        "zero or many",
        "many(0)",
        "0+",
    ] {
        let src = format!("erDiagram\n A {alias} to {alias} B : x\n");
        let out = render(&src);
        assert!(out.svg.contains("pele-rs-er-identifying"), "{alias}");
    }
}

#[test]
fn draws_the_accessible_title_and_description() {
    let out = render("erDiagram\n accTitle: the title\n accDescr: the description\n A\n");
    assert!(
        out.svg
            .contains("<title id=\"pele-rs-title\">the title</title>"),
        "{}",
        out.svg
    );
    assert!(
        out.svg
            .contains("<desc id=\"pele-rs-desc\">the description</desc>"),
        "{}",
        out.svg
    );
}

#[test]
fn puts_the_class_on_the_named_entity() {
    let out =
        render("erDiagram\n A\n B\n classDef hot fill:#f96\n class B hot\n style A stroke:red\n");
    let b = out.svg.split("data-id=\"B\"").next().expect("B is drawn");
    assert!(
        b.contains("pele-rs-entity hot"),
        "the class went to the wrong entity"
    );
    assert!(out.svg.contains("stroke:red"), "{}", out.svg);
}

#[test]
fn marks_non_identifying_relationships_dashed() {
    let out = render("erDiagram\n A ||--o{ B : x\n A }|..|{ C : y\n");
    assert!(out.svg.contains("pele-rs-er-identifying"), "{}", out.svg);
    assert!(
        out.svg.contains("pele-rs-er-non-identifying"),
        "{}",
        out.svg
    );
    assert!(out.svg.contains("stroke-dasharray=\"4 3\""), "{}", out.svg);
}

#[test]
fn snapshots() {
    let cases = [
        (
            "basic",
            "erDiagram\n CUSTOMER ||--o{ ORDER : places\n ORDER ||--|{ LINE-ITEM : contains\n CUSTOMER }|..|{ DELIVERY-ADDRESS : uses\n",
        ),
        (
            "attributes",
            "erDiagram\n CAR ||--o{ NAMED-DRIVER : allows\n CAR {\n string registrationNumber PK\n string make\n string model\n string[] parts\n }\n PERSON ||--o{ NAMED-DRIVER : is\n PERSON {\n string driversLicense PK \"The license #\"\n string(99) firstName \"Only 99 characters are allowed\"\n string lastName\n int age\n }\n NAMED-DRIVER {\n string carRegistrationNumber PK, FK\n string driverLicence PK, FK\n }\n MANUFACTURER only one to zero or more CAR : makes\n",
        ),
        (
            "subgraphs",
            "---\ntitle: Styled\n---\nerDiagram\n direction LR\n subgraph Sales [Sales area]\n c[\"Customer\"]:::hot\n o[Order]\n end\n c ||--o{ o : places\n o ||--|| o : self\n classDef hot fill:#f96,color:white\n",
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(src));
    }
}
