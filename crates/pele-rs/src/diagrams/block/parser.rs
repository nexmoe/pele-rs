//! The parser of a block diagram: Mermaid's jison grammar for `block.jison`. The grammar is
//! read with Pele 1.13's complete link chains. A composite whose header is a chain has no
//! id of its own, as in the reference parser.

use super::db::Kind;
use super::lexer::{T, Tokens, tokenize};
use crate::error::{Error, ErrorCode};

/// The statements a frame of the diagram holds, and the header of a composite.
#[derive(Debug, Clone)]
pub(crate) struct Parsed {
    /// The statements of the diagram, in the order Mermaid's grammar leaves them.
    pub statements: Vec<Statement>,
    /// The number of ids the parser has handed out.
    pub ids: usize,
}

/// A statement of a block diagram.
#[derive(Debug, Clone)]
pub(crate) enum Statement {
    /// A block, or a composite that holds other blocks.
    Block(ParsedBlock),
    /// A link between two blocks.
    Edge(ParsedEdge),
    /// A class of blocks, and the style it gives them.
    ClassDef { id: String, css: String },
    /// A class of blocks, and the class they are given.
    ApplyClass { id: String, style_class: String },
    /// A block, and the style it is given.
    ApplyStyles { id: String, styles_str: String },
    /// The number of columns of the grid the blocks are put in.
    Columns { columns: i64 },
}

/// A block, as the parser reads it.
#[derive(Debug, Clone, Default)]
pub(crate) struct ParsedBlock {
    pub id: Option<String>,
    pub kind: Kind,
    pub label: Option<String>,
    pub children: Vec<Statement>,
    pub width_in_columns: Option<i64>,
    pub width: Option<i64>,
    pub directions: Vec<String>,
}

/// A link, as the parser reads it.
#[derive(Debug, Clone)]
pub(crate) struct ParsedEdge {
    pub id: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub label: String,
    pub arrow_type_start: String,
    pub arrow_type_end: String,
    pub thickness: String,
    pub pattern: String,
}

/// A node statement: one node, or the chain of nodes and links that follows it.
enum NodeStatement {
    /// A single node.
    One(ParsedBlock),
    /// A chain: the first node, the link, and the last node.
    Chain(Vec<Statement>),
}

/// A node as the parser reads it.
struct Node {
    id: String,
    label: Option<String>,
    kind: Option<Kind>,
    directions: Vec<String>,
}

