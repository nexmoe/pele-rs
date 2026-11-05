//! The parser of an information diagram, over the grammar Mermaid gives with Langium. Ported
//! from pele's `diagrams/info/parser.ts` and checked against Mermaid's own grammar,
//! `packages/parser/src/language/info/info.langium`.
//!
//! Mermaid's grammar is small:
//!
//! ```text
//! entry Info:
//!     NEWLINE*
//!     "info" NEWLINE*
//!     ("showInfo" NEWLINE*)?
//!     TitleAndAccessibilities?
//! ;
//! ```
//!
//! `TitleAndAccessibilities` is the `((accDescr | accTitle | title) EOL)+` of the common grammar
//! and `EOL` its `NEWLINE+ | EOF`. So the type reads the keyword, the optional `showInfo`, and
//! then any number of accessible texts and titles, each of them on a line of its own. The last
//! title of a diagram is the one it keeps, as in every other type.

use super::tokens::{END, Kind, Reader, acc_descr_value, acc_title_value, title_value, tokenize};
use crate::error::Error;

/// What the parser reads out of the text.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct InfoAst {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
}

/// The end of a line: a line break, or the end of the text.
fn end_of_line(r: &mut Reader<'_>) -> Result<(), Error> {
    if r.kind() == END {
        return Ok(());
    }
    if r.kind() != Kind::Newline {
        return Err(r.fail("a line break or the end of input"));
    }
    while r.kind() == Kind::Newline {
        r.i += 1;
    }
    Ok(())
}

/// The line breaks before the next thing.
fn newlines(r: &mut Reader<'_>) {
    while r.kind() == Kind::Newline {
        r.i += 1;
    }
}

/// Reads an information diagram out of the text.
pub(crate) fn parse_info(src: &str) -> Result<InfoAst, Error> {
    let tokens = tokenize(src)?;
    let mut r = Reader::new(tokens);
    let mut ast = InfoAst::default();

    newlines(&mut r);
    r.expect(Kind::Info)?;
    newlines(&mut r);
    if r.accept(Kind::ShowInfo) {
        newlines(&mut r);
    }

    while r.kind() != END {
        match r.kind() {
            Kind::AccDescr => {
                ast.acc_descr = Some(acc_descr_value(r.take()));
                end_of_line(&mut r)?;
            }
            Kind::AccTitle => {
                ast.acc_title = Some(acc_title_value(r.take()));
                end_of_line(&mut r)?;
            }
            Kind::Title => {
                ast.title = Some(title_value(r.take()));
                end_of_line(&mut r)?;
            }
            _ => {
                return Err(r.fail("a title, an accessible text, or the end of input"));
            }
        }
    }
    Ok(ast)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> InfoAst {
        parse_info(src).expect("the text is read")
    }

    #[test]
    fn reads_the_keyword() {
        assert_eq!(parse("info\n"), InfoAst::default());
        assert_eq!(parse("info showInfo\n"), InfoAst::default());
        // The keyword ends at a space, a comment or the end of the text.
        assert!(parse_info("infx\n").is_err());
        assert!(parse_info("infox\n").is_err());
        // `showInfo` is only read right after the keyword.
        assert!(parse_info("info\ntitle X\nshowInfo\n").is_err());
    }

    #[test]
    fn reads_the_titles() {
        let ast = parse("info\ntitle Hello\n");
        assert_eq!(ast.title.as_deref(), Some("Hello"));
        assert_eq!(ast.acc_title, None);
        assert_eq!(ast.acc_descr, None);

        // The last title of a diagram is the one it keeps.
        let ast = parse("info\ntitle One\ntitle Two\n");
        assert_eq!(ast.title.as_deref(), Some("Two"));
    }

    #[test]
    fn reads_the_accessible_text() {
        let ast = parse("info\naccTitle: A title\naccDescr: A description\n");
        assert_eq!(ast.acc_title.as_deref(), Some("A title"));
        assert_eq!(ast.acc_descr.as_deref(), Some("A description"));

        // A description may be written in braces, over several lines.
        let ast = parse("info\naccDescr {\n  one\n  two\n}\n");
        assert_eq!(ast.acc_descr.as_deref(), Some("one\ntwo"));
    }

    #[test]
    fn keeps_an_empty_title() {
        // Mermaid's own grammar reads a title with no text as an empty title, and not as no
        // title at all. pele drops it.
        let ast = parse("info\ntitle\n");
        assert_eq!(ast.title.as_deref(), Some(""));
    }

    #[test]
    fn rejects_what_it_does_not_read() {
        // A word that the grammar does not know.
        assert!(parse_info("info unsupported\n").is_err());
        // A `showInfo` that does not follow the keyword.
        assert!(parse_info("info\ntitle X\nshowInfo\n").is_err());
        // A description in braces that is never closed.
        assert!(parse_info("info\naccDescr {open\n").is_err());
        // An accessible title without its colon.
        assert!(parse_info("info\naccTitle X\n").is_err());
    }

    #[test]
    fn a_title_takes_the_rest_of_its_line() {
        // Mermaid's `title` terminal runs to the end of the line, so a word after the title is
        // part of it.
        let ast = parse("info title X showInfo\n");
        assert_eq!(ast.title.as_deref(), Some("X showInfo"));
    }
}
