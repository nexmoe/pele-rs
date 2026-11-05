//! The icons and the math that the host of the library draws: the `icons` and `math` options.
//!
//! The resolvers here are fakes. What they answer is the host's and is put into the SVG as it
//! is, so these tests check what the library does around the answers: where it puts them, how
//! it makes room for them, and that the text of a diagram cannot reach the output but through
//! them.
// Every test needs one of these types, so with none of them the helpers below would not be used.
#![cfg(any(
    feature = "flowchart",
    feature = "swimlane",
    feature = "mindmap",
    feature = "kanban",
    feature = "er",
    feature = "requirement",
    feature = "class",
    feature = "block",
    feature = "usecase"
))]

mod support;

#[cfg(feature = "flowchart")]
use std::cell::RefCell;
#[cfg(feature = "flowchart")]
use std::rc::Rc;

#[cfg(any(feature = "flowchart", feature = "mindmap"))]
use pele_rs::IconResolver;
use pele_rs::{MathMarkup, MathResolver, RenderOptions};
#[cfg(feature = "flowchart")]
use support::PAYLOADS;
use support::assert_inert;

const ICON: &str = "<path d=\"M2 2h20v20H2z\"/>";
const MATH: &str = "<path d=\"M0 0h30v40H0z\"/>";

#[cfg(feature = "flowchart")]
fn icons() -> IconResolver {
    Box::new(|name| (name == "fa:fa-car").then(|| ICON.to_string()))
}

fn math(width: f64, height: f64) -> MathResolver {
    Box::new(move |_, _| {
        Some(MathMarkup::Svg {
            svg: MATH.to_string(),
            width,
            height,
        })
    })
}

fn svg(src: &str, options: &RenderOptions) -> String {
    pele_rs::render(src, options)
        .unwrap_or_else(|e| panic!("{src:?}: {e}"))
        .svg
}

#[cfg(feature = "flowchart")]
#[test]
fn without_resolvers_icons_are_left_out_and_math_stays_text() {
    let out = svg(
        "flowchart LR\n  A[fa:fa-car Car] --> B[\"$$x^2$$\"]",
        &RenderOptions::default(),
    );
    assert!(!out.contains("pele-rs-icon"));
    assert!(!out.contains("pele-rs-math"));
    assert!(out.contains("Car"));
    assert!(!out.contains("fa:fa-car"));
    assert!(out.contains("$$x^2$$"));
}

#[cfg(feature = "flowchart")]
#[test]
fn draws_an_icon_the_host_has_and_leaves_out_one_it_has_not() {
    let options = RenderOptions {
        icons: Some(icons()),
        ..Default::default()
    };
    let plain = svg("flowchart LR\n  A[Car]", &RenderOptions::default());
    let out = svg(
        "flowchart LR\n  A[fa:fa-car Car] --> B[fa:fa-boat Boat]",
        &options,
    );
    assert_eq!(out.matches("class=\"pele-rs-icon\"").count(), 1);
    assert!(out.contains("data-icon=\"fa:fa-car\""));
    assert!(out.contains(ICON));
    // The icon has room of its own, so the label is wider than the same label without it.
    assert!(out.contains("> Car</tspan>"), "{out}");
    assert!(out.contains(">Boat</text>"), "{out}");
    assert!(!out.contains("fa:fa-boat"));
    assert_ne!(out, plain);
    assert_inert(&out, "icon");
}

#[cfg(feature = "flowchart")]
#[test]
fn an_icon_makes_the_node_wider() {
    let options = RenderOptions {
        icons: Some(icons()),
        ..Default::default()
    };
    let with = pele_rs::render("flowchart LR\n  A[fa:fa-car Car]", &options).unwrap();
    let without = pele_rs::render("flowchart LR\n  A[Car]", &options).unwrap();
    // 1.25 em of icon, at 16 pixels.
    assert!(
        with.width > without.width + 19.0,
        "{} {}",
        with.width,
        without.width
    );
}

#[cfg(feature = "flowchart")]
#[test]
fn typesets_math_and_makes_room_for_it() {
    let options = RenderOptions {
        math: Some(math(30.0, 40.0)),
        ..Default::default()
    };
    let out = svg("flowchart LR\n  A[\"$$x^2$$\"]", &options);
    assert!(out.contains("class=\"pele-rs-math\""));
    assert!(out.contains(MATH));
    assert!(!out.contains("$$"));
    assert_inert(&out, "math");

    // A line that holds math is as tall as the math is: 40 and a quarter of the font size.
    let tall = pele_rs::render("flowchart LR\n  A[\"$$x$$\"]", &options).unwrap();
    let flat = pele_rs::render("flowchart LR\n  A[\"x\"]", &options).unwrap();
    assert!(
        tall.height > flat.height + 18.0,
        "{} {}",
        tall.height,
        flat.height
    );
}

