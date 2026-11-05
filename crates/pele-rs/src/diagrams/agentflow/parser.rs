//! The agentflow parser: a recursive descent parser over the tokens of Mermaid's jison
//! grammar for `agentflow.jison`, as pele's `parser.ts` emulates it.

use crate::error::{Error, ErrorCode};

use super::db::{AgentflowDb, FlowText, LabelType, LinkInfo, ModelError, destruct_link};
use super::lexer::{T, Tokens, tokenize};

type Res<T> = Result<T, Error>;

/// The tokens an id is written with.
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

/// The tokens a callback name is written with.
fn is_alpha_num(t: T) -> bool {
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

/// The tokens the name of a flow is written with: the id tokens and the keywords.
fn is_no_tags(t: T) -> bool {
    is_id(t)
        || matches!(
            t,
            T::Space
                | T::Style
                | T::LinkStyle
                | T::ClassDef
                | T::Class
                | T::Click
                | T::Graph
                | T::Dir
                | T::Flow
                | T::Connector
                | T::Global
                | T::EndKw
                | T::Up
        )
}

/// The tokens a style is written with.
fn is_style_part(t: T) -> bool {
    matches!(
        t,
        T::Num | T::NodeString | T::Colon | T::Space | T::Brkt | T::Style
    )
}

/// The tokens that end a statement.
fn is_separator(t: T) -> bool {
    matches!(t, T::Newline | T::Semi | T::Eof | T::Comment)
}

/// The tokens a label is written with.
fn is_text_token(t: T) -> bool {
    matches!(t, T::Text | T::TagStart | T::TagEnd | T::UnicodeText)
}

/// The tokens the label of an edge is written with.
fn is_edge_text_token(t: T) -> bool {
    matches!(t, T::EdgeText | T::UnicodeText)
}

const TEXT_TOKENS: &str = "'UNICODE_TEXT', 'TEXT', 'TAGSTART'";
const SEPARATORS: &str = "'COMMENT', 'SEMI', 'NEWLINE', 'EOF'";
const STATEMENT_START: &str = "'COMMENT', 'SEMI', 'NEWLINE', 'SPACE', 'EOF', 'flow', 'global', \
                               'connector', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', \
                               'STYLE', 'LINKSTYLE', 'CLASSDEF', 'CLASS', 'CLICK', 'NODE_STRING'";

/// A block of a diagram: a flow, or the shared agents of a `global` block.
struct Frame {
    global: bool,
    id: Option<FlowText>,
    title: Option<FlowText>,
    data: Option<String>,
    /// The agents the block holds.
    list: Vec<String>,
    /// The direction the block sets for itself.
    dir: Option<String>,
    /// Whether the agents of the block are collected at all.
    collect: bool,
}

/// What a statement of a diagram holds.
enum Statement {
    /// Nothing that a flow holds.
    Nothing,
    /// The agents it holds.
    Nodes(Vec<String>),
    /// The direction it sets.
    Direction(String),
}

struct Parser<'a> {
    types: Vec<T>,
    texts: Vec<String>,
    starts: Vec<usize>,
    i: usize,
    src: &'a str,
    db: &'a mut AgentflowDb,
    /// The metadata of the last agent that was read, waiting to be attached to it.
    pending_data: Option<String>,
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
        if !is_separator(self.t()) {
            return self.fail(SEPARATORS);
        }
        self.i += 1;
        Ok(())
    }

    /// The id of an agent, written with the id tokens.
    fn id_string(&mut self) -> Res<String> {
        if !is_id(self.t()) {
            return self.fail("'NODE_STRING'");
        }
        let mut s = self.take();
        while is_id(self.t()) {
            s.push_str(&self.take());
        }
        Ok(s)
    }

    /// A callback name, written with the alphanumeric tokens.
    fn alpha_num(&mut self) -> Res<String> {
        if !is_alpha_num(self.t()) {
            return self.fail("'NODE_STRING'");
        }
        let mut s = self.take();
        while is_alpha_num(self.t()) {
            s.push_str(&self.take());
        }
        Ok(s)
    }

    /// A label.
    fn text(&mut self) -> Res<FlowText> {
        let kind = match self.t() {
            T::Str => LabelType::Str,
            T::MdStr => LabelType::Markdown,
            T::Text | T::TagStart | T::TagEnd | T::UnicodeText => LabelType::Text,
            _ => return self.fail(&format!("{TEXT_TOKENS}, 'TAGEND', 'STR', 'MD_STR'")),
        };
        let mut s = self.take();
        while is_text_token(self.t()) {
            s.push_str(&self.take());
        }
        Ok(FlowText { text: s, kind })
    }

    /// The label of an edge.
    fn edge_text(&mut self) -> Res<FlowText> {
        let kind = match self.t() {
            T::Str => LabelType::Str,
            T::MdStr => LabelType::Markdown,
            T::EdgeText | T::UnicodeText => LabelType::Text,
            _ => return self.fail("'EDGE_TEXT', 'STR', 'MD_STR'"),
        };
        let mut s = self.take();
        while is_edge_text_token(self.t()) {
            s.push_str(&self.take());
        }
        Ok(FlowText { text: s, kind })
    }

    /// The name of a flow, which may not hold tags.
    fn text_no_tags(&mut self) -> Res<FlowText> {
        let kind = match self.t() {
            T::Str => LabelType::Text,
            T::MdStr => LabelType::Markdown,
            t if is_no_tags(t) => LabelType::Text,
            _ => return self.fail("'NODE_STRING', 'STR', 'MD_STR'"),
        };
        let mut s = self.take();
        while is_no_tags(self.t()) {
            s.push_str(&self.take());
        }
        Ok(FlowText { text: s, kind })
    }

    /// The metadata of an `@{ … }` block.
    fn shape_data(&mut self) -> Res<String> {
        if self.t() != T::ShapeData {
            return self.fail("'SHAPE_DATA'");
        }
        let mut s = self.take();
        while self.t() == T::ShapeData {
            s.push_str(&self.take());
        }
        Ok(s)
    }

    /// An agent, with the shape it was written with.
    fn vertex(&mut self) -> Res<String> {
        let id = self.id_string()?;
        let mut text: Option<FlowText> = None;
        let mut shape: Option<&'static str> = None;
        let mut props: Option<Vec<(String, String)>> = None;
        match self.t() {
            T::Sqs => {
                self.i += 1;
                text = Some(self.text()?);
                self.expect(T::Sqe, TEXT_TOKENS)?;
                shape = Some("square");
            }
            T::DoubleCircleStart => {
                self.i += 1;
                text = Some(self.text()?);
                self.expect(T::DoubleCircleEnd, TEXT_TOKENS)?;
                shape = Some("doublecircle");
            }
            T::Ps => {
                self.i += 1;
                if self.t() == T::Ps {
                    self.i += 1;
                    text = Some(self.text()?);
                    self.expect(T::Pe, TEXT_TOKENS)?;
                    self.expect(T::Pe, "")?;
                    shape = Some("circle");
                } else {
                    text = Some(self.text()?);
                    self.expect(T::Pe, TEXT_TOKENS)?;
                    shape = Some("round");
                }
            }
            T::EllipseStart => {
                self.i += 1;
                text = Some(self.text()?);
                self.expect(T::EllipseEnd, TEXT_TOKENS)?;
                shape = Some("ellipse");
            }
            T::StadiumStart => {
                self.i += 1;
                text = Some(self.text()?);
                self.expect(T::StadiumEnd, TEXT_TOKENS)?;
                shape = Some("stadium");
            }
            T::SubroutineStart => {
                self.i += 1;
                text = Some(self.text()?);
                self.expect(T::SubroutineEnd, TEXT_TOKENS)?;
                shape = Some("subroutine");
            }
            T::VertexWithPropsStart => {
                self.i += 1;
                let field = self.expect(T::NodeString, "")?;
                self.expect(T::Colon, "")?;
                let value = self.expect(T::NodeString, "")?;
                self.expect(T::Pipe, "")?;
                text = Some(self.text()?);
                self.expect(T::Sqe, TEXT_TOKENS)?;
                shape = Some("rect");
                props = Some(vec![(field, value)]);
            }
            T::CylinderStart => {
                self.i += 1;
                text = Some(self.text()?);
                self.expect(T::CylinderEnd, TEXT_TOKENS)?;
                shape = Some("cylinder");
            }
            T::DiamondStart => {
                self.i += 1;
                if self.t() == T::DiamondStart {
                    self.i += 1;
                    text = Some(self.text()?);
                    self.expect(T::DiamondStop, TEXT_TOKENS)?;
                    self.expect(T::DiamondStop, "")?;
                    shape = Some("hexagon");
                } else {
                    text = Some(self.text()?);
                    self.expect(T::DiamondStop, TEXT_TOKENS)?;
                    shape = Some("diamond");
                }
            }
            T::TagEnd => {
                self.i += 1;
                text = Some(self.text()?);
                self.expect(T::Sqe, TEXT_TOKENS)?;
                shape = Some("odd");
            }
            T::TrapStart => {
                self.i += 1;
                text = Some(self.text()?);
                if self.t() == T::InvTrapEnd {
                    self.i += 1;
                    shape = Some("lean_right");
                } else {
                    self.expect(T::TrapEnd, TEXT_TOKENS)?;
                    shape = Some("trapezoid");
                }
            }
            T::InvTrapStart => {
                self.i += 1;
                text = Some(self.text()?);
                if self.t() == T::TrapEnd {
                    self.i += 1;
                    shape = Some("lean_left");
                } else {
                    self.expect(T::InvTrapEnd, TEXT_TOKENS)?;
                    shape = Some("inv_trapezoid");
                }
            }
            _ => {}
        }
        self.db
            .add_vertex(&id, text.as_ref(), shape, None, None, None, props, None)
            .map_err(model_error)?;
        Ok(id)
    }

    /// An agent, with the class it was put in.
    fn styled_vertex(&mut self) -> Res<String> {
        let id = self.vertex()?;
        if self.t() == T::StyleSeparator {
            self.i += 1;
            let class = self.id_string()?;
            self.db.set_class(&id, &class);
        }
        Ok(id)
    }

    /// The agents of one statement, joined with `&`.
    fn node(&mut self) -> Res<Vec<String>> {
        let mut ids = vec![self.styled_vertex()?];
        loop {
            // A metadata block may follow a node.
            if self.t() == T::ShapeData {
                let data = self.shape_data()?;
                let mark = self.i;
                while self.t() == T::Space {
                    self.i += 1;
                }
                if self.t() == T::Amp {
                    // `node shapeData spaceList AMP spaceList styledVertex`
                    self.i += 1;
                    while self.t() == T::Space {
                        self.i += 1;
                    }
                    let next = self.styled_vertex()?;
                    self.attach(&ids[ids.len() - 1], &data)?;
                    ids.push(next);
                    continue;
                }
                // `node shapeData`: the metadata belongs to the node.
                self.i = mark;
                self.pending_data = Some(data);
                return Ok(ids);
            }
            if self.t() != T::Space {
                return Ok(ids);
            }
            let mark = self.i;
            while self.t() == T::Space {
                self.i += 1;
            }
            if self.t() == T::Amp {
                // `node spaceList AMP spaceList styledVertex`
                self.i += 1;
                while self.t() == T::Space {
                    self.i += 1;
                }
                ids.push(self.styled_vertex()?);
                continue;
            }
            // `node spaceList`: the spaces belong to the statement.
            let _ = mark;
            return Ok(ids);
        }
    }

    /// Gives an agent the metadata that was written after it.
    fn attach(&mut self, id: &str, data: &str) -> Res<()> {
        let metadata = super::metadata::parse(data).map_err(|message| {
            Error::new(
                ErrorCode::Syntax,
                format!("The metadata of \"{id}\" is not valid YAML. {message}"),
            )
        })?;
        self.db
            .add_metadata(id, Some(metadata))
            .map_err(model_error)
    }

    /// The link between two agents.
    fn link(&mut self) -> Res<LinkInfo> {
        let mut id: Option<String> = None;
        if self.t() == T::LinkId {
            id = Some(self.take());
        }
        if self.t() == T::Link {
            let text = self.take();
            let mut info = destruct_link(&text, None);
            info.id = id;
            if self.t() == T::Pipe {
                self.i += 1;
                info.text = Some(self.text()?);
                self.expect(T::Pipe, TEXT_TOKENS)?;
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
            let mut info = destruct_link(&end, Some(&start));
            info.text = Some(label);
            info.id = id;
            return Ok(info);
        }
        self.fail("'LINK', 'START_LINK'")
    }

    /// A statement that holds agents.
    fn vertex_statement(&mut self) -> Res<Vec<String>> {
        let mut stmt = self.node()?;
        if let Some(data) = self.pending_data.take() {
            let last = stmt.last().cloned().unwrap_or_default();
            self.attach(&last, &data)?;
        }
        let mut nodes = stmt.clone();
        loop {
            let t = self.t();
            if t != T::Link && t != T::StartLink && t != T::LinkId {
                return Ok(nodes);
            }
            let info = self.link()?;
            let next = self.node()?;
            if let Some(data) = self.pending_data.take() {
                let last = next.last().cloned().unwrap_or_default();
                self.attach(&last, &data)?;
            }
            self.db.add_link(&stmt, &next, &info);
            let mut all = next.clone();
            all.extend(nodes.iter().cloned());
            nodes = all;
            stmt = next;
        }
    }

    /// One style.
    fn style(&mut self) -> Res<String> {
        if !is_style_part(self.t()) {
            return self.fail("'NODE_STRING', 'NUM', 'COLON', 'BRKT'");
        }
        let mut s = self.take();
        while is_style_part(self.t()) {
            s.push_str(&self.take());
        }
        Ok(s)
    }

    /// The styles of a statement.
    fn styles_opt(&mut self) -> Res<Vec<String>> {
        let mut out = vec![self.style()?];
        while self.t() == T::Comma {
            self.i += 1;
            out.push(self.style()?);
        }
        Ok(out)
    }

    /// A `linkStyle` statement.
    fn link_style_statement(&mut self) -> Res<()> {
        self.i += 1;
        self.expect(T::Space, "")?;
        let mut positions: Vec<String> = Vec::new();
        if self.t() == T::Default {
            positions.push(self.take());
        } else {
            positions.push(self.expect(T::Num, "'DEFAULT'")?);
            while self.t() == T::Comma {
                self.i += 1;
                positions.push(self.expect(T::Num, "")?);
            }
        }
        self.expect(T::Space, "")?;
        if self.t() != T::Interpolate {
            let styles = self.styles_opt()?;
            self.db.update_link(&positions, &styles);
            return Ok(());
        }
        self.i += 1;
        self.expect(T::Space, "")?;
        let interpolate = self.alpha_num()?;
        if self.t() == T::Space {
            self.i += 1;
            let styles = self.styles_opt()?;
            self.db.update_link_interpolate(&positions, &interpolate);
            self.db.update_link(&positions, &styles);
        } else {
            self.db.update_link_interpolate(&positions, &interpolate);
        }
        Ok(())
    }

    /// The tail of a `click` statement that carries a link.
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

    /// A `click` statement.
    fn click_statement(&mut self) -> Res<()> {
        let id = self.take();
        match self.t() {
            T::CallbackName => {
                let name = self.take();
                let args = if self.t() == T::CallbackArgs {
                    Some(self.take())
                } else {
                    None
                };
                let mut tooltip: Option<String> = None;
                if self.t() == T::Space {
                    self.i += 1;
                    tooltip = Some(self.expect(T::Str, "")?);
                }
                self.db.set_click_event(&id, &name, args.as_deref());
                if let Some(tooltip) = tooltip {
                    self.db.set_tooltip(&id, &tooltip);
                }
                Ok(())
            }
            T::Href => {
                self.i += 1;
                let url = self.expect(T::Str, "")?;
                self.link_tail(&id, &url)
            }
            T::Str => {
                let url = self.take();
                self.link_tail(&id, &url)
            }
            _ => {
                let name = self.alpha_num()?;
                if self.t() == T::Space {
                    self.i += 1;
                    let tooltip = self.expect(T::Str, "")?;
                    self.db.set_click_event(&id, &name, None);
                    self.db.set_tooltip(&id, &tooltip);
                } else {
                    self.db.set_click_event(&id, &name, None);
                }
                Ok(())
            }
        }
    }

    /// The head of a `flow` or a `connector`: its name, its title and its metadata.
    fn header(&mut self) -> Res<Frame> {
        self.expect(T::Space, SEPARATORS)?;
        let id = self.text_no_tags()?;
        let mut title: Option<FlowText> = None;
        if self.t() == T::Sqs {
            self.i += 1;
            title = Some(self.text()?);
            self.expect(T::Sqe, TEXT_TOKENS)?;
        }
        let data = if self.t() == T::ShapeData {
            Some(self.shape_data()?)
        } else {
            None
        };
        if !is_separator(self.t()) {
            return self.fail(&format!("{SEPARATORS}, 'SHAPE_DATA'"));
        }
        Ok(Frame {
            global: false,
            id: Some(id),
            title,
            data,
            list: Vec::new(),
            dir: None,
            collect: false,
        })
    }

    /// One statement of a diagram.
    fn statement(&mut self) -> Res<Statement> {
        match self.t() {
            T::Style => {
                self.i += 1;
                self.expect(T::Space, "")?;
                let id = self.id_string()?;
                self.expect(T::Space, "")?;
                let styles = self.styles_opt()?;
                self.db
                    .add_vertex(&id, None, None, Some(&styles), None, None, None, None)
                    .map_err(model_error)?;
                self.separator()?;
                Ok(Statement::Nothing)
            }
            T::LinkStyle => {
                self.link_style_statement()?;
                self.separator()?;
                Ok(Statement::Nothing)
            }
            T::ClassDef => {
                self.i += 1;
                self.expect(T::Space, "")?;
                let id = self.id_string()?;
                self.expect(T::Space, "")?;
                let styles = self.styles_opt()?;
                self.db.add_class(&id, &styles);
                self.separator()?;
                Ok(Statement::Nothing)
            }
            T::Class => {
                self.i += 1;
                self.expect(T::Space, "")?;
                let ids = self.id_string()?;
                self.expect(T::Space, "")?;
                let class = self.id_string()?;
                self.db.set_class(&ids, &class);
                self.separator()?;
                Ok(Statement::Nothing)
            }
            T::Click => {
                self.click_statement()?;
                self.separator()?;
                Ok(Statement::Nothing)
            }
            T::Connector => {
                self.i += 1;
                let frame = self.header()?;
                self.i += 1;
                let id = frame
                    .id
                    .as_ref()
                    .map(|id| id.text.trim().to_string())
                    .unwrap_or_default();
                let added = self.db.add_connector(&id, frame.title.as_ref());
                if let Some(data) = &frame.data {
                    self.attach(&added, data)?;
                }
                Ok(Statement::Nodes(vec![added]))
            }
            T::DirectionTb | T::DirectionBt | T::DirectionRl | T::DirectionLr | T::DirectionTd => {
                let dir = match self.t() {
                    T::DirectionTb => "TB",
                    T::DirectionBt => "BT",
                    T::DirectionRl => "RL",
                    T::DirectionLr => "LR",
                    _ => "TD",
                };
                self.i += 1;
                Ok(Statement::Direction(dir.to_string()))
            }
            T::AccTitle => {
                self.i += 1;
                let value = self.expect(T::AccTitleValue, "")?.trim().to_string();
                self.db.acc_title = Some(value.clone());
                // Mermaid's grammar makes the statement the text it holds, so a flow that
                // holds one holds the text as an agent of its own.
                Ok(Statement::Nodes(vec![value]))
            }
            T::AccDescr => {
                self.i += 1;
                let value = self.expect(T::AccDescrValue, "")?.trim().to_string();
                self.db.acc_descr = Some(value.clone());
                Ok(Statement::Nodes(vec![value]))
            }
            T::AccDescrMultilineValue => {
                let value = self.take().trim().to_string();
                self.db.acc_descr = Some(value.clone());
                Ok(Statement::Nodes(vec![value]))
            }
            _ => {
                if !is_id(self.t()) {
                    return self.fail(STATEMENT_START);
                }
                let nodes = self.vertex_statement()?;
                self.separator()?;
                Ok(Statement::Nodes(nodes))
            }
        }
    }

    /// The statements of a diagram, with the blocks they hold.
    fn document(&mut self) -> Res<()> {
        let mut open: Vec<Frame> = Vec::new();
        let mut list: Vec<String> = Vec::new();
        let mut dir: Option<String> = None;
        let mut collect = false;
        loop {
            let t = self.t();
            if t == T::End {
                break;
            }
            if t == T::EndKw {
                let Some(frame) = open.pop() else {
                    break;
                };
                self.i += 1;
                if frame.global {
                    self.db.add_global(&list);
                } else {
                    let id = frame.id.as_ref().map(|id| id.text.trim().to_string());
                    let added = self.db.add_subgraph(
                        id.as_deref(),
                        &list,
                        frame.title.as_ref(),
                        dir.as_deref(),
                    );
                    if let Some(data) = &frame.data {
                        self.attach(&added, data)?;
                    }
                    list = frame.list;
                    collect = frame.collect;
                    dir = frame.dir;
                    if collect {
                        list.push(added);
                    }
                    continue;
                }
                list = frame.list;
                collect = frame.collect;
                dir = frame.dir;
                continue;
            }
            if matches!(t, T::Semi | T::Newline | T::Space | T::Eof | T::Comment) {
                self.i += 1;
                continue;
            }
            if t == T::Flow || t == T::Global {
                let global = t == T::Global;
                self.i += 1;
                let mut frame = if global || is_separator(self.t()) {
                    let frame = Frame {
                        global,
                        id: None,
                        title: None,
                        data: None,
                        list: std::mem::take(&mut list),
                        dir: dir.take(),
                        collect,
                    };
                    self.separator()?;
                    frame
                } else {
                    let mut frame = self.header()?;
                    frame.list = std::mem::take(&mut list);
                    frame.dir = dir.take();
                    frame.collect = collect;
                    self.i += 1;
                    frame
                };
                frame.collect = collect;
                open.push(frame);
                list = Vec::new();
                collect = true;
                continue;
            }
            match self.statement()? {
                Statement::Nothing => {}
                Statement::Direction(value) => dir = Some(value),
                Statement::Nodes(nodes) => {
                    if collect && !nodes.is_empty() {
                        list.extend(nodes);
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

/// The error of the model, as the error of the parser.
fn model_error(error: ModelError) -> Error {
    match error {
        ModelError::Shape(message) => Error::new(ErrorCode::Semantic, message),
    }
}

/// Removes the spaces and tabs between a closing brace and the end of its line, as Mermaid's
/// agentflow parser does before it parses.
fn trim_brace_trail(source: &str) -> String {
    if !source.contains('}') {
        return source.to_string();
    }
    let mut out = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'}' {
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'\n' {
                out.push_str("}\n");
                i = j + 1;
                continue;
            }
        }
        let ch = source[i..].chars().next().unwrap_or('\0');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Parses agentflow text into `db`.
pub(crate) fn parse_agentflow(source: &str, db: &mut AgentflowDb) -> Res<()> {
    let src = trim_brace_trail(source);
    let units: Vec<u16> = src.encode_utf16().collect();
    let Tokens {
        types,
        texts,
        starts,
    } = tokenize(&units);
    let mut p = Parser {
        types,
        texts,
        starts,
        i: 0,
        src: &src,
        db,
        pending_data: None,
    };
    while matches!(p.t(), T::Space | T::Newline | T::Comment) {
        p.i += 1;
    }
    p.expect(T::Graph, "'SPACE', 'NEWLINE', 'COMMENT'")?;
    if p.t() == T::NoDir {
        p.i += 1;
        p.db.set_direction("TB");
    } else {
        let dir = p.expect(T::Dir, "'NODIR'")?;
        if p.t() == T::Space {
            while p.t() == T::Space {
                p.i += 1;
            }
            if p.t() != T::Newline {
                return p.fail("'NEWLINE', 'SPACE'");
            }
        } else if !matches!(p.t(), T::Semi | T::Newline | T::Comment) {
            return p.fail("'COMMENT', 'SEMI', 'NEWLINE', 'SPACE'");
        }
        p.i += 1;
        p.db.set_direction(&dir);
    }
    p.document()?;
    if p.t() != T::End {
        return p.fail(STATEMENT_START);
    }
    Ok(())
}
