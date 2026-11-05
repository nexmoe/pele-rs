//! Gantt chart rendering, security, limits and speed.
#![cfg(feature = "gantt")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "gantt
dateFormat YYYY-MM-DD
title Adding GANTT diagram functionality to mermaid
excludes weekends
section A section
Completed task :done, des1, 2014-01-06,2014-01-08
Active task :active, des2, 2014-01-09, 3d
Future task : des3, after des2, 5d
Future task2 : des4, after des3, 5d
section Critical tasks
Completed task in the critical line :crit, done, 2014-01-06,24h
Implement parser and jison :crit, done, after des1, 2d
Create tests for parser :crit, active, 3d
Future task in critical line :crit, 5d
Create tests for renderer :2d
Add to mermaid :until isadded
Functionality added :milestone, isadded, 2014-01-25, 0d
section Documentation
Describe gantt syntax :active, a1, after des1, 3d
Add gantt diagram to demo page :after a1 , 20h
Release :vert, v1, 2014-01-20, 0d
click des2 href \"https://example.com/des2\"
";

const COMPACT: &str = "---
displayMode: compact
---
gantt
dateFormat YYYY-MM-DD
topAxis
tickInterval 1week
weekday monday
section S
A :a, 2024-01-01, 3d
B :b, 2024-01-02, 3d
C :c, 2024-01-05, 2d
section T
D :d, after a, 1d
";

/// 2014-01-15, in the middle of `FULL`.
const NOW: f64 = 1_389_744_000_000.0;

fn options(now: Option<f64>) -> RenderOptions {
    RenderOptions {
        now,
        ..RenderOptions::default()
    }
}

fn render(src: &str) -> pele_rs::Rendered {
    pele_rs::render(src, &options(Some(NOW))).unwrap_or_else(|e| panic!("{src:?}: {e}"))
}

fn svg(src: &str) -> String {
    render(src).svg
}

fn count(svg: &str, needle: &str) -> usize {
    svg.matches(needle).count()
}

/// Task groups, leaving out the group that holds them.
fn tasks(svg: &str) -> usize {
    count(svg, "<g class=\"pele-rs-task") - count(svg, "<g class=\"pele-rs-tasks\"")
}

fn try_render(src: &str) -> Option<String> {
    pele_rs::render(src, &options(Some(NOW)))
        .ok()
        .map(|r| r.svg)
}

#[test]
fn draws_tasks_sections_and_markers() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Gantt);
    assert_eq!(tasks(&out.svg), 13);
    assert_eq!(count(&out.svg, " pele-rs-crit"), 4);
    assert_eq!(count(&out.svg, "pele-rs-milestone"), 1);
    assert_eq!(count(&out.svg, "class=\"pele-rs-section-label\""), 3);
    assert_eq!(count(&out.svg, "class=\"pele-rs-vert\""), 1);
    assert!(out.svg.contains("pele-rs-excluded"));
    assert!(out.svg.contains("pele-rs-today"));
    assert!(out.svg.contains(">2014-01-15<"));
    assert_eq!(out.links.len(), 1);
    assert_eq!(out.links[0].href, "https://example.com/des2");
}

#[test]
fn today_follows_the_given_time() {
    // At the default time, 1970-01-01, the chart is long past and has no marker.
    let out = pele_rs::render(FULL, &options(None)).unwrap();
    assert!(!out.svg.contains("pele-rs-today"));
    // At the given time the marker is drawn, and `todayMarker off` takes it away. Without the
    // given time both statements hold whatever the chart says.
    assert!(render(FULL).svg.contains("pele-rs-today"));
    let off = FULL.replace("excludes weekends", "todayMarker off");
    assert!(!svg(&off).contains("pele-rs-today"));
}

#[test]
fn the_today_marker_sits_on_the_given_day() {
    let out = render(FULL);
    let marker = out.svg.split("pele-rs-today").nth(1).unwrap();
    let x = marker.split('M').nth(1).unwrap().split(',').next().unwrap();
    let tick = out
        .svg
        .split("<text x=\"")
        .find(|p| p.contains("2014-01-15"))
        .expect("the day is on the axis");
    assert_eq!(
        x,
        tick.split('"').next().unwrap(),
        "the marker is off its day"
    );
}

#[test]
fn packs_rows_in_compact_mode() {
    let out = render(COMPACT);
    assert_eq!(tasks(&out.svg), 4);
    // A and C share a row, as do the top and bottom axis labels. The chart to compare with is
    // the same one without the front matter: a chart with a title is taller either way, so
    // comparing with that would hold even if compact mode were ignored.
    let normal = render(&COMPACT.replace("---\ndisplayMode: compact\n---\n", ""));
    assert!(
        out.height < normal.height,
        "{} vs {}",
        out.height,
        normal.height
    );
}

#[test]
fn reports_errors() {
    let err = pele_rs::render("gantt\nA :a, 2024-01-01\n", &options(None)).unwrap_err();
    assert_eq!(err.kind, Some(DiagramKind::Gantt));
    assert_eq!(err.code, ErrorCode::Semantic);
    assert_eq!(err.message, "Invalid date:a");
    let err = pele_rs::render("gantt\ndateFormat\n", &options(None)).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.line, 2);
    let err = pele_rs::render("gantt\nA :a, 2024-13-45, 1d\n", &options(None)).unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert_eq!(err.message, "Invalid date:2024-13-45");
}

#[test]
fn fits_a_narrow_host() {
    let narrow = pele_rs::render(
        FULL,
        &RenderOptions {
            max_width: Some(400.0),
            ..options(Some(NOW))
        },
    )
    .unwrap();
    assert!(narrow.width < render(FULL).width);
}

