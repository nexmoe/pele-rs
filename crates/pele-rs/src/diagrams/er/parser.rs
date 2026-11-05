//! The ER diagram parser: Mermaid's `erDiagram.jison` grammar, as pele's `parser.ts` follows
//! it.

use super::db::{Cardinality, DocItem, ErAttribute, ErDb, RelSpec};
use super::lexer::{T, tokenize};
use crate::error::{Error, ErrorCode};
use crate::util::trim;

const NAME: &str = "'UNICODE_TEXT', 'NUM', 'ENTITY_NAME', 'DECIMAL_NUM', 'ENTITY_ONE'";
const CARDS: &str = "'ZERO_OR_ONE', 'ZERO_OR_MORE', 'ONE_OR_MORE', 'ONLY_ONE', 'MD_PARENT'";
const LINE: &str = "'EOF', 'NEWLINE', 'title', 'acc_title', 'acc_descr', \
     'acc_descr_multiline_value', 'SUBGRAPH', 'direction_tb', 'direction_bt', 'direction_rl', \
     'direction_lr', 'CLASSDEF', 'CLASS', 'STYLE', 'UNICODE_TEXT', 'NUM', 'ENTITY_NAME', \
     'DECIMAL_NUM', 'ENTITY_ONE'";
const DIRECTIONS: [&str; 4] = ["TB", "BT", "RL", "LR"];

type Res<T> = Result<T, Error>;

fn cardinality(t: T) -> Option<Cardinality> {
    match t {
        T::ZeroOrOne => Some(Cardinality::ZeroOrOne),
        T::ZeroOrMore => Some(Cardinality::ZeroOrMore),
        T::OneOrMore => Some(Cardinality::OneOrMore),
        T::OnlyOne => Some(Cardinality::OnlyOne),
        T::MdParent => Some(Cardinality::MdParent),
        _ => None,
    }
}

fn is_name(t: T) -> bool {
    matches!(
        t,
        T::UnicodeText | T::EntityName | T::Num | T::DecimalNum | T::EntityOne
    )
}

fn is_style_component(t: T) -> bool {
    matches!(t, T::StyleText | T::Num | T::Colon | T::Brkt)
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

/// Strips the quotes around a quoted token.
fn unquote(text: &str) -> String {
    let mut chars = text.chars();
    chars.next();
    chars.next_back();
    chars.as_str().to_string()
}

/// A subgraph being parsed, with what Mermaid's grammar hands to `addSubGraph`: the value of
/// every statement inside. Besides entity names that includes accessibility text and the
/// keywords `style`, `class` and `classDef`.
struct Frame {
    id: String,
    title: String,
    list: Vec<DocItem>,
}

struct Parser<'a> {
    types: Vec<T>,
    texts: Vec<String>,
    starts: Vec<usize>,
    i: usize,
    src: &'a str,
}

