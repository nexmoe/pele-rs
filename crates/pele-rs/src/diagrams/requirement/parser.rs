//! The requirement diagram parser: Mermaid's `requirementDiagram.jison` grammar, as pele's
//! `parser.ts` follows it.

use super::db::RequirementDb;
use super::lexer::{ALL, T, tokenize};
use crate::error::{Error, ErrorCode};
use crate::util::trim;

type Res<T> = Result<T, Error>;

/// What the keyword tokens stand for, from REQUIREMENT to TRACES in token order.
const VALUES: [&str; 21] = [
    "Requirement",
    "Functional Requirement",
    "Interface Requirement",
    "Performance Requirement",
    "Physical Requirement",
    "Design Constraint",
    "Low",
    "Medium",
    "High",
    "Analysis",
    "Demonstration",
    "Inspection",
    "Test",
    "",
    "contains",
    "copies",
    "derives",
    "satisfies",
    "verifies",
    "refines",
    "traces",
];

fn is_style_part(t: T) -> bool {
    matches!(t, T::Alpha | T::Colon | T::Brkt | T::Minus | T::Semicolon)
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

    /// Fails with Jison's wording, naming the token types `from` to `to` and `others` as
    /// expected.
    fn fail<X>(&self, from: T, to: T, others: &[T]) -> Res<X> {
        let t = self.t();
        if t == T::Error {
            return Err(
                Error::new(ErrorCode::Syntax, "Unrecognized text.").at(self.src, self.here())
            );
        }
        let expected: Vec<String> = ALL[from as usize..=to as usize]
            .iter()
            .chain(others)
            .map(|k| format!("'{}'", k.name()))
            .collect();
        Err(Error::new(
            ErrorCode::Syntax,
            format!("Expecting {}, got '{}'", expected.join(", "), t.name()),
        )
        .at(self.src, self.here()))
    }

    fn expect(&mut self, t: T) -> Res<String> {
        if self.t() != t {
            return self.fail(t, t, &[]);
        }
        Ok(self.take())
    }

    /// The value of the next token if it is a keyword in the given range of token types.
    fn keyword(&mut self, from: T, to: T) -> Res<&'static str> {
        let t = self.t();
        if t < from || t > to {
            return self.fail(from, to, &[]);
        }
        self.i += 1;
        Ok(VALUES[t as usize - T::Requirement as usize])
    }

    fn string(&mut self) -> Res<String> {
        if !matches!(self.t(), T::UnqString | T::QString) {
            return self.fail(T::QString, T::UnqString, &[]);
        }
        Ok(self.take())
    }

    fn directive(&mut self, db: &mut RequirementDb) -> Res<bool> {
        match self.t() {
            T::AccTitle => {
                self.i += 1;
                let value = self.expect(T::AccTitleValue)?;
                db.set_acc_title(trim(&value));
            }
            T::AccDescr => {
                self.i += 1;
                let value = self.expect(T::AccDescrValue)?;
                db.set_acc_description(trim(&value));
            }
            T::AccDescrMultilineValue => {
                let value = self.take();
                db.set_acc_description(trim(&value));
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn id_list(&mut self) -> Res<Vec<String>> {
        let mut list = Vec::new();
        loop {
            if !matches!(self.t(), T::Alpha | T::UnqString | T::QString) {
                return self.fail(T::QString, T::UnqString, &[T::Alpha]);
            }
            list.push(self.take());
            if self.t() != T::Comma {
                return Ok(list);
            }
            self.i += 1;
        }
    }

    fn styles(&mut self) -> Res<Vec<String>> {
        let mut styles = Vec::new();
        loop {
            if !is_style_part(self.t()) {
                return self.fail(T::Alpha, T::Semicolon, &[T::Minus, T::Brkt]);
            }
            let mut style = self.take();
            while is_style_part(self.t()) {
                style += &self.take();
            }
            styles.push(style);
            if self.t() != T::Comma {
                return Ok(styles);
            }
            self.i += 1;
        }
    }

    fn class_list(&mut self) -> Res<Option<Vec<String>>> {
        let mut classes = None;
        match self.t() {
            T::StyleSeparator => {
                self.i += 1;
                classes = Some(self.id_list()?);
                self.expect(T::StructStart)?;
            }
            T::StructStart => self.i += 1,
            _ => return self.fail(T::StructStart, T::StructStart, &[T::StyleSeparator]),
        }
        self.expect(T::Newline)?;
        Ok(classes)
    }

    /// Reads a body up to its closing brace. The grammar is right recursive, so Mermaid applies
    /// the fields last to first and the first of two repeated fields wins.
    fn body(&mut self, db: &mut RequirementDb, requirement: bool) -> Res<()> {
        let mut fields: Vec<(T, String)> = Vec::new();
        loop {
            let t = self.t();
            if t == T::StructStop {
                self.i += 1;
                break;
            }
            if t == T::Newline {
                self.i += 1;
                continue;
            }
            let ok = if requirement {
                (T::Id..=T::Verifymthd).contains(&t)
            } else {
                matches!(t, T::Type | T::Docref)
            };
            if !ok {
                return if requirement {
                    self.fail(T::Id, T::Verifymthd, &[T::Newline, T::StructStop])
                } else {
                    self.fail(T::Type, T::Docref, &[T::Newline, T::StructStop])
                };
            }
            self.i += 1;
            self.expect(T::Colonsep)?;
            let value = match t {
                T::Risk => self.keyword(T::LowRisk, T::HighRisk)?.to_string(),
                T::Verifymthd => self.keyword(T::VerifyAnalysis, T::VerifyTest)?.to_string(),
                _ => self.string()?,
            };
            fields.push((t, value));
            self.expect(T::Newline)?;
        }
        for (field, value) in fields.into_iter().rev() {
            match field {
                T::Id => db.new_id = value,
                T::Text => db.new_text = value,
                T::Risk => db.new_risk = value,
                T::Verifymthd => db.new_verify_method = value,
                T::Type => db.new_element_type = value,
                _ => db.new_doc_ref = value,
            }
        }
        Ok(())
    }
}

pub(crate) fn parse_requirement(src: &str, db: &mut RequirementDb) -> Res<()> {
    let units: Vec<u16> = src.encode_utf16().collect();
    let tokens = tokenize(&units);
    let mut p = Parser {
        types: tokens.types,
        texts: tokens.texts,
        starts: tokens.starts,
        i: 0,
        src,
    };

    while p.directive(db)? {
        if p.t() == T::Newline {
            p.i += 1;
        }
    }
    if p.t() != T::Rd {
        return p.fail(
            T::Rd,
            T::Rd,
            &[T::AccTitle, T::AccDescr, T::AccDescrMultilineValue],
        );
    }
    p.i += 1;
    p.expect(T::Newline)?;

    loop {
        let t = p.t();
        match t {
            T::Eof => return Ok(()),
            T::Newline => p.i += 1,
            T::AccTitle | T::AccDescr | T::AccDescrMultilineValue => {
                p.directive(db)?;
            }
            T::DirectionTb | T::DirectionBt | T::DirectionRl | T::DirectionLr => {
                p.i += 1;
                db.direction = match t {
                    T::DirectionTb => "TB",
                    T::DirectionBt => "BT",
                    T::DirectionRl => "RL",
                    _ => "LR",
                };
            }
            _ if (T::Requirement..=T::DesignConstraint).contains(&t) => {
                let kind = p.keyword(T::Requirement, T::DesignConstraint)?;
                let name = p.string()?;
                let classes = p.class_list()?;
                p.body(db, true)?;
                db.add_requirement(&name, kind);
                if let Some(classes) = classes {
                    db.set_class(std::slice::from_ref(&name), &classes)?;
                }
            }
            T::Element => {
                p.i += 1;
                let name = p.string()?;
                let classes = p.class_list()?;
                p.body(db, false)?;
                db.add_element(&name);
                if let Some(classes) = classes {
                    db.set_class(std::slice::from_ref(&name), &classes)?;
                }
            }
            T::Style => {
                p.i += 1;
                let ids = p.id_list()?;
                let styles = p.styles()?;
                db.set_css_style(&ids, &styles)?;
            }
            T::Classdef => {
                p.i += 1;
                let ids = p.id_list()?;
                let styles = p.styles()?;
                db.define_class(&ids, &styles)?;
            }
            T::Class => {
                p.i += 1;
                let ids = p.id_list()?;
                let names = p.id_list()?;
                db.set_class(&ids, &names)?;
            }
            T::UnqString | T::QString => {
                let id = p.take();
                let next = p.t();
                match next {
                    T::StyleSeparator => {
                        p.i += 1;
                        let names = p.id_list()?;
                        db.set_class(std::slice::from_ref(&id), &names)?;
                    }
                    T::EndArrowL | T::Line => {
                        p.i += 1;
                        let relationship = p.keyword(T::Contains, T::Traces)?;
                        p.expect(if next == T::Line {
                            T::EndArrowR
                        } else {
                            T::Line
                        })?;
                        let other = p.string()?;
                        if next == T::Line {
                            db.add_relationship(relationship, &id, &other);
                        } else {
                            db.add_relationship(relationship, &other, &id);
                        }
                    }
                    _ => {
                        return p.fail(
                            T::StyleSeparator,
                            T::StyleSeparator,
                            &[T::EndArrowL, T::Line],
                        );
                    }
                }
            }
            _ => {
                return p.fail(
                    T::Newline,
                    T::Eof,
                    &[
                        T::AccTitle,
                        T::AccDescr,
                        T::AccDescrMultilineValue,
                        T::DirectionTb,
                        T::DirectionBt,
                        T::DirectionRl,
                        T::DirectionLr,
                        T::Requirement,
                        T::FunctionalRequirement,
                        T::InterfaceRequirement,
                        T::PerformanceRequirement,
                        T::PhysicalRequirement,
                        T::DesignConstraint,
                        T::Element,
                        T::Classdef,
                        T::Class,
                        T::Style,
                        T::UnqString,
                        T::QString,
                    ],
                );
            }
        }
    }
}
