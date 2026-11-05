//! The metadata of an `@{ … }` block: the YAML it holds, read the way Mermaid reads it with
//! js-yaml and the JSON schema.

use crate::preprocess::{Config, Value, parse_yaml};

/// The keys that would reach an object's prototype in JavaScript. Mermaid drops them.
const UNSAFE_KEYS: &[&str] = &["__proto__", "constructor", "prototype"];

/// Parses the body of an `@{ … }` block. A block that holds no YAML is an empty map.
pub(crate) fn parse(data: &str) -> Result<Config, &'static str> {
    // A block that is written over several lines is block YAML; a block that is written on one
    // line is a flow mapping. Mermaid hands the body to js-yaml as it was written.
    let yaml = if data.contains('\n') {
        format!("{data}\n")
    } else {
        format!("{{\n{data}\n}}")
    };
    match parse_yaml(&yaml) {
        Ok(Value::Map(map)) => Ok(strip_prototype_keys(map)),
        // A body that parses into something else than a mapping holds no metadata.
        Ok(_) => Ok(Config::new()),
        Err(message) => {
            // Block YAML has no trailing commas, but a block that is written in the style of
            // the single-line form has them. Mermaid retries once with the line-trailing
            // commas stripped.
            if data.contains('\n') {
                let stripped = strip_line_trailing_commas(data);
                if stripped != data {
                    match parse_yaml(&format!("{stripped}\n")) {
                        Ok(Value::Map(map)) => return Ok(strip_prototype_keys(map)),
                        Ok(_) => return Ok(Config::new()),
                        // The commas were not the problem: the original error stands.
                        Err(_) => {}
                    }
                }
            }
            Err(message)
        }
    }
}

/// Removes the commas at the end of the lines of a block, the way Mermaid does.
fn strip_line_trailing_commas(body: &str) -> String {
    let mut in_single = false;
    let mut in_double = false;
    let mut depth = 0usize;
    let mut block_scalar_indent: Option<usize> = None;
    let mut out: Vec<String> = Vec::new();
    for line in body.split('\n') {
        if let Some(indent) = block_scalar_indent {
            let own = line.len() - line.trim_start().len();
            if line.trim().is_empty() || own > indent {
                // The content of a block scalar is text; its commas are content.
                out.push(line.to_string());
                continue;
            }
            block_scalar_indent = None;
        }
        let mut comment_start: Option<usize> = None;
        let mut escaped = false;
        for (i, ch) in line.char_indices() {
            if in_double {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_double = false;
                }
            } else if in_single {
                if ch == '\'' {
                    in_single = false;
                }
            } else if ch == '"' {
                in_double = true;
            } else if ch == '\'' {
                in_single = true;
            } else if ch == '#' && (i == 0 || line[..i].ends_with(' ') || line[..i].ends_with('\t'))
            {
                comment_start = Some(i);
                break;
            } else if ch == '[' || ch == '{' {
                depth += 1;
            } else if ch == ']' || ch == '}' {
                depth = depth.saturating_sub(1);
            }
        }
        if in_single || in_double || depth > 0 {
            // The line ends inside a quoted scalar or a flow collection: a comma there is
            // content or a separator, not a stray.
            out.push(line.to_string());
            continue;
        }
        let (code, comment) = match comment_start {
            Some(at) => (&line[..at], &line[at..]),
            None => (line, ""),
        };
        let trimmed = code.trim_end();
        if opens_block_scalar(trimmed) {
            // A block scalar opens here; its deeper lines are content.
            block_scalar_indent = Some(line.len() - line.trim_start().len());
            out.push(line.to_string());
            continue;
        }
        let stripped = strip_one_trailing_comma(code);
        out.push(format!("{stripped}{comment}"));
    }
    out.join("\n")
}

/// Tells whether a piece of code opens a block scalar: a colon, spaces, then `|` or `>` with an
/// optional chomping or indentation indicator, at the end of the line.
fn opens_block_scalar(code: &str) -> bool {
    let Some(colon) = code.rfind(':') else {
        return false;
    };
    let rest = code[colon + 1..].trim_start();
    let mut chars = rest.chars();
    if !matches!(chars.next(), Some('|' | '>')) {
        return false;
    }
    let tail = chars.as_str();
    let indicator: String = tail
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '+' || *c == '-')
        .collect();
    tail[indicator.len()..].trim().is_empty()
}

/// Removes the comma at the end of a piece of text, and the spaces that follow it.
fn strip_one_trailing_comma(code: &str) -> String {
    let trimmed = code.trim_end();
    if !trimmed.ends_with(',') {
        return code.to_string();
    }
    let rest = &trimmed[..trimmed.len() - 1];
    let spaces = &code[trimmed.len()..];
    format!("{rest}{spaces}")
}

/// Removes the keys that would reach an object's prototype, at every level.
fn strip_prototype_keys(map: Config) -> Config {
    let mut out = Config::new();
    for (key, value) in map {
        if UNSAFE_KEYS.contains(&key.as_str()) {
            continue;
        }
        out.insert(key, strip_value(value));
    }
    out
}

/// Removes the unsafe keys of a value.
fn strip_value(value: Value) -> Value {
    match value {
        Value::Map(map) => Value::Map(strip_prototype_keys(map)),
        Value::Seq(items) => Value::Seq(items.into_iter().map(strip_value).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_single_line_block() {
        let map = parse(" shape: task ").unwrap();
        assert_eq!(
            map.get("shape").map(|v| v.js_string()),
            Some("task".to_string())
        );
    }

    #[test]
    fn drops_the_prototype_keys() {
        let map = parse("    __proto__: {x: 1}\n    shape: task").unwrap();
        assert!(!map.contains_key("__proto__"));
    }

    #[test]
    fn reads_a_block_with_a_comment() {
        let map =
            parse("    protocol: \"mcp\", # main protocol\n    transport: \"stdio\"").unwrap();
        assert_eq!(
            map.get("protocol").map(|v| v.js_string()),
            Some("mcp".to_string())
        );
    }

    #[test]
    fn reads_a_flow_collection_over_lines() {
        let map = parse("    tools: [\"read\",\n      \"write\"],\n    protocol: \"mcp\"").unwrap();
        assert_eq!(
            map.get("protocol").map(|v| v.js_string()),
            Some("mcp".to_string())
        );
    }

    #[test]
    fn reads_a_body_that_is_not_a_mapping() {
        // A body that parses into a scalar holds no metadata, but it is not an error.
        assert!(parse(" javascript:alert(1)escription: \"apha node\"").is_ok());
    }
}