/// The reader of the tokens.
struct Parser<'a> {
    src: &'a str,
    tokens: Tokens,
    i: usize,
    /// The number of ids handed out so far.
    ids: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str, tokens: Tokens) -> Self {
        Self {
            src,
            tokens,
            i: 0,
            ids: 0,
        }
    }

    fn t(&self) -> T {
        self.tokens.types[self.i]
    }

    /// The byte offset of the token the parser is at. At the end of the text that is the end of
    /// the last token, as Mermaid points at the line the text ends on.
    fn here(&self) -> usize {
        if self.t() == T::Eof && self.i > 0 {
            let start = self.tokens.starts[self.i - 1];
            // A token that ends in whitespace, such as a link, points at its last word.
            let len = self.tokens.texts[self.i - 1]
                .trim_end()
                .encode_utf16()
                .count();
            return self.tokens.offsets[start + len];
        }
        self.tokens.offsets[self.tokens.starts[self.i]]
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

    fn expect(&mut self, t: T) -> Result<String, Error> {
        if self.t() != t {
            return self.fail(&format!("'{}'", t.name()));
        }
        let text = self.tokens.texts[self.i].clone();
        self.i += 1;
        Ok(text)
    }

    /// The next id Mermaid would make up.
    fn generate_id(&mut self) -> String {
        self.ids += 1;
        format!("id-{}", self.ids)
    }

    /// A node: its id, and its shape and label when it has them.
    fn node(&mut self) -> Result<Node, Error> {
        let id = self.expect(T::NodeId)?;
        if self.t() == T::NodeDStart {
            let open = self.tokens.texts[self.i].clone();
            self.i += 1;
            let label = self.expect(T::Str)?;
            let close = self.expect(T::NodeDEnd)?;
            let kind = Kind::from_type_str(&format!("{open}{close}"));
            return Ok(Node {
                id,
                label: Some(label),
                kind: Some(kind),
                directions: Vec::new(),
            });
        }
        if self.t() == T::BlockArrowStart {
            self.i += 1;
            let label = self.expect(T::Str)?;
            let mut directions = vec![self.expect(T::Dir)?];
            while self.t() == T::Dir {
                directions.push(self.tokens.texts[self.i].clone());
                self.i += 1;
            }
            self.expect(T::BlockArrowEnd)?;
            return Ok(Node {
                id,
                label: Some(label),
                kind: Some(Kind::BlockArrow),
                directions,
            });
        }
        Ok(Node {
            id,
            label: None,
            kind: None,
            directions: Vec::new(),
        })
    }

    /// A link: the text of the link, and the label it carries.
    fn link(&mut self) -> Result<(String, String), Error> {
        let mut label = String::new();
        if self.t() == T::StartLink {
            self.i += 1;
            self.expect(T::LinkLabel)?;
            label = self.expect(T::Str)?;
            self.expect(T::Link)?;
        } else {
            self.expect(T::Link)?;
        }
        let text = self.tokens.texts[self.i - 1].clone();
        Ok((label, text))
    }

    /// A node with the links that follow it.
    ///
    /// Preserve every node and link, following Pele's correction of Mermaid's lossy grammar.
    fn node_statement(&mut self) -> Result<NodeStatement, Error> {
        let first = self.node()?;
        // A node may take a width, and a link may follow it: Mermaid's grammar reads the width
        // as part of the node, and the link as part of the chain that follows.
        let mut width = None;
        if self.t() == T::Size {
            width = Some(self.tokens.texts[self.i].parse::<i64>().unwrap_or(i64::MAX));
            self.i += 1;
        }

        let mut links: Vec<(String, String, Node)> = Vec::new();
        while self.t() == T::Link || self.t() == T::StartLink {
            let (label, text) = self.link()?;
            let to = self.node()?;
            links.push((label, text, to));
        }

        if links.is_empty() {
            return Ok(NodeStatement::One(ParsedBlock {
                id: Some(first.id),
                kind: first.kind.unwrap_or(Kind::Na),
                label: first.label,
                width_in_columns: Some(width.unwrap_or(1)),
                directions: first.directions,
                ..ParsedBlock::default()
            }));
        }

        let mut chain = vec![Statement::Block(ParsedBlock {
            id: Some(first.id.clone()),
            kind: first.kind.unwrap_or(Kind::Na),
            label: first.label.clone(),
            directions: first.directions.clone(),
            ..ParsedBlock::default()
        })];
        let mut from = first.id;
        for (label, text, to) in links {
            let edge = ParsedEdge {
                id: format!("{}-{}", from, to.id),
                start: Some(from),
                end: Some(to.id.clone()),
                label,
                arrow_type_start: edge_str_to_edge_start_data(&text).to_string(),
                arrow_type_end: edge_str_to_edge_data(&text).to_string(),
                thickness: edge_str_to_thickness(&text).to_string(),
                pattern: edge_str_to_pattern(&text).to_string(),
            };
            from = to.id.clone();
            chain.push(Statement::Edge(edge));
            chain.push(Statement::Block(ParsedBlock {
                id: Some(to.id),
                kind: to.kind.unwrap_or(Kind::Na),
                label: to.label,
                directions: to.directions,
                ..ParsedBlock::default()
            }));
        }
        Ok(NodeStatement::Chain(chain))
    }

    /// The statements of a composite, or of the diagram itself.
    fn document(&mut self, depth: usize, max_depth: usize) -> Result<Vec<Statement>, Error> {
        let mut out: Vec<Statement> = Vec::new();
        let mut open: Vec<(Vec<Statement>, Option<NodeStatement>)> = Vec::new();
        loop {
            match self.t() {
                T::NodeId => {
                    let statement = self.node_statement()?;
                    out.extend(match statement {
                        NodeStatement::One(block) => vec![Statement::Block(block)],
                        NodeStatement::Chain(chain) => chain,
                    });
                }
                T::Columns => {
                    // JS accepts integer magnitudes beyond i64; only their sign matters once
                    // the column count exceeds the source-limited number of blocks.
                    let columns = self.tokens.texts[self.i].parse::<f64>().unwrap_or(f64::NAN);
                    if !columns.is_finite() || columns.fract() != 0.0 {
                        return Err(Error::new(
                            crate::ErrorCode::Semantic,
                            "Columns must be an integer !== 0.",
                        ));
                    }
                    let columns = columns as i64;
                    if columns == 0 {
                        return Err(Error::new(
                            crate::ErrorCode::Semantic,
                            "Columns must be an integer !== 0.",
                        ));
                    }
                    self.i += 1;
                    out.push(Statement::Columns { columns });
                }
                T::SpaceBlock => {
                    let width = self.tokens.texts[self.i].parse::<i64>().unwrap_or(i64::MAX);
                    self.i += 1;
                    let id = self.generate_id();
                    out.push(Statement::Block(ParsedBlock {
                        id: Some(id),
                        kind: Kind::Space,
                        label: Some(String::new()),
                        width: Some(width),
                        ..ParsedBlock::default()
                    }));
                }
                T::Key | T::IdBlock => {
                    if depth + open.len() >= max_depth {
                        return Err(Error::new(
                            ErrorCode::Limit,
                            format!("Blocks are nested deeper than the limit of {max_depth}."),
                        )
                        .at(self.src, self.here()));
                    }
                    let named = self.t() == T::IdBlock;
                    self.i += 1;
                    let header = if named {
                        if self.t() != T::NodeId {
                            return self.fail("'NODE_ID'");
                        }
                        Some(self.node_statement()?)
                    } else {
                        None
                    };
                    open.push((std::mem::take(&mut out), header));
                }
                T::ClassDef => {
                    self.i += 1;
                    let id = self.expect(T::ClassDefId)?.trim().to_string();
                    let css = self.expect(T::ClassDefStyleOpts)?.trim().to_string();
                    out.push(Statement::ClassDef { id, css });
                }
                T::Class => {
                    self.i += 1;
                    let id = self.expect(T::ClassEntityIds)?.trim().to_string();
                    let style_class = self.expect(T::StyleClass)?.trim().to_string();
                    out.push(Statement::ApplyClass { id, style_class });
                }
                T::Style => {
                    self.i += 1;
                    let id = self.expect(T::StyleEntityIds)?.trim().to_string();
                    let styles_str = self.expect(T::StyleDefinitionData)?.trim().to_string();
                    out.push(Statement::ApplyStyles { id, styles_str });
                }
                T::EndKeyword => {
                    let Some((parent, header)) = open.pop() else {
                        break;
                    };
                    if out.is_empty() {
                        return self.fail(STATEMENT_START);
                    }
                    self.i += 1;
                    self.ids += 1;
                    let mut block = ParsedBlock {
                        kind: Kind::Composite,
                        children: std::mem::take(&mut out),
                        ..ParsedBlock::default()
                    };
                    match header {
                        Some(NodeStatement::Chain(_)) => {}
                        Some(NodeStatement::One(header)) => {
                            block.id = header.id;
                            block.label = header.label;
                            block.width_in_columns = header.width_in_columns;
                            block.directions = header.directions;
                        }
                        None => {
                            block.id = Some(format!("id-{}", self.ids));
                            block.label = Some(String::new());
                        }
                    }
                    out = parent;
                    out.push(Statement::Block(block));
                }
                T::Eof => {
                    if !open.is_empty() {
                        return self.fail(&format!("{STATEMENT_START}, 'end'"));
                    }
                    break;
                }
                _ => {
                    let expected = if out.is_empty() {
                        STATEMENT_START.to_string()
                    } else if depth + open.len() > 0 {
                        format!("{STATEMENT_START}, 'end'")
                    } else {
                        format!("{STATEMENT_START}, 'EOF'")
                    };
                    return self.fail(&expected);
                }
            }
        }
        Ok(out)
    }
}

