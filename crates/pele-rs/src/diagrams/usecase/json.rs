//! The body of a `json` node of a use case diagram: JSON as Mermaid reads it with `JSON.parse`,
//! with the order its keys were written in kept, which `JSON.parse` loses for integer-like
//! keys.

/// A JSON value, with the keys of an object in the order they were written.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    /// The value as JavaScript's `String(value)` writes it.
    pub(crate) fn scalar(&self) -> String {
        match self {
            JsonValue::Null => "null".to_string(),
            JsonValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            JsonValue::Number(n) => crate::util::js_number(*n),
            JsonValue::String(s) => s.clone(),
            JsonValue::Array(_) | JsonValue::Object(_) => String::new(),
        }
    }

    /// The value of a key of an object, if it has one.
    #[allow(dead_code, reason = "the rows are read from the object itself")]
    fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }
}

/// Reads JSON text, or gives the offset of the first character that is not valid JSON.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    depth: usize,
    limit: usize,
}

impl Reader<'_> {
    fn ws(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, lit: &[u8]) -> bool {
        if self.bytes[self.pos..].starts_with(lit) {
            self.pos += lit.len();
            true
        } else {
            false
        }
    }

    fn digits(&mut self) -> usize {
        let n = self.bytes[self.pos..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        self.pos += n;
        n
    }

    fn value(&mut self) -> Result<JsonValue, usize> {
        if self.depth > self.limit {
            return Err(self.pos);
        }
        self.ws();
        match *self.bytes.get(self.pos).ok_or(self.pos)? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => self.string().map(JsonValue::String),
            b't' => {
                if self.eat(b"true") {
                    Ok(JsonValue::Bool(true))
                } else {
                    Err(self.pos)
                }
            }
            b'f' => {
                if self.eat(b"false") {
                    Ok(JsonValue::Bool(false))
                } else {
                    Err(self.pos)
                }
            }
            b'n' => {
                if self.eat(b"null") {
                    Ok(JsonValue::Null)
                } else {
                    Err(self.pos)
                }
            }
            _ => self.number(),
        }
    }

    fn object(&mut self) -> Result<JsonValue, usize> {
        self.pos += 1;
        self.depth += 1;
        let mut out: Vec<(String, JsonValue)> = Vec::new();
        self.ws();
        if self.eat(b"}") {
            self.depth -= 1;
            return Ok(JsonValue::Object(out));
        }
        loop {
            self.ws();
            if *self.bytes.get(self.pos).ok_or(self.pos)? != b'"' {
                return Err(self.pos);
            }
            let key = self.string()?;
            self.ws();
            if !self.eat(b":") {
                return Err(self.pos);
            }
            let value = self.value()?;
            // A repeated key keeps the place it took first and the value it was given last.
            match out.iter_mut().find(|(name, _)| *name == key) {
                Some(entry) => entry.1 = value,
                None => out.push((key, value)),
            }
            self.ws();
            if self.eat(b"}") {
                self.depth -= 1;
                return Ok(JsonValue::Object(out));
            }
            if !self.eat(b",") {
                return Err(self.pos);
            }
        }
    }

    fn array(&mut self) -> Result<JsonValue, usize> {
        self.pos += 1;
        self.depth += 1;
        let mut out = Vec::new();
        self.ws();
        if self.eat(b"]") {
            self.depth -= 1;
            return Ok(JsonValue::Array(out));
        }
        loop {
            out.push(self.value()?);
            self.ws();
            if self.eat(b"]") {
                self.depth -= 1;
                return Ok(JsonValue::Array(out));
            }
            if !self.eat(b",") {
                return Err(self.pos);
            }
        }
    }

    fn number(&mut self) -> Result<JsonValue, usize> {
        let start = self.pos;
        let _ = self.eat(b"-");
        if !self.eat(b"0") && self.digits() == 0 {
            return Err(start);
        }
        if self.eat(b".") && self.digits() == 0 {
            return Err(self.pos);
        }
        if matches!(self.bytes.get(self.pos), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.bytes.get(self.pos), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if self.digits() == 0 {
                return Err(self.pos);
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|_| start)?;
        text.parse().map(JsonValue::Number).map_err(|_| start)
    }

    fn hex4(&mut self) -> Result<u32, usize> {
        let digits = self.bytes.get(self.pos..self.pos + 4).ok_or(self.pos)?;
        let text = std::str::from_utf8(digits).map_err(|_| self.pos)?;
        let value = u32::from_str_radix(text, 16).map_err(|_| self.pos)?;
        self.pos += 4;
        Ok(value)
    }

    fn string(&mut self) -> Result<String, usize> {
        self.pos += 1;
        let mut out = String::new();
        loop {
            let b = *self.bytes.get(self.pos).ok_or(self.pos)?;
            self.pos += 1;
            match b {
                b'"' => return Ok(out),
                0..=0x1f => return Err(self.pos - 1),
                b'\\' => {
                    let e = *self.bytes.get(self.pos).ok_or(self.pos)?;
                    self.pos += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let high = self.hex4()?;
                            let c = if (0xd800..0xdc00).contains(&high)
                                && self.bytes[self.pos..].starts_with(b"\\u")
                            {
                                let save = self.pos;
                                self.pos += 2;
                                match self.hex4() {
                                    Ok(low) if (0xdc00..0xe000).contains(&low) => char::from_u32(
                                        0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00),
                                    )
                                    .unwrap_or('\u{fffd}'),
                                    _ => {
                                        self.pos = save;
                                        char::from_u32(high).unwrap_or('\u{fffd}')
                                    }
                                }
                            } else {
                                char::from_u32(high).unwrap_or('\u{fffd}')
                            };
                            out.push(c);
                        }
                        _ => return Err(self.pos - 1),
                    }
                }
                _ => {
                    // The bytes of a UTF-8 character are kept as they are.
                    let start = self.pos - 1;
                    let mut end = self.pos;
                    while end < self.bytes.len() && (self.bytes[end] & 0xc0) == 0x80 {
                        end += 1;
                    }
                    out.push_str(std::str::from_utf8(&self.bytes[start..end]).map_err(|_| start)?);
                    self.pos = end;
                }
            }
        }
    }
}

