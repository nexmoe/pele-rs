//! The parser of a use case diagram: Mermaid's Chevrotain parser for use cases, as pele's
//! `parser.ts` emulates it. It makes the same decisions from the same lookahead, so it accepts
//! the same text and names the same token in its errors. It builds the model directly, where
//! Mermaid builds a syntax tree and then visits it.

use crate::error::{Error, ErrorCode};

use super::db::{
    DraftBoundary, DraftClassAssign, DraftClassDef, DraftElement, DraftEndpoint, DraftJson,
    DraftLabel, DraftMetadata, DraftNote, DraftRelationship, DraftStyle, Drafts, EdgeOccurrence,
    ElementKind, GraphStatement, LabelType, MetadataOccurrence, NodeOccurrence, PropertyValue,
    RelationshipType, Span, StatementKind, UseCaseShape, UsecaseModel, resolve_usecase,
};
use super::json;
use super::lexer::{Source, line_column, tokenize};
use super::tokens::{
    BOUNDARY_LINE, CLOSES_LABEL, FOLLOWS_NAME, LINE, METADATA_FIRST, RELATION, STYLE_PART, T, flags,
};

const ARROW_SOLID: u8 = 0;
const ARROW_BACK: u8 = 1;
const ARROW_LINE: u8 = 2;
const ARROW_CIRCLE: u8 = 3;
const ARROW_CROSS: u8 = 4;
const ARROW_CIRCLE_REVERSED: u8 = 5;
const ARROW_CROSS_REVERSED: u8 = 6;

const NAMES_TEXT: &str = "IDENTIFIER|PLAIN_STRING|MARKDOWN_STRING";
const LABELS: &str = "PLAIN_STRING|MARKDOWN_STRING|LABEL_TEXT";
const STATEMENTS_TEXT: &str = "ACC_TITLE_LINE|ACC_DESCR_LINE|ACC_DESCR_BLOCK|DIRECTION|ACTOR|SYSTEM_BOUNDARY|NOTE|JSON_DECLARATION_START|CLASS_DEF|CLASS|STYLE";
const ARROWS_TEXT: &str = "DEPENDENCY_ARROW|GENERALIZATION|FORWARD_SOLID|BACKWARD_SOLID|MARKERLESS_SOLID|FORWARD_CIRCLE|BACKWARD_CIRCLE|FORWARD_CROSS|BACKWARD_CROSS";

/// The statements Mermaid turns away, as the people most likely to try write them.
const FORBIDDEN: &[&str] = &[
    "allowmixing",
    "newpage",
    "package",
    "rectangle",
    "skinparam",
];

/// An element as the parser reads it.
struct Item {
    id: String,
    label: DraftLabel,
    span: Span,
    generated: bool,
    shape: Option<UseCaseShape>,
    metadata: Option<Vec<super::db::DraftProperty>>,
    stereotype: Option<String>,
    stereotype_span: Option<Span>,
    classes: Vec<String>,
    class_spans: Vec<Span>,
    /// Whether this mention says something about the element, and so declares it.
    explicit: bool,
    node_span: Span,
}

/// The arrow of a relationship, as the parser reads it.
struct Tail {
    explicit_id: Option<String>,
    explicit_id_span: Option<Span>,
    relationship_type: RelationshipType,
    arrow_type: u8,
    label: Option<DraftLabel>,
    minlen: i32,
    target: Item,
    span: Span,
}

/// The parser of a use case diagram.
struct Parser {
    src: Source,
    types: Vec<T>,
    starts: Vec<usize>,
    ends: Vec<usize>,
    /// The token the parser is at.
    p: usize,
    /// Where the last token before the most recent line end stops, which is where its statement
    /// stops.
    last_end: usize,
    drafts: Drafts,
    statements: Vec<GraphStatement>,
    /// The boundary the parser is in.
    parent: Option<String>,
    /// The JSON nodes of the diagram, with the token of each of them.
    pending_json: Vec<(usize, usize)>,
    edge_count: usize,
    note_count: usize,
}

impl Parser {
    fn kind(&self, k: usize) -> T {
        self.types.get(k).copied().unwrap_or(T::Eof)
    }

    fn image(&self, k: usize) -> String {
        self.src.slice(
            self.starts.get(k).copied().unwrap_or(0),
            self.ends.get(k).copied().unwrap_or(0),
        )
    }

    fn span(&self, k: usize) -> Span {
        (
            self.starts.get(k).copied().unwrap_or(0),
            self.ends.get(k).copied().unwrap_or(0),
        )
    }

    fn fail(&self, detail: &str) -> Error {
        let at = self.starts.get(self.p).copied().unwrap_or(0);
        let (line, column) = line_column(&self.src, at);
        Error::new(
            ErrorCode::Syntax,
            format!(
                "Error parsing usecase diagram: {detail} at line {line}, column {column} [{at},{})",
                self.ends.get(self.p).copied().unwrap_or(at)
            ),
        )
        .at(&self.src.text, self.src.byte(at))
    }

