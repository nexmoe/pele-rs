//! JSONL adapter for the official-test differential runner (development only).
#[allow(dead_code)]
#[path = "../tests/conformance/support.rs"]
mod support;
use std::io::{self, BufRead, Write};
fn main() {
    let mut output = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let entry = serde_json::from_str(&line).unwrap();
        let result = std::panic::catch_unwind(|| support::render(&entry))
            .unwrap_or_else(|_| serde_json::json!({"error":{"code":"panic"}}));
        writeln!(output, "{result}").unwrap();
    }
}
