//! A hand-written parser for Mermaid's flowchart grammar, over the tokens of `lexer.rs`.
//! Ported from pele's `diagrams/flowchart/parser.ts`.

use super::db::{DocItem, FlowDb, FlowText, LabelType, LinkInfo};
use super::lexer::{T, tokenize};
use crate::error::{Error, ErrorCode};
use crate::util::{is_space, trim};

fn is_id(t: T) -> bool {
    matches!(
        t,
        T::Num
            | T::NodeString
            | T::Down
            | T::Minus
            | T::Default
            | T::Comma
            | T::Colon
            | T::Amp
            | T::Brkt
            | T::Mult
            | T::UnicodeText
    )
}

fn is_alphanum(t: T) -> bool {
    matches!(
        t,
        T::Num
            | T::UnicodeText
            | T::NodeString
            | T::Dir
            | T::Down
            | T::Minus
            | T::Comma
            | T::Colon
            | T::Amp
            | T::Brkt
            | T::Mult
    )
}

fn is_no_tags(t: T) -> bool {
    matches!(
        t,
        T::Num
            | T::NodeString
            | T::Space
            | T::Minus
            | T::Amp
            | T::UnicodeText
            | T::Colon
            | T::Mult
            | T::Brkt
            | T::Style
            | T::LinkStyle
            | T::ClassDef
            | T::Class
            | T::Click
            | T::Graph
            | T::Dir
            | T::Subgraph
            | T::EndKw
            | T::Down
            | T::Up
            | T::StartLink
    )
}

fn is_style_part(t: T) -> bool {
    matches!(
        t,
        T::Num | T::NodeString | T::Colon | T::Space | T::Brkt | T::Style
    )
}

const TEXT_TOKENS: &str = "'UNICODE_TEXT', 'TEXT', 'TAGSTART'";
const STATEMENT_START: &str = "'SEMI', 'NEWLINE', 'SPACE', 'EOF', 'subgraph', 'acc_title', 'acc_descr', \
     'acc_descr_multiline_value', 'STYLE', 'LINKSTYLE', 'CLASSDEF', 'CLASS', 'CLICK', 'NODE_STRING'";
const DIRECTIONS: [&str; 5] = ["TB", "BT", "RL", "LR", "TD"];

struct Frame {
    id: Option<FlowText>,
    title: Option<FlowText>,
    same: bool,
    list: Vec<DocItem>,
    collect: bool,
}

/// Mermaid trims whitespace between a closing brace and the end of its line before parsing:
/// `/}\s*\n/g` becomes `}\n`.
fn trim_brace_trail(src: &str) -> String {
    if !src.contains('}') {
        return src.to_string();
    }
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(at) = rest.find('}') {
        out.push_str(&rest[..=at]);
        rest = &rest[at + 1..];
        let ws = rest.len() - rest.trim_start_matches(is_space).len();
        if let Some(nl) = rest[..ws].rfind('\n') {
            out.push('\n');
            rest = &rest[nl + 1..];
        }
    }
    out.push_str(rest);
    out
}

struct Parser<'a> {
    types: Vec<T>,
    texts: Vec<String>,
    starts: Vec<usize>,
    i: usize,
    src: &'a str,
    db: &'a mut FlowDb,
    pending_data: Option<String>,
}

type Res<T> = Result<T, Error>;

