//! The xychart parser: Mermaid's jison grammar for `xychart.jison`, as pele's `parser.ts`
//! emulates it.

use super::db::{DataPoint, XyChartDb, XyText};
use super::lexer::{T, tokenize};
use crate::error::{Error, ErrorCode};
use crate::util::js_to_number;

/// Whether a token is one of those a text may be made of.
fn is_text(t: T) -> bool {
    matches!(
        t,
        T::Str
            | T::Amp
            | T::Num
            | T::Alpha
            | T::Plus
            | T::Equals
            | T::Mult
            | T::Dot
            | T::Brkt
            | T::Minus
            | T::Underscore
    )
}

/// Whether a token ends a statement.
fn is_end(t: T) -> bool {
    matches!(t, T::Newline | T::Semi | T::Eof)
}

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
    fn text(&mut self) -> Result<XyText, Error> {
        if !is_text(self.t()) {
            return self.fail("'STR', 'ALPHA', 'NUM'");
        }
        let mut text = self.texts[self.i].clone();
        self.i += 1;
        if self.types[self.i - 1] != T::Str {
            while is_text(self.t()) && self.t() != T::Str {
                text.push_str(&self.texts[self.i]);
                self.i += 1;
            }
        }
        Ok(XyText { text })
    }

    /// The two ends of a range of numbers.
    fn range(&mut self) -> Result<(f64, f64), Error> {
        let min = js_to_number(&self.expect(T::NumberWithDecimal)?);
        self.expect(T::ArrowDelimiter)?;
        let max = js_to_number(&self.expect(T::NumberWithDecimal)?);
        Ok((min, max))
    }

    /// The values of a series, with the labels that stand beside them.
    fn points(&mut self) -> Result<Vec<DataPoint>, Error> {
        let mut out = Vec::new();
        self.expect(T::SquareBracesStart)?;
        loop {
            let value = js_to_number(&self.expect(T::NumberWithDecimal)?);
            let label = if self.t() == T::Str {
                let label = self.texts[self.i].clone();
                self.i += 1;
                label
            } else {
                String::new()
            };
            out.push(DataPoint { value, label });
            if self.t() != T::Comma {
                break;
            }
            self.i += 1;
        }
        self.expect(T::SquareBracesEnd)?;
        Ok(out)
    }

    fn parse(&mut self, db: &mut XyChartDb) -> Result<(), Error> {
        // The header: the keyword, and the orientation that may follow it.
        loop {
            if is_end(self.t()) {
                self.i += 1;
            } else if self.t() != T::Xychart {
                break;
            } else {
                self.i += 1;
                if self.t() == T::ChartOrientation {
                    let orientation = self.texts[self.i].clone();
                    self.i += 1;
                    db.set_orientation(&orientation);
                }
            }
        }

        while self.t() != T::End {
            let t = self.types[self.i];
            self.i += 1;
            match t {
                T::Title => {
                    let title = self.text()?.text;
                    db.set_diagram_title(title.trim());
                }
                T::XAxis | T::YAxis => {
                    let title = if is_text(self.t()) {
                        Some(self.text()?)
                    } else {
                        None
                    };
                    if t == T::XAxis && self.t() == T::SquareBracesStart {
                        let mut categories = Vec::new();
                        loop {
                            self.i += 1;
                            categories.push(self.text()?);
                            if self.t() != T::Comma {
                                break;
                            }
                        }
                        self.expect(T::SquareBracesEnd)?;
                        db.set_x_axis_band(categories);
                    } else if self.t() == T::NumberWithDecimal || title.is_none() {
                        let (min, max) = self.range()?;
                        if t == T::XAxis {
                            db.set_x_axis_range_data(min, max);
                        } else {
                            db.set_y_axis_range_data(min, max);
                        }
                    }
                    let title = title.unwrap_or_default();
                    if t == T::XAxis {
                        db.set_x_axis_title(&title);
                    } else {
                        db.set_y_axis_title(&title);
                    }
                }
                T::Line | T::Bar => {
                    let title = if self.t() == T::SquareBracesStart {
                        XyText::default()
                    } else {
                        self.text()?
                    };
                    let data = self.points()?;
                    if t == T::Line {
                        db.set_line_data(&title, &data);
                    } else {
                        db.set_bar_data(&title, &data);
                    }
                }
                T::AccTitle => {
                    let text = self.expect(T::AccTitleValue)?;
                    db.set_acc_title(text.trim());
                }
                T::AccDescr => {
                    let text = self.expect(T::AccDescrValue)?;
                    db.set_acc_description(text.trim());
                }
                T::AccDescrMultilineValue => {
                    let text = self.texts[self.i - 1].clone();
                    db.set_acc_description(text.trim());
                }
                _ => {
                    self.i -= 1;
                    return self.fail(
                        "'title', 'X_AXIS', 'Y_AXIS', 'LINE', 'BAR', 'acc_title', 'acc_descr', \
                         'acc_descr_multiline_value'",
                    );
                }
            }
            while is_end(self.t()) {
                self.i += 1;
            }
        }
        Ok(())
    }
}

/// Reads an XY chart, as Mermaid's parser does.
pub(crate) fn parse_xychart(src: &str, db: &mut XyChartDb) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut parser = Parser {
        src,
        types: tokens.types,
        texts: tokens.texts,
        starts: tokens.starts,
        offsets: tokens.offsets,
        i: 0,
    };
    parser.parse(db)
}
