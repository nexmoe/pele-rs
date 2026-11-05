//! The parser of a C4 diagram: Mermaid's jison grammar for `c4Diagram.jison`, as pele's
//! `parser.ts` follows it. The statements it reads are the actions the model is built from.

use crate::error::{Error, ErrorCode};

use super::db::{Action, Attr, Named};
use super::lexer::{Tokens, tokenize};
use super::tokens::T;

/// The header keywords the grammar starts a diagram with.
const HEADERS: &str = "'direction_tb', 'direction_bt', 'direction_rl', 'direction_lr', 'C4_CONTEXT', 'C4_CONTAINER', 'C4_COMPONENT', 'C4_DYNAMIC', 'C4_DEPLOYMENT'";

/// The arguments a statement may carry.
const ATTRIBUTES: &str = "'STR', 'STR_KEY', 'ATTRIBUTE', 'ATTRIBUTE_EMPTY'";

/// The statements that may come next, as the error message of the grammar names them.
const STATEMENTS: &str = "'title', 'accDescription', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'PERSON', 'REL'";

/// What the parser is at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum P {
    /// A statement is required.
    Item,
    /// Only a diagram statement may come next: the first one in a boundary, or one written
    /// straight after a title.
    Diagram,
    ItemOrEnd,
    AfterOther,
    AfterDiagram,
}

/// Reads the statements of a C4 source.
pub(crate) fn parse_c4(src: &str) -> Result<Vec<Action>, Error> {
    let mut parser = Parser {
        src,
        tokens: tokenize(src),
        i: 0,
    };
    parser.run()
}

struct Parser<'a> {
    src: &'a str,
    tokens: Tokens,
    i: usize,
}