    fn mismatch(&self, name: &str) -> Error {
        self.fail(&format!(
            "Expecting token of type --> {name} <-- but found --> '{}' <--",
            self.image(self.p)
        ))
    }

    fn expect(&mut self, kind: T) -> Result<usize, Error> {
        if self.kind(self.p) != kind {
            return Err(self.mismatch(kind.name()));
        }
        let at = self.p;
        self.p += 1;
        Ok(at)
    }

    /// Chevrotain lists every token sequence that would have been accepted.
    fn no_alternative(&self, expected: &str) -> Error {
        let list = expected
            .split('|')
            .enumerate()
            .map(|(i, sequence)| format!("  {}. [{}]", i + 1, sequence))
            .collect::<Vec<_>>()
            .join("\n");
        self.fail(&format!(
            "Expecting: one of these possible Token sequences:\n{list}\nbut found: '{}'",
            self.image(self.p)
        ))
    }

    fn is_key(kind: T) -> bool {
        kind == T::Identifier || kind == T::PlainString
    }

    /// Where the last token before the most recent line end stops.
    fn line_end(&mut self) -> Result<(), Error> {
        self.last_end = self.ends.get(self.p - 1).copied().unwrap_or(0);
        if self.kind(self.p) == T::Newline {
            self.p += 1;
        } else if self.kind(self.p) != T::Eof {
            return Err(self.no_alternative("NEWLINE|EOF"));
        }
        Ok(())
    }

    /// The text of a name or string token, without its quotes.
    fn content(&self, k: usize) -> Span {
        let trim = match self.kind(k) {
            T::MarkdownString => 2,
            T::PlainString => 1,
            _ => 0,
        };
        (
            self.starts.get(k).copied().unwrap_or(0) + trim,
            self.ends.get(k).copied().unwrap_or(0) - trim,
        )
    }

    fn token_label(&self, k: usize) -> DraftLabel {
        let at = self.content(k);
        DraftLabel {
            text: self.src.slice(at.0, at.1),
            kind: if self.kind(k) == T::MarkdownString {
                LabelType::Markdown
            } else {
                LabelType::Text
            },
            span: at,
        }
    }

    /// An unquoted label runs to the next delimiter and is taken from the source as written.
    fn label(&mut self) -> Result<DraftLabel, Error> {
        let kind = self.kind(self.p);
        if kind == T::PlainString || kind == T::MarkdownString {
            let at = self.p;
            self.p += 1;
            return Ok(self.token_label(at));
        }
        if !kind.is_label_text() {
            return Err(self.no_alternative(LABELS));
        }
        let first = self.p;
        while self.kind(self.p).is_label_text() {
            self.p += 1;
        }
        let at = (
            self.starts[first],
            self.ends
                .get(self.p - 1)
                .copied()
                .unwrap_or(self.starts[first]),
        );
        Ok(DraftLabel {
            text: self.src.slice(at.0, at.1),
            kind: LabelType::Text,
            span: at,
        })
    }

    /// One property of a metadata block.
    fn property(&mut self) -> Result<super::db::DraftProperty, Error> {
        let key = self.p;
        if !Self::is_key(self.kind(key)) {
            return Err(self.no_alternative("IDENTIFIER|PLAIN_STRING"));
        }
        self.p += 1;
        self.expect(T::Colon)?;
        let value_at = self.p;
        let kind = self.kind(value_at);
        if !Self::is_key(kind) && kind != T::True && kind != T::False {
            return Err(self.no_alternative("IDENTIFIER|PLAIN_STRING|TRUE|FALSE"));
        }
        self.p += 1;
        let key_span = self.content(key);
        let value_span = self.content(value_at);
        let value = match kind {
            T::True => PropertyValue::Bool(true),
            T::False => PropertyValue::Bool(false),
            _ => PropertyValue::Text(self.src.slice(value_span.0, value_span.1)),
        };
        Ok(super::db::DraftProperty {
            key: self.src.slice(key_span.0, key_span.1),
            value,
            span: (
                self.starts.get(key).copied().unwrap_or(0),
                self.ends.get(value_at).copied().unwrap_or(0),
            ),
            key_span,
            value_span,
        })
    }

    /// Whether another property follows. A comma or line breaks before the closing brace do not
    /// count, but Chevrotain looks only three tokens ahead, so a comma and two line breaks, or
    /// three line breaks, are taken for a separator and the brace after them is then an error.
    fn separator_ahead(&self) -> bool {
        let a = self.kind(self.p);
        let b = self.kind(self.p + 1);
        let c = self.kind(self.p + 2);
        if a == T::Comma {
            return Self::is_key(b) || (b == T::Newline && (c == T::Newline || Self::is_key(c)));
        }
        if a != T::Newline {
            return false;
        }
        b == T::Comma
            || Self::is_key(b)
            || (b == T::Newline && (c == T::Newline || c == T::Comma || Self::is_key(c)))
    }

