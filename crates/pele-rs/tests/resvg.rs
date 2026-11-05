//! The palette option: output that a viewer without CSS variables (resvg) draws correctly.
//!
//! The SVG is read with usvg, the parser of resvg, which ignores `var()`.

#![cfg(feature = "all")]

use pele_rs::{Palette, RenderOptions};

const SOURCES: &[&str] = &[
    "pie title 寵物\n\"狗\" : 386\n\"貓\" : 85",
    "flowchart LR\n A[開始] --> B{判斷}\n B -->|是| C[結束]\n subgraph s[群組]\n D\n end",
    "sequenceDiagram\n A->>B: 你好\n B-->>A: こんにちは",
    "stateDiagram-v2\n [*] --> Still\n Still --> [*]",
    "classDiagram\n class Animal {\n +name\n }\n Animal <|-- Dog",
    "erDiagram\n CUSTOMER ||--o{ ORDER : places",
    "gantt\n title 計畫\n dateFormat YYYY-MM-DD\n section A\n 工作 :a1, 2024-01-01, 5d",
    "xychart-beta\n x-axis [a, b, c]\n bar [1, 2, 3]\n line [1, 2, 3]",
    "mindmap\n root((中心))\n  甲\n  乙",
    "gitGraph\n commit\n branch dev\n commit\n checkout main\n merge dev",
];

fn options(palette: Palette) -> RenderOptions {
    RenderOptions {
        palette: Some(palette),
        ..RenderOptions::default()
    }
}

/// The markup of the tags, without the text between them.
fn tags(svg: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in svg.chars() {
        match c {
            '<' => inside = true,
            '>' => {
                inside = false;
                out.push('>');
            }
            _ if inside => out.push(c),
            _ => {}
        }
    }
    out
}

fn fills(node: &usvg::Group, out: &mut Vec<usvg::Color>) {
    for child in node.children() {
        match child {
            usvg::Node::Group(g) => fills(g, out),
            usvg::Node::Path(p) => {
                if let Some(usvg::Paint::Color(c)) = p.fill().map(|f| f.paint()) {
                    out.push(*c);
                }
            }
            _ => {}
        }
    }
}

#[test]
fn default_output_keeps_its_variables() {
    for src in SOURCES {
        let svg = pele_rs::render(src, &RenderOptions::default()).unwrap().svg;
        assert!(svg.contains("var(--"), "{src}");
        assert_eq!(
            svg,
            pele_rs::render(src, &RenderOptions::default()).unwrap().svg
        );
    }
}

#[test]
fn palette_output_has_no_variables_and_no_black_fill() {
    for palette in [Palette::light(), Palette::dark()] {
        for src in SOURCES {
            let svg = pele_rs::render(src, &options(palette.clone())).unwrap().svg;
            assert!(!tags(&svg).contains("var("), "{src}\n{svg}");
            // The same input gives the same output.
            assert_eq!(
                svg,
                pele_rs::render(src, &options(palette.clone())).unwrap().svg
            );
            let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())
                .unwrap_or_else(|e| panic!("{src}: {e}"));
            let mut colors = Vec::new();
            fills(tree.root(), &mut colors);
            assert!(!colors.is_empty(), "{src}");
            // A variable that resvg cannot read is drawn black.
            assert!(
                colors.iter().all(|c| (c.red, c.green, c.blue) != (0, 0, 0)),
                "{src}\n{svg}"
            );
        }
    }
}

#[test]
fn palette_colors_reach_the_drawing() {
    let mut palette = Palette::light();
    palette.surface = "#123456".to_string();
    let svg = pele_rs::render("flowchart LR\n A --> B", &options(palette))
        .unwrap()
        .svg;
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).unwrap();
    let mut colors = Vec::new();
    fills(tree.root(), &mut colors);
    assert!(
        colors
            .iter()
            .any(|c| (c.red, c.green, c.blue) == (0x12, 0x34, 0x56))
    );
}

#[test]
fn an_unsafe_color_cannot_break_out_of_the_attribute() {
    let mut palette = Palette::light();
    palette.surface = "red\" onload=\"alert(1)".to_string();
    palette.series[0] = "<script>".to_string();
    let svg = pele_rs::render("flowchart LR\n A --> B", &options(palette))
        .unwrap()
        .svg;
    assert!(!svg.contains("onload"));
    assert!(!svg.contains("<script"));
    assert!(usvg::Tree::from_str(&svg, &usvg::Options::default()).is_ok());
}

#[test]
fn text_is_sized_and_falls_back_to_cjk_fonts() {
    let svg = pele_rs::render("flowchart LR\n A[開始] --> B", &options(Palette::light()))
        .unwrap()
        .svg;
    // resvg reads neither the `font` shorthand nor a style after a custom property.
    assert!(svg.contains("font-size:16px"));
    assert!(!svg.contains("--_"));
    assert!(svg.contains("font-family:sans-serif,'Noto Sans CJK TC'"));
    assert!(svg.contains("'PingFang TC'") && svg.contains("'Microsoft JhengHei'"));
    // The default output is as it was.
    let plain = pele_rs::render("flowchart LR\n A[開始] --> B", &RenderOptions::default())
        .unwrap()
        .svg;
    assert!(plain.contains("font-family:var(--pele-font,sans-serif)"));
    assert!(!plain.contains("CJK"));
}

#[test]
fn text_in_a_label_is_not_rewritten() {
    let svg = pele_rs::render(
        "flowchart LR\n A[\"var(--_fg)\"]",
        &options(Palette::light()),
    )
    .unwrap()
    .svg;
    assert!(svg.contains(">var(--_fg)<"));
}

#[test]
fn a_cjk_character_is_one_em_wide() {
    use pele_rs::{MetricsMeasurer, Style, TextMeasurer};
    for text in ["中", "あ", "ア", "한", "，", "Ａ"] {
        assert_eq!(
            MetricsMeasurer.width(text, 16.0, Style::NONE),
            16.0,
            "{text}"
        );
    }
}