impl Parser<'_> {
    fn t(&self) -> T {
        self.tokens.types.get(self.i).copied().unwrap_or(T::End)
    }

    fn text(&self, at: usize) -> &str {
        self.tokens.texts.get(at).map(String::as_str).unwrap_or("")
    }

    /// The byte offset of the token the parser is at.
    fn here(&self) -> usize {
        let start = self
            .tokens
            .starts
            .get(self.i)
            .copied()
            .unwrap_or_else(|| self.tokens.offsets.len().saturating_sub(1));
        self.tokens.offsets.get(start).copied().unwrap_or(0)
    }

    fn fail<X>(&self, expected: &str) -> Result<X, Error> {
        let at = self.here();
        if self.t() == T::Error {
            return Err(Error::new(ErrorCode::Syntax, "Unrecognized text.").at(self.src, at));
        }
        Err(Error::new(
            ErrorCode::Syntax,
            format!("Expecting {expected}, got '{}'", self.t().name()),
        )
        .at(self.src, at))
    }

    fn expect(&mut self, t: T) -> Result<(), Error> {
        if self.t() != t {
            return self.fail(&format!("'{}'", t.name()));
        }
        self.i += 1;
        Ok(())
    }

    /// The arguments of a statement.
    fn attributes(&mut self) -> Result<Vec<Attr>, Error> {
        let mut attrs: Vec<Attr> = Vec::new();
        loop {
            match self.t() {
                T::Str => {
                    let text = self.text(self.i).trim().to_string();
                    attrs.push(Attr::Text(text));
                    self.i += 1;
                }
                T::AttributeEmpty => {
                    attrs.push(Attr::Text(String::new()));
                    self.i += 1;
                }
                T::StrKey => {
                    self.i += 1;
                    if self.t() != T::StrValue {
                        return self.fail("'STR_VALUE'");
                    }
                    let key = self.text(self.i - 1).trim().to_string();
                    let value = self.text(self.i).trim().to_string();
                    attrs.push(Attr::Named(Named { key, value }));
                    self.i += 1;
                }
                _ => break,
            }
        }
        if attrs.is_empty() {
            return self.fail(ATTRIBUTES);
        }
        Ok(attrs)
    }

    /// A statement that sets the title or the description of the diagram.
    fn other(&mut self, t: T) -> Result<Action, Error> {
        match t {
            T::Title => {
                let text = substring(self.text(self.i), 6);
                self.i += 1;
                Ok(Action::SetTitle(text))
            }
            T::AccDescription => {
                let text = substring(self.text(self.i), 15);
                self.i += 1;
                Ok(Action::SetAccDescription(text))
            }
            T::AccDescrMultilineValue => {
                let text = self.text(self.i).trim().to_string();
                self.i += 1;
                Ok(Action::SetAccDescription(text))
            }
            T::AccTitle => {
                self.i += 1;
                if self.t() != T::AccTitleValue {
                    return self.fail("'acc_title_value'");
                }
                // The grammar hands `accTitle:` to setTitle, so it becomes the visible title.
                let text = self.text(self.i).trim().to_string();
                self.i += 1;
                Ok(Action::SetTitle(text))
            }
            _ => {
                self.i += 1;
                if self.t() != T::AccDescrValue {
                    return self.fail("'acc_descr_value'");
                }
                let text = self.text(self.i).trim().to_string();
                self.i += 1;
                Ok(Action::SetAccDescription(text))
            }
        }
    }

    /// A statement that draws something. Returns whether it opens a boundary.
    fn diagram(&mut self, t: T) -> Result<(Action, bool), Error> {
        self.i += 1;
        let mut attrs = self.attributes()?;
        match t {
            T::Person
            | T::PersonExt
            | T::System
            | T::SystemDb
            | T::SystemQueue
            | T::SystemExt
            | T::SystemExtDb
            | T::SystemExtQueue => {
                let kind = t.kind().unwrap_or_default().to_string();
                Ok((Action::PersonOrSystem { kind, attrs }, false))
            }
            T::Container
            | T::ContainerDb
            | T::ContainerQueue
            | T::ContainerExt
            | T::ContainerExtDb
            | T::ContainerExtQueue
            | T::Component
            | T::ComponentDb
            | T::ComponentQueue
            | T::ComponentExt
            | T::ComponentExtDb
            | T::ComponentExtQueue => {
                let kind = t.kind().unwrap_or_default().to_string();
                Ok((Action::Container { kind, attrs }, false))
            }
            T::EnterpriseBoundary | T::SystemBoundary | T::Boundary | T::ContainerBoundary => {
                let container = t == T::ContainerBoundary;
                match t {
                    T::EnterpriseBoundary => attrs.insert(2, Attr::Text("ENTERPRISE".to_string())),
                    T::SystemBoundary => attrs.insert(2, Attr::Text("SYSTEM".to_string())),
                    T::ContainerBoundary => attrs.insert(2, Attr::Text("CONTAINER".to_string())),
                    _ => {}
                }
                let action = Action::Boundary { container, attrs };
                self.boundary_start()?;
                Ok((action, true))
            }
            T::Node | T::NodeL | T::NodeR => {
                let node_type = t.node_type().unwrap_or_default().to_string();
                let action = Action::DeploymentNode { node_type, attrs };
                self.boundary_start()?;
                Ok((action, true))
            }
            T::Rel | T::BiRel | T::RelU | T::RelD | T::RelL | T::RelR | T::RelB | T::RelIndex => {
                // RelIndex takes a sequence number first, which Mermaid ignores.
                if t == T::RelIndex && !attrs.is_empty() {
                    attrs.remove(0);
                }
                let kind = t.rel_type().unwrap_or_default().to_string();
                Ok((Action::Rel { kind, attrs }, false))
            }
            T::UpdateElStyle => Ok((Action::UpdateElStyle(attrs), false)),
            T::UpdateRelStyle => Ok((Action::UpdateRelStyle(attrs), false)),
            _ => Ok((Action::UpdateLayoutConfig(attrs), false)),
        }
    }

    /// The brace that opens a boundary: `{` and a line break, or a line break and a brace.
    fn boundary_start(&mut self) -> Result<(), Error> {
        if self.t() == T::LBrace {
            self.i += 1;
            return self.expect(T::Newline);
        }
        if self.t() == T::Newline {
            self.i += 1;
            self.expect(T::LBrace)?;
            if self.t() == T::Newline {
                self.i += 1;
            }
            return Ok(());
        }
        self.fail("'NEWLINE', 'LBRACE'")
    }

    fn run(&mut self) -> Result<Vec<Action>, Error> {
        let mut actions: Vec<Action> = Vec::new();

        let header = self.t();
        if !header.is_header() {
            return self.fail(HEADERS);
        }
        self.i += 1;
        self.expect(T::Newline)?;

        let mut depth = 0usize;
        let mut state = P::Item;
        loop {
            let t = self.t();
            if state == P::AfterOther || state == P::AfterDiagram {
                if t == T::Newline {
                    self.i += 1;
                    state = P::ItemOrEnd;
                    continue;
                }
                if state == P::AfterOther && t.is_diagram() {
                    state = P::Diagram;
                    continue;
                }
            } else if t.is_diagram() {
                let (action, opens) = self.diagram(t)?;
                actions.push(action);
                if opens {
                    depth += 1;
                    state = P::Diagram;
                } else {
                    state = P::AfterDiagram;
                }
                continue;
            } else if t.is_other() && state != P::Diagram {
                let action = self.other(t)?;
                actions.push(action);
                state = P::AfterOther;
                continue;
            } else if state != P::ItemOrEnd {
                return self.fail(STATEMENTS);
            }

            if depth > 0 {
                if t != T::RBrace {
                    return self.fail("'NEWLINE', 'RBRACE'");
                }
                self.i += 1;
                actions.push(Action::PopBoundary);
                depth -= 1;
                state = P::AfterDiagram;
            } else {
                if t != T::Eof {
                    return self.fail("'NEWLINE', 'EOF'");
                }
                self.i += 1;
                self.expect(T::End)?;
                actions.push(Action::SetC4Type(self.text(0).to_string()));
                return Ok(actions);
            }
        }
    }
}

/// The text from a place in UTF-16 code units on, as JavaScript's `substring` reads it.
fn substring(text: &str, from: usize) -> String {
    let mut units = 0;
    let mut out = String::new();
    for c in text.chars() {
        if units >= from {
            out.push(c);
        }
        units += c.len_utf16();
    }
    out
}
