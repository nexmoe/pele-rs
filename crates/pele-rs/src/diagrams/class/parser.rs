//! The class diagram parser: Mermaid's `classDiagram.jison` grammar, as pele's `parser.ts`
//! follows it.

use super::db::{ClassDb, ClassRelation, RelationEnd, cleanup_label};
use super::lexer::{T, tokenize};
use crate::error::{Error, ErrorCode};
use crate::util::trim;

const NAME_TOKENS: &str = "'UNICODE_TEXT', 'NUM', 'ALPHA', 'MINUS', 'BQUOTE_STR'";
const STATEMENT: &str = "'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'NAMESPACE', \
     'ANNOTATION_START', 'CLASS', 'MEMBER', 'NOTE_FOR', 'NOTE', 'CLASSDEF', 'direction_tb', \
     'CALLBACK', 'LINK', 'CLICK', 'STYLE', 'CSSCLASS', 'ALPHA', 'MINUS', 'BQUOTE_STR'";

type Res<T> = Result<T, Error>;

fn alpha_num(t: T) -> bool {
    matches!(t, T::UnicodeText | T::Alpha | T::Minus)
}

fn style_part(t: T) -> bool {
    matches!(
        t,
        T::Alpha | T::Colon | T::Brkt | T::Style | T::Pct | T::Label
    )
}

/// Mermaid's relationType number of an end marker.
fn end_type(t: T) -> RelationEnd {
    match t {
        T::Aggregation => Some(0),
        T::Extension => Some(1),
        T::Composition => Some(2),
        T::Dependency => Some(3),
        T::Lollipop => Some(4),
        _ => None,
    }
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

struct Frame {
    id: String,
    classes: Vec<String>,
    notes: Vec<String>,
}

struct Parser<'a> {
    types: Vec<T>,
    texts: Vec<String>,
    starts: Vec<usize>,
    i: usize,
    src: &'a str,
    db: &'a mut ClassDb,
}

