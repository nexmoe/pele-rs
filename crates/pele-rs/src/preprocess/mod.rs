//! Prepares diagram text the way Mermaid does before a parser sees it: line endings, quoted
//! HTML attributes, front matter, directives and comments. Ported from pele's `preprocess.ts`.

mod json;
mod yaml;

#[cfg_attr(not(feature = "flowchart"), allow(unused_imports))]
pub(crate) use yaml::parse_yaml;

#[cfg_attr(not(feature = "sequence"), allow(unused_imports))]
pub(crate) use json::parse_json;

use std::collections::BTreeMap;

use crate::error::{Error, ErrorCode};
use crate::util::{chars, is_line_end, is_space, is_word, js_number, trim, trim_start};

/// A value read from front matter or an `init` directive.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Seq(Vec<Value>),
    Map(Config),
}

/// Mermaid configuration, keyed as in Mermaid's config schema.
pub(crate) type Config = BTreeMap<String, Value>;

impl Value {
    /// Whether JavaScript would treat the value as true.
    pub(crate) fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0 && !n.is_nan(),
            Value::String(s) => !s.is_empty(),
            Value::Seq(_) | Value::Map(_) => true,
        }
    }

    /// The value as JavaScript's `String(value)` writes it.
    pub(crate) fn js_string(&self) -> String {
        match self {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => js_number(*n),
            Value::String(s) => s.clone(),
            Value::Seq(items) => items
                .iter()
                .map(|v| match v {
                    Value::Null => String::new(),
                    v => v.js_string(),
                })
                .collect::<Vec<_>>()
                .join(","),
            Value::Map(_) => "[object Object]".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Preprocessed {
    /// The diagram text without front matter, directives and comment lines.
    pub text: String,
    pub title: Option<String>,
    pub config: Config,
    /// The text before its comment lines are removed.
    pub with_comments: String,
    /// The number of front matter lines removed before `with_comments`.
    pub line_offset: usize,
}

/// Keys that would reach an object's prototype in JavaScript. Mermaid ignores them.
const UNSAFE_KEYS: &[&str] = &["__proto__", "constructor", "prototype"];

fn merge(target: &mut Config, source: &Config) {
    for (key, value) in source {
        if UNSAFE_KEYS.contains(&key.as_str()) {
            continue;
        }
        match (target.get_mut(key), value) {
            (Some(Value::Map(current)), Value::Map(value)) => merge(current, value),
            (_, Value::Map(value)) => {
                let mut copy = Config::new();
                merge(&mut copy, value);
                target.insert(key.clone(), Value::Map(copy));
            }
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

/// The configuration of the host, given as JSON, with the configuration of the diagram merged
/// over it, so the diagram's own `%%{init}%%` and front matter win. A diagram's configuration
/// cannot raise a limit, because the limits are not part of it.
pub(crate) fn with_host_config(host: &str, config: &Config) -> Result<Config, Error> {
    let Some(Value::Map(base)) = json::parse_json(trim(host)) else {
        return Err(Error::new(
            ErrorCode::Syntax,
            "The config option is not a JSON object.",
        ));
    };
    let mut merged = Config::new();
    merge(&mut merged, &base);
    for (key, value) in config {
        if !UNSAFE_KEYS.contains(&key.as_str()) {
            merged.insert(key.clone(), value.clone());
        }
    }
    Ok(merged)
}

/// Removes `%%` comment lines, along with the blank lines before each.
pub(crate) fn cleanup_comments(text: &str) -> String {
    if !text.contains("%%") {
        return trim_start(text).to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    let mut pos = 0;
    while pos < text.len() {
        let at_line_start = pos == 0 || text[..pos].ends_with(is_line_end);
        if at_line_start {
            match comment_at(text, pos) {
                Ok(end) => {
                    out.push_str(&text[last..pos]);
                    last = end;
                    pos = end;
                    continue;
                }
                // Every line start in the whitespace before `next` fails the same way.
                Err(next) if next > pos => {
                    pos = next;
                    continue;
                }
                Err(_) => {}
            }
        }
        pos += text[pos..].chars().next().map_or(1, char::len_utf8);
    }
    out.push_str(&text[last..]);
    trim_start(&out).to_string()
}

/// `\s*%%(?!{)[^\n]+\n?` at `pos`: the end of the comment line that starts there, or, when
/// there is none, where the whitespace at `pos` ends.
fn comment_at(text: &str, pos: usize) -> Result<usize, usize> {
    let rest = trim_start(&text[pos..]);
    let fail = Err(text.len() - rest.len());
    let Some(body) = rest.strip_prefix("%%") else {
        return fail;
    };
    let len = body.find('\n').unwrap_or(body.len());
    if body.starts_with('{') || len == 0 {
        return fail;
    }
    let end = text.len() - body.len() + len;
    Ok(if text[end..].starts_with('\n') {
        end + 1
    } else {
        end
    })
}

fn is_line_break(c: Option<&char>) -> bool {
    matches!(c, Some('\n' | '\r'))
}

fn is_space_at(text: &[char], i: usize) -> bool {
    text.get(i).is_some_and(|&c| is_space(c))
}

struct FrontMatter {
    indent: Vec<char>,
    body: String,
    /// Length of the front matter in characters, up to and including its last line break.
    length: usize,
}

/// Finds a `---` fenced YAML block at the start of the text the way Mermaid's front matter
/// regex does.
fn match_front_matter(text: &[char]) -> Option<FrontMatter> {
    let mut cursor = 0;
    while is_space_at(text, cursor) && !is_line_break(text.get(cursor)) {
        cursor += 1;
    }
    let indent = text[..cursor].to_vec();
    if !chars::starts_with(text, cursor, &['-', '-', '-']) {
        return None;
    }

    let mut opening_ends = Vec::new();
    let mut i = cursor + 3;
    while is_space_at(text, i) {
        if is_line_break(text.get(i)) {
            opening_ends.push(i);
        }
        i += 1;
    }
    let first_opening = *opening_ends.first()?;

    let mut fence = indent.clone();
    fence.extend_from_slice(&['-', '-', '-']);
    let closing_end = |i: usize| -> Option<usize> {
        if !is_line_break(text.get(i)) || !chars::starts_with(text, i + 1, &fence) {
            return None;
        }
        let mut last = None;
        let mut j = i + 1 + fence.len();
        while is_space_at(text, j) {
            if is_line_break(text.get(j)) {
                last = Some(j);
            }
            j += 1;
        }
        last
    };

    let mut needle = vec!['\n'];
    needle.extend_from_slice(&fence);
    let mut last_fence = None;
    let mut found = chars::rfind(text, &needle, text.len());
    while let Some(i) = found.filter(|&i| i > first_opening) {
        if closing_end(i).is_some() {
            last_fence = Some(i);
            break;
        }
        found = chars::rfind(text, &needle, i - 1);
    }
    let last_fence = last_fence?;

    let opening_end = opening_ends
        .iter()
        .copied()
        .rev()
        .find(|&c| c < last_fence)
        .unwrap_or(first_opening);

    let body_start = opening_end + 1;
    let mut found = chars::find(text, &needle, body_start - 1);
    while let Some(i) = found.filter(|&i| i <= last_fence) {
        if i >= body_start {
            if let Some(end) = closing_end(i) {
                return Some(FrontMatter {
                    indent,
                    body: chars::string(&text[body_start..i]),
                    length: end + 1,
                });
            }
        }
        found = chars::find(text, &needle, i + 1);
    }
    None
}

/// A `%%{...}%%` directive: its span in characters, its type and its argument text.
struct Directive {
    start: usize,
    end: usize,
    kind: String,
    args: Option<String>,
}

/// Finds directives the way Mermaid's regex
/// `%{2}{\s*(?:(\w+)\s*:|(\w+))\s*(?:(\w+)|((?:(?!}%{2}).|\r?\n)*))?\s*(?:}%{2})?` does.
fn find_directives(text: &[char]) -> Vec<Directive> {
    let space = |mut i: usize| {
        while is_space_at(text, i) {
            i += 1;
        }
        i
    };
    let word = |mut i: usize| {
        while text.get(i).is_some_and(|&c| is_word(c)) {
            i += 1;
        }
        i
    };
    let close = ['}', '%', '%'];
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(start) = chars::find(text, &['%', '%', '{'], from) {
        let name_start = space(start + 3);
        let name_end = word(name_start);
        if name_end == name_start {
            from = start + 1;
            continue;
        }
        let kind = chars::string(&text[name_start..name_end]);
        let after_name = space(name_end);
        let mut i = if text.get(after_name) == Some(&':') {
            space(after_name + 1)
        } else {
            after_name
        };
        let mut args = None;
        if text.get(i).is_some_and(|&c| is_word(c)) {
            i = word(i);
        } else {
            let body_start = i;
            while i < text.len() && !chars::starts_with(text, i, &close) {
                match text[i] {
                    '\n' => i += 1,
                    '\r' if text.get(i + 1) == Some(&'\n') => i += 2,
                    c if is_line_end(c) => break,
                    _ => i += 1,
                }
            }
            args = Some(chars::string(&text[body_start..i]));
        }
        i = space(i);
        if chars::starts_with(text, i, &close) {
            i += 3;
        }
        out.push(Directive {
            start,
            end: i,
            kind,
            args,
        });
        from = i;
    }
    out
}

/// Applies `wrap` and `init` directives to the config and removes all directives.
fn directives(text: &str, config: &mut Config) -> String {
    if !text.contains("%%{") {
        return text.to_string();
    }
    let normalized: Vec<char> = trim(text).replace('\'', "\"").chars().collect();
    for directive in find_directives(&normalized) {
        match directive.kind.as_str() {
            "wrap" => {
                config.insert("wrap".to_string(), Value::Bool(true));
            }
            "init" | "initialize" => {
                // Mermaid ignores an init directive whose body is not valid JSON.
                let args = directive.args.as_deref().map(trim).unwrap_or_default();
                if let Some(Value::Map(args)) = json::parse_json(args) {
                    merge(config, &args);
                }
            }
            _ => {}
        }
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for directive in find_directives(&chars) {
        out.extend(&chars[last..directive.start]);
        last = directive.end;
    }
    out.extend(&chars[last..]);
    out
}

/// Rewrites `attr="value"` inside HTML-like tags as `attr='value'`, as Mermaid does.
fn single_quote_attributes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        let tag = &rest[at..];
        let is_tag = tag[1..].starts_with(is_word);
        match tag.find('>').filter(|_| is_tag) {
            Some(end) => {
                let inner = &tag[..end];
                let mut inner_rest = inner;
                while let Some(eq) = inner_rest.find("=\"") {
                    let value = &inner_rest[eq + 2..];
                    let Some(close) = value.find('"') else {
                        break;
                    };
                    out.push_str(&inner_rest[..eq]);
                    out.push_str("='");
                    out.push_str(&value[..close]);
                    out.push('\'');
                    inner_rest = &value[close + 1..];
                }
                out.push_str(inner_rest);
                out.push('>');
                rest = &tag[end + 1..];
            }
            None => {
                out.push('<');
                rest = &tag[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

pub(crate) fn preprocess(source: &str) -> Result<Preprocessed, Error> {
    let mut text = if source.contains('\r') {
        source.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        source.to_string()
    };
    // A tag needs a closing bracket, so nothing after the last one can match.
    if text.contains('<') {
        if let Some(last_close) = text.rfind('>').filter(|&i| i > 0) {
            let head = single_quote_attributes(&text[..=last_close]);
            text = head + &text[last_close + 1..];
        }
    }

    let mut config = Config::new();
    let mut title = None;
    let mut line_offset = 0;

    let chars: Vec<char> = text.chars().collect();
    if let Some(front) = text
        .contains("---")
        .then(|| match_front_matter(&chars))
        .flatten()
    {
        let body = if front.indent.is_empty() {
            front.body
        } else {
            let indent = chars::string(&front.indent);
            front
                .body
                .split('\n')
                .map(|line| line.strip_prefix(indent.as_str()).unwrap_or(line))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let parsed = yaml::parse_yaml(&body).map_err(|message| {
            Error::new(
                ErrorCode::Syntax,
                format!("Front matter is not valid YAML. {message}"),
            )
        })?;
        if let Value::Map(parsed) = parsed {
            if let Some(t) = parsed.get("title").filter(|t| t.truthy()) {
                title = Some(t.js_string());
            }
            if let Some(Value::Map(c)) = parsed.get("config") {
                merge(&mut config, c);
            }
            if let Some(mode) = parsed.get("displayMode").filter(|m| m.truthy()) {
                let gantt = config
                    .entry("gantt".to_string())
                    .or_insert_with(|| Value::Map(Config::new()));
                if !matches!(gantt, Value::Map(_)) {
                    *gantt = Value::Map(Config::new());
                }
                if let Value::Map(gantt) = gantt {
                    gantt.insert("displayMode".to_string(), Value::String(mode.js_string()));
                }
            }
        }
        line_offset = chars[..front.length].iter().filter(|&&c| c == '\n').count();
        text = chars::string(&chars[front.length..]);
    }

    let with_comments = directives(&text, &mut config);
    Ok(Preprocessed {
        text: cleanup_comments(&with_comments),
        title,
        config,
        with_comments,
        line_offset,
    })
}

/// Repeat only the text edits when a parser reports an error, retaining original byte offsets.
pub(crate) fn mapped_source(source: &str, keep_comments: bool) -> crate::source_map::Mapped {
    let mut mapped = crate::source_map::Mapped::new(source);
    // Attribute quote normalization substitutes ASCII characters without changing byte offsets.
    if let Some(last) = mapped.text.rfind('>') {
        mapped.text = single_quote_attributes(&mapped.text[..=last]) + &mapped.text[last + 1..];
    }
    let chars: Vec<char> = mapped.text.chars().collect();
    if let Some(front) = match_front_matter(&chars) {
        let end = mapped
            .text
            .char_indices()
            .nth(front.length)
            .map_or(mapped.text.len(), |(at, _)| at);
        mapped.remove(&[(0, end)]);
    }
    let chars: Vec<char> = mapped.text.chars().collect();
    let positions: Vec<usize> = mapped
        .text
        .char_indices()
        .map(|(at, _)| at)
        .chain(std::iter::once(mapped.text.len()))
        .collect();
    let ranges = find_directives(&chars)
        .into_iter()
        .map(|d| (positions[d.start], positions[d.end]))
        .collect::<Vec<_>>();
    mapped.remove(&ranges);
    if !keep_comments {
        let mut ranges = Vec::new();
        let mut pos = 0;
        while pos < mapped.text.len() {
            if pos == 0 || mapped.text[..pos].ends_with(is_line_end) {
                match comment_at(&mapped.text, pos) {
                    Ok(end) => {
                        ranges.push((pos, end));
                        pos = end;
                        continue;
                    }
                    Err(next) if next > pos => {
                        pos = next;
                        continue;
                    }
                    _ => {}
                }
            }
            pos += mapped.text[pos..].chars().next().map_or(1, char::len_utf8);
        }
        mapped.remove(&ranges);
        mapped.trim_start();
    }
    mapped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pre(src: &str) -> Preprocessed {
        preprocess(src).unwrap()
    }

    #[test]
    fn removes_comments_and_leading_space() {
        let p = pre("\n%% a comment\n\n  %% indented\npie\n%% end\n  \"A\": 1\n");
        assert_eq!(p.text, "pie\n  \"A\": 1\n");
        assert_eq!(
            p.with_comments,
            "\n%% a comment\n\n  %% indented\npie\n%% end\n  \"A\": 1\n"
        );
    }

    #[test]
    fn reads_front_matter() {
        let p = pre("---\ntitle: My chart\nconfig:\n  pie:\n    textPosition: 0.6\n---\npie\n");
        assert_eq!(p.title.as_deref(), Some("My chart"));
        assert_eq!(p.text, "pie\n");
        assert_eq!(p.line_offset, 6);
        let Some(Value::Map(pie)) = p.config.get("pie") else {
            panic!("no pie config");
        };
        assert_eq!(pie.get("textPosition"), Some(&Value::Number(0.6)));
    }

    #[test]
    fn reads_indented_front_matter() {
        let p = pre("  ---\n  title: 42\n  ---\npie");
        assert_eq!(p.title.as_deref(), Some("42"));
        assert_eq!(p.text, "pie");
    }

    #[test]
    fn rejects_bad_front_matter() {
        let err = preprocess("---\na: [1\n---\npie").unwrap_err();
        assert_eq!(err.code, ErrorCode::Syntax);
        assert!(err.message.starts_with("Front matter is not valid YAML."));
    }

    #[test]
    fn leaves_unclosed_front_matter() {
        assert_eq!(pre("---\ntitle: x\npie").text, "---\ntitle: x\npie");
    }

    #[test]
    fn applies_directives() {
        let p = pre("%%{init: {'theme': 'dark', 'pie': {'textPosition': 0.5}}}%%\n%%{wrap}%%\npie");
        assert_eq!(p.text, "pie");
        assert_eq!(p.config.get("theme"), Some(&Value::String("dark".into())));
        assert_eq!(p.config.get("wrap"), Some(&Value::Bool(true)));
    }

    #[test]
    fn ignores_invalid_init_and_unsafe_keys() {
        let p = pre("%%{init: {theme: dark}}%%\npie");
        assert!(p.config.is_empty());
        let p = pre("%%{init: {\"__proto__\": {\"x\": 1}, \"a\": 1}}%%\npie");
        assert_eq!(p.config.keys().collect::<Vec<_>>(), ["a"]);
    }

    #[test]
    fn front_matter_config_comes_before_directives() {
        let p = pre(
            "---\nconfig:\n  theme: base\n  pie: {a: 1}\n---\n%%{init: {\"theme\": \"dark\", \"pie\": {\"b\": 2}}}%%\npie",
        );
        assert_eq!(p.config.get("theme"), Some(&Value::String("dark".into())));
        let Some(Value::Map(pie)) = p.config.get("pie") else {
            panic!("no pie config");
        };
        assert_eq!(pie.len(), 2);
    }

    #[test]
    fn single_quotes_attributes() {
        assert_eq!(
            pre("graph\nA[<span class=\"x\">a</span>] --> B\"q\"").text,
            "graph\nA[<span class='x'>a</span>] --> B\"q\""
        );
        assert_eq!(single_quote_attributes("a < b=\"c\" >"), "a < b=\"c\" >");
    }

    #[test]
    fn display_mode_goes_to_gantt() {
        let p = pre("---\ndisplayMode: compact\n---\ngantt");
        let Some(Value::Map(gantt)) = p.config.get("gantt") else {
            panic!("no gantt config");
        };
        assert_eq!(
            gantt.get("displayMode"),
            Some(&Value::String("compact".into()))
        );
    }
}
