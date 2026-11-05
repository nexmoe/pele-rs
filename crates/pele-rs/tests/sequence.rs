//! Sequence diagram rendering, security, limits and speed.
#![cfg(feature = "sequence")]

mod support;

use std::fmt::Write as _;
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

include!("compat/sequence/cases.rs");

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
        .filter(|s| s.to_lowercase().contains("sequencediagram"))
        .collect()
}

#[test]
fn draws_participants_messages_and_notes() {
    let out = render(
        "sequenceDiagram\n participant A as Alice\n actor B as Bob\n A->>+B: Hello\n B-->>-A: Hi\n Note over A,B: shared\n A-xB: lost\n",
    );
    assert_eq!(out.kind, DiagramKind::Sequence);
    // Each participant is drawn at the top and mirrored at the bottom.
    assert_eq!(count(&out.svg, "pele-rs-actor-participant\""), 1);
    assert_eq!(count(&out.svg, "pele-rs-actor-footer"), 2);
    assert_eq!(count(&out.svg, "class=\"pele-rs-edge pele-rs-message\""), 3);
    assert_eq!(count(&out.svg, "class=\"pele-rs-activation\""), 1);
    assert_eq!(count(&out.svg, "class=\"pele-rs-note\""), 1);
    assert_eq!(count(&out.svg, "class=\"pele-rs-lifeline\""), 2);
    assert!(out.svg.contains(">Alice<"));
    assert!(out.svg.contains(">shared<"));
    assert!(out.svg.contains("stroke-dasharray=\"4 4\""));
}

#[test]
fn draws_blocks_boxes_numbers_and_lifecycles() {
    let svg = svg(
        "sequenceDiagram\n autonumber\n box Aqua Team\n participant A\n participant B\n end\n loop every minute\n A->>B: ping\n end\n alt ok\n B->>A: pong\n else failed\n B-xA: error\n end\n rect rgb(200, 220, 255)\n A->>A: think\n end\n create participant C\n A->>C: spawn\n destroy C\n C-->>A: done\n",
    );
    for needle in [
        "pele-rs-frame-loop",
        "pele-rs-frame-alt",
        "class=\"pele-rs-rect\"",
        "class=\"pele-rs-cluster pele-rs-box\"",
        "class=\"pele-rs-sequence-number\"",
        "class=\"pele-rs-destroy\"",
        ">[every minute]<",
        ">[failed]<",
    ] {
        assert!(svg.contains(needle), "{needle}");
    }
    assert_inert(&svg, "blocks");
}

#[test]
fn draws_every_participant_kind() {
    for kind in [
        "participant",
        "actor",
        "boundary",
        "control",
        "entity",
        "database",
        "collections",
        "queue",
    ] {
        let svg = svg(&format!(
            "sequenceDiagram\n participant A@{{ \"type\": \"{kind}\" }}\n A->>A: x\n"
        ));
        assert!(svg.contains(&format!("pele-rs-actor-{kind}")), "{kind}");
        // The class says which kind it is; the drawing has to be that shape.
        match kind {
            "participant" | "collections" => {
                assert!(svg.contains("<rect"), "{kind}");
                assert!(!svg.contains("<circle"), "{kind}");
            }
            "actor" | "boundary" | "control" | "entity" => {
                assert!(svg.contains("<circle"), "{kind}");
                assert!(!svg.contains("<rect"), "{kind}");
            }
            "database" | "queue" => {
                assert!(!svg.contains("<circle"), "{kind}");
                assert!(!svg.contains("<rect"), "{kind}");
            }
            _ => {}
        }
        assert_inert(&svg, kind);
    }
}

#[test]
fn never_emits_links_for_participant_menus() {
    let out = render(
        "sequenceDiagram\n participant A\n link A: x @ javascript:alert(1)\n links A: {\"y\": \"javascript:alert(1)\"}\n A->>B: hi\n",
    );
    assert!(!out.svg.contains("javascript"));
    assert!(!out.svg.contains("<a"));
    assert!(out.links.is_empty());
    // Not even a link the host would allow: the participant menus are not drawn at all.
    let safe = render(
        "sequenceDiagram\n participant A\n link A: Help @ https://example.com\n links A: {\"help\": \"https://example.com\"}\n A->>B: hi\n",
    );
    assert!(!safe.svg.contains("example.com"), "{}", safe.svg);
    assert!(!safe.svg.contains("<a"), "{}", safe.svg);
    assert!(safe.links.is_empty());
}

