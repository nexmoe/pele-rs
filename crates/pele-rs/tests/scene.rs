#![cfg(all(feature = "scene", feature = "flowchart"))]
use pele_rs::{
    CachingMeasurer, ErrorCode, ParseOptions, RenderOptions, Style, TextMeasurer,
    scene::{FontMeasurer, SvgScene, usvg},
};
use std::sync::Arc;

fn fonts() -> Arc<usvg::fontdb::Database> {
    let mut db = usvg::fontdb::Database::new();
    db.load_font_data(include_bytes!("fonts/test.ttf").to_vec());
    db.set_sans_serif_family("Pele Test");
    db.set_serif_family("Pele Test");
    db.set_monospace_family("Pele Test");
    Arc::new(db)
}

#[test]
fn matches_real_font_advances_including_spaces_and_cjk() {
    let measurer = CachingMeasurer::new(
        FontMeasurer::new(fonts(), "Pele Test", "Pele Test").unwrap(),
        32,
    );
    for text in ["AB", "A B", "中文", " AB ", "  "] {
        let width = measurer.width(text, 20., Style::NONE);
        assert!(
            (width - text.chars().count() as f64 * 12.).abs() < 0.01,
            "{text}: {width}"
        );
    }
    assert!((measurer.width("AB", 20., Style::SUB) - 20.).abs() < 0.01);
}

#[test]
fn scene_retains_vectors_and_export_and_can_be_shared() {
    fn shared<T: Send + Sync>() {}
    shared::<SvgScene>();
    let parsed =
        pele_rs::parse("flowchart LR\nA[AB] --> B[中文]", &ParseOptions::default()).unwrap();
    let fonts = fonts();
    let options = RenderOptions {
        measurer: Some(Box::new(CachingMeasurer::new(
            FontMeasurer::new(fonts.clone(), "Pele Test", "Pele Test").unwrap(),
            32,
        ))),
        font_family: Some("Pele Test".into()),
        ..Default::default()
    };
    let scene = parsed.scene(&options, fonts).unwrap();
    assert!(!scene.tree().root().children().is_empty());
    assert!(!scene.rendered().svg.contains("var("));
    assert!((f64::from(scene.tree().size().width()) - scene.rendered().width).abs() < 0.01);
    let svg = scene.rendered().svg.clone();
    assert_eq!(svg, scene.into_rendered().svg);
}

#[test]
fn missing_fonts_and_unsupported_resources_are_explicit_errors() {
    let rendered = pele_rs::render("flowchart LR\nA[AB]", &Default::default()).unwrap();
    assert!(
        matches!(SvgScene::new(rendered, Arc::new(usvg::fontdb::Database::new())), Err(e) if e.code == ErrorCode::Semantic)
    );
    assert!(FontMeasurer::new(Arc::new(usvg::fontdb::Database::new()), "A", "B").is_err());
    let mut rendered = pele_rs::render("flowchart LR\nA[AB]", &Default::default()).unwrap();
    rendered.svg = "<svg><image href=\"/secret\"/></svg>".into();
    assert!(matches!(SvgScene::new(rendered, fonts()), Err(e) if e.code == ErrorCode::Unsupported));
}

fn emoji_fonts() -> Arc<usvg::fontdb::Database> {
    let mut db = (*fonts()).clone();
    // The system fallback order that triggered the report: placeholder before real emoji.
    db.load_font_data(include_bytes!("fonts/last-resort.ttf").to_vec());
    db.load_font_data(include_bytes!("fonts/emoji.ttf").to_vec());
    Arc::new(db)
}

#[test]
fn emoji_measurement_keeps_real_latin_and_emoji_advances() {
    let fonts = emoji_fonts();
    let measurer = FontMeasurer::new(fonts.clone(), "Pele Test", "Pele Test").unwrap();
    for emoji in ['🌋', '🤓'] {
        let width = measurer.width(&format!("AB{emoji}"), 20., Style::NONE);
        assert!(
            (width - 44.).abs() < 0.01,
            "AB{emoji}: {width}, expected 24 Latin + 20 emoji"
        );
    }
    assert_eq!(fonts.faces().count(), 3, "host font database was mutated");
}

#[test]
fn scene_keeps_embedded_color_emoji_instead_of_placeholder_outlines() {
    fn images(group: &usvg::Group) -> usize {
        group
            .children()
            .iter()
            .map(|node| match node {
                usvg::Node::Group(group) => images(group),
                usvg::Node::Text(text) => images(text.flattened()),
                usvg::Node::Image(_) => 1,
                _ => 0,
            })
            .sum()
    }
    let options = RenderOptions {
        font_family: Some("Pele Test".into()),
        ..Default::default()
    };
    for emoji in ['🌋', '🤓'] {
        let rendered = pele_rs::render(&format!("flowchart LR\nA[AB{emoji}]"), &options).unwrap();
        let scene = SvgScene::new(rendered, emoji_fonts()).unwrap();
        assert_eq!(
            images(scene.tree().root()),
            1,
            "missing embedded {emoji} bitmap"
        );
    }
}