    fn metadata(&mut self) -> Result<Vec<super::db::DraftProperty>, Error> {
        self.expect(T::MetadataStart)?;
        let mut properties = Vec::new();
        while self.kind(self.p) == T::Newline {
            self.p += 1;
        }
        if Self::is_key(self.kind(self.p)) {
            properties.push(self.property()?);
            while self.separator_ahead() {
                if self.kind(self.p) == T::Comma {
                    self.p += 1;
                } else {
                    while self.kind(self.p) == T::Newline {
                        self.p += 1;
                    }
                    if self.kind(self.p) == T::Comma {
                        self.p += 1;
                    }
                }
                while self.kind(self.p) == T::Newline {
                    self.p += 1;
                }
                properties.push(self.property()?);
            }
            if self.kind(self.p) == T::Comma {
                self.p += 1;
            }
            while self.kind(self.p) == T::Newline {
                self.p += 1;
            }
        }
        self.expect(T::RBrace)?;
        Ok(properties)
    }

    fn class_suffix(
        &mut self,
        classes: &mut Vec<String>,
        spans: &mut Vec<Span>,
    ) -> Result<(), Error> {
        loop {
            self.p += 1;
            let id = self.expect(T::Identifier)?;
            classes.push(self.image(id));
            spans.push(self.span(id));
            if self.kind(self.p) != T::Comma {
                break;
            }
        }
        Ok(())
    }

    /// An id with an optional label in parentheses or brackets, or a string that stands for both.
    fn name(&mut self, brackets: bool) -> Result<Item, Error> {
        let k = self.p;
        let kind = self.kind(k);
        let span = self.content(k);
        let mut item = Item {
            id: String::new(),
            label: DraftLabel {
                text: String::new(),
                kind: LabelType::Text,
                span,
            },
            span,
            generated: kind != T::Identifier,
            shape: None,
            metadata: None,
            stereotype: None,
            stereotype_span: None,
            classes: Vec::new(),
            class_spans: Vec::new(),
            explicit: false,
            node_span: (0, 0),
        };
        if kind == T::Identifier {
            self.p += 1;
            item.id = self.image(k);
            let open = self.kind(self.p);
            if open == T::LParen || (brackets && open == T::LBracket) {
                self.p += 1;
                item.label = self.label()?;
                self.expect(if open == T::LParen {
                    T::RParen
                } else {
                    T::RBracket
                })?;
                item.shape = Some(if open == T::LParen {
                    UseCaseShape::Ellipse
                } else {
                    UseCaseShape::Rect
                });
            } else {
                item.label = self.token_label(k);
            }
        } else if kind == T::PlainString || kind == T::MarkdownString {
            self.p += 1;
            item.label = self.token_label(k);
            item.id = generated_id(&item.label.text);
        } else {
            return Err(self.no_alternative(NAMES_TEXT));
        }
        Ok(item)
    }

    /// An actor or a use case, as the parser reads it.
    fn element(&mut self, actor: bool) -> Result<Item, Error> {
        let first = self.p;
        let mut item = self.name(!actor)?;
        if actor {
            item.shape = None;
        }
        if self.kind(self.p) == T::MetadataStart {
            item.metadata = Some(self.metadata()?);
        }
        if self.kind(self.p) == T::StereotypeStart {
            self.p += 1;
            let text = self.expect(T::StereotypeText)?;
            self.expect(T::StereotypeEnd)?;
            item.stereotype = Some(self.image(text).trim().to_string());
            item.stereotype_span = Some(self.span(text));
        }
        if self.kind(self.p) == T::ClassSeparator {
            let mut classes = std::mem::take(&mut item.classes);
            let mut spans = std::mem::take(&mut item.class_spans);
            self.class_suffix(&mut classes, &mut spans)?;
            item.classes = classes;
            item.class_spans = spans;
        }
        item.explicit =
            item.shape.is_some() || item.metadata.is_some() || item.stereotype.is_some();
        item.node_span = (
            self.starts[first],
            self.ends
                .get(self.p - 1)
                .copied()
                .unwrap_or(self.starts[first]),
        );
        Ok(item)
    }

    /// Whether a label and then one of the operators that can close it come next.
    fn labeled_right(&self, closers: u8) -> bool {
        let kind = self.kind(self.p);
        if kind < T::Usecase || kind > T::MarkdownString {
            return false;
        }
        let mut k = self.p + 1;
        loop {
            let kind = self.kind(k);
            if kind == T::MarkerlessSolid || (flags(kind) & closers) != 0 {
                return true;
            }
            if kind < T::Usecase || kind > T::MarkdownString {
                return false;
            }
            k += 1;
        }
    }

    /// Each dash beyond the second asks for one more rank between the two ends.
    fn minimum_length(&self, k: usize) -> i32 {
        let dashes = self.ends.get(k).copied().unwrap_or(0)
            - self.starts.get(k).copied().unwrap_or(0)
            - if self.kind(k) == T::MarkerlessSolid {
                0
            } else {
                1
            };
        i32::try_from(dashes)
            .unwrap_or(i32::MAX)
            .saturating_sub(1)
            .max(1)
    }