#[test]
fn passes_colors_through_the_style_filter() {
    let drawn = svg(
        "sequenceDiagram\n rect url(javascript:alert(1))\n A->>B: hi\n end\n rect red\n B->>A: yo\n end\n",
    );
    assert!(!drawn.contains("url("));
    assert!(drawn.contains("style=\"rx:var(--_r);fill:red;\""));
    let boxed = svg("sequenceDiagram\n box rgb(1, 2, 3) Team\n participant A\n end\n");
    assert!(boxed.contains("rgb(1, 2, 3)"), "{boxed}");
}

#[test]
fn reports_errors_with_positions() {
    let err = pele_rs::render(
        "sequenceDiagram\n A->>B: x\n A->>: y\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Syntax);
    assert_eq!(err.kind, Some(DiagramKind::Sequence));
    assert_eq!((err.line, err.column), (3, 6));
    let err = pele_rs::render(
        "sequenceDiagram\n A->>B: x\n deactivate B\n",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Semantic);
    // A semantic error carries no position of its own.
    assert_eq!((err.line, err.column), (0, 0));
}

#[test]
fn enforces_limits() {
    let deep = format!(
        "sequenceDiagram\n{}A->>B: x\n{}",
        "loop\n".repeat(100),
        "end\n".repeat(100)
    );
    let err = pele_rs::render(&deep, &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Limit);
    assert!(
        err.message.contains("nested deeper than the limit"),
        "{}",
        err.message
    );
    let long = format!("sequenceDiagram\n{}", "A->>B: hello\n".repeat(5000));
    let err = pele_rs::render(&long, &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Limit);
    assert!(
        err.message.contains("source is longer than the limit"),
        "{}",
        err.message
    );
}

#[test]
fn reads_the_sequence_config() {
    let src = "sequenceDiagram\n A->>B: x\n B->>A: y\n";
    assert_eq!(count(&svg(src), "pele-rs-actor-footer"), 2);
    let plain = svg(&format!(
        "---\nconfig:\n  sequence:\n    mirrorActors: false\n    showSequenceNumbers: true\n---\n{src}"
    ));
    assert_eq!(count(&plain, "pele-rs-actor-footer"), 0);
    assert_eq!(count(&plain, "class=\"pele-rs-sequence-number\""), 2);
    let hidden = svg(
        "%%{init: {\"sequence\": {\"hideUnusedParticipants\": true}}}%%\nsequenceDiagram\n participant Q\n A->>B: x\n",
    );
    assert!(!hidden.contains("data-id=\"Q\""));
    // The participants that are used are still there.
    assert_eq!(count(&hidden, "class=\"pele-rs-lifeline\""), 2, "{hidden}");
}

#[test]
fn narrows_to_fit_a_narrow_host() {
    let src = "sequenceDiagram\n participant A as A participant with a long name\n participant B as Another participant with a long name\n A->>B: a message that is long enough to need wrapping somewhere\n";
    let wide = render(src);
    let options = RenderOptions {
        max_width: Some(320.0),
        ..RenderOptions::default()
    };
    let narrow = pele_rs::render(src, &options).unwrap();
    assert!(narrow.width < wide.width);
    assert!(narrow.width <= 320.0);
}

#[test]
fn output_is_deterministic() {
    let src = "sequenceDiagram\n autonumber\n A->>+B: x\n par a\n B->>C: y\n and b\n C->>A: z\n end\n B-->>-A: w\n";
    assert_eq!(svg(src), svg(src));
}

#[test]
fn is_inert_for_corpus_hostile_and_mutated_inputs() {
    let corpus = corpus();
    assert!(corpus.len() > 100);
    for src in &corpus {
        if let Some(svg) = try_render(src) {
            assert_inert(&svg, src);
        }
    }
    let json = |p: &str| {
        let mut out = String::from("\"");
        for c in p.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                c if (c as u32) < 0x20 => {
                    let _ = write!(out, "\\u{:04x}", c as u32);
                }
                c => out.push(c),
            }
        }
        out + "\""
    };
    let mut rendered = 0;
    for payload in PAYLOADS {
        let p = *payload;
        let j = json(p);
        for src in [
            format!("sequenceDiagram\n participant {p}\n A->>B: x"),
            format!("sequenceDiagram\n actor A as {p}\n A->>B: x"),
            format!(
                "sequenceDiagram\n participant A@{{ \"type\": \"database\", \"alias\": {j} }}\n A->>B: x"
            ),
            format!(
                "sequenceDiagram\n participant A@{{ {j}: \"x\", \"alias\": \"y\" }}\n A->>B: x"
            ),
            format!("sequenceDiagram\n {p}->>B: x"),
            format!("sequenceDiagram\n A->>B: {p}"),
            format!(
                "sequenceDiagram\n A->>B: wrap: {p} and some more words to wrap around the corner of the label"
            ),
            format!("sequenceDiagram\n A->>A: {p}"),
            format!("sequenceDiagram\n A()->>()B: {p}"),
            format!("sequenceDiagram\n Note over A,B: {p}"),
            format!("sequenceDiagram\n Note left of A: {p}"),
            format!("sequenceDiagram\n loop {p}\n A->>B: x\n end"),
            format!("sequenceDiagram\n alt {p}\n A->>B: x\n else {p}\n B->>A: y\n end"),
            format!("sequenceDiagram\n critical {p}\n A->>B: x\n option {p}\n B->>A: y\n end"),
            format!("sequenceDiagram\n rect {p}\n A->>B: x\n end"),
            format!("sequenceDiagram\n box red {p}\n participant A\n end\n A->>B: x"),
            format!("sequenceDiagram\n box rgb({p}) T\n participant A\n end\n A->>B: x"),
            format!("sequenceDiagram\n title {p}\n accTitle: {p}\n accDescr: {p}\n A->>B: x"),
            format!(
                "sequenceDiagram\n participant A\n links A: {{{j}: {j}}}\n link A: {p} @ {p}\n A->>B: x"
            ),
            format!(
                "sequenceDiagram\n participant A\n properties A: {{\"class\": {j}, {j}: {j}}}\n A->>B: x"
            ),
            format!("sequenceDiagram\n autonumber\n A->>B: {p}\n A->>A: {p}"),
            format!(
                "sequenceDiagram\n A->>B: x\n create actor C as {p}\n A->>C: y\n destroy C\n C-xA: {p}"
            ),
            format!(
                "---\ntitle: {j}\nconfig:\n  sequence:\n    messageAlign: {j}\n---\nsequenceDiagram\n A->>B: x"
            ),
            format!(
                "%%{{init: {{\"sequence\": {{\"width\": {j}, \"actorMargin\": {j}, \"mirrorActors\": {j}}}}}}}%%\nsequenceDiagram\n A->>B: x"
            ),
        ] {
            let Some(svg) = try_render(&src) else {
                continue;
            };
            assert_inert(&svg, &src);
            rendered += 1;
        }
    }
    assert!(rendered > 400, "only {rendered} hostile inputs rendered");
    let mut fragments: Vec<&str> = PAYLOADS.to_vec();
    fragments.extend([
        "\"",
        ":",
        "\n",
        "@{ \"type\": \"",
        "\" }",
        " as ",
        "rect ",
        "box ",
        "links A: {\"a\": \"",
        "properties A: {\"class\": \"",
        "Note over A: ",
        "title ",
        "end\n",
        "loop ",
        "<br/>",
        "wrap: ",
    ]);
    let mut rnd = Random::new(3);
    let mut rendered = 0;
    for _ in 0..1500 {
        let src = mutate(&corpus, &fragments, &mut rnd);
        if let Some(svg) = try_render(&src) {
            assert_inert(&svg, &src);
            rendered += 1;
        }
    }
    // A parser that refused every mutation would make the loop above assert nothing at all.
    assert!(rendered > 100, "only {rendered} of 1500 mutations rendered");
}

