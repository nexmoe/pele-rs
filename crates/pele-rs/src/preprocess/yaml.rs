//! A small YAML reader covering what Mermaid feeds to js-yaml with the JSON schema: block and
//! flow collections, quoted and plain scalars, and block scalars. Ported from pele's
//! `util/yaml.ts`. Text is handled as `char`s so that index arithmetic stays on characters.

use super::Value;
use crate::util::chars::{self, string, trim, trim_start};
use crate::util::is_space;
use std::collections::BTreeMap;

const MAX_DEPTH: usize = 64;

enum Fail {
    /// A quoted scalar or flow collection runs past the text read so far.
    Incomplete(&'static str),
    Invalid(&'static str),
}

type Res<T> = Result<T, Fail>;

impl Fail {
    fn message(self) -> &'static str {
        match self {
            Fail::Incomplete(m) | Fail::Invalid(m) => m,
        }
    }
}

struct Reader {
    lines: Vec<Vec<char>>,
    i: usize,
}

impl Reader {
    fn new(src: &str) -> Self {
        let src = src.replace("\r\n", "\n").replace('\r', "\n");
        Reader {
            lines: src.split('\n').map(|l| l.chars().collect()).collect(),
            i: 0,
        }
    }

    fn skip_blank(&mut self) {
        while self.i < self.lines.len() {
            let t = trim(&self.lines[self.i]);
            if !t.is_empty() && t[0] != '#' && t != ['-', '-', '-'] {
                break;
            }
            self.i += 1;
        }
    }

    fn indent(&self) -> Res<usize> {
        let line = &self.lines[self.i];
        let k = line.iter().take_while(|&&c| c == ' ').count();
        if line.get(k) == Some(&'\t') {
            return Err(Fail::Invalid("YAML: tabs are not allowed for indentation"));
        }
        Ok(k)
    }
}

fn is_blank(c: Option<&char>) -> bool {
    matches!(c, Some(' ' | '\t' | '\n' | '\r'))
}

fn starts_seq(line: &[char]) -> bool {
    line == ['-'] || line.starts_with(&['-', ' '])
}

fn leading_space(line: &[char]) -> usize {
    line.len() - trim_start(line).len()
}

/// Splits `key: rest` at the start of a line. Returns the end of the raw key and the offset
/// where the value starts.
fn split_key(line: &[char]) -> Option<(usize, usize)> {
    let first = *line.first()?;
    let end;
    let colon;
    if first == '"' || first == '\'' {
        let mut i = 1;
        loop {
            let c = *line.get(i)?;
            if first == '"' && c == '\\' {
                i += 2;
            } else if c == first {
                if first == '"' || line.get(i + 1) != Some(&'\'') {
                    break;
                }
                i += 2;
            } else {
                i += 1;
            }
        }
        end = i + 1;
        let mut c = end;
        while is_blank(line.get(c)) {
            c += 1;
        }
        if line.get(c) != Some(&':') {
            return None;
        }
        colon = c;
    } else {
        if is_space(first) || "#:,[]{}&*!|>'\"%@`".contains(first) {
            return None;
        }
        if first == '-'
            && (line.len() < 2 || is_blank(line.get(1)) || line[1] == ':' || line[1] == '#')
        {
            return None;
        }
        let mut c = 1;
        while c < line.len() && line[c] != ':' && line[c] != '#' {
            c += 1;
        }
        if line.get(c) != Some(&':') {
            return None;
        }
        colon = c;
        let mut e = c;
        while e > 0 && is_blank(line.get(e - 1)) {
            e -= 1;
        }
        end = e;
    }
    let mut length = colon + 1;
    if length < line.len() && !is_blank(line.get(length)) {
        return None;
    }
    while is_blank(line.get(length)) {
        length += 1;
    }
    Some((end, length))
}

fn is_digits(s: &str, radix: u32) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_digit(radix))
}

fn split_sign(s: &str) -> (bool, &str) {
    match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    }
}