    /// The arrow of a relationship, and what is on the other side of it.
    fn relation_tail(&mut self) -> Result<Tail, Error> {
        let first = self.p;
        let mut tail = Tail {
            explicit_id: None,
            explicit_id_span: None,
            relationship_type: RelationshipType::Association,
            arrow_type: ARROW_SOLID,
            label: None,
            minlen: 1,
            target: Item {
                id: String::new(),
                label: DraftLabel {
                    text: String::new(),
                    kind: LabelType::Text,
                    span: (0, 0),
                },
                span: (0, 0),
                generated: false,
                shape: None,
                metadata: None,
                stereotype: None,
                stereotype_span: None,
                classes: Vec::new(),
                class_spans: Vec::new(),
                explicit: false,
                node_span: (0, 0),
            },
            span: (0, 0),
        };
        if self.kind(self.p) == T::Identifier && self.kind(self.p + 1) == T::At {
            tail.explicit_id = Some(self.image(self.p));
            tail.explicit_id_span = Some(self.span(self.p));
            self.p += 2;
        }
        let k = self.p;
        let kind = self.kind(self.p);
        self.p += 1;
        if kind == T::DependencyArrow {
            self.expect(T::Colon)?;
            if self.kind(self.p) != T::Include && self.kind(self.p) != T::Extend {
                return Err(self.no_alternative("INCLUDE|EXTEND"));
            }
            tail.relationship_type = if self.kind(self.p) == T::Include {
                RelationshipType::Include
            } else {
                RelationshipType::Extend
            };
            tail.label = Some(DraftLabel {
                text: tail.relationship_type.as_str().to_string(),
                kind: LabelType::Text,
                span: self.span(self.p),
            });
            self.p += 1;
        } else if kind == T::Generalization {
            tail.relationship_type = RelationshipType::Generalization;
        } else if kind == T::ForwardSolid {
            tail.minlen = self.minimum_length(k);
        } else if kind == T::BackwardSolid {
            tail.arrow_type = ARROW_BACK;
            // Only the shortest operator can open a label: `A <-- label -- B`.
            let length = self.ends[k] - self.starts[k];
            if length == 3 && self.labeled_right(0) {
                tail.label = Some(self.label()?);
                let close = self.expect(T::MarkerlessSolid)?;
                tail.minlen = self.minimum_length(close);
            } else {
                tail.minlen = self.minimum_length(k);
            }
        } else if kind == T::MarkerlessSolid {
            tail.arrow_type = ARROW_LINE;
            let length = self.ends[k] - self.starts[k];
            if length == 2 && self.labeled_right(CLOSES_LABEL) {
                tail.label = Some(self.label()?);
                let close = self.kind(self.p);
                match close {
                    T::ForwardSolid => tail.arrow_type = ARROW_SOLID,
                    T::ForwardCircle => tail.arrow_type = ARROW_CIRCLE,
                    T::ForwardCross => tail.arrow_type = ARROW_CROSS,
                    T::MarkerlessSolid => {}
                    _ => {
                        return Err(self.no_alternative(
                            "FORWARD_SOLID|MARKERLESS_SOLID|FORWARD_CIRCLE|FORWARD_CROSS",
                        ));
                    }
                }
                if tail.arrow_type == ARROW_SOLID || tail.arrow_type == ARROW_LINE {
                    tail.minlen = self.minimum_length(self.p);
                }
                self.p += 1;
            } else {
                tail.minlen = self.minimum_length(k);
            }
        } else if kind == T::ForwardCircle {
            tail.arrow_type = ARROW_CIRCLE;
        } else if kind == T::ForwardCross {
            tail.arrow_type = ARROW_CROSS;
        } else if kind == T::BackwardCircle || kind == T::BackwardCross {
            tail.arrow_type = if kind == T::BackwardCircle {
                ARROW_CIRCLE_REVERSED
            } else {
                ARROW_CROSS_REVERSED
            };
            if self.labeled_right(0) {
                tail.label = Some(self.label()?);
                self.expect(T::MarkerlessSolid)?;
            }
        } else {
            self.p -= 1;
            return Err(self.no_alternative(ARROWS_TEXT));
        }
        tail.target = self.element(false)?;
        tail.span = (
            self.starts[first],
            self.ends
                .get(self.p - 1)
                .copied()
                .unwrap_or(self.starts[first]),
        );
        Ok(tail)
    }

    /// A relationship, as a statement of the source.
    fn relationship(
        &mut self,
        source: &Item,
        declaration: bool,
        tail: Tail,
        mut nodes: Vec<NodeOccurrence>,
    ) -> Result<GraphStatement, Error> {
        if tail.target.explicit {
            let draft = draft_element(&tail.target, ElementKind::Usecase, self.parent.clone());
            self.drafts.elements.push(draft);
        }
        self.drafts.relationships.push(DraftRelationship {
            source: endpoint(source, declaration),
            target: endpoint(&tail.target, tail.target.explicit),
            span: tail.span,
            id: tail.explicit_id.clone(),
            id_span: tail.explicit_id_span,
            relationship_type: tail.relationship_type,
            arrow_type: tail.arrow_type,
            label: tail.label.clone(),
            minlen: tail.minlen,
        });
        nodes.push(occurrence(&tail.target, tail.target.explicit));
        let mut out = statement(StatementKind::Edge);
        out.nodes = nodes;
        let id = match &tail.explicit_id {
            Some(id) => id.clone(),
            None => {
                let id = format!("edge-{}", self.edge_count);
                self.edge_count += 1;
                id
            }
        };
        out.edges = vec![EdgeOccurrence {
            id,
            span: out.span,
            label_span: tail.label.as_ref().map(|label| label.span),
            id_span: tail.explicit_id_span,
            metadata: Vec::new(),
        }];
        Ok(out)
    }