impl Parser<'_> {
    fn t(&self) -> T {
        self.types.get(self.i).copied().unwrap_or(T::End)
    }

    fn take(&mut self) -> String {
        let text = std::mem::take(&mut self.texts[self.i]);
        self.i += 1;
        text
    }

    fn here(&self) -> usize {
        let at = self.starts.get(self.i).or(self.starts.last()).copied();
        byte_offset(self.src, at.unwrap_or(0))
    }

    fn fail<X>(&self, expected: &str) -> Res<X> {
        let t = self.t();
        if t == T::Error {
            return Err(
                Error::new(ErrorCode::Syntax, "Unrecognized text.").at(self.src, self.here())
            );
        }
        let got = if t == T::Char {
            self.texts[self.i].as_str()
        } else {
            t.name()
        };
        Err(Error::new(
            ErrorCode::Syntax,
            format!("Expecting {expected}, got '{got}'"),
        )
        .at(self.src, self.here()))
    }

    fn expect(&mut self, t: T) -> Res<String> {
        if self.t() != t {
            return self.fail(&format!("'{}'", t.name()));
        }
        Ok(self.take())
    }

    fn separator(&mut self) -> Res<()> {
        if !matches!(self.t(), T::Newline | T::Semi | T::Eof) {
            return self.fail("'EOF', 'NEWLINE', 'SEMI'");
        }
        self.i += 1;
        Ok(())
    }

    fn entity_name(&mut self) -> Res<String> {
        let t = self.t();
        if !is_name(t) {
            return self.fail(NAME);
        }
        let text = self.take();
        Ok(if t == T::EntityName {
            unquote(&text)
        } else {
            text
        })
    }

    fn id_list(&mut self) -> Res<Vec<String>> {
        let mut ids = Vec::new();
        loop {
            if !matches!(self.t(), T::UnicodeText | T::StyleText) {
                return self.fail("'UNICODE_TEXT', 'STYLE_TEXT'");
            }
            ids.push(self.take());
            if self.t() != T::Comma {
                return Ok(ids);
            }
            self.i += 1;
        }
    }

    fn styles(&mut self) -> Res<Vec<String>> {
        let mut styles = Vec::new();
        loop {
            if !is_style_component(self.t()) {
                return self.fail("'STYLE_TEXT', 'COLON', 'NUM', 'BRKT'");
            }
            let mut style = self.take();
            while is_style_component(self.t()) {
                style += &self.take();
            }
            styles.push(style);
            if self.t() != T::Comma {
                return Ok(styles);
            }
            self.i += 1;
        }
    }

    fn rel_spec(&mut self) -> Res<RelSpec> {
        let card_b = cardinality(self.t()).expect("cardinality");
        self.i += 1;
        let identifying = match self.t() {
            T::Identifying => true,
            T::NonIdentifying => false,
            _ => return self.fail("'NON_IDENTIFYING', 'IDENTIFYING'"),
        };
        self.i += 1;
        let Some(card_a) = cardinality(self.t()) else {
            return self.fail(CARDS);
        };
        self.i += 1;
        Ok(RelSpec {
            card_a,
            identifying,
            card_b,
        })
    }

    fn role(&mut self) -> Res<String> {
        let t = self.t();
        if !matches!(t, T::Word | T::EntityName | T::UnicodeText) {
            return self.fail("'UNICODE_TEXT', 'ENTITY_NAME', 'WORD'");
        }
        let text = self.take();
        Ok(if t == T::UnicodeText {
            text
        } else {
            unquote(&text)
        })
    }

    fn is_char(&self, ch: &str) -> bool {
        self.t() == T::Char && self.texts[self.i] == ch
    }

    /// Returns the attributes last to first, as Mermaid's right-recursive rule builds them.
    fn attributes(&mut self) -> Res<Vec<ErAttribute>> {
        let mut list = Vec::new();
        loop {
            let mut kind = self.expect(T::AttributeWord)?;
            if self.is_char("?") {
                kind += &self.take();
            }
            if self.t() != T::AttributeWord {
                return self.fail(if kind.ends_with('?') {
                    "'ATTRIBUTE_WORD'"
                } else {
                    "'ATTRIBUTE_WORD', '?'"
                });
            }
            let name = self.take();
            let mut keys = Vec::new();
            if self.t() == T::AttributeKey {
                keys.push(self.take());
                while self.is_char(",") {
                    self.i += 1;
                    keys.push(self.expect(T::AttributeKey)?);
                }
            }
            let comment = if self.t() == T::Comment {
                unquote(&self.take())
            } else {
                String::new()
            };
            list.push(ErAttribute {
                kind,
                name,
                keys,
                comment,
            });
            if self.t() == T::BlockStop {
                break;
            }
        }
        list.reverse();
        Ok(list)
    }

    /// Parses an optional `{ ... }` block; `None` when there is none or it is empty.
    fn block(&mut self) -> Res<Option<Vec<ErAttribute>>> {
        if self.t() != T::BlockStart {
            return Ok(None);
        }
        self.i += 1;
        if self.t() == T::BlockStop {
            self.i += 1;
            return Ok(None);
        }
        let list = self.attributes()?;
        self.i += 1;
        Ok(Some(list))
    }

    fn entity_statement(&mut self, db: &mut ErDb) -> Res<Vec<String>> {
        let a = self.entity_name()?;
        if self.t() == T::Sqs {
            self.i += 1;
            let alias = self.entity_name()?;
            self.expect(T::Sqe)?;
            let mut classes = None;
            if self.t() == T::StyleSeparator {
                self.i += 1;
                classes = Some(self.id_list()?);
            }
            let attrs = self.block()?;
            db.add_entity(&a, &alias);
            if let Some(attrs) = attrs {
                db.add_attributes(&a, attrs);
            }
            if let Some(classes) = classes {
                db.set_class(std::slice::from_ref(&a), &classes);
            }
            return Ok(vec![a]);
        }
        let mut classes = None;
        if self.t() == T::StyleSeparator {
            self.i += 1;
            classes = Some(self.id_list()?);
        }
        if cardinality(self.t()).is_some() {
            let spec = self.rel_spec()?;
            let b = self.entity_name()?;
            let mut classes_b = None;
            if self.t() == T::StyleSeparator {
                self.i += 1;
                classes_b = Some(self.id_list()?);
            }
            self.expect(T::Colon)?;
            let label = self.role()?;
            db.add_entity(&a, "");
            db.add_entity(&b, "");
            db.add_relationship(&a, &label, &b, spec);
            if let Some(classes) = classes {
                db.set_class(std::slice::from_ref(&a), &classes);
            }
            if let Some(classes) = classes_b {
                db.set_class(std::slice::from_ref(&b), &classes);
            }
            return Ok(vec![a, b]);
        }
        let attrs = self.block()?;
        db.add_entity(&a, "");
        if let Some(attrs) = attrs {
            db.add_attributes(&a, attrs);
        }
        if let Some(classes) = classes {
            db.set_class(std::slice::from_ref(&a), &classes);
        }
        Ok(vec![a])
    }
}