#[test]
fn output_is_deterministic() {
    assert_eq!(svg(FULL), svg(FULL));
}

#[test]
fn is_inert_for_hostile_and_mutated_inputs() {
    let mut rendered = 0;
    for payload in PAYLOADS {
        let p = payload.replace('\n', " ");
        for src in [
            format!("gantt\ntitle {p}\nA :2024-01-01, 1d"),
            format!("gantt\nsection {p}\n{p} :a, 2024-01-01, 1d"),
            format!("gantt\n{p} :vert, v, 2024-01-01, 0d\nA :2024-01-01, 2d"),
            format!("gantt\nA :a, 2024-01-01, 1d\nclick a href \"{p}\""),
            format!("gantt\naxisFormat {p}\nA :2024-01-01, 1d"),
            format!("gantt\ntodayMarker {p}\nA :1970-01-01, 1d"),
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
    fragments.extend([
        "\n",
        ":",
        ", ",
        "after a1",
        ":crit, ",
        ":milestone, ",
        "excludes weekends\n",
        "tickInterval 1day\n",
        "dateFormat X\n",
        "click des1 href ",
    ]);
    let corpus = [FULL, COMPACT];
    let mut rnd = Random::new(13);
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
        ..options(None)
    };
    const N: usize = 50000;
    let head = "gantt\ndateFormat YYYY-MM-DD\n";
    let cases = [
        format!("{head}{} :2024-01-01, 1d", "x".repeat(N)),
        format!(
            "{head}{}",
            repeat(3000, |i| format!("t{i} :2024-01-01, {i}d\n"))
        ),
        format!(
            "{head}a0 :a0, 2024-01-01, 1d\n{}",
            repeat(3000, |i| format!("t :a{}, after a{i}, 1d\n", i + 1))
        ),
        format!(
            "{head}{}",
            repeat(3000, |i| format!("t :a{i}, after a{}, 1d\n", i + 1))
        ),
        format!("{head}excludes weekends\nA :2000-01-01, 9999y"),
        format!("{head}A :0001-01-01, 9999-12-31"),
        format!("{head}tickInterval 1millisecond\nA :2024-01-01, 1y"),
        format!("gantt\ndateFormat {}\nA :x, 1d", "Y".repeat(N / 2)),
        format!(
            "{head}excludes {}\nA :2024-01-01, 300d",
            "2024-01-01,".repeat(N / 11)
        ),
        format!("{head}{}", "%%\n".repeat(N / 3)),
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
        rendered >= 6,
        "only {rendered} of {} cases were drawn",
        cases.len()
    );
}

#[test]
fn draws_the_axis_in_the_given_format() {
    let svg = svg("gantt\ndateFormat YYYY-MM-DD\naxisFormat %d/%m\nA :a, 2024-01-01, 3d\n");
    assert!(svg.contains(">01/01<"), "{svg}");
    assert!(!svg.contains(">2024-01-01<"), "{svg}");
}

#[test]
fn shades_the_excluded_weekends() {
    let out = render(FULL);
    let at = out
        .svg
        .find("pele-rs-excluded")
        .expect("the weekends are shaded");
    let segment = &out.svg[at..];
    let end = segment.find("</g>").expect("the group ends");
    assert_eq!(segment[..end].matches("<rect").count(), 3, "{segment}");
}

#[test]
fn adds_the_last_day_when_the_end_dates_are_inclusive() {
    let plain = svg("gantt\ndateFormat YYYY-MM-DD\nA :a, 2024-01-01, 2024-01-03\n");
    assert!(!plain.contains(">2024-01-04<"), "{plain}");
    let inclusive =
        svg("gantt\ndateFormat YYYY-MM-DD\ninclusiveEndDates\nA :a, 2024-01-01, 2024-01-03\n");
    assert!(inclusive.contains(">2024-01-04<"), "{inclusive}");
}

#[test]
fn falls_back_to_the_date_parser_when_the_date_does_not_match_the_format() {
    // As in Mermaid, a date the format does not describe is read the way JavaScript reads it.
    let out = render("gantt\ndateFormat DD/MM/YYYY\nA :a, 2024-01-06, 3d\n");
    assert!(out.svg.contains(">2024-01-06<"), "{}", out.svg);
}

#[test]
fn reads_a_date_that_only_javascripts_own_parser_reads() {
    // Mermaid gives a date the format does not match a second chance with JavaScript's own date
    // parser, which reads "2014-1-12" and a bare "0" as well. pele-rs does the same, so the
    // chart is drawn as Mermaid draws it.
    let out = render("gantt\ndateFormat YYYY-MM-DD\nA :a, 2014-1-12, 3d\n");
    assert!(out.svg.contains(">2014-01-12<"), "{}", out.svg);
}

#[test]
fn draws_a_milestone_as_a_diamond() {
    let out = render(
        "gantt\ndateFormat YYYY-MM-DD\nFunctionality added :milestone, isadded, 2014-01-25, 0d\n",
    );
    assert!(
        out.svg
            .contains("pele-rs-milestone\" data-id=\"isadded\"><path"),
        "{}",
        out.svg
    );
}

#[test]
fn a_bare_today_marker_takes_the_next_line_as_its_value() {
    // Mermaid's lexer lets the value of `todayMarker` sit on the next line, so the task that
    // follows is read as the marker's value and the chart has no tasks. Mermaid does the same,
    // so this is ported as it is.
    let out = render("gantt\ndateFormat YYYY-MM-DD\ntodayMarker\nA :a, 2024-01-01, 3d\n");
    assert_eq!(
        tasks(&out.svg),
        0,
        "the task was read as the marker's value"
    );
}

#[test]
fn snapshots() {
    let cases = [("full", FULL.to_string()), ("compact", COMPACT.to_string())];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
