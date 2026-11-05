//! The parser of a venn diagram: Mermaid's jison grammar for `venn.jison`, as pele's
//! `parser.ts` emulates it. A token is read only when the grammar has to look at one to
//! decide, because the lexer asks the model whether an indented `text` continues a set, and
//! the statements are what tell the model so.

use super::db::VennDb;
use super::lexer::{Lexer, T};
use crate::error::{Error, ErrorCode};

/// The tokens a statement may start with.
const STATEMENT: &str = "'EOF', 'NEWLINE', 'TITLE', 'SET', 'UNION', 'TEXT', 'INDENT_TEXT', 'STYLE'";
/// The tokens a style value may be made of.
const VALUE: &str = "'NUMERIC', 'IDENTIFIER', 'HEXCOLOR', 'RGBCOLOR', 'RGBACOLOR'";

/// Parses a venn diagram into the model.
pub(crate) fn parse_venn(src: &str, db: &mut VennDb) -> Result<(), Error> {
    Parser {
        src,
        lexer: Lexer::new(src),
        peeked: None,
    }
    .parse(db)
}

struct Parser<'a> {
    src: &'a str,
    lexer: Lexer,
    /// The token that was read ahead, and its text.
    peeked: Option<(T, String)>,
}

impl Parser<'_> {
    /// The next token, read when it is not already there.
    fn peek(&mut self) -> T {
        if self.peeked.is_none() {
            let kind = self.lexer.next();
            let text = self.lexer.text().to_string();
            self.peeked = Some((kind, text));
        }
        self.peeked.as_ref().map_or(T::End, |(kind, _)| *kind)
    }

    /// The text of the token that was read ahead.
    fn take(&mut self) -> String {
        self.peeked
            .take()
            .map_or_else(String::new, |(_, text)| text)
    }

    fn fail(&mut self, expected: &str) -> Error {
        let at = self.lexer.start_byte();
        if self.peek() == T::Error {
            return Error::new(ErrorCode::Syntax, "Unrecognized text.").at(self.src, at);
        }
        Error::new(
            ErrorCode::Syntax,
            format!("Expecting {expected}, got '{}'", self.peek().name()),
        )
        .at(self.src, at)
    }

    fn expect(&mut self, wanted: T) -> Result<String, Error> {
        if self.peek() != wanted {
            return Err(self.fail(&format!("'{}'", wanted.name())));
        }
        Ok(self.take())
    }

    /// The id of a set, which may be a name or a quoted string.
    fn identifier(&mut self) -> Result<String, Error> {
        let kind = self.peek();
        if kind != T::Identifier && kind != T::String {
            return Err(self.fail("'IDENTIFIER', 'STRING'"));
        }
        Ok(self.take())
    }

    /// The ids of a list of sets.
    fn identifiers(&mut self) -> Result<Vec<String>, Error> {
        let mut list = vec![self.identifier()?];
        while self.peek() == T::Comma {
            self.take();
            list.push(self.identifier()?);
        }
        Ok(list)
    }

    /// A set or a union: the ids, an optional label in brackets and an optional size.
    fn subset(&mut self, db: &mut VennDb, ids: Vec<String>, union: bool) -> Result<(), Error> {
        let label = if self.peek() == T::BracketLabel {
            Some(self.take())
        } else {
            None
        };
        let mut size = None;
        if self.peek() == T::Colon {
            self.take();
            let text = self.expect(T::Numeric)?;
            size = Some(crate::util::js_parse_float(&text));
        }
        if union {
            if ids.len() < 2 {
                return Err(Error::new(
                    ErrorCode::Semantic,
                    "union requires multiple identifiers",
                ));
            }
            db.validate_union_identifiers(&ids)?;
        }
        db.add_subset_data(ids, label, size);
        // An indented `text` line below belongs to this set or union.
        self.lexer.set_indent(true);
        Ok(())
    }

    /// The id of a text, and the label in brackets that an id which is not a number may carry.
    fn text(&mut self, db: &mut VennDb, sets: &[String]) -> Result<(), Error> {
        let kind = self.peek();
        if kind != T::Identifier && kind != T::String && kind != T::Numeric {
            return Err(self.fail("'NUMERIC', 'IDENTIFIER', 'STRING'"));
        }
        let id = self.take();
        let label = if kind != T::Numeric && self.peek() == T::BracketLabel {
            Some(self.take())
        } else {
            None
        };
        db.add_text_data(sets.to_vec(), id, label);
        Ok(())
    }

    /// The declarations of a style: a property and its value, separated by commas.
    fn style(&mut self, db: &mut VennDb) -> Result<(), Error> {
        let ids = self.identifiers()?;
        let mut styles: Vec<(String, String)> = Vec::new();
        loop {
            let key = self.expect(T::Identifier)?;
            self.expect(T::Colon)?;
            let value = if self.peek() == T::String {
                self.take()
            } else {
                let mut parts: Vec<String> = Vec::new();
                while matches!(
                    self.peek(),
                    T::Identifier | T::Numeric | T::HexColor | T::RgbColor | T::RgbaColor
                ) {
                    parts.push(self.take());
                }
                if parts.is_empty() {
                    return Err(self.fail(VALUE));
                }
                parts.join(" ")
            };
            styles.push((key, value));
            if self.peek() != T::Comma {
                break;
            }
            self.take();
        }
        db.add_style_data(ids, styles);
        Ok(())
    }

    fn parse(&mut self, db: &mut VennDb) -> Result<(), Error> {
        while self.peek() == T::Newline {
            self.take();
        }
        self.expect(T::Venn)?;
        loop {
            let statement = self.peek();
            match statement {
                T::Eof | T::End => return Ok(()),
                T::Newline => {
                    self.take();
                }
                T::Title => {
                    let text = self.take();
                    db.set_diagram_title(strip_keyword(&text));
                }
                T::Set => {
                    self.take();
                    let id = self.identifier()?;
                    self.subset(db, vec![id], false)?;
                }
                T::Union => {
                    self.take();
                    let ids = self.identifiers()?;
                    self.subset(db, ids, true)?;
                }
                T::Text => {
                    self.take();
                    let ids = self.identifiers()?;
                    self.text(db, &ids)?;
                }
                T::IndentText => {
                    self.take();
                    let sets = db
                        .current_sets()
                        .map(<[String]>::to_vec)
                        .ok_or_else(|| Error::new(ErrorCode::Semantic, "text requires set"))?;
                    self.text(db, &sets)?;
                }
                T::Style => {
                    self.take();
                    self.style(db)?;
                }
                _ => return Err(self.fail(STATEMENT)),
            }
        }
    }
}

