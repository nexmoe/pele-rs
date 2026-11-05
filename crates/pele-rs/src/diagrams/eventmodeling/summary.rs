//! The model of an event modeling diagram as a string, for the golden tests and the
//! differential run. The format is the one `tools/oracle/summaries/eventmodeling.mjs` writes.

use std::fmt::Write;
use std::iter::Peekable;
use std::str::Chars;

use super::db::EventModel;

/// `JSON.stringify` of a string.
pub(crate) fn json(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A quoted string, or `-` for none.
pub(crate) fn q(text: Option<&str>) -> String {
    text.map_or_else(|| "-".to_string(), json)
}

/// A list of texts, as `list` in `tools/oracle/lib.mjs` writes it.
pub(crate) fn list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        json(&items.join("\u{1}"))
    }
}

/// The model in the form `eventModelingSummary` in `tools/oracle/summaries/eventmodeling.mjs`
/// writes it.
pub(crate) fn summary(model: &EventModel) -> String {
    let mut out: Vec<String> = Vec::new();
    for item in &model.boxes {
        let lane = model.lanes.iter().find(|lane| lane.index == item.lane);
        out.push(format!(
            "box:{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            json(&item.id),
            item.kind.name(),
            json(&item.entity),
            json(&item.name),
            q(item.namespace.as_deref()),
            item.reset,
            lane.map_or(0, |lane| lane.index),
            q(lane.map(|lane| lane.label.as_str())),
            q(item.data.as_deref()),
            q(item.data_type.as_deref()),
        ));
    }
    for lane in &model.lanes {
        out.push(format!(
            "lane:{}|{}|{}|{}",
            lane.index,
            json(&lane.label),
            lane.group.name(),
            q(lane.namespace.as_deref()),
        ));
    }
    for relation in &model.relations {
        out.push(format!(
            "rel:{}->{}",
            json(&relation.source),
            json(&relation.target)
        ));
    }
    for note in &model.notes {
        out.push(format!(
            "note:{}|{}|{}",
            json(&note.frame),
            json(&note.text),
            q(note.data_type.as_deref()),
        ));
    }
    for specification in &model.specifications {
        out.push(format!(
            "spec:{}|given={}|when={}|then={}",
            json(&specification.frame),
            list(&specification.given),
            list(&specification.when),
            list(&specification.then),
        ));
    }
    for entity in &model.entities {
        out.push(format!("entity:{}", json(entity)));
    }
    out.push(format!("title:{}", q(model.title.as_deref())));
    out.join("\n")
}

/// The name of an error code, as pele's errors carry it.
pub(crate) fn error_code(code: crate::error::ErrorCode) -> &'static str {
    use crate::error::ErrorCode;
    match code {
        ErrorCode::Syntax => "syntax",
        ErrorCode::Semantic => "semantic",
        ErrorCode::Limit => "limit",
        ErrorCode::Unsupported => "unsupported-diagram",
    }
}

/// Feeds the inputs `tools/oracle/diff.mjs` wrote to `$PELE_RS_ORACLE/<name>.json` (an array
/// of strings) through `outcome`, and writes the results to `<name>.out.json` next to it. Does
/// nothing when the variable is not set or the run has no inputs of this kind.
pub(crate) fn dump(name: &str, outcome: impl Fn(&str) -> String) {
    let Some(dir) = std::env::var_os("PELE_RS_ORACLE") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.json"))) else {
        return;
    };
    let results: Vec<String> = json_strings(&text)
        .iter()
        .map(|input| json(&outcome(input)))
        .collect();
    std::fs::write(
        dir.join(format!("{name}.out.json")),
        format!("[{}]", results.join(",\n")),
    )
    .expect("results");
}

/// The strings of a JSON array of strings.
fn json_strings(text: &str) -> Vec<String> {
    let mut chars = text.chars().peekable();
    let mut out = Vec::new();
    if chars.next() != Some('[') {
        return out;
    }
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        match chars.peek() {
            Some(']') => break,
            Some('"') => {
                chars.next();
                out.push(json_string(&mut chars));
            }
            Some(',') => {
                chars.next();
            }
            _ => break,
        }
    }
    out
}

/// A JSON string, from the quote that opens it, with its escapes resolved.
fn json_string(chars: &mut Peekable<Chars<'_>>) -> String {
    let mut out = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => break,
            '\\' => {
                let Some(escape) = chars.next() else { break };
                match escape {
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    'b' => out.push('\u{8}'),
                    'f' => out.push('\u{c}'),
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'u' => out.push(char::from_u32(hex4(chars)).unwrap_or('\u{fffd}')),
                    other => out.push(other),
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Four hexadecimal digits of a `\u` escape, with a surrogate pair read as one character.
fn hex4(chars: &mut Peekable<Chars<'_>>) -> u32 {
    let mut code = 0_u32;
    for _ in 0..4 {
        let Some(digit) = chars.next() else { break };
        code = code * 16 + digit.to_digit(16).unwrap_or(0);
    }
    if (0xd800..0xdc00).contains(&code) && chars.peek() == Some(&'\\') {
        chars.next();
        if chars.next() == Some('u') {
            let low = hex4(chars);
            if (0xdc00..0xe000).contains(&low) {
                return 0x10000 + ((code - 0xd800) << 10) + (low - 0xdc00);
            }
        }
    }
    code
}