impl Parser<'_> {
    fn t(&self) -> T {
        self.types[self.i]
    }

    fn take(&mut self) -> String {
        let text = std::mem::take(&mut self.texts[self.i]);
        self.i += 1;
        text
    }

    /// A byte offset in the source for a UTF-16 offset from the lexer.
    fn byte_offset(&self, units: usize) -> usize {
        let mut count = 0;
        for (i, c) in self.src.char_indices() {
            if count >= units {
                return i;
            }
            count += c.len_utf16();
        }
        self.src.len()
    }

    fn fail<X>(&self, expected: &str) -> Res<X> {
        let at = self.byte_offset(self.starts[self.i]);
        if self.t() == T::Error {
            return Err(Error::new(ErrorCode::Syntax, "Unrecognized text.").at(self.src, at));
        }
        Err(Error::new(
            ErrorCode::Syntax,
            format!("Expecting {expected}, got '{}'", self.t().name()),
        )
        .at(self.src, at))
    }

    fn expect(&mut self, t: T, also: &str) -> Res<String> {
        if self.t() != t {
            let sep = if also.is_empty() { "" } else { ", " };
            return self.fail(&format!("'{}'{sep}{also}", t.name()));
        }
        Ok(self.take())
    }

    fn separator(&mut self) -> Res<()> {
        if !matches!(self.t(), T::Newline | T::Semi | T::Eof) {
            return self.fail("'SEMI', 'NEWLINE', 'EOF'");
        }
        self.i += 1;
        Ok(())
    }

    fn run(&mut self, first: fn(T) -> bool, expected: &str) -> Res<String> {
        if !first(self.t()) {
            return self.fail(expected);
        }
        let mut s = self.take();
        while first(self.t()) {
            s += &self.take();
        }
        Ok(s)
    }

    fn id_string(&mut self) -> Res<String> {
        self.run(is_id, "'NODE_STRING'")
    }

    fn alpha_num(&mut self) -> Res<String> {
        self.run(is_alphanum, "'NODE_STRING'")
    }

    fn text(&mut self) -> Res<FlowText> {
        let kind = match self.t() {
            T::Str => LabelType::String,
            T::MdStr => LabelType::Markdown,
            T::Text | T::TagStart | T::TagEnd | T::UnicodeText => LabelType::Text,
            _ => return self.fail(&format!("{TEXT_TOKENS}, 'TAGEND', 'STR', 'MD_STR'")),
        };
        let mut s = self.take();
        while matches!(self.t(), T::Text | T::TagStart | T::TagEnd | T::UnicodeText) {
            s += &self.take();
        }
        Ok(FlowText { text: s, kind })
    }

    fn closed(&mut self, t: T) -> Res<()> {
        self.expect(t, TEXT_TOKENS).map(drop)
    }

    fn edge_text(&mut self) -> Res<FlowText> {
        let kind = match self.t() {
            T::Str => LabelType::String,
            T::MdStr => LabelType::Markdown,
            T::EdgeText | T::UnicodeText => LabelType::Text,
            _ => return self.fail("'EDGE_TEXT', 'STR', 'MD_STR'"),
        };
        let mut s = self.take();
        while matches!(self.t(), T::EdgeText | T::UnicodeText) {
            s += &self.take();
        }
        Ok(FlowText { text: s, kind })
    }

    fn text_no_tags(&mut self) -> Res<FlowText> {
        let t = self.t();
        let kind = if t == T::Str {
            LabelType::Text
        } else if t == T::MdStr {
            LabelType::Markdown
        } else if is_no_tags(t) {
            LabelType::Text
        } else {
            return self.fail("'NODE_STRING', 'STR', 'MD_STR'");
        };
        let mut s = self.take();
        while is_no_tags(self.t()) {
            s += &self.take();
        }
        Ok(FlowText { text: s, kind })
    }

    fn shape_data(&mut self) -> String {
        let mut s = self.take();
        while self.t() == T::ShapeData {
            s += &self.take();
        }
        s
    }

    fn vertex(&mut self) -> Res<String> {
        let id = self.id_string()?;
        let (open, close, shape) = match self.t() {
            T::Sqs => (true, T::Sqe, "square"),
            T::DoubleCircleStart => (true, T::DoubleCircleEnd, "doublecircle"),
            T::EllipseStart => (true, T::EllipseEnd, "ellipse"),
            T::StadiumStart => (true, T::StadiumEnd, "stadium"),
            T::SubroutineStart => (true, T::SubroutineEnd, "subroutine"),
            T::CylinderStart => (true, T::CylinderEnd, "cylinder"),
            T::TagEnd => (true, T::Sqe, "odd"),
            _ => (false, T::End, ""),
        };
        if open {
            self.i += 1;
            let label = self.text()?;
            self.closed(close)?;
            self.db
                .add_vertex(&id, Some(&label), Some(shape), None, None)?;
            return Ok(id);
        }
        match self.t() {
            T::Ps => {
                self.i += 1;
                if self.t() == T::Ps {
                    self.i += 1;
                    let label = self.text()?;
                    self.closed(T::Pe)?;
                    self.expect(T::Pe, "")?;
                    self.db
                        .add_vertex(&id, Some(&label), Some("circle"), None, None)?;
                } else {
                    let label = self.text()?;
                    self.closed(T::Pe)?;
                    self.db
                        .add_vertex(&id, Some(&label), Some("round"), None, None)?;
                }
            }
            T::VertexWithPropsStart => {
                self.i += 1;
                self.expect(T::NodeString, "")?;
                self.expect(T::Colon, "")?;
                self.expect(T::NodeString, "")?;
                self.expect(T::Pipe, "")?;
                let label = self.text()?;
                self.closed(T::Sqe)?;
                self.db
                    .add_vertex(&id, Some(&label), Some("rect"), None, None)?;
            }
            T::DiamondStart => {
                self.i += 1;
                if self.t() == T::DiamondStart {
                    self.i += 1;
                    let label = self.text()?;
                    self.closed(T::DiamondStop)?;
                    self.expect(T::DiamondStop, "")?;
                    self.db
                        .add_vertex(&id, Some(&label), Some("hexagon"), None, None)?;
                } else {
                    let label = self.text()?;
                    self.closed(T::DiamondStop)?;
                    self.db
                        .add_vertex(&id, Some(&label), Some("diamond"), None, None)?;
                }
            }
            T::TrapStart => {
                self.i += 1;
                let label = self.text()?;
                if self.t() == T::InvTrapEnd {
                    self.i += 1;
                    self.db
                        .add_vertex(&id, Some(&label), Some("lean_right"), None, None)?;
                } else {
                    self.closed(T::TrapEnd)?;
                    self.db
                        .add_vertex(&id, Some(&label), Some("trapezoid"), None, None)?;
                }
            }
            T::InvTrapStart => {
                self.i += 1;
                let label = self.text()?;
                if self.t() == T::TrapEnd {
                    self.i += 1;
                    self.db
                        .add_vertex(&id, Some(&label), Some("lean_left"), None, None)?;
                } else {
                    self.closed(T::InvTrapEnd)?;
                    self.db
                        .add_vertex(&id, Some(&label), Some("inv_trapezoid"), None, None)?;
                }
            }
            _ => self.db.add_vertex(&id, None, None, None, None)?,
        }
        Ok(id)
    }

    fn styled_vertex(&mut self) -> Res<String> {
        let id = self.vertex()?;
        if self.t() == T::StyleSeparator {
            self.i += 1;
            let class = self.id_string()?;
            self.db.set_class(&id, &class);
        }
        Ok(id)
    }

    fn skip_spaces(&mut self) {
        while self.t() == T::Space {
            self.i += 1;
        }
    }

    fn node(&mut self) -> Res<Vec<String>> {
        let mut ids = vec![self.styled_vertex()?];
        self.pending_data = None;
        loop {
            let data = (self.t() == T::ShapeData).then(|| self.shape_data());
            if self.t() != T::Space {
                self.pending_data = data;
                return Ok(ids);
            }
            self.skip_spaces();
            if self.t() != T::Amp {
                if data.is_some() {
                    return self.fail("'AMP'");
                }
                return Ok(ids);
            }
            self.i += 1;
            if self.t() != T::Space {
                return self.fail("'SPACE'");
            }
            self.skip_spaces();
            let next = self.styled_vertex()?;
            if let Some(data) = data {
                let last = ids[ids.len() - 1].clone();
                self.db.add_vertex(&last, None, None, None, Some(&data))?;
            }
            ids.push(next);
        }
    }

    fn link(&mut self) -> Res<LinkInfo> {
        let id = (self.t() == T::LinkId).then(|| self.take());
        if self.t() == T::Link {
            let end = self.take();
            let mut info = self.db.destruct_link(&end, None);
            info.id = id;
            if self.t() == T::Pipe {
                self.i += 1;
                info.text = Some(self.text()?);
                self.closed(T::Pipe)?;
                if self.t() == T::Space {
                    self.i += 1;
                }
            }
            return Ok(info);
        }
        if self.t() == T::StartLink {
            let start = self.take();
            let label = self.edge_text()?;
            let end = self.expect(T::Link, "'EDGE_TEXT'")?;
            let mut info = self.db.destruct_link(&end, Some(&start));
            info.text = Some(label);
            info.id = id;
            return Ok(info);
        }
        self.fail("'LINK', 'START_LINK'")
    }

    fn apply_pending(&mut self, ids: &[String]) -> Res<()> {
        if let Some(data) = self.pending_data.take() {
            let last = ids[ids.len() - 1].clone();
            self.db.add_vertex(&last, None, None, None, Some(&data))?;
        }
        Ok(())
    }

    fn vertex_statement(&mut self, collect: bool) -> Res<Vec<String>> {
        let mut stmt = self.node()?;
        self.apply_pending(&stmt)?;
        let mut nodes = stmt.clone();
        loop {
            if !matches!(self.t(), T::Link | T::StartLink | T::LinkId) {
                return Ok(nodes);
            }
            let info = self.link()?;
            let next = self.node()?;
            self.apply_pending(&next)?;
            self.db.add_link(&stmt, &next, &info)?;
            if collect {
                let mut joined = next.clone();
                joined.extend(nodes);
                nodes = joined;
            }
            stmt = next;
        }
    }

    fn style(&mut self) -> Res<String> {
        self.run(is_style_part, "'NODE_STRING', 'NUM', 'COLON', 'BRKT'")
    }

    fn styles_opt(&mut self) -> Res<Vec<String>> {
        let mut out = vec![self.style()?];
        while self.t() == T::Comma {
            self.i += 1;
            out.push(self.style()?);
        }
        Ok(out)
    }

    fn link_style_statement(&mut self) -> Res<()> {
        self.i += 1;
        self.expect(T::Space, "")?;
        let positions = if self.t() == T::Default {
            vec![self.take()]
        } else {
            let mut list = vec![self.expect(T::Num, "'DEFAULT'")?];
            while self.t() == T::Comma {
                self.i += 1;
                list.push(self.expect(T::Num, "")?);
            }
            list
        };
        self.expect(T::Space, "")?;
        if self.t() != T::Interpolate {
            let styles = self.styles_opt()?;
            return self.db.update_link(&positions, &styles);
        }
        self.i += 1;
        self.expect(T::Space, "")?;
        let interpolate = self.alpha_num()?;
        if self.t() == T::Space {
            self.i += 1;
            let styles = self.styles_opt()?;
            self.db.update_link_interpolate(&positions, &interpolate)?;
            self.db.update_link(&positions, &styles)
        } else {
            self.db.update_link_interpolate(&positions, &interpolate)
        }
    }

    fn link_tail(&mut self, id: &str, url: &str) -> Res<()> {
        if self.t() != T::Space {
            self.db.set_link(id, url, None);
            return Ok(());
        }
        self.i += 1;
        if self.t() == T::LinkTarget {
            let target = self.take();
            self.db.set_link(id, url, Some(&target));
            return Ok(());
        }
        let tooltip = self.expect(T::Str, "'LINK_TARGET'")?;
        if self.t() == T::Space {
            self.i += 1;
            let target = self.expect(T::LinkTarget, "")?;
            self.db.set_link(id, url, Some(&target));
        } else {
            self.db.set_link(id, url, None);
        }
        self.db.set_tooltip(id, &tooltip);
        Ok(())
    }

    fn click_statement(&mut self) -> Res<()> {
        let id = self.take();
        match self.t() {
            T::CallbackName => {
                self.i += 1;
                if self.t() == T::CallbackArgs {
                    self.i += 1;
                }
                let mut tooltip = None;
                if self.t() == T::Space {
                    self.i += 1;
                    tooltip = Some(self.expect(T::Str, "")?);
                }
                self.db.set_click_event(&id);
                if let Some(tooltip) = tooltip {
                    self.db.set_tooltip(&id, &tooltip);
                }
            }
            T::Href => {
                self.i += 1;
                let url = self.expect(T::Str, "")?;
                self.link_tail(&id, &url)?;
            }
            T::Str => {
                let url = self.take();
                self.link_tail(&id, &url)?;
            }
            _ => {
                self.alpha_num()?;
                if self.t() == T::Space {
                    self.i += 1;
                    let tooltip = self.expect(T::Str, "")?;
                    self.db.set_click_event(&id);
                    self.db.set_tooltip(&id, &tooltip);
                } else {
                    self.db.set_click_event(&id);
                }
            }
        }
        Ok(())
    }

    fn subgraph_header(&mut self) -> Res<Frame> {
        self.i += 1;
        let empty = Frame {
            id: None,
            title: None,
            same: false,
            list: Vec::new(),
            collect: false,
        };
        if matches!(self.t(), T::Newline | T::Semi | T::Eof) {
            self.i += 1;
            return Ok(empty);
        }
        self.expect(T::Space, "'SEMI', 'NEWLINE', 'EOF'")?;
        let id = self.text_no_tags()?;
        let mut title = None;
        if self.t() == T::Sqs {
            self.i += 1;
            title = Some(self.text()?);
            self.closed(T::Sqe)?;
        }
        if !matches!(self.t(), T::Newline | T::Semi | T::Eof) {
            return self.fail("'SEMI', 'NEWLINE', 'EOF', 'SQS'");
        }
        self.i += 1;
        let same = title.is_none();
        let title = title.unwrap_or_else(|| id.clone());
        Ok(Frame {
            id: Some(id),
            title: Some(title),
            same,
            ..empty
        })
    }

    /// A statement, and what it adds to the list of the subgraph it is in.
    fn statement(&mut self, collect: bool) -> Res<Option<DocItem>> {
        let t = self.t();
        match t {
            T::Style => {
                self.i += 1;
                self.expect(T::Space, "")?;
                let id = self.id_string()?;
                self.expect(T::Space, "")?;
                let styles = self.styles_opt()?;
                self.db.add_vertex(&id, None, None, Some(styles), None)?;
                self.separator()?;
                Ok(None)
            }
            T::LinkStyle => {
                self.link_style_statement()?;
                self.separator()?;
                Ok(None)
            }
            T::ClassDef => {
                self.i += 1;
                self.expect(T::Space, "")?;
                let id = self.id_string()?;
                self.expect(T::Space, "")?;
                let styles = self.styles_opt()?;
                self.db.add_class(&id, &styles);
                self.separator()?;
                Ok(None)
            }
            T::Class => {
                self.i += 1;
                self.expect(T::Space, "")?;
                let ids = self.id_string()?;
                self.expect(T::Space, "")?;
                let class = self.id_string()?;
                self.db.set_class(&ids, &class);
                self.separator()?;
                Ok(None)
            }
            T::Click => {
                self.click_statement()?;
                self.separator()?;
                Ok(None)
            }
            T::DirectionTb | T::DirectionBt | T::DirectionRl | T::DirectionLr | T::DirectionTd => {
                self.i += 1;
                let k = t as usize - T::DirectionTb as usize;
                Ok(Some(DocItem::Dir(DIRECTIONS[k].to_string())))
            }
            T::AccTitle => {
                self.i += 1;
                let value = trim(&self.expect(T::AccTitleValue, "")?).to_string();
                self.db.set_acc_title(&value);
                Ok(Some(DocItem::Id(value)))
            }
            T::AccDescr => {
                self.i += 1;
                let value = trim(&self.expect(T::AccDescrValue, "")?).to_string();
                self.db.set_acc_description(&value);
                Ok(Some(DocItem::Id(value)))
            }
            T::AccDescrMultilineValue => {
                let value = trim(&self.take()).to_string();
                self.db.set_acc_description(&value);
                Ok(Some(DocItem::Id(value)))
            }
            _ => {
                if !is_id(t) {
                    return self.fail(STATEMENT_START);
                }
                let nodes = self.vertex_statement(collect)?;
                self.separator()?;
                Ok((!nodes.is_empty()).then_some(DocItem::Ids(nodes)))
            }
        }
    }

    /// Subgraphs nest without recursion: each open one keeps the list and collect flag of its
    /// parent.
    fn document(&mut self, max_depth: usize) -> Res<()> {
        let mut open: Vec<Frame> = Vec::new();
        let mut list: Vec<DocItem> = Vec::new();
        let mut collect = false;
        loop {
            let t = self.t();
            if t == T::End {
                break;
            }
            if t == T::EndKw {
                let Some(frame) = open.pop() else { break };
                self.i += 1;
                let id = self.db.add_subgraph(
                    frame.id.as_ref(),
                    &list,
                    frame.title.as_ref(),
                    frame.same,
                );
                list = frame.list;
                collect = frame.collect;
                if collect {
                    list.push(DocItem::Id(id));
                }
            } else if matches!(t, T::Semi | T::Newline | T::Space | T::Eof) {
                if collect {
                    list.push(DocItem::Id(self.texts[self.i].clone()));
                }
                self.i += 1;
            } else if t == T::Subgraph {
                if open.len() >= max_depth {
                    return Err(Error::new(
                        ErrorCode::Limit,
                        format!("Subgraphs are nested deeper than the limit of {max_depth}."),
                    ));
                }
                let mut frame = self.subgraph_header()?;
                frame.list = std::mem::take(&mut list);
                frame.collect = collect;
                open.push(frame);
                collect = true;
            } else {
                let item = self.statement(collect)?;
                if collect {
                    if let Some(item) = item {
                        list.push(item);
                    }
                }
            }
        }
        if !open.is_empty() {
            return self.fail("'end'");
        }
        Ok(())
    }
}