/// The text of a title, without the word `title` and the space that follows it.
fn strip_keyword(text: &str) -> &str {
    // The keyword is five characters and the space one, so the title starts at the seventh.
    match text.char_indices().nth(6) {
        Some((at, _)) => &text[at..],
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Result<VennDb, Error> {
        let mut db = VennDb::default();
        parse_venn(src, &mut db)?;
        Ok(db)
    }

    #[test]
    fn reads_the_sets_and_their_sizes() {
        let db = parse("venn-beta\n  set A\n  set B:20\n").unwrap();
        assert_eq!(db.subsets.len(), 2);
        assert_eq!(db.subsets[0].sets, ["A"]);
        assert_eq!(db.subsets[0].size, 10.0);
        assert_eq!(db.subsets[1].size, 20.0);
    }

    #[test]
    fn reads_a_union() {
        let db = parse("venn-beta\n  set A\n  set B\n  union A,B[\"Both\"]\n").unwrap();
        assert_eq!(db.subsets.len(), 3);
        assert_eq!(db.subsets[2].sets, ["A", "B"]);
        assert_eq!(db.subsets[2].label.as_deref(), Some("Both"));
        assert_eq!(db.subsets[2].size, 2.5);
    }

    #[test]
    fn refuses_a_union_of_unknown_sets() {
        let err = parse("venn-beta\n  union A,B\n").unwrap_err();
        assert_eq!(err.code, ErrorCode::Semantic);
    }

    #[test]
    fn reads_a_title() {
        let db = parse("venn-beta\n  title What a good feature\n").unwrap();
        assert_eq!(db.title.as_deref(), Some("What a good feature"));
    }

    #[test]
    fn reads_an_indented_text() {
        let db = parse("venn-beta\n  set A\n    text A1\n").unwrap();
        assert_eq!(db.text_nodes.len(), 1);
        assert_eq!(db.text_nodes[0].sets, ["A"]);
        assert_eq!(db.text_nodes[0].id, "A1");
    }

    #[test]
    fn refuses_a_text_without_a_set() {
        let err = parse("venn-beta\n  text A1\n").unwrap_err();
        assert_eq!(err.code, ErrorCode::Syntax);
    }

    #[test]
    fn reads_the_styles() {
        let db = parse("venn-beta\n  set A\n  style A fill:#ff0000,stroke-width:2px\n").unwrap();
        assert_eq!(db.styles.len(), 1);
        assert_eq!(db.styles[0].targets, ["A"]);
        assert_eq!(
            db.styles[0].styles[0],
            ("fill".to_string(), "#ff0000".to_string())
        );
        assert_eq!(
            db.styles[0].styles[1],
            ("stroke-width".to_string(), "2 px".to_string())
        );
    }
}
