//! A strict JSON reader for `%%{init: ...}%%` directives, which Mermaid reads with `JSON.parse`.

use super::Value;
use std::collections::BTreeMap;

const MAX_DEPTH: usize = 64;

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while matches!(self.s.get(self.pos), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, lit: &[u8]) -> Option<()> {
        self.s[self.pos..]
            .starts_with(lit)
            .then(|| self.pos += lit.len())
    }

    fn digits(&mut self) -> usize {
        let n = self.s[self.pos..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        self.pos += n;
        n
    }

    fn value(&mut self, depth: usize) -> Option<Value> {
        if depth > MAX_DEPTH {
            return None;
        }
        self.ws();
        match *self.s.get(self.pos)? {
            b'{' => {
                self.pos += 1;
                let mut out = BTreeMap::new();
                self.ws();
                if self.eat(b"}").is_some() {
                    return Some(Value::Map(out));
                }
                loop {
                    self.ws();
                    let key = self.string()?;
                    self.ws();
                    self.eat(b":")?;
                    let value = self.value(depth + 1)?;
                    out.insert(key, value);
                    self.ws();
                    if self.eat(b"}").is_some() {
                        return Some(Value::Map(out));
                    }
                    self.eat(b",")?;
                }
            }
            b'[' => {
                self.pos += 1;
                let mut out = Vec::new();
                self.ws();
                if self.eat(b"]").is_some() {
                    return Some(Value::Seq(out));
                }
                loop {
                    out.push(self.value(depth + 1)?);
                    self.ws();
                    if self.eat(b"]").is_some() {
                        return Some(Value::Seq(out));
                    }
                    self.eat(b",")?;
                }
            }
            b'"' => self.string().map(Value::String),
            b't' => self.eat(b"true").map(|()| Value::Bool(true)),
            b'f' => self.eat(b"false").map(|()| Value::Bool(false)),
            b'n' => self.eat(b"null").map(|()| Value::Null),
            _ => self.number(),
        }
    }

    fn number(&mut self) -> Option<Value> {
        let start = self.pos;
        let _ = self.eat(b"-");
        if self.eat(b"0").is_none() && self.digits() == 0 {
            return None;
        }
        if self.eat(b".").is_some() && self.digits() == 0 {
            return None;
        }
        if matches!(self.s.get(self.pos), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.s.get(self.pos), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if self.digits() == 0 {
                return None;
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.pos]).ok()?;
        text.parse().ok().map(Value::Number)
    }

    fn hex4(&mut self) -> Option<u32> {
        let digits = self.s.get(self.pos..self.pos + 4)?;
        let value = u32::from_str_radix(std::str::from_utf8(digits).ok()?, 16).ok()?;
        self.pos += 4;
        Some(value)
    }

    fn string(&mut self) -> Option<String> {
        self.eat(b"\"")?;
        let mut out = Vec::new();
        loop {
            let b = *self.s.get(self.pos)?;
            self.pos += 1;
            match b {
                b'"' => return String::from_utf8(out).ok(),
                0..=0x1f => return None,
                b'\\' => {
                    let e = *self.s.get(self.pos)?;
                    self.pos += 1;
                    let c = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\x08',
                        b'f' => '\x0c',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.unicode()?,
                        _ => return None,
                    };
                    out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
                }
                _ => out.push(b),
            }
        }
    }

    /// A `\u` escape, joining a surrogate pair. A lone surrogate, which JavaScript keeps but a
    /// Rust string cannot hold, becomes U+FFFD.
    fn unicode(&mut self) -> Option<char> {
        let high = self.hex4()?;
        if (0xd800..0xdc00).contains(&high) && self.s[self.pos..].starts_with(b"\\u") {
            let save = self.pos;
            self.pos += 2;
            let low = self.hex4()?;
            if (0xdc00..0xe000).contains(&low) {
                return char::from_u32(0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00));
            }
            self.pos = save;
        }
        Some(char::from_u32(high).unwrap_or('\u{fffd}'))
    }
}

/// Parses JSON text, or returns `None` where `JSON.parse` would throw. Nesting deeper than 64
/// levels is refused too.
pub(crate) fn parse_json(text: &str) -> Option<Value> {
    let mut p = Parser {
        s: text.as_bytes(),
        pos: 0,
    };
    let value = p.value(0)?;
    p.ws();
    (p.pos == p.s.len()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json() {
        let value =
            parse_json(r#" {"theme": "dark", "n": -1.5e2, "a": [true, null], "s": "é😀"} "#);
        let Some(Value::Map(map)) = value else {
            panic!("expected a map");
        };
        assert_eq!(map["theme"], Value::String("dark".into()));
        assert_eq!(map["n"], Value::Number(-150.0));
        assert_eq!(map["a"], Value::Seq(vec![Value::Bool(true), Value::Null]));
        assert_eq!(map["s"], Value::String("é😀".into()));
    }

    #[test]
    fn rejects_what_json_parse_rejects() {
        for bad in [
            "{a: 1}",
            "[1,]",
            "01",
            "1.",
            "'x'",
            "{\"a\":1} x",
            "\"\t\"",
            "",
        ] {
            assert_eq!(parse_json(bad), None, "{bad}");
        }
        assert_eq!(parse_json(&("[".repeat(100) + &"]".repeat(100))), None);
    }
}
