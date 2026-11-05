//! The state diagram parser: Mermaid's `stateDiagram.jison` grammar, as pele's `parser.ts`
//! follows it. It builds the documents and hands the root one to the model.

use super::db::{Desc, StateDb, StateNote, StateStmt, Stmt};
use super::lexer::{T, tokenize};
use crate::error::{Error, ErrorCode};
use crate::util::trim;

const DIRECTIONS: [&str; 4] = ["TB", "BT", "RL", "LR"];
const STATEMENT: &str = "'NL', 'ID', 'HIDE_EMPTY', 'scale', 'COMPOSIT_STATE', 'STATE_DESCR', 'FORK', 'JOIN', \
     'CHOICE', 'CONCURRENT', 'note', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'CLICK', 'classDef', \
     'style', 'class', 'direction_tb', 'direction_bt', 'direction_rl', 'direction_lr', 'EDGE_STATE'";

type Res<T> = Result<T, Error>;

struct Parser<'a> {
    types: Vec<T>,
    texts: Vec<String>,
    starts: Vec<usize>,
    error: Option<String>,
    i: usize,
    src: &'a str,
}

/// The byte offset of a UTF-16 offset.
fn byte_offset(src: &str, units: usize) -> usize {
    let mut count = 0;
    for (i, c) in src.char_indices() {
        if count >= units {
            return i;
        }
        count += c.len_utf16();
    }
    src.len()
}

/// `text.startsWith(':') ? text.slice(1).trim() : text.trim()`.
fn trim_colon(text: &str) -> String {
    trim(text.strip_prefix(':').unwrap_or(text)).to_string()
}

impl Parser<'_> {
    fn t(&self) -> T {
        self.types[self.i]
    }

    fn take(&mut self) -> String {
        let text = std::mem::take(&mut self.texts[self.i]);
        self.i += 1;
        text
    }

    fn fail<X>(&self, expected: &str) -> Res<X> {
        let at = byte_offset(self.src, self.starts[self.i]);
        if self.t() == T::Error {
            let message = self
                .error
                .clone()
                .unwrap_or_else(|| "Unrecognized text.".to_string());
            return Err(Error::new(ErrorCode::Syntax, message).at(self.src, at));
        }
        Err(Error::new(
            ErrorCode::Syntax,
            format!("Expecting {expected}, got '{}'", self.t().name()),
        )
        .at(self.src, at))
    }

    fn expect(&mut self, t: T) -> Res<String> {
        if self.t() != t {
            return self.fail(&format!("'{}'", t.name()));
        }
        Ok(self.take())
    }

    fn id_statement(&mut self) -> Res<StateStmt> {
        if !matches!(self.t(), T::Id | T::EdgeState) {
            return self.fail("'ID', 'EDGE_STATE'");
        }
        let id = trim(&self.take()).to_string();
        let mut s = StateStmt::new(id, "default");
        s.description = Some(Desc::Text(String::new()));
        if self.t() == T::StyleSeparator {
            self.i += 1;
            s.classes = Some(vec![trim(&self.expect(T::Id)?).to_string()]);
        }
        Ok(s)
    }
}