/// `[0-9][0-9]*(\.[0-9]*)?` or `\.[0-9]+`, then an optional exponent.
fn is_float(s: &str) -> bool {
    let (_, s) = split_sign(s);
    let (mantissa, exponent) = match s.find(['e', 'E']) {
        Some(at) => (&s[..at], Some(&s[at + 1..])),
        None => (s, None),
    };
    let mantissa_ok = match mantissa.split_once('.') {
        Some((int, frac)) => {
            (is_digits(int, 10) && (frac.is_empty() || is_digits(frac, 10)))
                || (int.is_empty() && is_digits(frac, 10))
        }
        None => is_digits(mantissa, 10),
    };
    mantissa_ok && exponent.is_none_or(|e| is_digits(split_sign(e).1, 10))
}

fn radix_value(digits: &str, radix: u32) -> f64 {
    digits.chars().fold(0.0, |acc, c| {
        acc * f64::from(radix) + f64::from(c.to_digit(radix).unwrap_or(0))
    })
}

fn scalar(raw: &[char]) -> Value {
    let s = string(trim(raw));
    match s.as_str() {
        "" | "~" | "null" | "Null" | "NULL" => return Value::Null,
        "true" | "True" | "TRUE" => return Value::Bool(true),
        "false" | "False" | "FALSE" => return Value::Bool(false),
        _ => {}
    }
    let c = s.as_bytes()[0];
    if c != b'-' && c != b'+' && c != b'.' && !c.is_ascii_digit() {
        return Value::String(s);
    }
    if is_float(&s) {
        return Value::Number(s.parse().unwrap_or(f64::NAN));
    }
    let (neg, body) = split_sign(&s);
    for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if let Some(digits) = body.strip_prefix(prefix).filter(|d| is_digits(d, radix)) {
            let v = radix_value(digits, radix);
            return Value::Number(if neg { -v } else { v });
        }
    }
    if matches!(body, ".inf" | ".Inf" | ".INF") {
        return Value::Number(if neg {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    if matches!(s.as_str(), ".nan" | ".NaN" | ".NAN") {
        return Value::Number(f64::NAN);
    }
    Value::String(s)
}

fn escape(c: char) -> Option<char> {
    Some(match c {
        '0' => '\0',
        'a' => '\x07',
        'b' => '\x08',
        't' => '\t',
        'n' => '\n',
        'v' => '\x0b',
        'f' => '\x0c',
        'r' => '\r',
        'e' => '\x1b',
        ' ' => ' ',
        '"' => '"',
        '/' => '/',
        '\\' => '\\',
        'N' => '\u{85}',
        '_' => '\u{a0}',
        'L' => '\u{2028}',
        'P' => '\u{2029}',
        _ => return None,
    })
}

/// Folds the line breaks of a scalar: blanks around a break go, one break becomes a space, and
/// each further break stays.
fn fold(text: &[char]) -> Vec<char> {
    if !text.contains(&'\n') {
        return text.to_vec();
    }
    let lines: Vec<&[char]> = text.split(|&c| c == '\n').collect();
    let last = lines.len() - 1;
    let mut joined = Vec::with_capacity(text.len());
    for (i, line) in lines.iter().enumerate() {
        let blank = |c: &&char| **c == ' ' || **c == '\t';
        let from = if i > 0 {
            line.iter().take_while(blank).count()
        } else {
            0
        };
        let to = if i < last {
            line.len() - line[from..].iter().rev().take_while(blank).count()
        } else {
            line.len()
        };
        if i > 0 {
            joined.push('\n');
        }
        joined.extend_from_slice(&line[from..to]);
    }
    let mut out = Vec::with_capacity(joined.len());
    let mut i = 0;
    while i < joined.len() {
        if joined[i] == '\n' {
            let run = joined[i..].iter().take_while(|&&c| c == '\n').count();
            if run == 1 {
                out.push(' ');
            } else {
                out.extend(std::iter::repeat_n('\n', run - 1));
            }
            i += run;
        } else {
            out.push(joined[i]);
            i += 1;
        }
    }
    out
}

fn unquote_double(body: &[char]) -> Res<String> {
    if !body.contains(&'\\') {
        return Ok(string(&fold(body)));
    }
    // Escaped line breaks join lines without a space.
    let mut joined = Vec::with_capacity(body.len());
    let mut i = 0;
    while i < body.len() {
        if body[i] == '\\' && body.get(i + 1) == Some(&'\n') {
            i += 2;
            while matches!(body.get(i), Some(' ' | '\t')) {
                i += 1;
            }
        } else {
            joined.push(body[i]);
            i += 1;
        }
    }
    let s = fold(&joined);
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let ch = s[i];
        i += 1;
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let e = s.get(i).copied();
        i += 1;
        let len = match e {
            Some('x') => 2,
            Some('u') => 4,
            Some('U') => 8,
            _ => {
                out.push(
                    e.and_then(escape)
                        .ok_or(Fail::Invalid("YAML: unknown escape sequence"))?,
                );
                continue;
            }
        };
        let digits = string(s.get(i..i + len).unwrap_or_default());
        let c = u32::from_str_radix(&digits, 16)
            .ok()
            .and_then(char::from_u32)
            .ok_or(Fail::Invalid("YAML: unknown escape sequence"))?;
        out.push(c);
        i += len;
    }
    Ok(out)
}

fn skip(s: &[char], mut pos: usize, commas: bool) -> usize {
    while pos < s.len() {
        let c = s[pos];
        if c == ' ' || c == '\n' || c == '\t' || c == '\r' || (commas && c == ',') {
            pos += 1;
        } else if c == '#' && (pos == 0 || is_space(s[pos - 1])) {
            pos = chars::find(s, &['\n'], pos).unwrap_or(s.len());
        } else {
            break;
        }
    }
    pos
}

/// Entries of a flow collection are separated by exactly one comma, as js-yaml requires.
fn entry_start(ch: char, separated: bool) -> Res<()> {
    if !separated {
        return Err(Fail::Invalid(
            "YAML: missed comma between flow collection entries",
        ));
    }
    if ch == ',' {
        return Err(Fail::Invalid(
            "YAML: expected the node content, but found ','",
        ));
    }
    Ok(())
}

const UNCLOSED_FLOW: Fail =
    Fail::Incomplete("YAML: unexpected end of the stream within a flow collection");

/// Parses a flow value starting at `s[pos]`; returns the value and the index after it.
fn flow(s: &[char], pos: usize, stops: &str, depth: usize) -> Res<(Value, usize)> {
    if depth > MAX_DEPTH {
        return Err(Fail::Invalid("YAML: nesting is too deep"));
    }
    let mut pos = skip(s, pos, false);
    match s.get(pos) {
        Some('{') => {
            let mut out = BTreeMap::new();
            pos += 1;
            let mut separated = true;
            loop {
                pos = skip(s, pos, false);
                let &c = s.get(pos).ok_or(UNCLOSED_FLOW)?;
                if c == '}' {
                    return Ok((Value::Map(out), pos + 1));
                }
                entry_start(c, separated)?;
                let (key, next) = flow(s, pos, ":,}", depth + 1)?;
                pos = skip(s, next, false);
                let mut value = Value::Null;
                if s.get(pos) == Some(&':') {
                    (value, pos) = flow(s, pos + 1, ",}", depth + 1)?;
                }
                out.insert(key.js_string(), value);
                pos = skip(s, pos, false);
                separated = s.get(pos) == Some(&',');
                if separated {
                    pos += 1;
                }
            }
        }
        Some('[') => {
            let mut out = Vec::new();
            pos += 1;
            let mut separated = true;
            loop {
                pos = skip(s, pos, false);
                let &c = s.get(pos).ok_or(UNCLOSED_FLOW)?;
                if c == ']' {
                    return Ok((Value::Seq(out), pos + 1));
                }
                entry_start(c, separated)?;
                let (value, next) = flow(s, pos, ",]", depth + 1)?;
                out.push(value);
                pos = skip(s, next, false);
                separated = s.get(pos) == Some(&',');
                if separated {
                    pos += 1;
                }
            }
        }
        Some('"') => {
            let mut end = pos + 1;
            while end < s.len() && s[end] != '"' {
                end += if s[end] == '\\' { 2 } else { 1 };
            }
            if end >= s.len() {
                return Err(Fail::Incomplete(
                    "YAML: unexpected end of the stream within a double quoted scalar",
                ));
            }
            Ok((Value::String(unquote_double(&s[pos + 1..end])?), end + 1))
        }
        Some('\'') => {
            let mut end = pos + 1;
            while end < s.len() {
                if s[end] == '\'' {
                    if s.get(end + 1) != Some(&'\'') {
                        break;
                    }
                    end += 1;
                }
                end += 1;
            }
            if end >= s.len() {
                return Err(Fail::Incomplete(
                    "YAML: unexpected end of the stream within a single quoted scalar",
                ));
            }
            let text = string(&fold(&s[pos + 1..end])).replace("''", "'");
            Ok((Value::String(text), end + 1))
        }
        _ => {
            let mut end = pos;
            while end < s.len() {
                let c = s[end];
                if stops.contains(c)
                    && (c != ':'
                        || end + 1 >= s.len()
                        || is_space(s[end + 1])
                        || ",[]{}".contains(s[end + 1]))
                {
                    break;
                }
                if c == '#' && end > pos && is_space(s[end - 1]) {
                    let nl = chars::find(s, &['\n'], end).unwrap_or(s.len());
                    return Ok((scalar(&fold(trim(&s[pos..end]))), nl));
                }
                end += 1;
            }
            Ok((scalar(&fold(trim(&s[pos..end]))), end))
        }
    }
}

fn strip_comment(s: &[char]) -> &[char] {
    let mut quote = None;
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        match quote {
            Some(q) => {
                if c == '\\' && q == '"' {
                    i += 1;
                } else if c == q {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' && (i == 0 || s[i - 1] == ' ' || s[i - 1] == '\t') => {
                return &s[..i];
            }
            None => {}
        }
        i += 1;
    }
    s
}

fn block_scalar(r: &mut Reader, header: &[char], parent_indent: isize) -> String {
    let folded = header[0] == '>';
    let chomp = if header.contains(&'-') {
        '-'
    } else if header.contains(&'+') {
        '+'
    } else {
        ' '
    };
    let explicit = header
        .iter()
        .find_map(|c| c.to_digit(10).filter(|&d| d > 0));
    let mut indent: isize = explicit.map_or(-1, |d| parent_indent + d as isize);
    let mut out: Vec<Vec<char>> = Vec::new();
    while r.i < r.lines.len() {
        let line = &r.lines[r.i];
        if trim(line).is_empty() {
            out.push(Vec::new());
            r.i += 1;
            continue;
        }
        let k = leading_space(line) as isize;
        if indent == -1 {
            if k <= parent_indent {
                break;
            }
            indent = k;
        }
        if k < indent {
            break;
        }
        out.push(line[indent as usize..].to_vec());
        r.i += 1;
    }
    let mut trailing = 0;
    while out.last().is_some_and(Vec::is_empty) {
        out.pop();
        trailing += 1;
    }
    if out.is_empty() {
        return if chomp == '+' {
            "\n".repeat(trailing)
        } else {
            String::new()
        };
    }
    let mut text = String::new();
    for (i, cur) in out.iter().enumerate() {
        if i > 0 {
            if !folded {
                text.push('\n');
            } else {
                let prev = &out[i - 1];
                let indented = |l: &[char]| matches!(l.first(), Some(' ' | '\t'));
                if cur.is_empty() {
                    text.push('\n');
                } else if !prev.is_empty() {
                    text.push(if indented(prev) || indented(cur) {
                        '\n'
                    } else {
                        ' '
                    });
                }
            }
        }
        text.extend(cur.iter());
    }
    match chomp {
        '-' => text,
        '+' => text + &"\n".repeat(trailing + 1),
        _ => text + "\n",
    }
}

/// `[|>][-+0-9]*\s*(#.*)?`: a block scalar header.
fn is_block_header(rest: &[char]) -> bool {
    let mut i = 1 + rest[1..]
        .iter()
        .take_while(|&&c| c == '-' || c == '+' || c.is_ascii_digit())
        .count();
    i += rest[i..].iter().take_while(|&&c| is_space(c)).count();
    i == rest.len() || rest[i] == '#'
}

fn inline_value(r: &mut Reader, rest: &[char], indent: isize) -> Res<Value> {
    let first = rest.first().copied();
    if matches!(first, Some('|' | '>')) && is_block_header(rest) {
        return Ok(Value::String(block_scalar(r, rest, indent)));
    }
    if matches!(first, Some('{' | '[' | '"' | '\'')) {
        // The value may continue on later lines. Read them in growing batches until it closes.
        let start = r.i;
        let mut text = rest.to_vec();
        let mut taken = 0;
        let mut batch = 1;
        loop {
            match flow(&text, 0, "", 0) {
                Ok((value, end)) => {
                    let used = text[..end].iter().filter(|&&c| c == '\n').count();
                    let line_end = chars::find(&text, &['\n'], end).unwrap_or(text.len());
                    if !trim(strip_comment(&text[end..line_end])).is_empty() {
                        return Err(Fail::Invalid("YAML: unexpected content after a value"));
                    }
                    r.i = start + used;
                    return Ok(value);
                }
                Err(Fail::Incomplete(message)) => {
                    if start + taken >= r.lines.len() {
                        return Err(Fail::Invalid(message));
                    }
                    let to = (start + taken + batch).min(r.lines.len());
                    for line in &r.lines[start + taken..to] {
                        text.push('\n');
                        text.extend_from_slice(line);
                    }
                    taken = to - start;
                    batch *= 2;
                }
                Err(fail) => return Err(fail),
            }
        }
    }
    let mut text = trim(strip_comment(rest)).to_vec();
    while r.i < r.lines.len() {
        let line = &r.lines[r.i];
        if trim(line).is_empty() {
            let mut j = r.i;
            while j < r.lines.len() && trim(&r.lines[j]).is_empty() {
                j += 1;
            }
            if j >= r.lines.len() || leading_space(&r.lines[j]) as isize <= indent {
                break;
            }
            text.extend(std::iter::repeat_n('\n', j - r.i));
            r.i = j;
            continue;
        }
        if leading_space(line) as isize <= indent {
            break;
        }
        if text.last() != Some(&'\n') {
            text.push(' ');
        }
        text.extend_from_slice(trim(strip_comment(line)));
        r.i += 1;
    }
    Ok(scalar(&text))
}

fn block(r: &mut Reader, min_indent: usize, depth: usize) -> Res<Value> {
    if depth > MAX_DEPTH {
        return Err(Fail::Invalid("YAML: nesting is too deep"));
    }
    r.skip_blank();
    if r.i >= r.lines.len() {
        return Ok(Value::Null);
    }
    let indent = r.indent()?;
    if indent < min_indent {
        return Ok(Value::Null);
    }
    let line = r.lines[r.i][indent..].to_vec();

    if starts_seq(&line) {
        let mut out = Vec::new();
        while r.i < r.lines.len() {
            r.skip_blank();
            if r.i >= r.lines.len() || r.indent()? != indent {
                break;
            }
            let cur = r.lines[r.i][indent..].to_vec();
            if !starts_seq(&cur) {
                break;
            }
            let rest = &cur[1..];
            let pad = leading_space(rest) + 1;
            let body = trim(strip_comment(rest));
            if body.is_empty() {
                r.i += 1;
                out.push(block(r, indent + 1, depth + 1)?);
            } else if split_key(body).is_some() {
                let mut moved = vec![' '; indent + pad];
                moved.extend_from_slice(trim_start(rest));
                r.lines[r.i] = moved;
                out.push(block(r, indent + pad, depth + 1)?);
            } else {
                r.i += 1;
                out.push(inline_value(r, trim(rest), indent as isize)?);
            }
        }
        return Ok(Value::Seq(out));
    }

    if split_key(&line).is_none() {
        r.i += 1;
        return inline_value(r, trim(&line), indent as isize - 1);
    }

    let mut out = BTreeMap::new();
    while r.i < r.lines.len() {
        r.skip_blank();
        if r.i >= r.lines.len() {
            break;
        }
        let k = r.indent()?;
        if k < indent {
            break;
        }
        if k > indent {
            return Err(Fail::Invalid("YAML: bad indentation of a mapping entry"));
        }
        let cur = r.lines[r.i][indent..].to_vec();
        let (key_end, length) =
            split_key(&cur).ok_or(Fail::Invalid("YAML: can not read a block mapping entry"))?;
        let key = flow(&cur[..key_end], 0, "", 0)?.0.js_string();
        let rest = &cur[length..];
        r.i += 1;
        let value = if trim(strip_comment(rest)).is_empty() {
            r.skip_blank();
            if r.i < r.lines.len() {
                let next = r.indent()?;
                let seq = starts_seq(&r.lines[r.i][next..]);
                if next > indent || (seq && next == indent) {
                    block(r, if seq { indent } else { indent + 1 }, depth + 1)?
                } else {
                    Value::Null
                }
            } else {
                Value::Null
            }
        } else {
            inline_value(r, trim(rest), indent as isize)?
        };
        out.insert(key, value);
    }
    Ok(Value::Map(out))
}

/// Parses YAML text. Returns js-yaml's message for text it rejects.
pub(crate) fn parse_yaml(src: &str) -> Result<Value, &'static str> {
    let trimmed: Vec<char> = crate::util::trim(src).chars().collect();
    if matches!(trimmed.first(), Some('{' | '[')) {
        match flow(&trimmed, 0, "", 0) {
            Ok((value, end)) if skip(&trimmed, end, false) == trimmed.len() => return Ok(value),
            Ok(_) => {}
            Err(fail) => return Err(fail.message()),
        }
    }
    let mut r = Reader::new(src);
    let value = block(&mut r, 0, 0).map_err(Fail::message)?;
    r.skip_blank();
    if r.i < r.lines.len() && trim(&r.lines[r.i]) != ['.', '.', '.'] {
        return Err("YAML: end of the stream or a document separator is expected");
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, Value)]) -> Value {
        Value::Map(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        )
    }

    fn s(text: &str) -> Value {
        Value::String(text.to_string())
    }

    #[test]
    fn reads_block_mappings() {
        let value = parse_yaml(
            "title: Hello world\nconfig:\n  theme: dark\n  pie:\n    textPosition: 0.5\n",
        )
        .unwrap();
        assert_eq!(
            value,
            map(&[
                ("title", s("Hello world")),
                (
                    "config",
                    map(&[
                        ("theme", s("dark")),
                        ("pie", map(&[("textPosition", Value::Number(0.5))])),
                    ])
                ),
            ])
        );
    }

    #[test]
    fn reads_scalars() {
        let value =
            parse_yaml("a: ~\nb: TRUE\nc: 0x1F\nd: -.inf\ne: '1'\nf: \"x\\ty\"\ng: 1e3\nh: 0o17")
                .unwrap();
        assert_eq!(
            value,
            map(&[
                ("a", Value::Null),
                ("b", Value::Bool(true)),
                ("c", Value::Number(31.0)),
                ("d", Value::Number(f64::NEG_INFINITY)),
                ("e", s("1")),
                ("f", s("x\ty")),
                ("g", Value::Number(1000.0)),
                ("h", Value::Number(15.0)),
            ])
        );
    }

    #[test]
    fn reads_sequences_and_flow() {
        let value =
            parse_yaml("list:\n  - one\n  - k: v\n    j: w\nflow: {a: [1, 2], b: x}").unwrap();
        assert_eq!(
            value,
            map(&[
                (
                    "list",
                    Value::Seq(vec![s("one"), map(&[("k", s("v")), ("j", s("w"))])])
                ),
                (
                    "flow",
                    map(&[
                        (
                            "a",
                            Value::Seq(vec![Value::Number(1.0), Value::Number(2.0)])
                        ),
                        ("b", s("x")),
                    ])
                ),
            ])
        );
    }

    #[test]
    fn reads_multiline_values() {
        let value =
            parse_yaml("a: |\n  one\n  two\nb: >-\n  x\n  y\nc: [1,\n  2]\nd: plain\n  more # c")
                .unwrap();
        assert_eq!(
            value,
            map(&[
                ("a", s("one\ntwo\n")),
                ("b", s("x y")),
                (
                    "c",
                    Value::Seq(vec![Value::Number(1.0), Value::Number(2.0)])
                ),
                ("d", s("plain more")),
            ])
        );
    }

    #[test]
    fn rejects_bad_yaml() {
        assert!(parse_yaml("a: [1, 2").is_err());
        assert!(parse_yaml("\ta: 1").is_err());
        assert!(parse_yaml("a: \"\\q\"").is_err());
    }

    #[test]
    fn folds_an_indented_continuation_into_a_plain_scalar() {
        assert_eq!(parse_yaml("a: 1\n b: 2"), Ok(map(&[("a", s("1 b: 2"))])));
    }

    #[test]
    fn limits_nesting() {
        let deep = "[".repeat(200) + &"]".repeat(200);
        assert_eq!(parse_yaml(&deep), Err("YAML: nesting is too deep"));
        let nested: String = (0..100).map(|i| "  ".repeat(i) + "k:\n").collect();
        assert_eq!(parse_yaml(&nested), Err("YAML: nesting is too deep"));
    }
}