    /// `actor A, B` declares several actors; at the top level `actor A --> B` declares one and
    /// relates it.
    fn actors(&mut self, top: bool) -> Result<GraphStatement, Error> {
        self.p += 1;
        let mut items = vec![self.element(true)?];
        let mut tail = None;
        if self.kind(self.p) == T::Comma {
            loop {
                self.p += 1;
                items.push(self.element(true)?);
                if self.kind(self.p) != T::Comma {
                    break;
                }
            }
        } else if top && (flags(self.kind(self.p)) & RELATION) != 0 {
            tail = Some(self.relation_tail()?);
        }
        self.line_end()?;
        for item in &items {
            let draft = draft_element(item, ElementKind::Actor, self.parent.clone());
            self.drafts.elements.push(draft);
        }
        let nodes: Vec<NodeOccurrence> = items.iter().map(|item| occurrence(item, true)).collect();
        match tail {
            Some(tail) => self.relationship(&items[0], true, tail, nodes),
            None => {
                let mut out = statement(StatementKind::Node);
                out.nodes = nodes;
                Ok(out)
            }
        }
    }

    /// A use case, as the parser reads it.
    fn entity(&mut self, top: bool) -> Result<GraphStatement, Error> {
        let mut source = self.element(false)?;
        let tail = if top && (flags(self.kind(self.p)) & RELATION) != 0 {
            Some(self.relation_tail()?)
        } else {
            None
        };
        self.line_end()?;
        if tail.is_none() {
            source.explicit = true;
            if source.shape.is_none() {
                source.shape = Some(UseCaseShape::Ellipse);
            }
        }
        if source.explicit {
            let draft = draft_element(&source, ElementKind::Usecase, self.parent.clone());
            self.drafts.elements.push(draft);
        }
        let nodes = vec![occurrence(&source, source.explicit)];
        match tail {
            Some(tail) => self.relationship(&source, source.explicit, tail, nodes),
            None => {
                let mut out = statement(StatementKind::Node);
                out.nodes = nodes;
                Ok(out)
            }
        }
    }

    /// A system boundary, as the parser reads it.
    fn boundary(&mut self) -> Result<GraphStatement, Error> {
        self.p += 1;
        let mut named = self.name(true)?;
        let inline = if self.kind(self.p) == T::MetadataStart {
            Some(self.metadata()?)
        } else {
            None
        };
        if self.kind(self.p) == T::ClassSeparator {
            let mut classes = std::mem::take(&mut named.classes);
            let mut spans = std::mem::take(&mut named.class_spans);
            self.class_suffix(&mut classes, &mut spans)?;
            named.classes = classes;
            named.class_spans = spans;
        }
        let group = DraftBoundary {
            id: named.id.clone(),
            label: named.label.clone(),
            span: named.span,
            generated: named.generated,
            classes: named.classes.clone(),
        };
        self.line_end()?;
        self.drafts.boundaries.push(group.clone());
        self.parent = Some(group.id.clone());
        let mut children: Vec<GraphStatement> = Vec::new();
        while (flags(self.kind(self.p)) & BOUNDARY_LINE) != 0 {
            children.push(self.line(false)?);
        }
        self.parent = None;
        let end = self.expect(T::End)?;
        self.line_end()?;
        let mut out = statement(StatementKind::Group);
        out.group = Some(group.id.clone());
        out.id_span = Some(group.span);
        out.title_span = Some(group.label.span);
        out.end_span = Some(self.span(end));
        out.class_spans = named.class_spans.clone();
        if !children.is_empty() {
            out.children = children;
        }
        if let Some(inline) = inline {
            out.metadata = occurrences(&inline);
            let index = self.statements.len();
            self.drafts.metadata.push(DraftMetadata {
                target: group.id.clone(),
                span: group.span,
                properties: inline,
                statement: index,
            });
        }
        Ok(out)
    }

