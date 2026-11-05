//! Packet diagram rendering, security, limits and speed.
#![cfg(feature = "packet")]

mod support;

use std::time::{Duration, Instant};

use pele_rs::{DiagramKind, ErrorCode, Limits, RenderOptions};
use support::{PAYLOADS, Random, assert_inert, mutate};

const FULL: &str = "packet
0-15: \"Source Port\"
16-31: \"Destination Port\"
32-63: \"Sequence Number\"
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
fn draws_the_fields_and_the_bits() {
    let out = render(FULL);
    assert_eq!(out.kind, DiagramKind::Packet);
    // The three fields.
    assert_eq!(count(&out.svg, "<g class=\"pele-node\""), 3);
    // The names of the fields.
    assert!(out.svg.contains(">Source Port<"), "{}", out.svg);
    assert!(out.svg.contains(">Destination Port<"), "{}", out.svg);
    assert!(out.svg.contains(">Sequence Number<"), "{}", out.svg);
    // The bits of the first two fields, at both ends.
    assert!(out.svg.contains(">0</text>"), "{}", out.svg);
    assert!(out.svg.contains(">15</text>"), "{}", out.svg);
    assert!(out.svg.contains(">16</text>"), "{}", out.svg);
    assert!(out.svg.contains(">31</text>"), "{}", out.svg);
    // The field that is wider than a row is drawn over two rows.
    let wide = render("packet\n0-63: \"Wide\"\n");
    assert_eq!(count(&wide.svg, "<g class=\"pele-node\""), 2);
}

#[test]
fn draws_a_field_of_one_bit() {
    // A field of one bit has its number in the middle, under the field.
    let out = render("packet\n0: \"Flag\"\n");
    assert!(
        out.svg.contains("text-anchor=\"middle\">0</text>"),
        "{}",
        out.svg
    );
}

#[test]
fn reads_the_config() {
    // The number of bits a row holds.
    let wide = svg("packet\n0-15: \"A\"\n");
    let narrow = svg("---\nconfig:\n  packet:\n    bitsPerRow: 8\n---\npacket\n0-15: \"A\"\n");
    assert_ne!(wide, narrow);
    // The bit numbers can be left out.
    let with = svg("packet\n0-7: \"A\"\n");
    let without = svg("---\nconfig:\n  packet:\n    showBits: false\n---\npacket\n0-7: \"A\"\n");
    assert!(with.contains(">0</text>"));
    assert!(!without.contains(">0</text>"));
    // The rows can be drawn from their highest bit down to their lowest.
    let up = svg("packet\n0-15: \"A\"\n");
    let down = svg("---\nconfig:\n  packet:\n    bitOrder: descending\n---\npacket\n0-15: \"A\"\n");
    assert_ne!(up, down);
}

#[test]
fn draws_a_title() {
    // The title of the diagram, and the title of the front matter.
    let out = render("packet\ntitle A packet\n0-7: \"A\"\n");
    assert!(out.svg.contains(">A packet<"), "{}", out.svg);
    assert!(out.svg.contains("pele-title"));
    let front = render("---\ntitle: A packet\n---\npacket\n0-7: \"A\"\n");
    assert!(front.svg.contains(">A packet<"), "{}", front.svg);
}

#[test]
fn draws_the_accessible_text() {
    let out =
        render("packet\naccTitle: The packet\naccDescr: The fields of a packet\n0-7: \"A\"\n");
    assert!(out.svg.contains("The packet"), "{}", out.svg);
    assert!(out.svg.contains("The fields of a packet"), "{}", out.svg);
}

#[test]
fn reports_errors_with_positions() {
    // The fields have to follow one another.
    let err = pele_rs::render(
        "packet\n0-7: \"A\"\n16-23: \"B\"\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert_eq!(err.kind, Some(DiagramKind::Packet));
    assert_eq!(
        err.message,
        "Packet block 16 - 23 is not contiguous. It should start from 8."
    );
    // A field cannot end before it starts.
    let err = pele_rs::render("packet\n8-4: \"A\"\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert_eq!(
        err.message,
        "Packet block 8 - 4 is invalid. End must be greater than start."
    );
    // A field cannot take no bits at all.
    let err = pele_rs::render("packet\n+0: \"A\"\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    assert_eq!(
        err.message,
        "Packet block 0 is invalid. Cannot have a zero bit field."
    );
    // A name has to be a string.
    let err = pele_rs::render("packet\n0-7: A\n", &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Packet));
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
fn fits_a_narrow_host() {
    let narrow = narrow(FULL);
    assert!(narrow.width <= 360.0, "{}", narrow.width);
    assert!(narrow.height > 0.0);
    // The fields are still drawn, and the field that is wider than a row is cut in two.
    assert_eq!(count(&narrow.svg, "<g class=\"pele-node\""), 4);
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
            format!("packet\n0-7: \"{p}\"\n"),
            format!("packet\n+8: \"{p}\"\n"),
            format!("packet\ntitle {p}\n0-7: \"A\"\n"),
            format!("packet\naccTitle: {p}\n0-7: \"A\"\n"),
            format!("packet\naccDescr: {p}\n0-7: \"A\"\n"),
            format!("---\ntitle: \"{p}\"\n---\npacket\n0-7: \"A\"\n"),
        ] {
            let Some(svg) = try_render(&src) else {
                continue;
            };
            assert_inert(&svg, &src);
            rendered += 1;
        }
    }
    assert!(rendered > 40, "only {rendered} hostile inputs rendered");
    let mut fragments: Vec<&str> = PAYLOADS.to_vec();
    fragments.extend([
        "packet\n",
        "packet-beta\n",
        "title ",
        "accTitle: ",
        "accDescr: ",
        "accDescr {",
        "}",
        "0-15",
        "16-31",
        "+8",
        "+16",
        ": ",
        ":",
        "\"",
        "'",
        "0",
        "1",
        "-",
        "+",
        "\n",
        " ",
        "%%",
        "---\n",
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
    // A parser that refused every mutation would make the loop above assert nothing at all.
    assert!(rendered > 150, "only {rendered} of 1500 mutations rendered");
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
    let cases = [
        format!("packet\n0-{}: \"{}\"\n", N, "x".repeat(N)),
        format!("packet\n{}", "+8: \"A\"\n".repeat(N / 2)),
        format!("packet\n{}", "0-7: \"A\"\n".repeat(N / 2)),
        format!("packet\n0-7: \"A\"\n{}", "+8: \"B\"\n".repeat(N / 2)),
        format!("packet\ntitle {}\n0-7: \"A\"\n", "t".repeat(N)),
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
        ("bits", "packet\n+8: \"Byte\"\n+16: \"Word\"\n".to_string()),
        (
            "rows",
            "---\nconfig:\n  packet:\n    bitsPerRow: 16\n---\npacket\n0-15: \"A\"\n16-31: \"B\"\n"
                .to_string(),
        ),
        (
            "title",
            "packet\ntitle A packet\n0-7: \"A\"\n8-15: \"B\"\n".to_string(),
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(&src));
    }
}
