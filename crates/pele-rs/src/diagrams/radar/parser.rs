//! The tokens and the parser of a radar chart, over the grammar Mermaid gives with Langium.
//! Ported from pele's `diagrams/radar/parser.ts` and checked against Mermaid's own grammar,
//! `packages/parser/src/language/radar/radar.langium`.
//!
//! Mermaid is the specification. The two read the text the same way: the values of a curve are
//! either a list of numbers or a list of `axis: value` pairs, and never a mix of the two in one
//! curve. Mermaid's grammar tries the numbers first and falls back to the pairs; here the first
//! value decides which of the two it is, and every other value has to be of that kind too.

use crate::diagrams::common::tokens::Scanner;
use crate::diagrams::common::tokens::{
    ACC_DESCR, ACC_TITLE, DIRECTIVE, END, Found, NEWLINE, Pattern, Reader, SINGLE_LINE_COMMENT,
    STRING, TITLE, TokenType, WHITESPACE, YAML, acc_descr_value, acc_title_value,
    keyword_token_with, literal_token, match_token, string_value, title_value, tokenize,
    word_token,
};
use crate::error::Error;
use crate::util::is_word;

// The kinds of the tokens, in the order they are tried, as in the grammar.
const RADAR_COLON: usize = 0;
const RADAR: usize = 1;
const SHOW_LEGEND: usize = 2;
const GRATICULE: usize = 3;
const CURVE: usize = 4;
const TICKS: usize = 5;
const AXIS: usize = 6;
const MAX: usize = 7;
const MIN: usize = 8;
const COLON: usize = 9;
const COMMA: usize = 10;
const OPEN: usize = 11;
const CLOSE: usize = 12;
const OPEN_BRACE: usize = 13;
const CLOSE_BRACE: usize = 14;
const GRATICULE_VALUE: usize = 15;
const BOOLEAN: usize = 16;
const ACC_DESCR_KIND: usize = 17;
const ACC_TITLE_KIND: usize = 18;
const TITLE_KIND: usize = 19;
const NUMBER: usize = 20;
const STRING_KIND: usize = 21;
const ID: usize = 22;
const NEWLINE_KIND: usize = 23;
// The tokens after it are hidden: whitespace, the front matter, the directives and the comments.

/// The kinds of the tokens that may follow a word, so that `axes` is a name and not the word
/// `axis`.
const NAME: &[usize] = &[ID];

static TOKENS: [TokenType; 28] = [
    literal_token("radar-beta:"),
    keyword_token_with("radar-beta", NAME),
    word_token("showLegend", NAME),
    word_token("graticule", NAME),
    word_token("curve", NAME),
    word_token("ticks", NAME),
    word_token("axis", NAME),
    word_token("max", NAME),
    word_token("min", NAME),
    literal_token(":"),
    literal_token(","),
    literal_token("["),
    literal_token("]"),
    literal_token("{"),
    literal_token("}"),
    match_token("GRATICULE", graticule),
    match_token("BOOLEAN", boolean),
    ACC_DESCR,
    ACC_TITLE,
    TITLE,
    TokenType {
        name: "NUMBER",
        pattern: Pattern::Scanner(new_numbers),
        hidden: false,
        longer: &[],
    },
    STRING,
    match_token("ID", id),
    NEWLINE,
    WHITESPACE,
    YAML,
    DIRECTIVE,
    SINGLE_LINE_COMMENT,
];

/// `circle|polygon`: the shape of the grid of a radar chart.
fn graticule(src: &str, at: usize) -> Found {
    for word in ["circle", "polygon"] {
        if src[at..].starts_with(word) {
            return Found::End(at + word.len());
        }
    }
    Found::No
}

/// `true|false`
fn boolean(src: &str, at: usize) -> Found {
    for word in ["true", "false"] {
        if src[at..].starts_with(word) {
            return Found::End(at + word.len());
        }
    }
    Found::No
}