    /// One style value.
    fn style(&mut self) -> Result<String, Error> {
        let first = self.p;
        let kind = self.kind(self.p);
        if kind.is_word() || kind == T::CssIdentifier {
            self.p += 1;
        } else if kind == T::MarkerlessSolid {
            self.p += 1;
            if !self.kind(self.p).is_word() {
                return Err(self.mismatch("WORD"));
            }
            self.p += 1;
        } else {
            return Err(self.no_alternative("WORD|CSS_IDENTIFIER|MARKERLESS_SOLID"));
        }
        self.expect(T::Colon)?;
        if (flags(self.kind(self.p)) & STYLE_PART) == 0 {
            return Err(self.fail(&format!(
                "Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[{}]>\nbut found: '{}'",
                STYLE_PARTS_TEXT,
                self.image(self.p)
            )));
        }
        while (flags(self.kind(self.p)) & STYLE_PART) != 0 {
            self.p += 1;
        }
        // The value is taken from the source, so the spaces inside `1px solid red` survive.
        Ok(self
            .src
            .slice(self.starts[first], self.ends[self.p - 1])
            .replace("\\,", ","))
    }

    fn styles(&mut self) -> Result<Vec<String>, Error> {
        let mut out = vec![self.style()?];
        while self.kind(self.p) == T::Comma {
            self.p += 1;
            out.push(self.style()?);
        }
        Ok(out)
    }

    fn identifiers(&mut self) -> Result<Vec<usize>, Error> {
        let mut out = vec![self.expect(T::Identifier)?];
        while self.kind(self.p) == T::Comma {
            self.p += 1;
            out.push(self.expect(T::Identifier)?);
        }
        Ok(out)
    }

    fn reference(&self, k: usize) -> NodeOccurrence {
        let at = self.content(k);
        NodeOccurrence {
            id: self.image(k),
            span: at,
            id_span: at,
            label_span: None,
            defines: false,
            stereotype_span: None,
            metadata: Vec::new(),
            class_spans: Vec::new(),
        }
    }

    /// The token sequences Mermaid would accept where a statement is expected.
    fn statement_sequences(&self) -> String {
        let names = NAMES_TEXT.split('|').collect::<Vec<_>>();
        let mut metadata: Vec<String> = Vec::new();
        for name in &names {
            for next in ["NEWLINE", "IDENTIFIER", "PLAIN_STRING", "RBRACE"] {
                metadata.push(format!("{name}, METADATA_START, {next}"));
            }
        }
        let mut follows = vec![
            "STEREOTYPE_START".to_string(),
            "CLASS_SEPARATOR".to_string(),
            "IDENTIFIER".to_string(),
        ];
        follows.extend(ARROWS_TEXT.split('|').map(str::to_string));
        follows.push("NEWLINE".to_string());
        follows.push("EOF".to_string());
        let mut entities: Vec<String> = Vec::new();
        for (i, name) in names.iter().enumerate() {
            let mut nexts = follows.clone();
            if i == 0 {
                nexts.insert(0, "LPAREN".to_string());
                nexts.insert(1, "LBRACKET".to_string());
            }
            for next in nexts {
                entities.push(format!("{name}, {next}"));
            }
        }
        let mut all = vec![STATEMENTS_TEXT.to_string()];
        all.extend(metadata.clone());
        all.extend(entities);
        all.extend(metadata);
        all.join("|")
    }

