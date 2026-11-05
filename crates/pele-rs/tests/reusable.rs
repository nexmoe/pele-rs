//! Public contracts needed by an editor with background layout and device-dependent painting.
#![cfg(feature = "all")]

use pele_rs::{
    CachingMeasurer, DiagramKind, ErrorCode, MetricsMeasurer, ParseOptions, RenderOptions, Style,
    TextMeasurer, parse, render,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn parsed_models_match_one_shot_and_can_be_shared_between_workers() {
    fn shareable<T: Send + Sync>() {}
    shareable::<pele_rs::ParsedDiagram>();
    let sources = [
        "flowchart LR\nA-->B",
        "sequenceDiagram\nA->>B: Hello",
        "gantt\ndateFormat YYYY-MM-DD\nTask :2026-01-01, 2d",
        "agentflow-beta\nA[First] --> B[Second]",
        "pie\n\"a\":1",
    ];
    for src in sources {
        let parsed = parse(src, &ParseOptions::default()).unwrap();
        assert_eq!(parsed.source(), src);
        let workers = [240.0, 640.0, 1200.0].map(|width| {
            let parsed = parsed.clone();
            let src = src.to_string();
            std::thread::spawn(move || {
                let options = RenderOptions {
                    max_width: Some(width),
                    now: Some(1000.),
                    ..Default::default()
                };
                let output = parsed.render(&options).unwrap();
                assert_eq!(output, render(&src, &options).unwrap());
                assert_eq!(output, parsed.render(&options).unwrap());
            })
        });
        for worker in workers {
            worker.join().unwrap();
        }
    }
}

#[test]
fn parsed_models_retain_parsing_config_and_limits() {
    let parsed = parse("flowchart LR\nA-->B", &ParseOptions::default()).unwrap();
    let options = RenderOptions {
        config: Some("{\"flowchart\":{\"inheritDir\":true}}".into()),
        ..Default::default()
    };
    assert!(parsed.render(&options).is_ok());
    let mut options = RenderOptions::default();
    options.limits.max_edges = 1;
    assert!(parsed.render(&options).is_ok());
    options.limits = Default::default();
    options.limits.output = 20;
    assert_eq!(parsed.render(&options).unwrap_err().code, ErrorCode::Limit);
}

#[test]
fn maps_diagnostics_through_front_matter_comments_directives_and_crlf() {
    for prefix in [
        "---\ntitle: Demo\n---\n%% comment\n",
        "%%{init: {\n\"theme\":\"dark\"\n}}%%\n%% comment\n",
    ] {
        for ending in ["\n", "\r\n", "\r"] {
            let src = format!("{prefix}flowchart LR\n A[未闭合\n").replace('\n', ending);
            let err = render(&src, &Default::default()).unwrap_err();
            assert_eq!(err.code, ErrorCode::Syntax);
            assert_eq!(err.line as usize, prefix.lines().count() + 2, "{err:?}");
            assert!(err.snippet.contains("未闭合"), "{err:?}");
        }
    }
}

#[test]
fn author_palettes_survive_literal_host_colors_without_touching_labels() {
    let source = "%%{init:{\"themeVariables\":{\"pie1\":\"#ff0000\",\"pie2\":\"#00ff00\"}}}%%\npie\n\"var(--pele-series-1,#4c78a8)\":1\n\"B\":1";
    let options = RenderOptions {
        palette: Some(pele_rs::Palette::dark()),
        ..Default::default()
    };
    let out = render(source, &options).unwrap();
    assert_eq!(out.kind, DiagramKind::Pie);
    assert!(out.svg.contains("fill=\"#ff0000\""));
    assert!(out.svg.contains("fill=\"#00ff00\""));
    assert!(out.svg.contains(">var(--pele-series-1,#4c78a8)<"));
    for payload in [
        "url(javascript:alert(1))",
        "var(--pele-series-1,red)",
        "red\" onload=\"alert(1)",
    ] {
        let src =
            format!("%%{{init:{{\"themeVariables\":{{\"pie1\":\"{payload}\"}}}}}}%%\npie\n\"a\":1");
        let result = render(&src, &options).unwrap();
        assert!(!result.svg.contains("onload="));
        assert!(!result.svg.contains("javascript:"));
    }
}

#[test]
fn subnormal_sankey_flows_have_visible_geometry() {
    let out = render("sankey\na,b,5e-324", &Default::default()).unwrap();
    assert!(out.svg.contains("height=\"400\""), "{}", out.svg);
    assert!(!out.svg.contains("NaN"));
    assert!(out.svg.contains("L590,400"), "{}", out.svg);
}

struct Counting(Arc<AtomicUsize>);
impl TextMeasurer for Counting {
    fn width(&self, text: &str, size: f64, style: Style) -> f64 {
        self.0.fetch_add(1, Ordering::Relaxed);
        MetricsMeasurer.width(text, size, style)
    }
}

#[test]
fn caches_font_size_and_style_separately_and_stays_bounded() {
    let calls = Arc::new(AtomicUsize::new(0));
    let cached = CachingMeasurer::new(Counting(calls.clone()), 2);
    cached.width("中文", 16., Style::NONE);
    cached.width("中文", 16., Style::NONE);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    cached.width("中文", 16., Style::BOLD);
    cached.width("中文", 20., Style::NONE);
    assert_eq!(calls.load(Ordering::Relaxed), 3);
    assert_eq!(cached.len(), 2);
    cached.clear();
    assert!(cached.is_empty());
    let disabled = CachingMeasurer::new(Counting(calls), 0);
    disabled.width("a", 16., Style::NONE);
    assert!(disabled.is_empty());
}

#[test]
fn rejects_invalid_host_metrics_and_recovers_on_the_next_render() {
    struct Invalid;
    impl TextMeasurer for Invalid {
        fn width(&self, _: &str, _: f64, _: Style) -> f64 {
            f64::NAN
        }
    }
    let parsed = parse(
        "flowchart LR\nA[Text] --> B[Other]",
        &ParseOptions::default(),
    )
    .unwrap();
    let invalid = RenderOptions {
        measurer: Some(Box::new(Invalid)),
        ..Default::default()
    };
    assert_eq!(parsed.render(&invalid).unwrap_err().code, ErrorCode::Limit);
    assert!(parsed.render(&RenderOptions::default()).is_ok());
}

#[test]
fn eof_points_before_trailing_comments_and_entities_do_not_shift_columns() {
    let src = "%% comment\nflowchart LR\n A[未闭合\n%% trailing comment\n";
    let error = render(src, &Default::default()).unwrap_err();
    assert_eq!(error.line, 3);
    assert!(error.snippet.starts_with(" A[未闭合\n"));
    let src = "---\ntitle: Example\n---\nflowchart LR\n A[\"#quot;中文#quot;\"] --> B[open";
    let error = render(src, &Default::default()).unwrap_err();
    assert_eq!(error.line, 5);
    assert_eq!(
        error.column as usize,
        src.lines().last().unwrap().chars().count() + 1
    );
}