impl Parser<'_> {
    fn t(&self) -> T {
        self.types[self.i]
    }

    fn at(&self, k: usize) -> T {
        self.types.get(k).copied().unwrap_or(T::End)
    }

    fn take(&mut self) -> String {
        let text = std::mem::take(&mut self.texts[self.i]);
        self.i += 1;
        text
    }

    fn here(&self) -> usize {
        byte_offset(self.src, self.starts[self.i])
    }

    fn fail<X>(&self, expected: &str) -> Res<X> {
        if self.t() == T::Error {
            return Err(
                Error::new(ErrorCode::Syntax, "Unrecognized text.").at(self.src, self.here())
            );
        }
        Err(Error::new(
            ErrorCode::Syntax,
            format!("Expecting {expected}, got '{}'", self.t().name()),
        )
        .at(self.src, self.here()))
    }

    fn expect(&mut self, t: T) -> Res<String> {
        if self.t() != t {
            return self.fail(&format!("'{}'", t.name()));
        }
        Ok(self.take())
    }

    fn alpha_num(&mut self) -> Res<String> {
        if !alpha_num(self.t()) {
            return self.fail("'UNICODE_TEXT', 'NUM', 'ALPHA', 'MINUS'");
        }
        Ok(self.take())
    }

    fn name(&mut self, generic: bool) -> Res<String> {
        let mut out = String::new();
        loop {
            if self.t() == T::BquoteStr {
                out += &self.take();
                break;
            }
            if !alpha_num(self.t()) {
                return self.fail(NAME_TOKENS);
            }
            out += &self.take();
            let t = self.t();
            if t == T::Dot {
                out.push('.');
                self.i += 1;
            } else if !alpha_num(t) && t != T::BquoteStr {
                break;
            }
        }
        if generic && self.t() == T::GenericType {
            out.push('~');
            out += &self.take();
            out.push('~');
        }
        Ok(out)
    }

    fn class_label(&mut self) -> Res<String> {
        self.i += 1;
        let label = self.expect(T::Str)?;
        self.expect(T::Sqe)?;
        Ok(label)
    }

    fn members(&mut self) -> Res<Vec<String>> {
        if self.t() != T::Member {
            return self.fail("'STRUCT_STOP', 'MEMBER'");
        }
        let mut list = Vec::new();
        while self.t() == T::Member {
            list.push(self.take());
        }
        self.expect(T::StructStop)?;
        Ok(list)
    }

    fn add_members(&mut self, id: &str, members: &[String]) {
        for member in members {
            self.db.add_member(id, member);
        }
    }

    /// A body in braces, which may be empty.
    fn body(&mut self, id: &str) -> Res<()> {
        self.i += 1;
        if self.t() == T::StructStop {
            self.i += 1;
        } else {
            let members = self.members()?;
            self.add_members(id, &members);
        }
        Ok(())
    }

    fn class_statement(&mut self) -> Res<String> {
        self.i += 1;
        let id = self.name(true)?;
        self.db.add_class(&id);
        if self.t() == T::Sqs {
            let label = self.class_label()?;
            self.db.set_class_label(&id, label);
        }
        match self.t() {
            T::StyleSeparator => {
                self.i += 1;
                let css = self.alpha_num()?;
                self.db.set_css_class(&id, &css);
                if self.t() == T::StructStart {
                    self.i += 1;
                    let members = self.members()?;
                    self.add_members(&id, &members);
                }
            }
            T::StructStart => self.body(&id)?,
            T::AnnotationStart => {
                self.i += 1;
                let annotation = self.alpha_num()?;
                self.expect(T::AnnotationEnd)?;
                self.db.add_annotation(&id, annotation);
                if self.t() == T::StructStart {
                    self.body(&id)?;
                }
            }
            _ => {}
        }
        Ok(id)
    }

    fn note_statement(&mut self) -> Res<String> {
        let t = self.t();
        self.i += 1;
        if t == T::Note {
            let text = self.expect(T::Str)?;
            return Ok(self.db.add_note(text, None));
        }
        let id = self.name(true)?;
        let text = self.expect(T::Str)?;
        Ok(self.db.add_note(text, Some(id)))
    }

    /// Namespaces nest, so they are parsed with a stack of open ones.
    fn namespace_statement(&mut self) -> Res<()> {
        let mut open: Vec<Frame> = Vec::new();
        loop {
            self.i += 1;
            let at = self.here();
            let ns = self.name(false)?;
            let label = if self.t() == T::Sqs {
                Some(self.class_label()?)
            } else {
                None
            };
            let id = self
                .db
                .add_namespace(&ns, label)
                .map_err(|e| e.at(self.src, at))?;
            self.expect(T::StructStart)?;
            if self.t() == T::Newline {
                self.i += 1;
            }
            open.push(Frame {
                id,
                classes: Vec::new(),
                notes: Vec::new(),
            });
            let mut item = true;
            while item {
                let t = self.t();
                if t == T::Namespace {
                    break;
                }
                match t {
                    T::Class => {
                        let id = self.class_statement()?;
                        open.last_mut().expect("frame").classes.push(id);
                    }
                    T::Note | T::NoteFor => {
                        let id = self.note_statement()?;
                        open.last_mut().expect("frame").notes.push(id);
                    }
                    _ => return self.fail("'NAMESPACE', 'CLASS', 'NOTE_FOR', 'NOTE'"),
                }
                loop {
                    if self.t() == T::Newline {
                        let next = self.at(self.i + 1);
                        if matches!(next, T::Namespace | T::Class | T::Note | T::NoteFor) {
                            self.i += 1;
                            break;
                        }
                        self.i += 1;
                    }
                    self.expect(T::StructStop)?;
                    let done = open.pop().expect("frame");
                    self.db
                        .add_classes_to_namespace(&done.id, &done.classes, &done.notes);
                    self.db.pop_namespace();
                    if open.is_empty() {
                        item = false;
                        break;
                    }
                }
            }
            if open.is_empty() {
                return Ok(());
            }
        }
    }

    fn relation_end(&mut self) -> RelationEnd {
        let end = end_type(self.t());
        if end.is_some() {
            self.i += 1;
        }
        end
    }

    fn class_or_relation(&mut self) -> Res<()> {
        let id1 = self.name(true)?;
        let mut t = self.t();
        if t == T::Label {
            let label = cleanup_label(&self.take());
            self.db.add_member(&id1, &label);
            return Ok(());
        }
        let mut title1 = "none".to_string();
        if t == T::Str {
            title1 = self.take();
            t = self.t();
        } else if end_type(t).is_none() && t != T::Line && t != T::DottedLine {
            return Ok(());
        }
        let type1 = self.relation_end();
        let line = self.t();
        if line != T::Line && line != T::DottedLine {
            return self.fail(
                "'AGGREGATION', 'EXTENSION', 'COMPOSITION', 'DEPENDENCY', 'LOLLIPOP', 'LINE', \
                 'DOTTED_LINE'",
            );
        }
        let _ = t;
        self.i += 1;
        let type2 = self.relation_end();
        let title2 = if self.t() == T::Str {
            self.take()
        } else {
            "none".to_string()
        };
        let id2 = self.name(true)?;
        let title = (self.t() == T::Label).then(|| cleanup_label(&self.take()));
        self.db.add_relation(ClassRelation {
            id1,
            id2,
            title1,
            title2,
            title,
            type1,
            type2,
            dotted: line == T::DottedLine,
        });
        Ok(())
    }

    fn styles(&mut self) -> Res<Vec<String>> {
        let mut out = Vec::new();
        loop {
            if !style_part(self.t()) {
                return self.fail(
                    "'ALPHA', 'NUM', 'COLON', 'UNIT', 'SPACE', 'BRKT', 'STYLE', 'PCT', 'LABEL'",
                );
            }
            let mut s = self.take();
            while style_part(self.t()) {
                s += &self.take();
            }
            out.push(s);
            if self.t() != T::Comma {
                return Ok(out);
            }
            self.i += 1;
        }
    }

    fn tooltip(&mut self, id: &str) {
        if self.t() == T::Str {
            let tip = self.take();
            self.db.set_tooltip(id, Some(&tip));
        }
    }

    fn link_statement(&mut self, id: &str) -> Res<()> {
        let href = self.expect(T::Str)?;
        let tip = (self.t() == T::Str).then(|| self.take());
        let target = (self.t() == T::LinkTarget).then(|| self.take());
        self.db.set_link(id, &href, target.as_deref());
        self.db.set_tooltip(id, tip.as_deref());
        Ok(())
    }

    fn statement(&mut self) -> Res<()> {
        let t = self.t();
        match t {
            T::UnicodeText | T::Alpha | T::Minus | T::BquoteStr => self.class_or_relation()?,
            T::Namespace => self.namespace_statement()?,
            T::Class => {
                self.class_statement()?;
            }
            T::Member => self.i += 1,
            T::AnnotationStart => {
                self.i += 1;
                let annotation = self.alpha_num()?;
                self.expect(T::AnnotationEnd)?;
                let id = self.name(true)?;
                self.db.add_annotation(&id, annotation);
            }
            T::Callback => {
                self.i += 1;
                let id = self.name(true)?;
                self.expect(T::Str)?;
                self.db.set_click_event(&id);
                self.tooltip(&id);
            }
            T::Link => {
                self.i += 1;
                let id = self.name(true)?;
                self.link_statement(&id)?;
            }
            T::Click => {
                self.i += 1;
                let id = self.name(true)?;
                if self.t() == T::Href {
                    self.i += 1;
                    return self.link_statement(&id);
                }
                self.expect(T::CallbackName)?;
                if self.t() == T::CallbackArgs {
                    self.i += 1;
                }
                self.db.set_click_event(&id);
                self.tooltip(&id);
            }
            T::Style => {
                self.i += 1;
                let id = self.expect(T::Alpha)?;
                let styles = self.styles()?;
                self.db.set_css_style(&id, &styles);
            }
            T::CssClass => {
                self.i += 1;
                let ids = self.expect(T::Str)?;
                let class = self.expect(T::Alpha)?;
                self.db.set_css_class(&ids, &class);
            }
            T::Note | T::NoteFor => {
                self.note_statement()?;
            }
            T::ClassDef => {
                self.i += 1;
                let mut ids = vec![self.expect(T::Alpha)?];
                while self.t() == T::Comma {
                    self.i += 1;
                    ids.push(self.expect(T::Alpha)?);
                }
                let styles = self.styles()?;
                self.db.define_class(&ids, &styles);
            }
            T::DirectionTb | T::DirectionBt | T::DirectionRl | T::DirectionLr => {
                self.i += 1;
                self.db.direction = match t {
                    T::DirectionTb => "TB",
                    T::DirectionBt => "BT",
                    T::DirectionRl => "RL",
                    _ => "LR",
                }
                .to_string();
            }
            T::AccTitle => {
                self.i += 1;
                let value = self.expect(T::AccTitleValue)?;
                self.db.set_acc_title(trim(&value));
            }
            T::AccDescr => {
                self.i += 1;
                let value = self.expect(T::AccDescrValue)?;
                self.db.set_acc_description(trim(&value));
            }
            T::AccDescrMultilineValue => {
                let value = self.take();
                self.db.set_acc_description(trim(&value));
            }
            _ => return self.fail(STATEMENT),
        }
        Ok(())
    }
}