    /// A statement at the top level of the diagram.
    fn top_statement(&mut self) -> Result<GraphStatement, Error> {
        let k = self.p;
        let kind = self.kind(self.p);
        if matches!(kind, T::AccTitleLine | T::AccDescrLine | T::AccDescrBlock) {
            self.p += 1;
            let text = self.image(k);
            if kind == T::AccDescrBlock {
                let start = text.find('{').map(|at| at + 1).unwrap_or(0);
                let end = text.rfind('}').unwrap_or(text.len());
                self.drafts.acc_description = text[start..end].trim().to_string();
            } else {
                let start = text.find(':').map(|at| at + 1).unwrap_or(0);
                if kind == T::AccDescrLine {
                    self.drafts.acc_description = text[start..].trim().to_string();
                } else {
                    self.drafts.acc_title = text[start..].trim().to_string();
                }
            }
            let out = statement(if kind == T::AccTitleLine {
                StatementKind::AccTitle
            } else {
                StatementKind::AccDescr
            });
            self.line_end()?;
            return Ok(out);
        }
        if kind == T::Direction {
            self.p += 1;
            let value = self.kind(self.p);
            if value < T::Td || value > T::Rl {
                return Err(self.no_alternative("TD|TB|BT|LR|RL"));
            }
            self.drafts.direction = Some(if value == T::Td {
                super::db::Direction::Tb
            } else {
                match self.image(self.p).as_str() {
                    "BT" => super::db::Direction::Bt,
                    "LR" => super::db::Direction::Lr,
                    "RL" => super::db::Direction::Rl,
                    _ => super::db::Direction::Tb,
                }
            });
            self.p += 1;
            self.line_end()?;
            return Ok(statement(StatementKind::Direction));
        }
        if kind == T::Actor {
            return self.actors(true);
        }
        if kind == T::SystemBoundary {
            return self.boundary();
        }
        if kind == T::Note {
            self.p += 1;
            self.expect(T::For)?;
            let target = self.expect(T::Identifier)?;
            let text = self.label()?;
            self.drafts.notes.push(DraftNote {
                target: self.image(target),
                span: self.span(target),
                label: text.clone(),
            });
            let mut out = statement(StatementKind::Note);
            out.reference = Some(format!("note-{}", self.note_count));
            self.note_count += 1;
            out.reference_span = Some(text.span);
            out.nodes = vec![self.reference(target)];
            self.line_end()?;
            return Ok(out);
        }
        if kind == T::JsonDeclarationStart {
            self.p += 1;
            let literal = self.expect(T::JsonObjectLiteral)?;
            let mut classes = Vec::new();
            let mut class_spans = Vec::new();
            if self.kind(self.p) == T::ClassSeparator {
                self.class_suffix(&mut classes, &mut class_spans)?;
            }
            let mut from = self.starts[k] + 4;
            while self.src.at(from) == 32 || self.src.at(from) == 9 {
                from += 1;
            }
            let mut to = from;
            while super::lexer::is_word_char(self.src.at(to)) {
                to += 1;
            }
            let json = DraftJson {
                id: self.src.slice(from, to),
                value: json::JsonValue::Object(Vec::new()),
                span: (from, to),
                classes: classes.clone(),
            };
            self.drafts.jsons.push(json);
            self.pending_json
                .push((self.drafts.jsons.len() - 1, literal));
            let mut out = statement(StatementKind::Json);
            out.nodes = vec![NodeOccurrence {
                id: self.src.slice(from, to),
                span: (from, to),
                id_span: (from, to),
                label_span: None,
                defines: true,
                stereotype_span: None,
                metadata: Vec::new(),
                class_spans: class_spans.clone(),
            }];
            out.class_spans = class_spans;
            self.line_end()?;
            return Ok(out);
        }
        if kind == T::ClassDef {
            self.p += 1;
            let ids = self.identifiers()?;
            let styles = self.styles()?;
            self.drafts.class_defs.push(DraftClassDef {
                ids: ids.iter().map(|id| self.image(*id)).collect(),
                styles,
            });
            let mut out = statement(StatementKind::ClassDef);
            out.reference = Some(self.image(ids[0]));
            out.reference_span = Some(self.span(ids[0]));
            self.line_end()?;
            return Ok(out);
        }
        if kind == T::Class {
            self.p += 1;
            let targets = self.identifiers()?;
            let classes = self.identifiers()?;
            self.drafts.classes.push(DraftClassAssign {
                targets: targets
                    .iter()
                    .map(|id| (self.image(*id), self.span(*id)))
                    .collect(),
                classes: classes.iter().map(|id| self.image(*id)).collect(),
            });
            let mut out = statement(StatementKind::ClassAssign);
            out.reference = Some(self.image(classes[0]));
            out.reference_span = Some(self.span(classes[0]));
            out.nodes = targets.iter().map(|id| self.reference(*id)).collect();
            self.line_end()?;
            return Ok(out);
        }
        if kind == T::Style {
            self.p += 1;
            let target = self.expect(T::Identifier)?;
            let styles = self.styles()?;
            self.drafts.styles.push(DraftStyle {
                target: self.image(target),
                span: self.span(target),
                styles,
            });
            let mut out = statement(StatementKind::Style);
            out.nodes = vec![self.reference(target)];
            self.line_end()?;
            return Ok(out);
        }
        let next = self.kind(self.p + 1);
        if next == T::MetadataStart {
            if (flags(self.kind(self.p + 2)) & METADATA_FIRST) == 0 {
                return Err(self.no_alternative(&self.statement_sequences()));
            }
            self.p += 1;
            let mut target = self.reference(k);
            if kind != T::Identifier {
                target.id = generated_id(&self.src.slice(target.span.0, target.span.1));
            }
            let assigned = self.metadata()?;
            let mut out = statement(StatementKind::Metadata);
            out.nodes = vec![target.clone()];
            out.metadata = occurrences(&assigned);
            let index = self.statements.len();
            self.drafts.metadata.push(DraftMetadata {
                target: target.id.clone(),
                span: target.span,
                properties: assigned,
                statement: index,
            });
            self.line_end()?;
            return Ok(out);
        }
        let follows = (flags(next) & FOLLOWS_NAME) != 0
            || (kind == T::Identifier && (next == T::LParen || next == T::LBracket));
        // Mermaid turns away the PlantUML statements people are most likely to try.
        if !follows
            || (kind == T::Identifier && FORBIDDEN.contains(&self.image(k).to_lowercase().as_str()))
        {
            return Err(self.no_alternative(&self.statement_sequences()));
        }
        self.entity(true)
    }

    /// A line of the diagram.
    fn line(&mut self, top: bool) -> Result<GraphStatement, Error> {
        let k = self.p;
        let kind = self.kind(self.p);
        if kind == T::Newline {
            self.p += 1;
            let mut out = statement(StatementKind::Blank);
            out.span = self.span(k);
            return Ok(out);
        }
        if kind == T::Comment {
            self.p += 1;
            self.line_end()?;
            let mut out = statement(StatementKind::Comment);
            out.span = self.span(k);
            return Ok(out);
        }
        let mut out = if top {
            self.top_statement()?
        } else if kind == T::Actor {
            self.actors(false)?
        } else {
            self.entity(false)?
        };
        let span = (self.starts[k], self.last_end);
        out.span = span;
        for edge in out.edges.iter_mut() {
            edge.span = span;
        }
        Ok(out)
    }
}

