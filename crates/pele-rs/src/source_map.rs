//! Error-only source mapping. Successful parses allocate no mapping tables.

use crate::Error;

pub(crate) struct Mapped {
    pub text: String,
    pub origins: Vec<usize>,
}

impl Mapped {
    pub fn new(source: &str) -> Self {
        let mut out = Self {
            text: String::new(),
            origins: Vec::new(),
        };
        let mut chars = source.char_indices().peekable();
        while let Some((at, ch)) = chars.next() {
            let ch = if ch == '\r' {
                if chars.peek().is_some_and(|(_, c)| *c == '\n') {
                    chars.next();
                }
                '\n'
            } else {
                ch
            };
            out.push(ch, at);
        }
        out
    }

    pub fn push(&mut self, ch: char, origin: usize) {
        self.text.push(ch);
        self.origins
            .extend(std::iter::repeat_n(origin, ch.len_utf8()));
    }

    pub fn remove(&mut self, ranges: &[(usize, usize)]) {
        let mut text = String::new();
        let mut origins = Vec::new();
        let mut last = 0;
        for &(start, end) in ranges {
            text.push_str(&self.text[last..start]);
            origins.extend_from_slice(&self.origins[last..start]);
            last = end;
        }
        text.push_str(&self.text[last..]);
        origins.extend_from_slice(&self.origins[last..]);
        self.text = text;
        self.origins = origins;
    }

    pub fn trim_start(&mut self) {
        let removed = self.text.len() - crate::util::trim_start(&self.text).len();
        self.remove(&[(0, removed)]);
    }
}

pub(crate) fn remap(
    error: Error,
    original: &str,
    parser_source: &str,
    keep_comments: bool,
) -> Error {
    if error.line == 0 {
        return error;
    }
    let mut mapped = crate::preprocess::mapped_source(original, keep_comments);
    crate::text::entities::encode_mapped(&mut mapped);
    let end = mapped
        .text
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_whitespace())
        .map_or(0, |(i, _)| {
            let at = mapped.origins[i];
            at + original[at..].chars().next().map_or(0, char::len_utf8)
        });
    mapped.push('\n', end);
    // A parser may have its own rewriting. Do not invent a mapping for a different text.
    if mapped.text != parser_source {
        return error;
    }
    let offset = parser_source
        .split_inclusive('\n')
        .take(error.line.saturating_sub(1) as usize)
        .map(str::len)
        .sum::<usize>();
    let tail = &parser_source[offset.min(parser_source.len())..];
    let column = tail
        .char_indices()
        .nth(error.column.saturating_sub(1) as usize)
        .map_or(tail.len(), |(i, _)| i);
    let at = mapped.origins.get(offset + column).copied().unwrap_or(end);
    // EOF diagnostics belong to the end of the last source token, not an artificial blank line.
    let at = if original[at.min(original.len())..].trim().is_empty() {
        original.trim_end_matches(char::is_whitespace).len()
    } else {
        at
    };
    let normalized = Mapped::new(original);
    let offset = normalized.origins.partition_point(|&origin| origin < at);
    error.at(&normalized.text, offset)
}