#[cfg(feature = "flowchart")]
#[test]
fn math_that_the_host_does_not_typeset_stays_as_written() {
    let options = RenderOptions {
        math: Some(Box::new(|tex, _| {
            (tex == "ok").then(|| MathMarkup::Svg {
                svg: MATH.to_string(),
                width: 10.0,
                height: 10.0,
            })
        })),
        ..Default::default()
    };
    let out = svg("flowchart LR\n  A[\"$$ok$$ and $$other$$\"]", &options);
    assert_eq!(out.matches("class=\"pele-rs-math\"").count(), 1);
    assert!(out.contains("$$other$$"));
}

#[cfg(feature = "flowchart")]
#[test]
fn ignores_math_with_a_size_that_cannot_be_drawn() {
    for (width, height) in [
        (0.0, 10.0),
        (10.0, -1.0),
        (f64::NAN, 10.0),
        (10.0, f64::INFINITY),
    ] {
        let options = RenderOptions {
            math: Some(math(width, height)),
            ..Default::default()
        };
        let out = svg("flowchart LR\n  A[\"$$x$$\"]", &options);
        assert!(!out.contains("pele-rs-math"), "{width} {height}");
        assert!(out.contains("$$x$$"));
    }
}

#[cfg(feature = "flowchart")]
#[test]
fn asks_the_host_once_for_the_same_math() {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let log = asked.clone();
    let options = RenderOptions {
        math: Some(Box::new(move |tex, size| {
            log.borrow_mut().push((tex.to_string(), size));
            None
        })),
        ..Default::default()
    };
    svg(
        "flowchart LR\n  A[\"$$a$$\"] --> B[\"$$a$$ $$b$$\"]",
        &options,
    );
    let asked = asked.borrow();
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert_eq!(asked[0], ("a".to_string(), 16.0));
    assert_eq!(asked[1].0, "b");
}

#[cfg(feature = "flowchart")]
#[test]
fn a_title_is_plain_text() {
    let options = RenderOptions {
        math: Some(math(30.0, 40.0)),
        icons: Some(icons()),
        ..Default::default()
    };
    let out = svg("---\ntitle: $$a$$\n---\nflowchart LR\n  A", &options);
    assert!(!out.contains("pele-rs-math"));
    assert!(out.contains("$$a$$"));
}

#[cfg(feature = "flowchart")]
#[test]
fn output_is_deterministic() {
    let options = || RenderOptions {
        icons: Some(icons()),
        math: Some(math(30.0, 40.0)),
        ..Default::default()
    };
    let src = "flowchart LR\n  A[fa:fa-car $$x$$ text] --> B[\"a <b>b</b> fa:fa-car\"]";
    assert_eq!(svg(src, &options()), svg(src, &options()));
}

#[cfg(feature = "flowchart")]
#[test]
fn the_text_of_a_diagram_reaches_the_output_only_escaped() {
    // The host answers for every name and every expression, with markup of its own. What the
    // diagram says is handed over as a name or as TeX, and drawn nowhere else.
    let options = RenderOptions {
        icons: Some(Box::new(|_| Some(ICON.to_string()))),
        math: Some(Box::new(|_, _| None)),
        ..Default::default()
    };
    for payload in PAYLOADS {
        let src = format!(
            "flowchart LR\n  A[\"fa:fa-car {} $${}$$\"]",
            payload.replace('"', "#quot;"),
            payload.replace('"', "#quot;")
        );
        if let Ok(rendered) = pele_rs::render(&src, &options) {
            assert_inert(&rendered.svg, &src);
        }
    }
}

#[cfg(feature = "mindmap")]
#[test]
fn mindmap_draws_the_icon_of_a_node_the_host_has() {
    let src =
        "mindmap\n  root\n    Book\n    ::icon(fa fa-book)\n    Other\n    ::icon(fa fa-other)\n";
    let book: IconResolver = Box::new(|name| (name == "fa fa-book").then(|| ICON.to_string()));
    let with = RenderOptions {
        icons: Some(book),
        ..Default::default()
    };
    let out = pele_rs::render(src, &with).unwrap();
    let plain = pele_rs::render(src, &RenderOptions::default()).unwrap();
    assert_eq!(out.svg.matches("class=\"pele-rs-icon\"").count(), 1);
    assert!(out.svg.contains("data-icon=\"fa fa-book\""));
    assert!(!plain.svg.contains("pele-rs-icon"));
    // The icon has room in the node, so the map is wider.
    assert!(out.width > plain.width);
    assert_inert(&out.svg, src);
}

#[cfg(feature = "mindmap")]
#[test]
fn mindmap_typesets_math() {
    let src = "mindmap\n  root\n    a[A $$x^2$$ b]\n";
    let options = RenderOptions {
        math: Some(math(30.0, 40.0)),
        ..Default::default()
    };
    let out = svg(src, &options);
    assert!(out.contains("class=\"pele-rs-math\""));
    assert!(!out.contains("$$"));
}