pub(crate) fn parse_state(src: &str, db: &mut StateDb, max_depth: usize) -> Res<()> {
    let units: Vec<u16> = src.encode_utf16().collect();
    let tokens = tokenize(&units);
    let mut p = Parser {
        types: tokens.types,
        texts: tokens.texts,
        starts: tokens.starts,
        error: tokens.error,
        i: 0,
        src,
    };
    while p.t() == T::Nl {
        p.i += 1;
    }
    if p.t() != T::Sd {
        return p.fail("'NL', 'SD'");
    }
    p.i += 1;

    // The open documents; each one below the top is the document of the composite state that
    // ends its list.
    let mut docs: Vec<Vec<Stmt>> = vec![Vec::new()];
    // The grammar drops a statement whose value is the string it uses for a blank line.
    let add = |docs: &mut Vec<Vec<Stmt>>, value: Stmt| {
        if value != Stmt::Text("nl".to_string()) {
            docs.last_mut().expect("document").push(value);
        }
    };
    let open = |p: &mut Parser<'_>,
                docs: &mut Vec<Vec<Stmt>>,
                id: String,
                description: String|
     -> Res<()> {
        if docs.len() > max_depth {
            return Err(Error::new(
                ErrorCode::Limit,
                format!("Composite states are nested deeper than the limit of {max_depth}."),
            )
            .at(p.src, byte_offset(p.src, p.starts[p.i])));
        }
        p.i += 1;
        let mut s = StateStmt::new(id, "default");
        s.description = Some(Desc::Text(description));
        docs.last_mut().expect("document").push(Stmt::State(s));
        docs.push(Vec::new());
        Ok(())
    };

    loop {
        let t = p.t();
        match t {
            T::Nl => p.i += 1,
            T::Id | T::EdgeState => {
                let mut first = p.id_statement()?;
                if p.t() == T::Descr {
                    first.description = Some(Desc::Text(trim_colon(&p.take())));
                    add(&mut docs, Stmt::State(first));
                } else if p.t() == T::Arrow {
                    p.i += 1;
                    let second = p.id_statement()?;
                    let description = (p.t() == T::Descr).then(|| trim_colon(&p.take()));
                    add(
                        &mut docs,
                        Stmt::Relation {
                            state1: Box::new(first),
                            state2: Box::new(second),
                            description,
                        },
                    );
                } else {
                    add(&mut docs, Stmt::State(first));
                }
            }
            T::HideEmpty => {
                let text = p.take();
                add(&mut docs, Stmt::Text(text));
            }
            T::Scale => {
                let text = p.take();
                p.expect(T::Width)?;
                add(&mut docs, Stmt::Text(text));
            }
            T::CompositState => {
                if p.types.get(p.i + 1) == Some(&T::StructStart) {
                    let id = p.take();
                    open(&mut p, &mut docs, id, String::new())?;
                } else {
                    let text = p.take();
                    add(&mut docs, Stmt::Text(text));
                }
            }
            T::StateDescr => {
                let description = p.take();
                p.expect(T::As)?;
                let id = p.expect(T::Id)?;
                if p.t() == T::StructStart {
                    open(&mut p, &mut docs, id, description)?;
                } else if let Some((head, rest)) = id.split_once(':') {
                    // `id.split(':')`, of which the first two parts count.
                    let second = rest.split(':').next().unwrap_or("").to_string();
                    let mut s = StateStmt::new(head.to_string(), "default");
                    s.description = Some(Desc::List(vec![trim(&description).to_string(), second]));
                    add(&mut docs, Stmt::State(s));
                } else {
                    let mut s = StateStmt::new(id, "default");
                    s.description = Some(Desc::Text(trim(&description).to_string()));
                    add(&mut docs, Stmt::State(s));
                }
            }
            T::Fork | T::Join | T::Choice => {
                let kind = match t {
                    T::Fork => "fork",
                    T::Join => "join",
                    _ => "choice",
                };
                let id = p.take();
                add(&mut docs, Stmt::State(StateStmt::new(id, kind)));
            }
            T::Concurrent => {
                p.i += 1;
                let id = db.divider_id();
                add(&mut docs, Stmt::State(StateStmt::new(id, "divider")));
            }
            T::Note => {
                let keyword = p.take();
                if matches!(p.t(), T::LeftOf | T::RightOf) {
                    let position = trim(&p.take()).to_string();
                    let id = trim(&p.expect(T::Id)?).to_string();
                    let text = trim(&p.expect(T::NoteText)?).to_string();
                    let mut s = StateStmt::new(id, "default");
                    s.kind = None;
                    s.note = Some(StateNote {
                        position: Some(position),
                        text,
                    });
                    add(&mut docs, Stmt::State(s));
                } else {
                    if p.t() != T::NoteText {
                        return p.fail("'NOTE_TEXT', 'left_of', 'right_of'");
                    }
                    p.i += 1;
                    p.expect(T::As)?;
                    p.expect(T::Id)?;
                    add(&mut docs, Stmt::Text(keyword));
                }
            }
            T::DirectionTb | T::DirectionBt | T::DirectionRl | T::DirectionLr => {
                let k = [
                    T::DirectionTb,
                    T::DirectionBt,
                    T::DirectionRl,
                    T::DirectionLr,
                ]
                .iter()
                .position(|&d| d == t)
                .unwrap_or(0);
                p.i += 1;
                add(&mut docs, Stmt::Dir(DIRECTIONS[k].to_string()));
            }
            T::AccTitle => {
                p.i += 1;
                let value = trim(&p.expect(T::AccTitleValue)?).to_string();
                db.set_acc_title(&value);
                add(&mut docs, Stmt::Text(value));
            }
            T::AccDescr => {
                p.i += 1;
                let value = trim(&p.expect(T::AccDescrValue)?).to_string();
                db.set_acc_description(&value);
                add(&mut docs, Stmt::Text(value));
            }
            T::AccDescrMultilineValue => {
                let value = trim(&p.take()).to_string();
                db.set_acc_description(&value);
                add(&mut docs, Stmt::Text(value));
            }
            T::Click => {
                p.i += 1;
                let id = p.id_statement()?.id;
                let (url, tooltip) = if p.t() == T::Href {
                    p.i += 1;
                    (p.expect(T::String)?, String::new())
                } else {
                    if p.t() != T::String {
                        return p.fail("'STRING', 'HREF'");
                    }
                    let url = p.take();
                    (url, p.expect(T::String)?)
                };
                p.expect(T::Nl)?;
                add(&mut docs, Stmt::Click { id, url, tooltip });
            }
            T::ClassDef => {
                p.i += 1;
                let id = trim(&p.expect(T::ClassDefId)?).to_string();
                let classes = trim(&p.expect(T::ClassDefStyleOpts)?).to_string();
                add(&mut docs, Stmt::ClassDef { id, classes });
            }
            T::Style => {
                p.i += 1;
                let id = trim(&p.expect(T::StyleIds)?).to_string();
                let style_class = trim(&p.expect(T::StyleDefStyleOpts)?).to_string();
                add(&mut docs, Stmt::Style { id, style_class });
            }
            T::Class => {
                p.i += 1;
                let id = trim(&p.expect(T::ClassEntityIds)?).to_string();
                let style_class = trim(&p.expect(T::StyleClass)?).to_string();
                add(&mut docs, Stmt::ApplyClass { id, style_class });
            }
            T::StructStop => {
                if docs.len() == 1 {
                    return p.fail(&format!("'$end', {STATEMENT}"));
                }
                p.i += 1;
                let inner = docs.pop().expect("document");
                if let Some(Stmt::State(s)) = docs.last_mut().and_then(|d| d.last_mut()) {
                    s.doc = Some(inner);
                }
            }
            T::End => {
                if docs.len() > 1 {
                    return p.fail(&format!("'STRUCT_STOP', {STATEMENT}"));
                }
                let root = docs.pop().expect("document");
                return db.set_root_doc(root);
            }
            _ => {
                let expected = if docs.len() > 1 {
                    "'STRUCT_STOP'"
                } else {
                    "'$end'"
                };
                return p.fail(&format!("{expected}, {STATEMENT}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Res<StateDb> {
        let mut db = StateDb::default();
        parse_state(src, &mut db, 64)?;
        Ok(db)
    }

    #[test]
    fn parses_states_and_transitions() {
        let db = parse("stateDiagram-v2\n[*] --> A\nA --> B : go\nB --> [*]\n").unwrap();
        let ids: Vec<_> = db.states.items.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["root_start", "A", "B", "root_end"]);
        assert_eq!(db.relations[1].title, "go");
    }

    #[test]
    fn rejects_an_unclosed_note_and_deep_nesting() {
        let err = parse("stateDiagram-v2\nnote right of A\nno end\n").unwrap_err();
        assert_eq!(err.code, ErrorCode::Syntax);
        let deep = format!(
            "stateDiagram-v2\n{}x\n{}",
            "state a {\n".repeat(100),
            "}\n".repeat(100)
        );
        assert_eq!(parse(&deep).unwrap_err().code, ErrorCode::Limit);
    }
}
