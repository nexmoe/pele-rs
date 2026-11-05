//! Independent replay of https://pele.run/examples, including its mixed Latin/emoji label.
#![cfg(feature = "all")]
#[path = "conformance/support.rs"]
mod support;
use serde_json::Value;
use std::io::BufRead;

#[test]
fn official_website_examples_match_text_styles_and_geometry() {
    let bytes = include_bytes!("website/renders.jsonl.gz");
    let records = std::io::BufReader::new(flate2::read::GzDecoder::new(&bytes[..]));
    let mut count = 0;
    let mut emoji_cases = 0;
    let mut failures = Vec::new();
    for line in records.lines() {
        let expected: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let actual = support::render(&expected);
        if expected["source"].as_str().unwrap().contains('🌋') {
            emoji_cases += 1;
            assert!(
                actual["result"]["svg"]
                    .as_str()
                    .unwrap()
                    .contains("Unicode works too: 🌋")
            );
        }
        if let Some(diff) = support::difference(&expected, &actual) {
            failures.push(format!(
                "{} #{} {:?}: {diff}",
                expected["title"], expected["index"], expected["options"]["maxWidth"]
            ));
        }
        count += 1;
    }
    let manifest: Value = serde_json::from_str(include_str!("website/manifest.json")).unwrap();
    assert_eq!(Some(count), manifest["renders"].as_u64());
    assert_eq!(emoji_cases, 4);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
