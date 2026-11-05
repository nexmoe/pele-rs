//! The options that every diagram type reads.
//!
//! The per-type test files cover each diagram's own syntax. The options that are shared
//! across the types — the nesting limit, the fonts, the fitting, the link allowlist — are
//! covered here, once.

// The file draws a pie chart, a flowchart, a state diagram, a class diagram and a sequence
// diagram, so it needs all five.
#![cfg(all(
    feature = "pie",
    feature = "flowchart",
    feature = "state",
    feature = "class",
    feature = "sequence"
))]

use pele_rs::{ErrorCode, Limits, RenderOptions, Style, TextMeasurer};

fn render(src: &str, options: &RenderOptions) -> pele_rs::Rendered {
    pele_rs::render(src, options).expect("the diagram renders")
}

fn svg(src: &str, options: &RenderOptions) -> String {
    render(src, options).svg
}

/// The diagrams that nest, with the word each of them uses for what it nests and a source
/// that nests it `depth` deep.
fn nested(kind: &str, depth: usize) -> String {
    match kind {
        "flowchart" => {
            let mut out = String::from("flowchart TD\n");
            for _ in 0..depth {
                out.push_str("subgraph s\n");
            }
            out.push_str("x\n");
            for _ in 0..depth {
                out.push_str("end\n");
            }
            out
        }
        "state" => {
            let mut out = String::from("stateDiagram-v2\n");
            for _ in 0..depth {
                out.push_str("state s {\n");
            }
            out.push_str("x\n");
            for _ in 0..depth {
                out.push_str("}\n");
            }
            out
        }
        "class" => {
            let mut out = String::from("classDiagram\n");
            for _ in 0..depth {
                out.push_str("namespace N {\n");
            }
            out.push_str("class C\n");
            for _ in 0..depth {
                out.push_str("}\n");
            }
            out
        }
        "sequence" => {
            let mut out = String::from("sequenceDiagram\n");
            for _ in 0..depth {
                out.push_str("alt a\n");
            }
            out.push_str("x->>y: m\n");
            for _ in 0..depth {
                out.push_str("end\n");
            }
            out
        }
        other => panic!("unknown kind {other}"),
    }
}

#[test]
fn stops_at_the_nesting_limit() {
    // Every diagram that nests reports the limit rather than running out of stack: a Rust
    // stack overflow aborts the process and cannot be caught.
    for (kind, word) in [
        ("flowchart", "Subgraphs"),
        ("state", "Composite states"),
        ("class", "Namespaces"),
        ("sequence", "Blocks"),
    ] {
        let options = RenderOptions {
            limits: Limits {
                max_depth: 2,
                ..Limits::default()
            },
            ..RenderOptions::default()
        };
        let err = pele_rs::render(&nested(kind, 3), &options).unwrap_err();
        assert_eq!(err.code, ErrorCode::Limit, "{kind}");
        assert_eq!(
            err.message,
            format!("{word} are nested deeper than the limit of 2."),
            "{kind}"
        );

        // The same diagram within the limit renders.
        let ok = RenderOptions {
            limits: Limits {
                max_depth: 3,
                ..Limits::default()
            },
            ..RenderOptions::default()
        };
        assert!(
            pele_rs::render(&nested(kind, 3), &ok).is_ok(),
            "{kind} nests three deep"
        );
    }
}

#[test]
fn draws_with_the_given_font_family() {
    let options = RenderOptions {
        font_family: Some("Inter".to_string()),
        font_family_mono: Some("Fira Code".to_string()),
        ..RenderOptions::default()
    };
    let svg = svg("pie\n\"a\" : 1\n", &options);
    assert!(svg.contains("--pele-rs-font-mono,Fira Code"), "{svg}");
    assert!(
        svg.contains("font-family:var(--pele-rs-font,Inter)"),
        "{svg}"
    );
}

#[test]
fn falls_back_to_the_generic_fonts_for_a_family_list_that_could_end_the_declaration() {
    let options = RenderOptions {
        font_family: Some("Inter; fill:url(javascript:alert(1))".to_string()),
        font_family_mono: Some("Fira Code".to_string()),
        ..RenderOptions::default()
    };
    let svg = svg("pie\n\"a\" : 1\n", &options);
    assert!(
        svg.contains("font-family:var(--pele-rs-font,sans-serif)"),
        "{svg}"
    );
    assert!(!svg.contains("javascript"), "{svg}");
}

#[test]
fn draws_with_the_given_font_size() {
    let options = RenderOptions {
        font_size: Some(24.0),
        ..RenderOptions::default()
    };
    let svg = svg("pie\n\"a\" : 1\n", &options);
    assert!(svg.contains("font:24px sans-serif"), "{svg}");
}

#[test]
fn adds_the_responsive_style_only_when_asked() {
    let responsive = RenderOptions {
        responsive: true,
        ..RenderOptions::default()
    };
    assert!(
        svg("pie\n\"a\" : 1\n", &responsive).contains("max-width:100%;height:auto;"),
        "the responsive style is written"
    );

    let fixed = RenderOptions {
        responsive: false,
        ..RenderOptions::default()
    };
    assert!(
        !svg("pie\n\"a\" : 1\n", &fixed).contains("max-width:100%"),
        "the fixed style leaves it out"
    );
}