/// `\w([-\w]*\w)?`: a name of letters, digits and underscores, which may hold dashes but may not
/// end with one.
fn id(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    let Some(&first) = bytes.get(at) else {
        return Found::No;
    };
    if !is_word_byte(first) {
        return Found::No;
    }
    let mut end = at + 1;
    while matches!(
        bytes.get(end),
        Some(b'-') | Some(b'_') | Some(b'0'..=b'9') | Some(b'a'..=b'z') | Some(b'A'..=b'Z')
    ) {
        end += 1;
    }
    // The name ends at the last character that may end it, a letter or a digit.
    while end > at + 1 && !is_word_byte(bytes[end - 1]) {
        end -= 1;
    }
    Found::End(end)
}

fn is_word_byte(b: u8) -> bool {
    is_word(b as char)
}

/// The end of the run of digits that starts at `at`.
fn digits(src: &str, at: usize) -> usize {
    let bytes = src.as_bytes();
    let mut end = at;
    while matches!(bytes.get(end), Some(b'0'..=b'9')) {
        end += 1;
    }
    end
}

/// The numbers of a radar chart, as Mermaid's grammar reads them: a float is a run of digits, a
/// dot and a run of digits, and an integer is a zero or a run of digits that starts with a digit
/// other than a zero. Neither may be followed by a dot.
struct Numbers {
    run_from: usize,
    run_end: usize,
}

impl Scanner for Numbers {
    fn scan(&mut self, src: &str, at: usize) -> Option<usize> {
        if at < self.run_from || at >= self.run_end {
            self.run_from = at;
            self.run_end = digits(src, at);
        }
        let whole = self.run_end;
        if whole == at {
            return None;
        }
        let bytes = src.as_bytes();
        let dot = bytes.get(whole) == Some(&b'.');
        if dot {
            let fraction = digits(src, whole + 1);
            let length = fraction - whole - 1;
            // The float takes the whole fraction, unless a dot follows it, and then it gives up
            // the last digit of it.
            if length > 0 && bytes.get(fraction) != Some(&b'.') {
                return Some(fraction);
            }
            if length > 1 {
                return Some(fraction - 1);
            }
        }
        if bytes[at] == b'0' {
            return Some(at + 1);
        }
        if !dot {
            return Some(whole);
        }
        // The integer gives up its last digit, so that it does not run into the dot.
        if whole - 1 > at {
            return Some(whole - 1);
        }
        None
    }
}

fn new_numbers() -> Box<dyn Scanner> {
    Box::new(Numbers {
        run_from: 0,
        run_end: 0,
    })
}

/// The name of an option of a radar chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OptionName {
    ShowLegend,
    Graticule,
    Ticks,
    Max,
    Min,
}

/// The value of an option of a radar chart.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OptionValue {
    Bool(bool),
    Graticule(String),
    Number(f64),
}

/// An axis of a radar chart.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RadarAxis {
    pub name: String,
    pub label: Option<String>,
}

/// One value of a curve of a radar chart.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RadarEntry {
    /// The axis the value belongs to, when the curve names its axes.
    pub axis: Option<String>,
    pub value: f64,
}

/// A curve of a radar chart.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RadarCurve {
    pub name: String,
    pub label: Option<String>,
    pub entries: Vec<RadarEntry>,
}

/// An option of a radar chart.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RadarOption {
    pub name: OptionName,
    pub value: OptionValue,
}

/// What the parser reads out of the text.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RadarAst {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub axes: Vec<RadarAxis>,
    pub curves: Vec<RadarCurve>,
    pub options: Vec<RadarOption>,
}

/// The end of a line: a line break, or the end of the text.
fn end_of_line(r: &mut Reader<'_>) -> Result<(), Error> {
    if r.kind() == END {
        return Ok(());
    }
    if r.kind() != NEWLINE_KIND {
        return Err(r.fail("a line break or the end of input"));
    }
    while r.kind() == NEWLINE_KIND {
        r.i += 1;
    }
    Ok(())
}

/// The line breaks before the next thing.
fn newlines(r: &mut Reader<'_>) {
    while r.kind() == NEWLINE_KIND {
        r.i += 1;
    }
}

/// A label in brackets: `["text"]`.
fn label(r: &mut Reader<'_>) -> Result<Option<String>, Error> {
    if !r.accept(OPEN) {
        return Ok(None);
    }
    let text = r.expect(STRING_KIND)?;
    r.expect(CLOSE)?;
    Ok(Some(string_value(text)))
}

