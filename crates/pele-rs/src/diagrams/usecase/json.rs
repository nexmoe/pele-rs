//! The body of a `json` node of a use case diagram: JSON as Mermaid reads it with `JSON.parse`,
//! with the order its keys were written in kept, which `JSON.parse` loses for integer-like
//! keys.

/// A JSON value, with the keys of an object in the order they were written.
#[derive(Debug, PartialEq)]
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

// Parsed JSON can be much deeper than the Rust call stack. Cloning and releasing it
// use the same explicit traversal as parsing and flattening.
impl Clone for JsonValue {
    fn clone(&self) -> Self {
        let mut stack = vec![(self, false)];
        let mut values = Vec::new();
        while let Some((node, done)) = stack.pop() {
            if !done {
                match node {
                    JsonValue::Array(items) => {
                        stack.push((node, true));
                        stack.extend(items.iter().rev().map(|v| (v, false)));
                        continue;
                    }
                    JsonValue::Object(entries) => {
                        stack.push((node, true));
                        stack.extend(entries.iter().rev().map(|(_, v)| (v, false)));
                        continue;
                    }
                    _ => {}
                }
            }
            let value = match node {
                JsonValue::Null => JsonValue::Null,
                JsonValue::Bool(v) => JsonValue::Bool(*v),
                JsonValue::Number(v) => JsonValue::Number(*v),
                JsonValue::String(v) => JsonValue::String(v.clone()),
                JsonValue::Array(items) => {
                    let children = values.split_off(values.len() - items.len());
                    JsonValue::Array(children)
                }
                JsonValue::Object(entries) => {
                    let children = values.split_off(values.len() - entries.len());
                    JsonValue::Object(
                        entries
                            .iter()
                            .map(|(k, _)| k.clone())
                            .zip(children)
                            .collect(),
                    )
                }
            };
            values.push(value);
        }
        values.pop().unwrap()
    }
}
impl Drop for JsonValue {
    fn drop(&mut self) {
        fn drain(value: &mut JsonValue, pending: &mut Vec<JsonValue>) {
            match value {
                JsonValue::Array(items) => pending.extend(std::mem::take(items)),
                JsonValue::Object(entries) => {
                    pending.extend(std::mem::take(entries).into_iter().map(|(_, v)| v))
                }
                _ => {}
            }
        }
        let mut pending = Vec::new();
        drain(self, &mut pending);
        while let Some(mut value) = pending.pop() {
            drain(&mut value, &mut pending);
        }
    }
}

/// Reads JSON text, or gives the offset of the first character that is not valid JSON.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
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
        enum Frame {
            Array(Vec<JsonValue>),
            Object(Vec<(String, JsonValue)>, String),
        }
        let mut stack: Vec<Frame> = Vec::new();
        loop {
            if stack.len() > self.limit {
                return Err(self.pos);
            }
            self.ws();
            let mut value = match *self.bytes.get(self.pos).ok_or(self.pos)? {
                b'[' => {
                    self.pos += 1;
                    self.ws();
                    if self.eat(b"]") {
                        JsonValue::Array(Vec::new())
                    } else {
                        stack.push(Frame::Array(Vec::new()));
                        continue;
                    }
                }
                b'{' => {
                    self.pos += 1;
                    self.ws();
                    if self.eat(b"}") {
                        JsonValue::Object(Vec::new())
                    } else {
                        let key = self.string()?;
                        self.ws();
                        if !self.eat(b":") {
                            return Err(self.pos);
                        }
                        stack.push(Frame::Object(Vec::new(), key));
                        continue;
                    }
                }
                b'"' => JsonValue::String(self.string()?),
                b't' if self.eat(b"true") => JsonValue::Bool(true),
                b'f' if self.eat(b"false") => JsonValue::Bool(false),
                b'n' if self.eat(b"null") => JsonValue::Null,
                _ => self.number()?,
            };
            loop {
                let Some(frame) = stack.last_mut() else {
                    return Ok(value);
                };
                match frame {
                    Frame::Array(items) => {
                        items.push(value);
                        self.ws();
                        if self.eat(b"]") {
                            value = JsonValue::Array(std::mem::take(items));
                            stack.pop();
                        } else {
                            if !self.eat(b",") {
                                return Err(self.pos);
                            }
                            break;
                        }
                    }
                    Frame::Object(entries, key) => {
                        if let Some(entry) = entries.iter_mut().find(|(name, _)| name == key) {
                            entry.1 = value;
                        } else {
                            entries.push((std::mem::take(key), value));
                        }
                        self.ws();
                        if self.eat(b"}") {
                            value = JsonValue::Object(std::mem::take(entries));
                            stack.pop();
                        } else {
                            if !self.eat(b",") {
                                return Err(self.pos);
                            }
                            self.ws();
                            *key = self.string()?;
                            self.ws();
                            if !self.eat(b":") {
                                return Err(self.pos);
                            }
                            break;
                        }
                    }
                }
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
        if self.bytes.get(self.pos) != Some(&b'"') {
            return Err(self.pos);
        }
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
