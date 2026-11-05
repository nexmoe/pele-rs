//! The model of a block diagram: the blocks, the links between them, and the classes that style
//! them. Ported from pele's `diagrams/block/db.ts`, which follows Mermaid's own block database.

use super::parser::{Parsed, Statement};
use crate::diagrams::common::ordered::Ordered;
use crate::error::{Error, ErrorCode};

/// The most space blocks a diagram may hold.
const MAX_SPACES: i64 = 100_000;

/// The kind of a block: the shape it is drawn in, or what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Kind {
    /// A plain block.
    #[default]
    Na,
    /// A block that holds other blocks.
    Composite,
    /// Room for nothing.
    Space,
    /// A block that points in one or two directions.
    BlockArrow,
    Square,
    Round,
    Circle,
    DoubleCircle,
    Diamond,
    Hexagon,
    Stadium,
    Subroutine,
    Cylinder,
    RectLeftInvArrow,
    LeanRight,
    LeanLeft,
    Trapezoid,
    InvTrapezoid,
}

impl Kind {
    /// The kind a shape in Mermaid's notation is.
    pub(crate) fn from_type_str(type_str: &str) -> Kind {
        match type_str {
            "[]" => Kind::Square,
            "()" => Kind::Round,
            "(())" => Kind::Circle,
            ">]" => Kind::RectLeftInvArrow,
            "{}" => Kind::Diamond,
            "{{}}" => Kind::Hexagon,
            "([])" => Kind::Stadium,
            "[[]]" => Kind::Subroutine,
            "[()]" => Kind::Cylinder,
            "((()))" => Kind::DoubleCircle,
            "[//]" => Kind::LeanRight,
            "[\\\\]" => Kind::LeanLeft,
            "[/\\]" => Kind::Trapezoid,
            "[\\/]" => Kind::InvTrapezoid,
            "<[]>" => Kind::BlockArrow,
            _ => Kind::Na,
        }
    }

    /// The name Mermaid gives the kind of a block.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "the golden tests read the name of a kind")
    )]
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Kind::Na => "na",
            Kind::Composite => "composite",
            Kind::Space => "space",
            Kind::BlockArrow => "block_arrow",
            Kind::Square => "square",
            Kind::Round => "round",
            Kind::Circle => "circle",
            Kind::DoubleCircle => "doublecircle",
            Kind::Diamond => "diamond",
            Kind::Hexagon => "hexagon",
            Kind::Stadium => "stadium",
            Kind::Subroutine => "subroutine",
            Kind::Cylinder => "cylinder",
            Kind::RectLeftInvArrow => "rect_left_inv_arrow",
            Kind::LeanRight => "lean_right",
            Kind::LeanLeft => "lean_left",
            Kind::Trapezoid => "trapezoid",
            Kind::InvTrapezoid => "inv_trapezoid",
        }
    }

    /// The shape the block is drawn in, as the SVG writer names it.
    pub(crate) fn shape(self) -> &'static str {
        match self {
            Kind::Square => "rect",
            Kind::Round => "rounded",
            Kind::Circle => "circle",
            Kind::DoubleCircle => "dbl-circ",
            Kind::Diamond => "diam",
            Kind::Hexagon => "hex",
            Kind::Stadium => "stadium",
            Kind::Subroutine => "fr-rect",
            Kind::Cylinder => "cyl",
            Kind::RectLeftInvArrow => "odd",
            Kind::LeanRight => "lean-r",
            Kind::LeanLeft => "lean-l",
            Kind::Trapezoid => "trap-b",
            Kind::InvTrapezoid => "trap-t",
            _ => "rect",
        }
    }
}

/// A block of the diagram.
#[derive(Debug, Clone, Default)]
pub(crate) struct Block {
    pub id: String,
    pub kind: Kind,
    pub label: Option<String>,
    /// The blocks this one holds, as places in the arena.
    pub children: Vec<usize>,
    /// The width of the grid the blocks are put in: a count, or -1 to put them on one row.
    pub columns: Option<i64>,
    /// The number of columns of the grid a block takes.
    pub width_in_columns: Option<i64>,
    /// The directions a block arrow points in.
    pub directions: Vec<String>,
    /// The classes the block is given.
    pub classes: Vec<String>,
    /// The styles the block is given.
    pub styles: Vec<String>,
    /// The number of the color the block takes, for the composites in the order they are read.
    pub color_index: Option<usize>,
}

/// A link between two blocks.
#[derive(Debug, Clone)]
pub(crate) struct Edge {
    pub id: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub label: String,
    pub arrow_type_start: String,
    pub arrow_type_end: String,
    pub thickness: String,
    pub pattern: String,
}