#[test]
fn prefixes_the_ids_it_writes_with_the_given_prefix() {
    let options = RenderOptions {
        id_prefix: "d1".to_string(),
        ..RenderOptions::default()
    };
    let svg = svg("pie\naccTitle: the title\n\"a\" : 1\n", &options);
    assert!(
        svg.contains("<title id=\"d1-title\">the title</title>"),
        "{svg}"
    );
}

#[test]
fn measures_the_text_with_the_given_measurer() {
    /// Every character is one em wide, so the drawing is as wide as its longest label.
    struct OneEm;
    impl TextMeasurer for OneEm {
        fn width(&self, text: &str, size: f64, _style: Style) -> f64 {
            text.chars().count() as f64 * size
        }
    }

    let options = RenderOptions {
        measurer: Some(Box::new(OneEm)),
        ..RenderOptions::default()
    };
    let wide = render("pie\n\"abcd\" : 1\n", &options);
    let narrow = render("pie\n\"a\" : 1\n", &options);
    assert!(
        wide.width > narrow.width,
        "the measurer decides the width: {} vs {}",
        wide.width,
        narrow.width
    );
}

#[test]
fn allows_only_the_listed_link_schemes() {
    let options = RenderOptions {
        link_schemes: Some(vec!["https".to_string()]),
        ..RenderOptions::default()
    };
    let out = render(
        "flowchart TD\n A[\"a\"] --> B\n click A \"http://example.com\" \"t\"\n click B \"https://example.com\" \"t\"\n",
        &options,
    );
    let hrefs: Vec<&str> = out.links.iter().map(|l| l.href.as_str()).collect();
    assert_eq!(hrefs, ["https://example.com/"]);
}

#[test]
fn fits_the_drawing_into_the_given_width() {
    // A chart whose plot can be any width is drawn again until it fits.
    let options = RenderOptions {
        max_width: Some(200.0),
        ..RenderOptions::default()
    };
    let out = render("pie\n\"a long label\" : 1\n\"b\" : 2\n", &options);
    assert!(out.width <= 200.0, "the drawing fits: {}", out.width);
    assert!(
        out.svg.contains(&format!("viewBox=\"0 0 {} ", out.width)),
        "{}",
        out.svg
    );
}

#[test]
fn narrows_a_diagram_that_does_not_fit() {
    // A diagram whose layout cannot be made any narrower is drawn turned down instead.
    let src =
        "flowchart LR\n A[First step] --> B[Second step] --> C[Third step] --> D[Fourth step]\n";
    let wide = render(src, &RenderOptions::default());
    let options = RenderOptions {
        max_width: Some(320.0),
        ..RenderOptions::default()
    };
    let narrow = render(src, &options);
    assert!(narrow.width < wide.width);
    assert!(narrow.height > wide.height);
}

#[test]
fn keeps_the_padding_around_the_drawing() {
    let narrow = RenderOptions {
        padding: Some(0.0),
        ..RenderOptions::default()
    };
    let roomy = RenderOptions {
        padding: Some(20.0),
        ..RenderOptions::default()
    };
    let src = "flowchart TD\n A[\"a\"] --> B\n";
    assert!(
        render(src, &roomy).width > render(src, &narrow).width,
        "the padding is space around the drawing"
    );
}

#[test]
fn the_config_option_sits_under_the_diagrams_own_config() {
    let src = "flowchart LR\n A --> B\n A --> C\n";
    let plain = svg(src, &RenderOptions::default());
    let spaced = RenderOptions {
        config: Some(r#"{"flowchart": {"nodeSpacing": 120}}"#.to_string()),
        ..Default::default()
    };
    assert_ne!(svg(src, &spaced), plain);
    // The diagram's own directive wins over the host's.
    let own =
        "%%{init: {\"flowchart\": {\"nodeSpacing\": 50}}}%%\nflowchart LR\n A --> B\n A --> C\n";
    let own_only = svg(own, &RenderOptions::default());
    assert_eq!(svg(own, &spaced), own_only);
}

#[test]
fn a_config_that_is_not_an_object_is_an_error() {
    let options = RenderOptions {
        config: Some("[1, 2]".to_string()),
        ..Default::default()
    };
    let error = pele_rs::render("flowchart LR\n A --> B\n", &options).unwrap_err();
    assert_eq!(error.code, ErrorCode::Syntax);
}

#[test]
fn the_links_switch_leaves_every_link_out() {
    let src = "flowchart LR\n A --> B\n click A \"https://example.com\"\n";
    assert_eq!(render(src, &RenderOptions::default()).links.len(), 1);
    let off = render(
        src,
        &RenderOptions {
            links: false,
            ..Default::default()
        },
    );
    assert!(off.links.is_empty());
    assert!(!off.svg.contains("<a "));
}

#[test]
fn the_images_switch_leaves_every_image_out() {
    let src = "flowchart LR\n A@{ img: \"https://example.com/a.png\", label: \"A\" }\n";
    assert!(svg(src, &RenderOptions::default()).contains("<image"));
    let off = svg(
        src,
        &RenderOptions {
            images: false,
            ..Default::default()
        },
    );
    assert!(!off.contains("<image"));
}
