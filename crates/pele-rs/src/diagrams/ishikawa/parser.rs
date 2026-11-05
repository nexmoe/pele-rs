//! The parser of an Ishikawa diagram: the grammar Mermaid gives with `ishikawa.jison`, read
//! the way its LALR automaton reads it.
//!
//! The grammar is odd in two ways, and Mermaid reads it as it is: a blank line right after the
//! keyword is a statement of its own, and a statement must be ended by a blank line, a line
//! break or the end of the text, so a blank line between the keyword and the effect is refused.
//! pele's parser reads the same grammar the same way.

use super::lexer::{T, Tokens, tokenize};
use crate::error::{Error, ErrorCode};
use crate::util::trim;

/// A line of the diagram: its text and the level it is indented by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line {
    /// How far the line is indented, in characters.
    pub level: usize,
    /// The text of the line, without the white space around it.
    pub text: String,
}

/// The lines of an Ishikawa diagram, in the order they are read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct IshikawaAst {
    pub lines: Vec<Line>,
}

/// Reads the lines of an Ishikawa diagram.
pub(crate) fn parse_ishikawa(src: &str) -> Result<IshikawaAst, Error> {
    Parser {
        src,
        tokens: tokenize(src),
        i: 0,
    }
    .parse()
}

struct Parser<'a> {
    src: &'a str,
    tokens: Tokens,
    /// The token that is read next.
    i: usize,
}

impl Parser<'_> {
    fn kind(&self) -> T {
        self.tokens.kinds.get(self.i).copied().unwrap_or(T::Eof)
    }

    /// The text of the token that is read next.
    fn text(&self) -> &str {
        match self.tokens.kinds.get(self.i) {
            Some(_) => &self.src[self.tokens.starts[self.i]..self.tokens.ends[self.i]],
            None => "",
        }
    }

    fn fail(&self, expected: &str) -> Error {
        Error::new(
            ErrorCode::Syntax,
            format!("Expecting {expected}, got '{}'", self.kind().name()),
        )
        .at(
            self.src,
            self.tokens
                .starts
                .get(self.i)
                .copied()
                .unwrap_or(self.src.len()),
        )
    }

    fn parse(&mut self) -> Result<IshikawaAst, Error> {
        // `start: ishikawa | spaceLines ishikawa`
        if self.kind() == T::SpaceLine {
            // `spaceLines: SPACELINE | spaceLines SPACELINE | spaceLines NL`
            self.i += 1;
            while self.kind() == T::SpaceLine || self.kind() == T::Nl {
                self.i += 1;
            }
            if self.kind() != T::Ishikawa {
                return Err(self.fail("'SPACELINE', 'NL', 'ISHIKAWA'"));
            }
        } else if self.kind() != T::Ishikawa {
            return Err(self.fail("'SPACELINE', 'ISHIKAWA'"));
        }
        self.i += 1;
        // `ishikawa: ISHIKAWA document | ISHIKAWA NL document`
        if self.kind() == T::Nl {
            self.i += 1;
        }
        // `document: document statement stop | statement stop`
        let mut lines = Vec::new();
        loop {
            self.statement(&mut lines)?;
            self.stop()?;
            if self.kind() == T::Eof {
                break;
            }
        }
        Ok(IshikawaAst { lines })
    }

    /// `statement: SPACELIST TEXT | TEXT | SPACELINE | SPACELIST`
    fn statement(&mut self, lines: &mut Vec<Line>) -> Result<(), Error> {
        match self.kind() {
            T::SpaceList => {
                // The level of a line is the length of its indentation, as the grammar reads it.
                let start = self.tokens.starts[self.i];
                let end = self.tokens.ends[self.i];
                let level = self.src[start..end].chars().count();
                // `statement: SPACELIST TEXT`
                if self.tokens.kinds.get(self.i + 1) == Some(&T::Text) {
                    let text = trim(self.text_of(self.i + 1)).to_string();
                    self.i += 2;
                    lines.push(Line { level, text });
                    return Ok(());
                }
                // `statement: SPACELIST`, which a line of text may follow.
                self.i += 1;
                Ok(())
            }
            // `statement: TEXT`
            T::Text => {
                let text = trim(self.text()).to_string();
                self.i += 1;
                lines.push(Line { level: 0, text });
                Ok(())
            }
            // `statement: SPACELINE`
            T::SpaceLine => {
                self.i += 1;
                Ok(())
            }
            _ => Err(self.fail("'SPACELINE', 'SPACELIST', 'TEXT'")),
        }
    }

    /// `stop: NL | EOF | SPACELINE | stop NL | stop EOF`
    fn stop(&mut self) -> Result<(), Error> {
        match self.kind() {
            T::Nl | T::SpaceLine => self.i += 1,
            // The end of the text ends the document as well.
            T::Eof => {}
            _ => return Err(self.fail("'SPACELINE', 'NL', 'EOF'")),
        }
        while self.kind() == T::Nl {
            self.i += 1;
        }
        Ok(())
    }

    /// The text of the token at `i`.
    fn text_of(&self, i: usize) -> &str {
        &self.src[self.tokens.starts[i]..self.tokens.ends[i]]
    }
}