/// A class of blocks, and the style it gives them.
#[derive(Debug, Clone, Default)]
pub(crate) struct ClassDef {
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

/// The model of a block diagram.
#[derive(Debug, Clone)]
pub(crate) struct BlockModel {
    pub title: Option<String>,
    /// The blocks of the diagram, in the arena they were built in.
    pub arena: Vec<Block>,
    /// The place of the block that holds them all.
    pub root: usize,
    pub edges: Vec<Edge>,
    pub classes: Ordered<ClassDef>,
    /// The blocks by their id.
    pub by_id: Ordered<usize>,
    /// The warnings the diagram raised.
    pub warnings: Vec<String>,
    /// The ids a class or style statement named before any block had them.
    pending: Vec<String>,
    /// The links of each id, and how many there are.
    edge_count: Ordered<i64>,
    /// The number of ids handed out so far.
    ids: usize,
    /// The number of composites read, which is also the number of colors they take.
    colors: usize,
    /// The number of space blocks the diagram holds.
    spaces: i64,
}

/// A frame of the diagram: the statements of a composite, and the blocks they hold.
struct Frame {
    list: Vec<Statement>,
    at: usize,
    /// The place of the block that holds them.
    parent: usize,
    children: Vec<usize>,
    /// The width of the grid, for the warning about a block that is too wide.
    columns: i64,
}

/// The frame of a list of statements.
fn frame(list: Vec<Statement>, parent: usize) -> Frame {
    let mut columns = -1;
    for statement in &list {
        if let Statement::Columns { columns: c } = statement {
            columns = *c;
            break;
        }
    }
    Frame {
        list,
        at: 0,
        parent,
        children: Vec::new(),
        columns,
    }
}

/// Replaces the first occurrence of a string, as JavaScript's `String.replace` does with a
/// string to search for.
fn replace_first(text: &str, from: &str, to: &str) -> String {
    match text.find(from) {
        Some(at) => format!("{}{}{}", &text[..at], to, &text[at + from.len()..]),
        None => text.to_string(),
    }
}

impl BlockModel {
    /// The block of an id, or a new one that waits for the block of that id.
    ///
    /// Mermaid throws on a style for an unknown id, and a class for one makes the block of that
    /// id disappear when it is declared later. Here both wait for the block.
    fn named(&mut self, id: &str) -> usize {
        if let Some(index) = self.by_id.get(id) {
            return *index;
        }
        let index = self.arena.len();
        self.arena.push(Block {
            id: id.to_string(),
            kind: Kind::Na,
            ..Block::default()
        });
        self.by_id.set(id, index);
        self.pending.push(id.to_string());
        index
    }

    /// The style a class gives the blocks of it.
    fn add_style_class(&mut self, id: &str, style_attributes: &str) {
        if self.classes.get(id).is_none() {
            self.classes.set(
                id,
                ClassDef {
                    styles: Vec::new(),
                    text_styles: Vec::new(),
                },
            );
        }
        let mut styles = Vec::new();
        let mut text_styles = Vec::new();
        for attribute in style_attributes.split(',') {
            let fixed = match attribute.find(';') {
                Some(at) => format!("{}{}", &attribute[..at], &attribute[at + 1..]),
                None => attribute.to_string(),
            };
            let fixed = fixed.trim().to_string();
            if attribute.contains("color") {
                text_styles.push(replace_first(
                    &replace_first(&fixed, "fill", "bgFill"),
                    "color",
                    "fill",
                ));
            }
            styles.push(fixed);
        }
        if let Some(def) = self.classes.get_mut(id) {
            def.styles.extend(styles);
            def.text_styles.extend(text_styles);
        }
    }

    /// The style a block is given.
    fn add_style_2_node(&mut self, id: &str, styles: &str) {
        let index = self.named(id);
        self.arena[index].styles = styles.split(',').map(str::to_string).collect();
    }

