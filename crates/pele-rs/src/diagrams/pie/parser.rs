//! Pie chart syntax, as Mermaid's Langium grammar defines it. Ported from pele's
//! `diagrams/pie/parser.ts`.

use crate::diagrams::common::tokens::{
    ACC_DESCR, ACC_TITLE, DIRECTIVE, END, NEWLINE, Pattern, Reader, SINGLE_LINE_COMMENT, STRING,
    Scanner, TITLE, TokenType, WHITESPACE, YAML, acc_descr_value, acc_title_value, keyword_token,
    literal_token, string_value, title_value, tokenize,
};
use crate::error::Error;

const SHOW_DATA: usize = 0;
const PIE: usize = 1;
const COLON: usize = 2;
const NUMBER: usize = 3;
const ACC_DESCR_T: usize = 4;
const ACC_TITLE_T: usize = 5;
const TITLE_T: usize = 6;
const STRING_T: usize = 7;
const NEWLINE_T: usize = 8;

fn digits(src: &[u8], mut at: usize) -> usize {
    while src.get(at).is_some_and(u8::is_ascii_digit) {
        at += 1;
    }
    at
}

/// Answers as NUMBER_PIE's pattern, `(?:-?[0-9]+\.[0-9]+(?!\.))|(?:-?(0|[1-9][0-9]*)(?!\.))`,
/// does. The pattern reads a whole run of digits before it finds there is no fraction, and would
/// do so again from every digit of a long run of zeros, so the run is remembered.
#[derive(Default)]
struct NumberScanner {
    from: usize,
    end: usize,
    fraction: Option<usize>,
}

impl Scanner for NumberScanner {
    fn scan(&mut self, src: &str, p: usize) -> Option<usize> {
        let b = src.as_bytes();
        let at = if b.get(p) == Some(&b'-') { p + 1 } else { p };
        if at < self.from || at >= self.end {
            self.from = at;
            self.end = digits(b, at);
            self.fraction = None;
            if self.end > at && b.get(self.end) == Some(&b'.') {
                let stop = digits(b, self.end + 1);
                let length = stop - self.end - 1;
                if length > 0 && b.get(stop) != Some(&b'.') {
                    self.fraction = Some(stop);
                } else if length > 1 {
                    self.fraction = Some(stop - 1);
                }
            }
        }
        if self.end == at {
            return None;
        }
        if let Some(fraction) = self.fraction {
            return Some(fraction);
        }
        if b[at] == b'0' {
            return (b.get(at + 1) != Some(&b'.')).then_some(at + 1);
        }
        if b.get(self.end) != Some(&b'.') {
            return Some(self.end);
        }
        (self.end - 1 > at).then_some(self.end - 1)
    }
}

fn number_scanner() -> Box<dyn Scanner> {
    Box::new(NumberScanner::default())
}

pub(crate) const PIE_TOKENS: &[TokenType] = &[
    keyword_token("showData"),
    keyword_token("pie"),
    literal_token(":"),
    TokenType {
        name: "NUMBER_PIE",
        pattern: Pattern::Scanner(number_scanner),
        hidden: false,
        longer: &[],
    },
    ACC_DESCR,
    ACC_TITLE,
    TITLE,
    STRING,
    NEWLINE,
    WHITESPACE,
    YAML,
    DIRECTIVE,
    SINGLE_LINE_COMMENT,
];

#[derive(Debug, Default, PartialEq)]
pub(crate) struct PieAst {
    pub show_data: bool,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub sections: Vec<(String, f64)>,
}

pub(crate) fn parse_pie(src: &str) -> Result<PieAst, Error> {
    let mut r = Reader::new(tokenize(src, PIE_TOKENS)?, PIE_TOKENS);
    let mut ast = PieAst::default();

    fn end_of_line(r: &mut Reader<'_>) -> Result<(), Error> {
        if r.kind() == END {
            return Ok(());
        }
        if r.kind() != NEWLINE_T {
            return Err(r.fail("a line break or the end of input"));
        }
        while r.kind() == NEWLINE_T {
            r.i += 1;
        }
        Ok(())
    }

    while r.kind() == NEWLINE_T {
        r.i += 1;
    }
    r.expect(PIE)?;
    ast.show_data = r.accept(SHOW_DATA);
    while r.kind() != END {
        match r.kind() {
            NEWLINE_T => r.i += 1,
            ACC_DESCR_T => {
                ast.acc_descr = Some(acc_descr_value(r.take()));
                end_of_line(&mut r)?;
            }
            ACC_TITLE_T => {
                ast.acc_title = Some(acc_title_value(r.take()));
                end_of_line(&mut r)?;
            }
            TITLE_T => {
                ast.title = Some(title_value(r.take()));
                end_of_line(&mut r)?;
            }
            STRING_T => {
                let label = string_value(r.take());
                r.expect(COLON)?;
                // A run of digits too long for a float reads as infinity, as `Number` does.
                let value = r.expect(NUMBER)?.parse().unwrap_or(f64::INFINITY);
                ast.sections.push((label, value));
                end_of_line(&mut r)?;
            }
            _ => return Err(r.fail("a section, a title, or a line break")),
        }
    }
    Ok(ast)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(src: &str) -> Option<usize> {
        NumberScanner::default().scan(src, 0)
    }

    #[test]
    fn reads_numbers_like_the_pattern() {
        assert_eq!(number("12"), Some(2));
        assert_eq!(number("-1.5"), Some(4));
        assert_eq!(number("0"), Some(1));
        assert_eq!(number("01"), Some(1));
        assert_eq!(number("0.5"), Some(3));
        assert_eq!(number("0."), None);
        assert_eq!(number("12."), Some(1));
        assert_eq!(number("1."), None);
        assert_eq!(number("1.2.3"), None);
        assert_eq!(number("1.23.4"), Some(3));
        assert_eq!(number(".5"), None);
        assert_eq!(number("-"), None);
    }

    #[test]
    fn parses_sections_and_headers() {
        let ast =
            parse_pie("pie showData title T %% c\n accTitle: a\n\"x\" : 1.5\n'y':2\n").unwrap();
        assert!(ast.show_data);
        assert_eq!(ast.title.as_deref(), Some("T"));
        assert_eq!(ast.acc_title.as_deref(), Some("a"));
        assert_eq!(
            ast.sections,
            vec![("x".to_string(), 1.5), ("y".to_string(), 2.0)]
        );
    }

    #[test]
    fn reports_positions() {
        let err = parse_pie("pie\n\"a\" 1\n").unwrap_err();
        assert_eq!((err.line, err.column), (2, 5));
        assert_eq!(err.message, "Expecting token of type ':' but found `1`.");
        let err = parse_pie("pie\n x\n").unwrap_err();
        assert_eq!((err.line, err.column), (2, 2));
        assert_eq!(err.message, "Unrecognized text.");
        let err = parse_pie("\"a\": 1").unwrap_err();
        assert_eq!(
            err.message,
            "Expecting token of type 'pie' but found `\"a\"`."
        );
    }
}