/// One row of a JSON table: a leaf of the object, with the path that leads to it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct JsonRow {
    /// The name of the row, blank on the second and later values of an array.
    pub key: String,
    /// The path that leads to the row, for the screen reader.
    pub accessible_key: String,
    pub value: String,
}

/// The rows of a JSON table: one for each leaf of the object, depth first in the order the keys
/// were written. Nesting is kept on a stack of its own, so depth is limited by memory and not
/// by the call stack.
pub(crate) fn flatten_rows(value: &JsonValue) -> Vec<JsonRow> {
    let mut rows: Vec<JsonRow> = Vec::new();
    let mut stack: Vec<(&JsonValue, String, String)> = vec![(value, String::new(), String::new())];
    while let Some((current, path, pointer)) = stack.pop() {
        let mut children: Vec<(&JsonValue, String, String)> = Vec::new();
        match current {
            JsonValue::Array(items) => {
                if items.is_empty() {
                    rows.push(JsonRow {
                        key: path.clone(),
                        accessible_key: path,
                        value: "[]".to_string(),
                    });
                } else if items
                    .iter()
                    .all(|item| !matches!(item, JsonValue::Array(_) | JsonValue::Object(_)))
                {
                    for (i, item) in items.iter().enumerate() {
                        rows.push(JsonRow {
                            key: if i == 0 { path.clone() } else { String::new() },
                            accessible_key: path.clone(),
                            value: item.scalar(),
                        });
                    }
                } else {
                    for (i, item) in items.iter().enumerate() {
                        children.push((item, format!("{path}[{i}]"), format!("{pointer}/{i}")));
                    }
                }
            }
            JsonValue::Object(entries) => {
                if entries.is_empty() {
                    rows.push(JsonRow {
                        key: path.clone(),
                        accessible_key: path.clone(),
                        value: "{}".to_string(),
                    });
                }
                for (key, item) in entries {
                    let child_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    let child_pointer = format!("{pointer}/{}", escape_pointer(key));
                    children.push((item, child_path, child_pointer));
                }
            }
            _ => rows.push(JsonRow {
                key: path.clone(),
                accessible_key: path,
                value: current.scalar(),
            }),
        }
        for child in children.into_iter().rev() {
            stack.push(child);
        }
    }
    rows
}

/// A key of an object as it is written in a JSON Pointer.
fn escape_pointer(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Reads the body of a `json` node, or gives the byte offset in the text of the first character
/// that is not valid JSON. `limit` is how deep the JSON may be nested.
pub(crate) fn parse_json(text: &str, limit: usize) -> Result<JsonValue, usize> {
    let mut reader = Reader {
        bytes: text.as_bytes(),
        pos: 0,
        depth: 0,
        limit,
    };
    match reader.value() {
        Ok(value) => {
            reader.ws();
            if reader.pos != reader.bytes.len() {
                Err(reader.pos)
            } else {
                Ok(value)
            }
        }
        Err(at) => Err(at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_order_of_integer_like_keys() {
        let value = parse_json(r#"{"2":"b","1":"a"}"#, 64).unwrap();
        let rows = flatten_rows(&value);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].key, "2");
        assert_eq!(rows[1].key, "1");
    }

    #[test]
    fn refuses_what_json_parse_refuses() {
        assert!(parse_json("{a:1}", 64).is_err());
        assert!(parse_json("[1,]", 64).is_err());
        assert!(parse_json("01", 64).is_err());
    }
}
