//! The tokens and the parser of a git graph, over the grammar Mermaid gives with Langium.
//! Ported from pele's `diagrams/git/parser.ts`.

use crate::diagrams::common::tokens::{
    ACC_DESCR, ACC_TITLE, DIRECTIVE, END, Found, NEWLINE, Reader, SINGLE_LINE_COMMENT, STRING,
    TITLE, WHITESPACE, YAML, acc_descr_value, acc_title_value, keyword_token_with, literal_token,
    match_token, string_value, title_value, tokenize, word_token,
};
use crate::error::Error;
use crate::util::is_word;

// The kinds of the tokens, in the order they are tried, as in the grammar.
const CHERRY_PICK: usize = 0;
const GIT_GRAPH_COLON: usize = 1;
const HIGHLIGHT: usize = 2;
const GIT_GRAPH: usize = 3;
const CHECKOUT: usize = 4;
const REVERSE: usize = 5;
const PARENT: usize = 6;
const COMMIT: usize = 7;
const NORMAL: usize = 8;
const BRANCH: usize = 9;
const ORDER: usize = 10;
const SWITCH: usize = 11;
const TYPE: usize = 12;
const MERGE: usize = 13;
const MSG: usize = 14;
const TAG: usize = 15;
const ID: usize = 16;
const LR: usize = 17;
const TB: usize = 18;
const BT: usize = 19;
const COLON: usize = 20;
const ACC_DESCR_KIND: usize = 21;
const ACC_TITLE_KIND: usize = 22;
const TITLE_KIND: usize = 23;
const INT: usize = 24;
const STRING_KIND: usize = 25;
const NEWLINE_KIND: usize = 26;
const REFERENCE: usize = 31;

/// The kinds of the tokens that may follow a word, so that `commits` is a name and not the
/// word `commit`.
const NAME: &[usize] = &[REFERENCE];

static TOKENS: [crate::diagrams::common::tokens::TokenType; 32] = [
    word_token("cherry-pick", NAME),
    literal_token("gitGraph:"),
    word_token("HIGHLIGHT", NAME),
    keyword_token_with("gitGraph", NAME),
    word_token("checkout", NAME),
    word_token("REVERSE", NAME),
    literal_token("parent:"),
    word_token("commit", NAME),
    word_token("NORMAL", NAME),
    word_token("branch", NAME),
    literal_token("order:"),
    word_token("switch", NAME),
    literal_token("type:"),
    word_token("merge", NAME),
    literal_token("msg:"),
    literal_token("tag:"),
    literal_token("id:"),
    word_token("LR", NAME),
    word_token("TB", NAME),
    word_token("BT", NAME),
    literal_token(":"),
    ACC_DESCR,
    ACC_TITLE,
    TITLE,
    match_token("INT", int),
    STRING,
    NEWLINE,
    WHITESPACE,
    YAML,
    DIRECTIVE,
    SINGLE_LINE_COMMENT,
    match_token("REFERENCE", reference),
];

/// `0|[1-9][0-9]*(?!\.)`: a number without a leading zero, and not the start of a decimal.
fn int(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    match bytes.get(at) {
        Some(b'0') => Found::End(at + 1),
        Some(b'1'..=b'9') => {
            let mut end = at + 1;
            while matches!(bytes.get(end), Some(b'0'..=b'9')) {
                end += 1;
            }
            if bytes.get(end) == Some(&b'.') {
                Found::No
            } else {
                Found::End(end)
            }
        }
        _ => Found::No,
    }
}

/// `\w([-\./\w]*[-\w])?`: a name of letters, digits, underscores, dashes, dots and slashes,
/// which may not end with a dot or a slash.
fn reference(src: &str, at: usize) -> Found {
    let bytes = src.as_bytes();
    let Some(&first) = bytes.get(at) else {
        return Found::No;
    };
    if !is_word_byte(first) {
        return Found::No;
    }
    let rest = &bytes[at + 1..];
    let run = rest.iter().take_while(|&&b| is_name_byte(b)).count();
    // The name ends at the last character that may end it: a dash or a letter.
    for i in (0..run).rev() {
        if rest[i] == b'-' || is_word_byte(rest[i]) {
            return Found::End(at + i + 2);
        }
    }
    Found::End(at + 1)
}

fn is_word_byte(b: u8) -> bool {
    is_word(b as char)
}

fn is_name_byte(b: u8) -> bool {
    is_word_byte(b) || matches!(b, b'-' | b'.' | b'/')
}

/// The kind of a commit a statement gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommitType {
    Normal,
    Reverse,
    Highlight,
}

/// The way the history is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    LeftToRight,
    TopToBottom,
    BottomToTop,
}

/// A statement of a git graph.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Statement {
    Commit {
        id: Option<String>,
        message: Option<String>,
        tags: Vec<String>,
        kind: Option<CommitType>,
    },
    Branch {
        name: String,
        order: Option<f64>,
    },
    Merge {
        branch: String,
        id: Option<String>,
        tags: Vec<String>,
        kind: Option<CommitType>,
    },
    Checkout {
        branch: String,
    },
    CherryPicking {
        id: Option<String>,
        tags: Vec<String>,
        parent: Option<String>,
    },
}