/// Renders a source with the icon `myicon` and math of 30 by 40, and with neither.
#[cfg(any(
    feature = "flowchart",
    feature = "swimlane",
    feature = "kanban",
    feature = "er",
    feature = "requirement",
    feature = "class",
    feature = "block",
    feature = "usecase"
))]
fn both(src: &str) -> (String, String) {
    let hosted = RenderOptions {
        icons: Some(Box::new(|name| {
            (name == "myicon" || name == "fa:fa-car").then(|| ICON.to_string())
        })),
        math: Some(math(30.0, 40.0)),
        ..Default::default()
    };
    (svg(src, &hosted), svg(src, &RenderOptions::default()))
}

#[cfg(feature = "flowchart")]
#[test]
fn flowchart_draws_the_icon_of_an_icon_shape() {
    let src = "flowchart LR\n  A@{ icon: \"myicon\", form: \"square\", label: \"Car\" }\n  B@{ icon: \"other\", form: \"square\" }";
    let (hosted, plain) = both(src);
    assert!(hosted.contains(ICON));
    assert_eq!(hosted.matches("<svg class=\"pele-rs-icon\"").count(), 1);
    // An icon the host has not is a dashed frame, as it is without a host.
    assert!(hosted.contains("stroke-dasharray=\"3 3\""));
    assert!(!plain.contains(ICON));
    assert_inert(&hosted, src);
}

#[cfg(feature = "kanban")]
#[test]
fn kanban_draws_icons_and_widens_for_math() {
    let src = "kanban\n  todo[To do]@{ icon: 'myicon' }\n    t1[Task]@{ icon: 'myicon' }\n    t2[Math $$x$$]\n";
    let (hosted, plain) = both(src);
    assert_eq!(hosted.matches("<svg class=\"pele-rs-icon\"").count(), 2);
    assert!(hosted.contains("class=\"pele-rs-math\""));
    assert!(!plain.contains("pele-rs-icon"));
    assert_inert(&hosted, src);
    // The column is at least as wide as the math and its room.
    let wide = pele_rs::render(
        src,
        &RenderOptions {
            math: Some(math(400.0, 20.0)),
            ..Default::default()
        },
    )
    .unwrap();
    let narrow = pele_rs::render(src, &RenderOptions::default()).unwrap();
    assert!(wide.width > narrow.width + 100.0);
}

#[cfg(feature = "er")]
#[test]
fn er_draws_icons_and_math_in_a_cell() {
    let src = "erDiagram\n  CUSTOMER {\n    string name \"fa:fa-car $$x$$\"\n  }\n";
    let (hosted, plain) = both(src);
    assert!(hosted.contains("class=\"pele-rs-icon\""), "{hosted}");
    assert!(hosted.contains("class=\"pele-rs-math\""));
    assert!(!plain.contains("pele-rs-icon"));
    assert_inert(&hosted, src);
}

#[cfg(feature = "requirement")]
#[test]
fn requirement_draws_math() {
    let src = "requirementDiagram\n  requirement r {\n    id: 1\n    text: \"$$x$$\"\n    risk: high\n    verifymethod: test\n  }\n";
    let (hosted, plain) = both(src);
    assert!(hosted.contains("class=\"pele-rs-math\""), "{hosted}");
    assert!(plain.contains("$$x$$"));
    assert_inert(&hosted, src);
}

#[cfg(feature = "class")]
#[test]
fn class_typesets_cardinalities_and_annotations() {
    let src = "classDiagram\n  class A {\n    <<$$x$$>>\n  }\n  A \"$$y$$\" --> \"1\" B\n";
    let (hosted, plain) = both(src);
    assert!(hosted.contains("class=\"pele-rs-cardinality\""));
    assert_eq!(
        hosted.matches("class=\"pele-rs-math\"").count(),
        2,
        "{hosted}"
    );
    assert!(plain.contains("$$y$$"));
    assert_inert(&hosted, src);
}

#[cfg(feature = "block")]
#[test]
fn block_typesets_math() {
    let src = "block-beta\n  a[\"$$x$$\"]\n";
    let (hosted, plain) = both(src);
    assert!(hosted.contains("class=\"pele-rs-math\""));
    assert!(plain.contains("$$x$$"));
    assert_inert(&hosted, src);
}

#[cfg(feature = "usecase")]
#[test]
fn usecase_draws_the_icon_of_an_actor() {
    let src = "usecase-beta\nactor C@{ icon: \"myicon\" }\nA\nC --> A\n";
    let (hosted, plain) = both(src);
    assert!(hosted.contains(ICON), "{hosted}");
    assert!(!plain.contains(ICON));
    assert_inert(&hosted, src);
}