#[test]
fn holds_for_extreme_numbers_in_the_config() {
    for value in [
        "-1", "0", "1e308", "1e-308", "\"12\"", "null", ".nan", ".inf",
    ] {
        let src = format!(
            "---\nconfig:\n  sequence:\n    width: {value}\n    height: {value}\n    actorMargin: {value}\n    messageMargin: {value}\n    noteMargin: {value}\n    boxMargin: {value}\n---\nsequenceDiagram\n box B\n participant A\n end\n A->>C: x\n Note over A,C: n\n loop l\n C->>C: y\n end\n"
        );
        let svg = svg(&src);
        assert_inert(&svg, &src);
    }
    let src = format!(
        "sequenceDiagram\n autonumber {} {}\n A->>B: x\n B->>A: y\n",
        "9".repeat(400),
        "9".repeat(400)
    );
    assert_inert(&svg(&src), "autonumber");
}

fn repeat(n: usize, line: impl Fn(usize) -> String) -> String {
    (0..n).fold(String::new(), |mut s, i| {
        s += &line(i);
        s
    })
}

#[test]
fn stays_fast_on_large_and_hostile_input() {
    // Debug builds are several times slower than release.
    let budget = Duration::from_secs(if cfg!(debug_assertions) { 20 } else { 2 });
    let options = RenderOptions {
        limits: Limits {
            source: usize::MAX,
            output: usize::MAX,
            ..Limits::default()
        },
        ..RenderOptions::default()
    };
    const N: usize = 50000;
    let head = "sequenceDiagram\n";
    let cases = [
        format!("{head}A->>B: {}", "word ".repeat(N / 5)),
        format!("{head}A->>B: wrap: {}", "word ".repeat(N / 5)),
        format!("{head}Note over A: wrap: {}", "x".repeat(N)),
        format!("{head}A->>B: {}", "#35;<br/>".repeat(N / 9)),
        format!("{head}participant {}\nA->>B: x", "n".repeat(N)),
        format!("{head}participant A{}x", " ".repeat(N)),
        format!("{head}participant a{}:", " #b".repeat(N / 3)),
        format!("{head}participant A@{{ {}", "\"a\": 1, ".repeat(N / 8)),
        format!(
            "{head}participant A@{{ \"a\": {} }}\nA->>A: x",
            "[".repeat(N)
        ),
        format!("{head}A{}>>B: x", "-".repeat(N)),
        format!("{head}A{}->>B: x", "/(|)\\".repeat(N / 5)),
        format!("{head}{}x", "1".repeat(N)),
        format!("{head}{}", "1.2.".repeat(N / 4)),
        format!("{head}accDescr {{{}", " x".repeat(N / 2)),
        format!("{head}{}", "a%%".repeat(N / 3)),
        format!(
            "{head}box rgb({}) T\nparticipant A\nend",
            "1, ".repeat(N / 3)
        ),
        format!(
            "{head}participant A\nlinks A: {}{}",
            "[".repeat(N / 2),
            "]".repeat(N / 2)
        ),
        format!("{head}{}", "A->>B: x\n".repeat(8000)),
        format!("{head}{}", repeat(4000, |i| format!("participant p{i}\n"))),
        format!(
            "{head}{}",
            repeat(2500, |i| format!("p{i}->>p{}: m\n", 2499 - i))
        ),
        format!(
            "{head}{}",
            repeat(1500, |i| format!(
                "p{i}->>p{}: a long message {i}\n",
                (i * 7) % 1500
            ))
        ),
        format!(
            "{head}{}A->>B: x\n{}",
            "loop\n".repeat(10000),
            "end\n".repeat(10000)
        ),
        format!("{head}alt a\n{}end\n", "else b\n".repeat(10000)),
        format!(
            "{head}par a\nA->>B: x\n{}end\n",
            "and b\nB->>A: y\n".repeat(5000)
        ),
        format!(
            "{head}{}{}",
            "A->>+B: x\n".repeat(5000),
            "B-->>-A: y\n".repeat(5000)
        ),
        format!("{head}{}", "activate A\n".repeat(4500)),
        format!(
            "{head}participant A\nparticipant B\nparticipant C\n{}",
            "Note over A,C: n\n".repeat(3000)
        ),
        format!("{head}{}", "A->>A: x\n".repeat(5000)),
        format!(
            "{head}{}",
            repeat(1000, |i| format!(
                "create participant c{i}\nA->>c{i}: x\ndestroy c{i}\nA-xc{i}: y\n"
            ))
        ),
        format!(
            "{head}{}",
            repeat(1500, |i| format!(
                "box Aqua Group {i}\nparticipant b{i}\nend\n"
            ))
        ),
        format!("{head}autonumber\n{}", "A()->>()B: x\n".repeat(3000)),
        format!("{head}{}", ">".repeat(N)),
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
        rendered >= 25,
        "only {rendered} of {} cases were drawn",
        cases.len()
    );
}