/// The statements a diagram may start with.
const STATEMENT_START: &str = "'NODE_ID', 'COLUMNS', 'SPACE_BLOCK', 'BLOCK_DIAGRAM_KEY', 'id-block', \
                              'classDef', 'class', 'style'";

/// The arrow a link ends in, as Mermaid names it.
fn edge_str_to_edge_data(type_str: &str) -> &'static str {
    match type_str.trim().chars().next_back() {
        Some('x') => "arrow_cross",
        Some('o') => "arrow_circle",
        Some('>') => "arrow_point",
        _ => "",
    }
}

/// The arrow a link starts in, as Mermaid names it.
fn edge_str_to_edge_start_data(type_str: &str) -> &'static str {
    match type_str.trim().chars().next() {
        Some('x') => "arrow_cross",
        Some('o') => "arrow_circle",
        Some('<') => "arrow_point",
        _ => "arrow_open",
    }
}

/// The thickness of a link, as Mermaid names it.
fn edge_str_to_thickness(type_str: &str) -> &'static str {
    if type_str.contains("==") {
        "thick"
    } else {
        "normal"
    }
}

/// The pattern of a link, as Mermaid names it.
fn edge_str_to_pattern(type_str: &str) -> &'static str {
    if type_str.contains(".-") {
        "dotted"
    } else {
        "solid"
    }
}

/// The statements of a block diagram, in the order Mermaid's grammar leaves them.
pub(crate) fn parse_block(src: &str, max_depth: usize) -> Result<Parsed, Error> {
    let tokens = tokenize(src);
    let mut p = Parser::new(src, tokens);
    p.expect(T::Key)?;
    let statements = p.document(0, max_depth)?;
    if p.t() != T::Eof || statements.is_empty() {
        return p.fail(&format!("{STATEMENT_START}, 'EOF'"));
    }
    Ok(Parsed {
        statements,
        ids: p.ids,
    })
}
