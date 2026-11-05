//! The quadrantChart parser: Mermaid's jison grammar for `quadrant.jison`, as pele's
//! `parser.ts` emulates it.

use super::db::{QuadrantDb, QuadrantText};
use super::lexer::{T, tokenize};
use crate::error::{Error, ErrorCode};

/// The tokens a text may be made of.
fn is_word(t: T) -> bool {
    matches!(
        t,
        T::Punctuation
            | T::Amp
            | T::Num
            | T::Alpha
            | T::Comma
            | T::Plus
            | T::Equals
            | T::Mult
            | T::Dot
            | T::Brkt
            | T::Underscore
            | T::UnicodeText
    )
}

/// The tokens a name may be made of.
fn is_id(t: T) -> bool {
    matches!(
        t,
        T::Alpha
            | T::Num
            | T::Minus
            | T::Comma
            | T::Colon
            | T::Amp
            | T::Brkt
            | T::Mult
            | T::UnicodeText
    )
}

/// The tokens a style may be made of.
fn is_style(t: T) -> bool {
    matches!(
        t,
        T::Alpha | T::Num | T::Colon | T::Space | T::Brkt | T::Minus
    )
}

/// Whether a token ends a statement.
fn is_end(t: T) -> bool {
    matches!(t, T::Newline | T::Semi | T::Eof)
}

/// Whether a token may start a text.
fn is_text(t: T) -> bool {
    t == T::Str || t == T::MdStr || is_word(t)
}

/// The tokens the parser names when a text does not start.
const TEXT_TOKENS: &str = "'STR', 'MD_STR', 'ALPHA', 'NUM', 'UNICODE_TEXT', 'PUNCTUATION'";

struct Parser<'a> {
    src: &'a str,
    types: Vec<T>,
    texts: Vec<String>,
    starts: Vec<usize>,
    /// The byte offset of each UTF-16 code unit, and one for the end of the text.
    offsets: Vec<usize>,
    i: usize,
}

