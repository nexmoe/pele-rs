//! Actual calls made by the pinned official test suites, replayed entirely in Rust.
#[path = "conformance/support.rs"]
mod support;
use serde_json::Value;
use std::io::BufRead;

fn replay(kind: &str) {
    // Official stress suites deliberately exceed the native API's default nesting limit.
    // Use a larger test stack for those fixtures without changing consumer thread settings.
    let kind = kind.to_string();
    std::thread::Builder::new()
        .name(kind.clone())
        .stack_size(32 * 1024 * 1024)
        .spawn(move || replay_on_large_stack(&kind))
        .unwrap()
        .join()
        .unwrap();
}

fn replay_on_large_stack(kind: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/conformance")
        .join(format!("{kind}.jsonl.gz"));
    let reader = std::io::BufReader::new(flate2::read::GzDecoder::new(
        std::fs::File::open(path).unwrap(),
    ));
    let mut cases = 0;
    let mut failures = Vec::new();
    for line in reader.lines() {
        let entry: Value = serde_json::from_str(&line.unwrap()).unwrap();
        cases += 1;
        let actual = support::render(&entry);
        if let Some(diff) = support::difference(&entry, &actual) {
            failures.push(format!("{} {}: {diff}", entry["id"], entry["origins"][0]));
        }
    }
    static INVENTORY: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    let inventory = INVENTORY
        .get_or_init(|| serde_json::from_str(include_str!("conformance/manifest.json")).unwrap());
    assert_eq!(
        Some(cases),
        inventory["counts"][kind]["cases"].as_u64(),
        "official corpus count: {kind}"
    );
    assert!(
        failures.is_empty(),
        "{kind}: {}/{} differences\n{}",
        failures.len(),
        cases,
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

macro_rules! cases {
    ($($feature:literal => $test:ident: $kind:literal),* $(,)?) => { $(
        #[cfg(feature = $feature)]
        #[test]
        fn $test() { replay($kind); }
    )* };
}
cases! {
    "agentflow" => agentflow: "agentflow", "architecture" => architecture: "architecture",
    "block" => block: "block", "c4" => c4: "c4", "class" => class: "class",
    "cynefin" => cynefin: "cynefin", "er" => er: "er", "eventmodeling" => eventmodeling: "eventmodeling",
    "flowchart" => flowchart: "flowchart", "gantt" => gantt: "gantt", "git" => git: "gitGraph",
    "info" => info: "info", "ishikawa" => ishikawa: "ishikawa", "journey" => journey: "journey",
    "kanban" => kanban: "kanban", "mindmap" => mindmap: "mindmap", "packet" => packet: "packet",
    "pie" => pie: "pie", "quadrantchart" => quadrant: "quadrantChart", "radar" => radar: "radar",
    "railroad" => railroad: "railroad", "requirement" => requirement: "requirement", "sankey" => sankey: "sankey",
    "sequence" => sequence: "sequence", "state" => state: "state", "swimlane" => swimlane: "swimlane",
    "timeline" => timeline: "timeline", "treeview" => treeview: "treeView", "treemap" => treemap: "treemap",
    "usecase" => usecase: "usecase", "venn" => venn: "venn", "wardley" => wardley: "wardley", "xychart" => xychart: "xychart",
}
#[test]
fn unknown() {
    replay("unknown");
}

#[test]
fn comparison_checks_structure_styles_and_finite_geometry() {
    let result = |svg: &str| {
        serde_json::json!({"result": {
            "type": "pie", "links": [], "width": 100.0, "height": 100.0, "svg": svg,
        }})
    };
    let expected = result("<svg><path d=\"M1,2L3,4\" fill=\"red\"/></svg>");
    assert!(
        support::difference(
            &expected,
            &result("<svg><path fill=\"red\" d=\"M1.01,2L3,4\"/></svg>")
        )
        .is_none()
    );
    for svg in [
        "<svg><path d=\"M1,2L3,4\" fill=\"blue\"/></svg>",
        "<svg><path d=\"M1,2L3.1,4\" fill=\"red\"/></svg>",
        "<svg><path d=\"M1,2L3,4\" fill=\"red\"/><text>extra</text></svg>",
    ] {
        assert!(support::difference(&expected, &result(svg)).is_some());
    }
    let mut invalid = expected.clone();
    invalid["result"]["width"] = Value::Null;
    assert!(support::difference(&expected, &invalid).is_some());
    invalid = expected.clone();
    invalid["result"]["svg"] = Value::Null;
    assert!(support::difference(&expected, &invalid).is_some());
}