/// Parses an ER diagram into `db`. Subgraphs nest no deeper than `max_depth`.
pub(crate) fn parse_er(src: &str, db: &mut ErDb, max_depth: usize) -> Res<()> {
    let units: Vec<u16> = src.encode_utf16().collect();
    let tokens = tokenize(&units);
    let mut p = Parser {
        types: tokens.types,
        texts: tokens.texts,
        starts: tokens.starts,
        i: 0,
        src,
    };

    p.expect(T::ErDiagram)?;
    // Subgraphs nest on this stack rather than on the call stack.
    let mut frames: Vec<Frame> = Vec::new();

    loop {
        let t = p.t();
        let nested = !frames.is_empty();
        match t {
            T::Newline => p.i += 1,
            T::Eof => {
                if nested {
                    return p.fail(&format!("'END', {LINE}"));
                }
                return Ok(());
            }
            T::EndKw => {
                let Some(frame) = frames.pop() else {
                    return p.fail(LINE);
                };
                p.i += 1;
                let id = db.add_subgraph(&frame.id, &frame.list, &frame.title);
                if let Some(parent) = frames.last_mut() {
                    parent.list.push(DocItem::Name(id));
                }
            }
            T::Subgraph => {
                if frames.len() >= max_depth {
                    return Err(Error::new(
                        ErrorCode::Limit,
                        format!("Subgraphs are nested deeper than the limit of {max_depth}."),
                    )
                    .at(p.src, p.here()));
                }
                p.i += 1;
                let id = p.entity_name()?;
                let mut title = id.clone();
                if p.t() == T::Sqs {
                    p.i += 1;
                    title = p.entity_name()?;
                    while is_name(p.t()) {
                        title.push(' ');
                        title += &p.entity_name()?;
                    }
                    p.expect(T::Sqe)?;
                }
                p.separator()?;
                frames.push(Frame {
                    id,
                    title,
                    list: Vec::new(),
                });
            }
            T::AccTitle | T::AccDescr | T::AccDescrMultilineValue => {
                let value = if t == T::AccDescrMultilineValue {
                    trim(&p.take()).to_string()
                } else {
                    p.i += 1;
                    let want = if t == T::AccTitle {
                        T::AccTitleValue
                    } else {
                        T::AccDescrValue
                    };
                    trim(&p.expect(want)?).to_string()
                };
                if t == T::AccTitle {
                    db.set_acc_title(&value);
                } else {
                    db.set_acc_description(&value);
                }
                if let Some(frame) = frames.last_mut() {
                    frame.list.push(DocItem::Name(value));
                }
            }
            T::DirectionTb | T::DirectionBt | T::DirectionRl | T::DirectionLr => {
                p.i += 1;
                let k = match t {
                    T::DirectionTb => 0,
                    T::DirectionBt => 1,
                    T::DirectionRl => 2,
                    _ => 3,
                };
                match frames.last_mut() {
                    Some(frame) => frame.list.push(DocItem::Dir(DIRECTIONS[k])),
                    None => db.direction = DIRECTIONS[k],
                }
            }
            T::ClassDef | T::Style => {
                let keyword = p.take();
                let ids = p.id_list()?;
                let styles = p.styles()?;
                p.separator()?;
                if t == T::ClassDef {
                    db.add_class(&ids, &styles);
                } else {
                    db.add_css_styles(&ids, &styles);
                }
                if let Some(frame) = frames.last_mut() {
                    frame.list.push(DocItem::Name(keyword));
                }
            }
            T::Class => {
                let keyword = p.take();
                let ids = p.id_list()?;
                let names = p.id_list()?;
                db.set_class(&ids, &names);
                if let Some(frame) = frames.last_mut() {
                    frame.list.push(DocItem::Name(keyword));
                }
            }
            _ => {
                if !is_name(t) {
                    return if nested {
                        p.fail(&format!("'END', {LINE}"))
                    } else {
                        p.fail(LINE)
                    };
                }
                let names = p.entity_statement(db)?;
                if let Some(frame) = frames.last_mut() {
                    frame.list.extend(names.into_iter().map(DocItem::Name));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Res<ErDb> {
        let mut db = ErDb::new();
        parse_er(src, &mut db, 50)?;
        Ok(db)
    }

    #[test]
    fn parses_relationships() {
        let db = parse("erDiagram\nCUSTOMER ||--o{ ORDER : places\n").unwrap();
        assert_eq!(db.entities.len(), 2);
        let rel = &db.relationships[0];
        assert_eq!(rel.role_a, "places");
        assert_eq!(rel.rel_spec.card_b, Cardinality::OnlyOne);
        assert_eq!(rel.rel_spec.card_a, Cardinality::ZeroOrMore);
        assert!(rel.rel_spec.identifying);
    }

    #[test]
    fn parses_attributes_in_order() {
        let db = parse("erDiagram\nA {\n string name PK \"the name\"\n int age\n}\n").unwrap();
        let attrs = &db.entities.get("A").unwrap().attributes;
        assert_eq!(attrs.len(), 2);
        assert_eq!(attrs[0].name, "name");
        assert_eq!(attrs[0].keys, ["PK"]);
        assert_eq!(attrs[0].comment, "the name");
        assert_eq!(attrs[1].kind, "int");
    }

    #[test]
    fn reports_positions() {
        let err = parse("erDiagram\nA ||--o{ B\n").unwrap_err();
        assert_eq!(err.code, ErrorCode::Syntax);
        assert_eq!(err.line, 2);
    }

    #[test]
    fn limits_subgraph_depth() {
        let deep = format!(
            "erDiagram\n{}{}",
            "subgraph a\n".repeat(100),
            "end\n".repeat(100)
        );
        assert_eq!(parse(&deep).unwrap_err().code, ErrorCode::Limit);
    }
}