/// What the parser reads out of the text.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct GitAst {
    pub direction: Option<Direction>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub statements: Vec<Statement>,
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

/// The name of a branch: a name or a string.
fn name(r: &mut Reader<'_>) -> Result<String, Error> {
    if r.kind() == REFERENCE {
        return Ok(r.take().to_string());
    }
    if r.kind() == STRING_KIND {
        return Ok(string_value(r.take()));
    }
    Err(r.fail("a branch name"))
}

/// A string of a statement.
fn string(r: &mut Reader<'_>) -> Result<String, Error> {
    let text = r.expect(STRING_KIND)?;
    Ok(string_value(text))
}

/// The kind of a commit a statement gives it.
fn commit_type(r: &mut Reader<'_>) -> Result<CommitType, Error> {
    match r.kind() {
        NORMAL => {
            r.i += 1;
            Ok(CommitType::Normal)
        }
        REVERSE => {
            r.i += 1;
            Ok(CommitType::Reverse)
        }
        HIGHLIGHT => {
            r.i += 1;
            Ok(CommitType::Highlight)
        }
        _ => Err(r.fail("NORMAL, REVERSE, or HIGHLIGHT")),
    }
}

/// Reads a git graph out of the text.
pub(crate) fn parse_git(src: &str) -> Result<GitAst, Error> {
    let tokens = tokenize(src, &TOKENS)?;
    let mut r = Reader::new(tokens, &TOKENS);
    let mut ast = GitAst::default();

    while r.kind() == NEWLINE_KIND {
        r.i += 1;
    }
    if !r.accept(GIT_GRAPH_COLON) {
        r.expect(GIT_GRAPH)?;
        ast.direction = match r.kind() {
            LR => {
                r.i += 1;
                Some(Direction::LeftToRight)
            }
            TB => {
                r.i += 1;
                Some(Direction::TopToBottom)
            }
            BT => {
                r.i += 1;
                Some(Direction::BottomToTop)
            }
            _ => {
                r.accept(COLON);
                None
            }
        };
        if ast.direction.is_some() {
            r.expect(COLON)?;
        }
    }

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
            COMMIT => {
                r.i += 1;
                let mut id = None;
                let mut message = None;
                let mut tags = Vec::new();
                let mut kind = None;
                loop {
                    if r.accept(ID) {
                        id = Some(string(&mut r)?);
                    } else if r.accept(TAG) {
                        tags.push(string(&mut r)?);
                    } else if r.accept(TYPE) {
                        kind = Some(commit_type(&mut r)?);
                    } else if r.accept(MSG) || r.kind() == STRING_KIND {
                        message = Some(string(&mut r)?);
                    } else {
                        break;
                    }
                }
                end_of_line(&mut r)?;
                ast.statements.push(Statement::Commit {
                    id,
                    message,
                    tags,
                    kind,
                });
            }
            BRANCH => {
                r.i += 1;
                let name = name(&mut r)?;
                let order = if r.accept(ORDER) {
                    let text = r.expect(INT)?;
                    Some(text.parse::<f64>().unwrap_or(0.0))
                } else {
                    None
                };
                end_of_line(&mut r)?;
                ast.statements.push(Statement::Branch { name, order });
            }
            MERGE => {
                r.i += 1;
                let branch = name(&mut r)?;
                let mut id = None;
                let mut tags = Vec::new();
                let mut kind = None;
                loop {
                    if r.accept(ID) {
                        id = Some(string(&mut r)?);
                    } else if r.accept(TAG) {
                        tags.push(string(&mut r)?);
                    } else if r.accept(TYPE) {
                        kind = Some(commit_type(&mut r)?);
                    } else {
                        break;
                    }
                }
                end_of_line(&mut r)?;
                ast.statements.push(Statement::Merge {
                    branch,
                    id,
                    tags,
                    kind,
                });
            }
            CHECKOUT | SWITCH => {
                r.i += 1;
                let branch = name(&mut r)?;
                end_of_line(&mut r)?;
                ast.statements.push(Statement::Checkout { branch });
            }
            CHERRY_PICK => {
                r.i += 1;
                let mut id = None;
                let mut tags = Vec::new();
                let mut parent = None;
                loop {
                    if r.accept(ID) {
                        id = Some(string(&mut r)?);
                    } else if r.accept(TAG) {
                        tags.push(string(&mut r)?);
                    } else if r.accept(PARENT) {
                        parent = Some(string(&mut r)?);
                    } else {
                        break;
                    }
                }
                end_of_line(&mut r)?;
                ast.statements
                    .push(Statement::CherryPicking { id, tags, parent });
            }
            _ => return Err(r.fail("a statement, a title, or a line break")),
        }
    }
    Ok(ast)
}