#[test]
fn draws_every_block_kind() {
    for (src, kind) in [
        (
            "sequenceDiagram\n par one\n A->>B: x\n and two\n B->>A: y\n end\n",
            "par",
        ),
        ("sequenceDiagram\n opt maybe\n A->>B: x\n end\n", "opt"),
        (
            "sequenceDiagram\n critical go\n A->>B: x\n option retry\n B->>A: y\n end\n",
            "critical",
        ),
        (
            "sequenceDiagram\n break when done\n A->>B: x\n end\n",
            "break",
        ),
    ] {
        let svg = svg(src);
        assert!(
            svg.contains(&format!("pele-rs-frame-{kind}")),
            "{kind}: {svg}"
        );
        assert_inert(&svg, src);
    }
    // The `and` of a `par` and the `option` of a `critical` carry their own labels.
    let par = svg("sequenceDiagram\n par one\n A->>B: x\n and two\n B->>A: y\n end\n");
    assert!(par.contains(">[two]<"), "{par}");
    let critical =
        svg("sequenceDiagram\n critical go\n A->>B: x\n option retry\n B->>A: y\n end\n");
    assert!(critical.contains(">[retry]<"), "{critical}");
}

#[test]
fn activates_and_deactivates_a_participant() {
    let svg = svg("sequenceDiagram\n participant A\n activate A\n A->>A: work\n deactivate A\n");
    assert_eq!(count(&svg, "class=\"pele-rs-activation\""), 1, "{svg}");
    assert_inert(&svg, "activate");
}