impl Parser<'_> {
    fn t(&self) -> T {
        self.types.get(self.i).copied().unwrap_or(T::End)
    }

    fn fail<X>(&self, expected: &str) -> Result<X, Error> {
        let at = self.offsets[self.starts[self.i]];
        if self.t() == T::Error {
            return Err(Error::new(ErrorCode::Syntax, "Unrecognized text.").at(self.src, at));
        }
        Err(Error::new(
            ErrorCode::Syntax,
            format!("Expecting {expected}, got '{}'", self.t().name()),
        )
        .at(self.src, at))
    }

    fn expect(&mut self, t: T) -> Result<String, Error> {
        if self.t() != t {
            return self.fail(&format!("'{}'", t.name()));
        }
        let text = self.texts[self.i].clone();
        self.i += 1;
        Ok(text)
    }

    /// A text, which the grammar reads as one token or as several that follow each other.
    fn text(&mut self) -> Result<QuadrantText, Error> {
        let first = self.t();
        if !is_text(first) {
            return self.fail(TEXT_TOKENS);
        }
        let mut text = self.texts[self.i].clone();
        self.i += 1;
        while is_word(self.t()) || self.t() == T::Space || self.t() == T::Minus {
            text.push_str(&self.texts[self.i]);
            self.i += 1;
        }
        Ok(QuadrantText {
            text,
            markdown: first == T::MdStr,
        })
    }

    /// The styles of a point or of a class, which are separated by commas.
    fn styles(&mut self) -> Result<Vec<String>, Error> {
        let mut out = Vec::new();
        loop {
            if !is_style(self.t()) {
                return self.fail("'ALPHA', 'NUM', 'COLON', 'SPACE', 'BRKT', 'MINUS'");
            }
            let mut style = self.texts[self.i].clone();
            self.i += 1;
            while is_style(self.t()) {
                style.push_str(&self.texts[self.i]);
                self.i += 1;
            }
            out.push(style.trim().to_string());
            if self.t() != T::Comma {
                return Ok(out);
            }
            self.i += 1;
        }
    }

    fn parse(&mut self, db: &mut QuadrantDb) -> Result<(), Error> {
        while is_end(self.t()) || self.t() == T::Space {
            self.i += 1;
        }
        self.expect(T::Quadrant)?;

        while self.t() != T::End {
            while self.t() == T::Space {
                self.i += 1;
            }
            let t = self.t();
            match t {
                T::Classdef => {
                    self.i += 1;
                    self.expect(T::Space)?;
                    if !is_id(self.t()) {
                        return self.fail("'ALPHA', 'NUM', 'UNICODE_TEXT'");
                    }
                    let mut id = self.texts[self.i].clone();
                    self.i += 1;
                    while is_id(self.t()) {
                        id.push_str(&self.texts[self.i]);
                        self.i += 1;
                    }
                    self.expect(T::Space)?;
                    let styles = self.styles()?;
                    db.add_class(&id, &styles)?;
                }
                T::XAxis | T::YAxis => {
                    self.i += 1;
                    let mut from = self.text()?;
                    let mut to: Option<QuadrantText> = None;
                    if self.t() == T::Delimiter {
                        self.i += 1;
                        if is_text(self.t()) {
                            to = Some(self.text()?);
                        } else {
                            from.text.push_str(" \u{27f6} ");
                        }
                    }
                    if t == T::XAxis {
                        db.set_x_axis_left_text(&from);
                        if let Some(to) = &to {
                            db.set_x_axis_right_text(to);
                        }
                    } else {
                        db.set_y_axis_bottom_text(&from);
                        if let Some(to) = &to {
                            db.set_y_axis_top_text(to);
                        }
                    }
                }
                T::Quadrant1 | T::Quadrant2 | T::Quadrant3 | T::Quadrant4 => {
                    self.i += 1;
                    let label = self.text()?;
                    let quadrant = match t {
                        T::Quadrant1 => 0,
                        T::Quadrant2 => 1,
                        T::Quadrant3 => 2,
                        _ => 3,
                    };
                    db.set_quadrant_text(quadrant, &label);
                }
                T::Title => {
                    self.i += 1;
                    let title = self.expect(T::TitleValue)?;
                    db.set_diagram_title(title.trim());
                }
                T::AccTitle => {
                    self.i += 1;
                    let title = self.expect(T::AccTitleValue)?;
                    db.set_acc_title(title.trim());
                }
                T::AccDescr => {
                    self.i += 1;
                    let descr = self.expect(T::AccDescrValue)?;
                    db.set_acc_description(descr.trim());
                }
                T::AccDescrMultilineValue => {
                    let descr = self.texts[self.i].clone();
                    self.i += 1;
                    db.set_acc_description(descr.trim());
                }
                _ => {
                    if is_text(t) {
                        let label = self.text()?;
                        let class_name = if self.t() == T::ClassName {
                            let name = self.texts[self.i].clone();
                            self.i += 1;
                            name
                        } else {
                            String::new()
                        };
                        self.expect(T::PointStart)?;
                        let x = self.expect(T::PointX)?;
                        let y = self.expect(T::PointY)?;
                        let styles = if is_style(self.t()) {
                            self.styles()?
                        } else {
                            Vec::new()
                        };
                        db.add_point(&label, &class_name, &x, &y, &styles)?;
                    }
                }
            }
            if !is_end(self.t()) {
                return self.fail("'NEWLINE', 'SEMI', 'EOF'");
            }
            self.i += 1;
        }
        Ok(())
    }
}

/// Reads a quadrant chart, as Mermaid's parser does.
pub(crate) fn parse_quadrant(src: &str, db: &mut QuadrantDb) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut parser = Parser {
        src,
        types: tokens.types,
        texts: tokens.texts,
        starts: tokens.starts,
        offsets: crate::diagrams::common::chars::byte_offsets(src),
        i: 0,
    };
    parser.parse(db)
}