/// Parses flowchart text into `db`. `max_depth` caps how deeply subgraphs nest.
pub(crate) fn parse_flowchart(
    source: &str,
    db: &mut FlowDb,
    max_depth: usize,
) -> Result<(), Error> {
    let src = trim_brace_trail(source);
    let units: Vec<u16> = src.encode_utf16().collect();
    let tokens = tokenize(&units);
    let mut p = Parser {
        types: tokens.types,
        texts: tokens.texts,
        starts: tokens.starts,
        i: 0,
        src: &src,
        db,
        pending_data: None,
    };
    while matches!(p.t(), T::Space | T::Newline) {
        p.i += 1;
    }
    p.expect(T::Graph, "'SPACE', 'NEWLINE'")?;
    if p.t() == T::NoDir {
        p.i += 1;
        p.db.set_direction("TB");
    } else {
        let dir = p.expect(T::Dir, "'NODIR'")?;
        if p.t() == T::Space {
            p.skip_spaces();
            if p.t() != T::Newline {
                return p.fail("'NEWLINE', 'SPACE'");
            }
        } else if !matches!(p.t(), T::Semi | T::Newline) {
            return p.fail("'SEMI', 'NEWLINE', 'SPACE'");
        }
        p.i += 1;
        p.db.set_direction(&dir);
    }
    p.document(max_depth)?;
    if p.t() != T::End {
        return p.fail(STATEMENT_START);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> FlowDb {
        let mut db = FlowDb::new(500, false);
        parse_flowchart(src, &mut db, 64).unwrap_or_else(|e| panic!("{src:?}: {}", e.message));
        db
    }

    #[test]
    fn parses_nodes_and_edges() {
        let db = parse("graph LR\nA[Start] -->|yes| B((Circle))\nB -.- C & D\n");
        assert_eq!(db.direction.as_deref(), Some("LR"));
        let ids: Vec<&str> = db.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["A", "B", "C", "D"]);
        assert_eq!(db.nodes[1].kind.as_deref(), Some("circle"));
        assert_eq!(db.edges.len(), 3);
        assert_eq!(db.edges[0].text, "yes");
        assert_eq!(db.edges[1].stroke.as_deref(), Some("dotted"));
    }

    #[test]
    fn parses_subgraphs_and_styles() {
        let db = parse(
            "flowchart TB\nsubgraph one [First]\nA --> B\nend\nclassDef red fill:#f00\nclass A red\nstyle B stroke:#0f0\n",
        );
        assert_eq!(db.subgraphs.len(), 1);
        assert_eq!(db.subgraphs[0].title, "First");
        assert_eq!(db.subgraphs[0].nodes, ["B", "A"]);
        assert_eq!(db.nodes[0].classes, ["red"]);
        assert_eq!(db.nodes[1].styles, ["stroke:#0f0"]);
    }

    #[test]
    fn parses_shape_data_and_clicks() {
        let db = parse(
            "flowchart TD\nA@{ shape: cyl, label: \"Store\" }\nclick A \"https://x.org\" \"tip\"\n",
        );
        assert_eq!(db.nodes[0].kind.as_deref(), Some("cyl"));
        assert_eq!(db.nodes[0].text.as_deref(), Some("Store"));
        assert_eq!(db.nodes[0].link.as_deref(), Some("https://x.org/"));
        assert_eq!(db.tooltips.get("A").map(String::as_str), Some("tip"));
    }

    #[test]
    fn reports_errors() {
        let mut db = FlowDb::new(500, false);
        let err = parse_flowchart("graph TD\nA --> \n", &mut db, 64).unwrap_err();
        assert_eq!(err.code, ErrorCode::Syntax);
        assert_eq!(err.line, 3);
        let mut db = FlowDb::new(2, false);
        let err = parse_flowchart("graph TD\nA --> B --> C --> D\n", &mut db, 64).unwrap_err();
        assert_eq!(err.code, ErrorCode::Limit);
        let deep = format!(
            "graph TD\n{}A\n{}",
            "subgraph s\n".repeat(70),
            "end\n".repeat(70)
        );
        let mut db = FlowDb::new(500, false);
        assert_eq!(
            parse_flowchart(&deep, &mut db, 64).unwrap_err().code,
            ErrorCode::Limit
        );
    }
}