#[test]
fn numbers_messages_from_a_start_and_a_step() {
    let numbered = svg("sequenceDiagram\n autonumber 10 10\n A->>B: x\n B->>A: y\n");
    assert!(
        numbered.contains(">10<") && numbered.contains(">20<"),
        "{numbered}"
    );
    let dec = svg("sequenceDiagram\n autonumber 10.5 0.5\n A->>B: x\n B->>A: y\n");
    assert!(dec.contains(">10.5<") && dec.contains(">11<"), "{dec}");
}

#[test]
fn draws_right_angled_self_messages_when_asked() {
    let curved = svg("sequenceDiagram\n A->>A: self\n");
    let right =
        svg("%%{init: {\"sequence\": {\"rightAngles\": true}}}%%\nsequenceDiagram\n A->>A: self\n");
    assert!(curved.contains("A6,6"), "{curved}");
    assert!(!right.contains("A6,6"), "{right}");
}

#[test]
fn wraps_message_labels_when_asked() {
    let src = "sequenceDiagram\n participant A\n participant B\n A->>B: one two three four five six seven eight\n";
    assert!(!svg(src).contains("<tspan"), "the label is in one line");
    let wrapped = svg(&format!(
        "%%{{init: {{\"sequence\": {{\"wrap\": true}}}}}}%%\n{src}"
    ));
    assert!(wrapped.contains("<tspan"), "the label is wrapped");
}

#[test]
fn writes_the_accessible_title_and_description() {
    let svg = svg("sequenceDiagram\n accTitle: the title\n accDescr: the description\n A->>B: x\n");
    assert!(
        svg.contains("<title id=\"pele-rs-title\">the title</title>"),
        "{svg}"
    );
    assert!(
        svg.contains("<desc id=\"pele-rs-desc\">the description</desc>"),
        "{svg}"
    );
    assert!(svg.contains("aria-labelledby=\"pele-rs-title\""), "{svg}");
    assert!(svg.contains("aria-describedby=\"pele-rs-desc\""), "{svg}");
}

#[test]
fn places_notes_left_and_right_of_a_participant() {
    let left = svg("sequenceDiagram\n participant A\n participant B\n Note left of A: L\n");
    let right = svg("sequenceDiagram\n participant B\n participant A\n Note right of B: R\n");
    assert_eq!(count(&left, "class=\"pele-rs-note\""), 1, "{left}");
    assert!(left.contains(">L<") && right.contains(">R<"));
    assert_ne!(left, right);
}

#[test]
fn draws_a_created_participant_at_its_creation() {
    let svg = svg("sequenceDiagram\n participant A\n create participant B\n A->>B: hi\n");
    let actor = svg.find("pele-rs-actor").expect("a participant is drawn");
    let message = svg
        .find("pele-rs-edge pele-rs-message")
        .expect("a message is drawn");
    assert!(
        actor > message,
        "the created participant is drawn at its creation, not at the top"
    );
}

#[test]
fn snapshots() {
    let cases = [
        (
            "basic",
            "sequenceDiagram\n participant Alice\n actor Bob\n Alice->>+John: Hello John, how are you?\n John-->>-Alice: Great!\n Alice-)Bob: See you later!\n Note right of John: Thinks\n",
        ),
        (
            "blocks",
            "sequenceDiagram\n autonumber\n loop Every minute\n A->>B: ping\n end\n alt is sick\n B->>A: Not so good\n else is well\n B->>A: Feeling fresh\n end\n opt Extra\n A->>A: think\n end\n",
        ),
        (
            "boxes_and_lifecycles",
            "---\ntitle: Lifecycle\n---\nsequenceDiagram\n box Aqua Front\n participant A\n end\n box rgb(200,200,255) Back\n participant B\n end\n create participant C\n A->>C: spawn\n C()->>()B: relay\n destroy C\n B-xC: stop\n rect rgb(240,240,240)\n A<<->>B: sync\n end\n",
        ),
    ];
    for (name, src) in cases {
        insta::assert_snapshot!(name, svg(src));
    }
}