/// An axis: a name and, at most, a label.
fn axis(r: &mut Reader<'_>) -> Result<RadarAxis, Error> {
    let name = r.expect(ID)?.to_string();
    let label = label(r)?;
    Ok(RadarAxis { name, label })
}

/// One value of a curve: a number, or a number that names its axis.
fn entry(r: &mut Reader<'_>, named: bool) -> Result<RadarEntry, Error> {
    let axis = if named {
        let name = r.expect(ID)?.to_string();
        r.accept(COLON);
        Some(name)
    } else {
        None
    };
    let value = r.expect(NUMBER)?;
    Ok(RadarEntry {
        axis,
        value: value.parse::<f64>().unwrap_or(0.0),
    })
}

/// The values of a curve. They are either all numbers, in the order of the axes, or all name the
/// axis they belong to.
fn entries(r: &mut Reader<'_>) -> Result<Vec<RadarEntry>, Error> {
    newlines(r);
    let named = r.kind() == ID;
    let mut entries = vec![entry(r, named)?];
    while r.accept(COMMA) {
        newlines(r);
        entries.push(entry(r, named)?);
    }
    newlines(r);
    Ok(entries)
}

/// A curve: a name, at most a label, and its values.
fn curve(r: &mut Reader<'_>) -> Result<RadarCurve, Error> {
    let name = r.expect(ID)?.to_string();
    let label = label(r)?;
    r.expect(OPEN_BRACE)?;
    let entries = entries(r)?;
    r.expect(CLOSE_BRACE)?;
    Ok(RadarCurve {
        name,
        label,
        entries,
    })
}

/// An option of the chart.
fn option(r: &mut Reader<'_>) -> Result<RadarOption, Error> {
    let kind = r.kind();
    r.i += 1;
    let value = match kind {
        SHOW_LEGEND => {
            let text = r.expect(BOOLEAN)?;
            OptionValue::Bool(text == "true")
        }
        GRATICULE => OptionValue::Graticule(r.expect(GRATICULE_VALUE)?.to_string()),
        _ => {
            let text = r.expect(NUMBER)?;
            OptionValue::Number(text.parse::<f64>().unwrap_or(0.0))
        }
    };
    let name = match kind {
        SHOW_LEGEND => OptionName::ShowLegend,
        GRATICULE => OptionName::Graticule,
        TICKS => OptionName::Ticks,
        MAX => OptionName::Max,
        _ => OptionName::Min,
    };
    Ok(RadarOption { name, value })
}

/// Reads a radar chart out of the text.
pub(crate) fn parse_radar(src: &str) -> Result<RadarAst, Error> {
    let tokens = tokenize(src, &TOKENS)?;
    let mut r = Reader::new(tokens, &TOKENS);
    let mut ast = RadarAst::default();

    newlines(&mut r);
    if !r.accept(RADAR_COLON) {
        r.expect(RADAR)?;
        r.accept(COLON);
    }
    newlines(&mut r);

    while r.kind() != END {
        match r.kind() {
            NEWLINE_KIND => r.i += 1,
            ACC_DESCR_KIND => {
                ast.acc_descr = Some(acc_descr_value(r.take()));
                end_of_line(&mut r)?;
            }
            ACC_TITLE_KIND => {
                ast.acc_title = Some(acc_title_value(r.take()));
                end_of_line(&mut r)?;
            }
            TITLE_KIND => {
                ast.title = Some(title_value(r.take()));
                end_of_line(&mut r)?;
            }
            AXIS => {
                r.i += 1;
                loop {
                    ast.axes.push(axis(&mut r)?);
                    if !r.accept(COMMA) {
                        break;
                    }
                }
            }
            CURVE => {
                r.i += 1;
                loop {
                    ast.curves.push(curve(&mut r)?);
                    if !r.accept(COMMA) {
                        break;
                    }
                }
            }
            SHOW_LEGEND | GRATICULE | TICKS | MAX | MIN => {
                ast.options.push(option(&mut r)?);
                while r.accept(COMMA) {
                    ast.options.push(option(&mut r)?);
                }
            }
            _ => {
                return Err(r.fail("an axis, a curve, an option, a title, or a line break"));
            }
        }
    }
    Ok(ast)
}