/// Parses class diagram text into `db`, whose depth limit caps how deeply namespaces nest.
pub(crate) fn parse_class(src: &str, db: &mut ClassDb) -> Res<()> {
    let units: Vec<u16> = src.encode_utf16().collect();
    let tokens = tokenize(&units);
    let mut p = Parser {
        types: tokens.types,
        texts: tokens.texts,
        starts: tokens.starts,
        i: 0,
        src,
        db,
    };
    // Mermaid's grammar also accepts statements with no `classDiagram` line before them.
    let header = p.t() == T::ClassDiagram;
    if header {
        p.i += 1;
        p.expect(T::Newline)?;
    }
    loop {
        p.statement()?;
        if p.t() != T::Newline {
            break;
        }
        p.i += 1;
        if matches!(p.t(), T::Eof | T::End) {
            break;
        }
    }
    if header {
        p.expect(T::Eof)?;
    }
    p.expect(T::End)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Res<ClassDb> {
        let mut db = ClassDb::new(64);
        parse_class(src, &mut db)?;
        Ok(db)
    }

    #[test]
    fn parses_classes_and_relations() {
        let db = parse(
            "classDiagram\nclass Animal {\n  +String name\n  +eat() void\n}\nAnimal <|-- Dog : is\n",
        )
        .unwrap();
        let animal = db.classes.get("Animal").unwrap();
        assert_eq!(animal.members.len(), 1);
        assert_eq!(animal.methods.len(), 1);
        assert_eq!(db.relations[0].type1, Some(1));
        assert_eq!(db.relations[0].title.as_deref(), Some("is"));
    }

    #[test]
    fn limits_namespace_nesting() {
        let deep = format!(
            "classDiagram\n{}class A\n{}",
            "namespace n {\n".repeat(100),
            "}\n".repeat(100)
        );
        assert_eq!(parse(&deep).unwrap_err().code, ErrorCode::Limit);
        assert!(parse("classDiagram\nclass A {\n").is_err());
    }
}