    /// The class a block is given.
    ///
    /// Mermaid looks each id up before trimming it, so `class a, b hot` misses b. Here it is
    /// trimmed first.
    fn set_css_class(&mut self, item_ids: &str, css_class_name: &str) {
        for id in item_ids.split(',') {
            let index = self.named(id.trim());
            self.arena[index].classes.push(css_class_name.to_string());
        }
    }
}

/// The model of a block diagram, from the statements the parser read.
pub(crate) fn build_block(parsed: &Parsed, title: Option<&str>) -> Result<BlockModel, Error> {
    let mut model = BlockModel {
        title: title.map(str::to_string),
        arena: vec![Block {
            id: "root".to_string(),
            kind: Kind::Composite,
            columns: Some(-1),
            ..Block::default()
        }],
        root: 0,
        edges: Vec::new(),
        classes: Ordered::default(),
        by_id: Ordered::default(),
        warnings: Vec::new(),
        pending: Vec::new(),
        edge_count: Ordered::default(),
        ids: parsed.ids,
        colors: 0,
        spaces: 0,
    };

    let mut stack = vec![frame(parsed.statements.clone(), model.root)];
    while !stack.is_empty() {
        let (at, len) = {
            let f = stack.last().expect("the stack is never empty here");
            (f.at, f.list.len())
        };
        if at == len {
            let f = stack.pop().expect("the stack is never empty here");
            model.arena[f.parent].children = f.children;
            continue;
        }
        let statement = stack.last().expect("the stack is never empty here").list[at].clone();
        stack.last_mut().expect("the stack is never empty here").at += 1;
        let parent = stack.last().expect("the stack is never empty here").parent;
        let columns = stack.last().expect("the stack is never empty here").columns;

        let block = match statement {
            Statement::ClassDef { id, css } => {
                model.add_style_class(&id, &css);
                continue;
            }
            Statement::ApplyClass { id, style_class } => {
                model.set_css_class(&id, &style_class);
                continue;
            }
            Statement::ApplyStyles { id, styles_str } => {
                if !styles_str.is_empty() {
                    model.add_style_2_node(&id, &styles_str);
                }
                continue;
            }
            Statement::Columns { columns } => {
                model.arena[parent].columns = Some(columns);
                continue;
            }
            Statement::Edge(edge) => {
                let count = match model.edge_count.get_mut(&edge.id) {
                    Some(count) => {
                        *count += 1;
                        *count
                    }
                    None => {
                        model.edge_count.set(&edge.id, 1);
                        1
                    }
                };
                model.edges.push(Edge {
                    id: format!("{count}-{}", edge.id),
                    start: edge.start,
                    end: edge.end,
                    label: edge.label,
                    arrow_type_start: edge.arrow_type_start,
                    arrow_type_end: edge.arrow_type_end,
                    thickness: edge.thickness,
                    pattern: edge.pattern,
                });
                continue;
            }
            Statement::Block(block) => block,
        };

        // A block that is wider than the number of columns of its composite is a warning.
        if let Some(span) = block
            .width_in_columns
            .filter(|&span| columns > 0 && span > columns)
        {
            model.warnings.push(format!(
                "Block {} width {span} exceeds configured column width {columns}",
                block.id.as_deref().unwrap_or("")
            ));
        }

        // A block without an id of its own gets one, as Mermaid does.
        let id = match block.id.clone() {
            Some(id) => id,
            None => {
                model.ids += 1;
                format!("id-{}", model.ids)
            }
        };
        // A block without a label of its own is named after its id, and a composite is empty.
        let label = match block.label.clone() {
            Some(label) if !label.is_empty() => label,
            _ => {
                if block.kind == Kind::Composite {
                    String::new()
                } else {
                    id.clone()
                }
            }
        };

        let known = model.by_id.get(&id).copied();
        let fresh = match known {
            None => true,
            Some(_) => {
                if let Some(at) = model.pending.iter().position(|p| *p == id) {
                    model.pending.remove(at);
                    true
                } else {
                    false
                }
            }
        };

        let mut classes = Vec::new();
        let mut styles = Vec::new();
        let mut color_index = None;
        if fresh {
            if let Some(k) = known {
                classes = model.arena[k].classes.clone();
                styles = model.arena[k].styles.clone();
            }
            if block.kind == Kind::Composite {
                color_index = Some(model.colors);
                model.colors += 1;
            }
        }

        let index = model.arena.len();
        model.arena.push(Block {
            id: id.clone(),
            kind: block.kind,
            label: Some(label.clone()),
            children: Vec::new(),
            columns: None,
            width_in_columns: block.width_in_columns,
            directions: block.directions.clone(),
            classes,
            styles,
            color_index,
        });

        if fresh {
            model.by_id.set(&id, index);
        } else {
            let k = known.expect("a block that is not fresh is known");
            if block.kind != Kind::Na {
                model.arena[k].kind = block.kind;
            }
            if label != id {
                model.arena[k].label = Some(label.clone());
            }
        }

        let inner = block.children.clone();
        if block.kind == Kind::Space {
            let width = block.width.unwrap_or(1);
            if width > MAX_SPACES - model.spaces {
                return Err(Error::new(
                    ErrorCode::Limit,
                    format!("A block diagram can hold at most {MAX_SPACES} space blocks."),
                ));
            }
            model.spaces += width.max(0);
            for j in 0..width {
                let copy_id = format!("{id}-{j}");
                let copy = model.arena.len();
                model.arena.push(Block {
                    id: copy_id.clone(),
                    kind: block.kind,
                    label: Some(label.clone()),
                    children: Vec::new(),
                    columns: None,
                    width_in_columns: block.width_in_columns,
                    directions: block.directions.clone(),
                    classes: model.arena[index].classes.clone(),
                    styles: model.arena[index].styles.clone(),
                    color_index: model.arena[index].color_index,
                });
                model.by_id.set(&copy_id, copy);
                stack
                    .last_mut()
                    .expect("the stack is never empty here")
                    .children
                    .push(copy);
            }
        } else if fresh {
            stack
                .last_mut()
                .expect("the stack is never empty here")
                .children
                .push(index);
        }

        if !inner.is_empty() {
            stack.push(frame(inner, index));
        }
    }

    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_columns() {
        let parsed = super::super::parser::parse_block("block\ncolumns 2\nA\nB\n", 64).unwrap();
        let model = build_block(&parsed, None).unwrap();
        assert_eq!(model.arena[model.root].columns, Some(2));
    }
}
