//! The sankey parser: Mermaid's jison grammar for `sankey.jison`, as pele's `parser.ts`
//! emulates it. A sankey chart is written as CSV: a `source,target,value` record per line.

use super::db::SankeyDb;
use super::lexer::{T, Tokens, tokenize};
use crate::error::{Error, ErrorCode};
use crate::text::entities::decode_entities;
use crate::util::{is_space, js_parse_float, trim};

/// The characters Mermaid's `prepareTextForParsing` counts as blank: the whitespace that is
/// not a line break.
fn is_blank(c: char) -> bool {
    is_space(c) && c != '\n' && c != '\r'
}

/// The characters XML refuses, which a node's name may not hold.
fn has_invalid_xml_char(text: &str) -> bool {
    text.chars().any(|c| {
        let c = c as u32;
        (c <= 0x08)
            || c == 0x0b
            || c == 0x0c
            || (0x0e..=0x1f).contains(&c)
            || (0xd800..=0xdfff).contains(&c)
            || c == 0xfffe
            || c == 0xffff
    })
}

/// Mermaid's `prepareTextForParsing`: the blanks at both ends go, the runs of line breaks
/// become one line break, and the rest is trimmed.
pub(crate) fn prepare_text_for_parsing(text: &str) -> String {
    let start = text.find(|c: char| !is_blank(c)).unwrap_or(text.len());
    let end = text.rfind(|c: char| !is_blank(c)).map_or(start, |i| {
        i + text[i..].chars().next().map_or(0, char::len_utf8)
    });
    let mut out = String::with_capacity(end - start);
    let mut in_breaks = false;
    for c in text[start..end].chars() {
        if c == '\n' || c == '\r' {
            if !in_breaks {
                out.push('\n');
                in_breaks = true;
            }
        } else {
            in_breaks = false;
            out.push(c);
        }
    }
    trim(&out).to_string()
}

/// The byte offset of each UTF-16 code unit, and one for the end of the text.
fn byte_offsets(src: &str) -> Vec<usize> {
    let mut out = Vec::with_capacity(src.len() + 1);
    let mut bytes = 0;
    for c in src.chars() {
        for _ in 0..c.len_utf16() {
            out.push(bytes);
        }
        bytes += c.len_utf8();
    }
    out.push(bytes);
    out
}

struct Parser<'a> {
    src: &'a str,
    /// The byte offset of each UTF-16 code unit, so that a token's place can be reported.
    offsets: Vec<usize>,
    tokens: Tokens,
    i: usize,
}

impl Parser<'_> {
    fn t(&self) -> T {
        self.tokens.types[self.i]
    }

    fn fail(&self, expected: &str) -> Error {
        let t = self.t();
        // A character the lexer cannot read is reported as the token before it.
        let got = if t == T::Stuck {
            self.tokens.types[self.i.saturating_sub(1)].name()
        } else {
            t.name()
        };
        Error::new(
            ErrorCode::Syntax,
            format!("Expecting {expected}, got '{got}'"),
        )
        .at(self.src, self.offsets[self.tokens.starts[self.i]])
    }

    fn expect(&mut self, t: T, also: &str) -> Result<(), Error> {
        if self.t() != t {
            let sep = if also.is_empty() { "" } else { ", " };
            return Err(self.fail(&format!("'{}'{sep}{also}", t.name())));
        }
        self.i += 1;
        Ok(())
    }

    /// A field of a record: either plain text or a quoted one.
    fn field(&mut self) -> Result<String, Error> {
        if self.t() == T::NonEscapedText {
            let text = self.tokens.texts[self.i].clone();
            self.i += 1;
            return Ok(text);
        }
        if self.t() != T::Dquote {
            return Err(self.fail("'DQUOTE', 'NON_ESCAPED_TEXT'"));
        }
        self.i += 1;
        if self.t() != T::EscapedText {
            return Err(self.fail("'ESCAPED_TEXT'"));
        }
        let text = self.tokens.texts[self.i].clone();
        self.i += 1;
        self.expect(T::Dquote, "")?;
        Ok(text)
    }

    /// The name of a node: a field, trimmed, with the doubled quotes of a quoted field made
    /// one.
    fn name(&mut self) -> Result<String, Error> {
        let start = self.tokens.starts[self.i];
        let text = self.field()?;
        let text = trim(&text).replace("\"\"", "\"");
        // Entity codes can encode characters the lexer excludes.
        if has_invalid_xml_char(&decode_entities(&text)) {
            return Err(
                Error::new(ErrorCode::Syntax, "Invalid XML character in node name")
                    .at(self.src, self.offsets[start]),
            );
        }
        Ok(text)
    }
}

/// Reads a sankey chart, as Mermaid's parser does.
pub(crate) fn parse_sankey(src: &str, db: &mut SankeyDb) -> Result<(), Error> {
    let units: Vec<u16> = src.encode_utf16().collect();
    let mut p = Parser {
        src,
        offsets: byte_offsets(src),
        tokens: tokenize(&units),
        i: 0,
    };

    p.expect(T::Sankey, "")?;
    p.expect(T::Newline, "")?;
    loop {
        let from = p.name()?;
        p.expect(T::Comma, "'NEWLINE', 'EOF'")?;
        let to = p.name()?;
        p.expect(T::Comma, "'NEWLINE', 'EOF'")?;
        let amount = p.field()?;
        let source = db.find_or_create_node(&from);
        let target = db.find_or_create_node(&to);
        db.add_link(source, target, js_parse_float(&amount));
        if p.t() != T::Newline {
            break;
        }
        p.i += 1;
    }
    if p.t() == T::Eof {
        p.i += 1;
    }
    if p.t() != T::End {
        return Err(p.fail("'NEWLINE', 'EOF'"));
    }
    Ok(())
}