/// The style parts, as the error message of the parser writes them.
const STYLE_PARTS_TEXT: &str = "WORD|PLAIN_STRING|NUMBER|HASH_COLOR|CSS_IDENTIFIER|CSS_ESCAPED_COMMA|DASH|DOT|PERCENT|CSS_PUNCTUATION|COLON|LPAREN|RPAREN|LBRACKET|RBRACKET|LBRACE|RBRACE|AT|MARKERLESS_SOLID";

/// An id made of a label: every character that is not a word character becomes an underscore.
fn generated_id(label: &str) -> String {
    label
        .encode_utf16()
        .map(|c| {
            if (48..=57).contains(&c)
                || (65..=90).contains(&c)
                || (97..=122).contains(&c)
                || c == 95
            {
                char::from_u32(c as u32).unwrap_or('_')
            } else {
                '_'
            }
        })
        .collect()
}

fn statement(kind: StatementKind) -> GraphStatement {
    GraphStatement {
        kind,
        span: (0, 0),
        nodes: Vec::new(),
        edges: Vec::new(),
        group: None,
        id_span: None,
        title_span: None,
        end_span: None,
        reference: None,
        reference_span: None,
        children: Vec::new(),
        metadata: Vec::new(),
        class_spans: Vec::new(),
    }
}

fn occurrences(properties: &[super::db::DraftProperty]) -> Vec<MetadataOccurrence> {
    properties
        .iter()
        .map(|property| MetadataOccurrence {
            key: property.key.clone(),
            span: property.span,
            key_span: property.key_span,
            value_span: property.value_span,
        })
        .collect()
}

fn draft_element(item: &Item, kind: ElementKind, parent: Option<String>) -> DraftElement {
    DraftElement {
        id: item.id.clone(),
        kind,
        label: item.label.clone(),
        span: item.span,
        generated: item.generated,
        parent,
        shape: item.shape,
        metadata: item.metadata.clone(),
        stereotype: item.stereotype.clone(),
        classes: item.classes.clone(),
    }
}

fn endpoint(item: &Item, declares: bool) -> DraftEndpoint {
    DraftEndpoint {
        id: item.id.clone(),
        label: item.label.clone(),
        span: item.span,
        generated: item.generated,
        declares,
        classed: !item.class_spans.is_empty(),
    }
}

fn occurrence(item: &Item, defines: bool) -> NodeOccurrence {
    NodeOccurrence {
        id: item.id.clone(),
        span: item.node_span,
        id_span: item.span,
        label_span: Some(item.label.span),
        defines,
        stereotype_span: item.stereotype_span,
        metadata: item
            .metadata
            .as_ref()
            .map(|properties| occurrences(properties))
            .unwrap_or_default(),
        class_spans: item.class_spans.clone(),
    }
}

/// Reads a use case diagram.
pub(crate) fn parse_usecase(
    source: &str,
    title: Option<&str>,
    max_depth: usize,
) -> Result<UsecaseModel, Error> {
    let src = Source::new(source);
    let tokens = tokenize(&src)?;
    let mut parser = Parser {
        src,
        types: tokens.types,
        starts: tokens.starts,
        ends: tokens.ends,
        p: 0,
        last_end: 0,
        drafts: Drafts::default(),
        statements: Vec::new(),
        parent: None,
        pending_json: Vec::new(),
        edge_count: 0,
        note_count: 0,
    };
    let header = parser.expect(T::Usecase)?;
    parser.line_end()?;
    while (flags(parser.kind(parser.p)) & LINE) != 0 {
        let out = parser.line(true)?;
        parser.statements.push(out);
    }
    if parser.kind(parser.p) != T::Eof {
        return Err(parser.fail(&format!(
            "Redundant input, expecting EOF but found: {}",
            parser.image(parser.p)
        )));
    }
    // The whole text has to be well formed before the JSON in it is looked at, as in Mermaid.
    let pending = parser.pending_json.clone();
    for (index, literal) in pending {
        let at = parser.starts[literal];
        let end = parser.ends[literal];
        let text = parser.src.slice(at, end);
        match json::parse_json(&text, max_depth) {
            Ok(value) => {
                parser.drafts.jsons[index].value = value;
            }
            Err(offset) => {
                let units = text[..offset].encode_utf16().count();
                let byte = parser.src.byte(at + units);
                return Err(Error::new(
                    ErrorCode::Syntax,
                    format!(
                        "Invalid JSON in a json node: {}",
                        &text[offset..offset.saturating_add(1)]
                    ),
                )
                .at(&parser.src.text, byte));
            }
        }
    }
    let header_span = parser.span(header);
    resolve_usecase(
        &parser.src,
        &mut parser.drafts,
        header_span,
        parser.statements,
        title,
    )
}
