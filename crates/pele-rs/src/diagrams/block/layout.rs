//! The layout of a block diagram: the grid of the blocks of every composite, and the place of
//! every block in it. Ported from pele's `diagrams/block/layout.ts`.

use crate::error::{Error, ErrorCode};

/// The widest a block may span.
const MAX_SPAN: i64 = 1000;

/// A cell of the grid: a block, and the blocks it holds.
#[derive(Debug, Clone)]
pub(crate) struct Cell {
    /// The cells this one holds.
    pub children: Vec<usize>,
    /// The grid width of the container: a positive count, or -1 to put every child on one row.
    pub columns: i64,
    pub span: i64,
    pub space: bool,
    /// The height reserved above a container's grid for its title.
    pub head: f64,
    /// The size the content needs.
    pub nw: f64,
    pub nh: f64,
    /// The placement: the grid position, then the centre and the size in pixels.
    pub col: i64,
    pub row: i64,
    pub cols: i64,
    pub heights: Vec<f64>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Cell {
    /// A cell with nothing in it.
    pub(crate) fn new(nw: f64, nh: f64, span: i64) -> Self {
        Self {
            children: Vec::new(),
            columns: -1,
            span,
            space: false,
            head: 0.0,
            nw,
            nh,
            col: 0,
            row: 0,
            cols: 0,
            heights: Vec::new(),
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        }
    }
}

/// The place of a block in the grid of its container.
pub(crate) fn calculate_block_position(columns: i64, position: i64) -> Result<(i64, i64), Error> {
    if columns == 0 {
        return Err(Error::new(
            ErrorCode::Semantic,
            "Columns must be an integer !== 0.",
        ));
    }
    if position < 0 {
        return Err(Error::new(
            ErrorCode::Semantic,
            format!("Position must be a non-negative integer.{position}"),
        ));
    }
    if columns < 0 {
        return Ok((position, 0));
    }
    Ok((position % columns, position / columns))
}

/// Puts each child in its grid cell and sizes the container. Every column is as wide as the
/// widest block needs. Every row that holds a plain block is as tall as the tallest block in the
/// container, as in Mermaid; a row of nothing but containers is only as tall as they need.
fn measure(cells: &mut [Cell], at: usize, gap: f64, inset: f64) -> Result<(), Error> {
    let columns = cells[at].columns;
    let mut heights: Vec<f64> = Vec::new();
    let mut position: i64 = 0;
    let mut cols: i64 = 0;
    let mut cw: f64 = 0.0;
    let mut tallest: f64 = 0.0;
    for child in cells[at].children.clone() {
        let (px, py) = calculate_block_position(columns, position)?;
        // Mermaid lets a block wider than the rest of its row stick out of the grid. Here it stops
        // at the last column.
        let mut span = if cells[child].span >= 1 {
            cells[child].span.min(MAX_SPAN)
        } else {
            1
        };
        if columns > 0 && span > columns - px {
            span = columns - px;
        }
        cells[child].span = span;
        cells[child].col = px;
        cells[child].row = py;
        position += span;
        if px + span > cols {
            cols = px + span;
        }
        if py as usize == heights.len() {
            heights.push(0.0);
        }
        if cells[child].children.is_empty() {
            heights[py as usize] = -1.0;
        } else if heights[py as usize] >= 0.0 && cells[child].nh > heights[py as usize] {
            heights[py as usize] = cells[child].nh;
        }
        if cells[child].space {
            continue;
        }
        let width = (cells[child].nw - gap * (span - 1) as f64) / span as f64;
        if width > cw {
            cw = width;
        }
        if cells[child].nh > tallest {
            tallest = cells[child].nh;
        }
    }
    let mut height = (heights.len() as f64 - 1.0) * gap;
    for row in heights.iter_mut() {
        if *row < 0.0 {
            *row = tallest;
        }
        height += *row;
    }
    cells[at].cols = cols;
    cells[at].heights = heights;
    cells[at].nw = cells[at]
        .nw
        .max(cols as f64 * cw + (cols as f64 - 1.0) * gap + 2.0 * inset);
    cells[at].nh = height + 2.0 * inset + cells[at].head;
    Ok(())
}

/// Gives each child its cell, sharing out any room the container has beyond what it needs.
fn arrange(cells: &mut [Cell], at: usize, gap: f64, inset: f64) {
    let cols = cells[at].cols;
    let heights = cells[at].heights.clone();
    let rows = heights.len();
    let cw = (cells[at].w - 2.0 * inset - (cols as f64 - 1.0) * gap) / cols as f64;
    let room = cells[at].h - 2.0 * inset - cells[at].head - (rows as f64 - 1.0) * gap;
    let needed: f64 = heights.iter().sum();
    let scale = if needed > 0.0 { room / needed } else { 0.0 };
    let share = if needed > 0.0 {
        0.0
    } else {
        room / rows as f64
    };
    let left = cells[at].x - cells[at].w / 2.0 + inset;
    let mut top = cells[at].y - cells[at].h / 2.0 + inset + cells[at].head;
    let mut row = 0usize;
    for child in cells[at].children.clone() {
        while (row as i64) < cells[child].row {
            top += heights[row] * scale + share + gap;
            row += 1;
        }
        cells[child].w = cw * cells[child].span as f64 + gap * (cells[child].span - 1) as f64;
        cells[child].h = heights[row] * scale + share;
        cells[child].x = left + cells[child].col as f64 * (cw + gap) + cells[child].w / 2.0;
        cells[child].y = top + cells[child].h / 2.0;
    }
}

/// Lays out a tree of cells in place. The root has no border, so its grid starts at the origin.
pub(crate) fn layout(cells: &mut [Cell], root: usize, gap: f64) -> Result<(f64, f64), Error> {
    // The cells are measured from the leaves up, and placed from the root down.
    let mut order: Vec<usize> = vec![root];
    let mut at = 0;
    while at < order.len() {
        let parent = order[at];
        at += 1;
        order.extend(cells[parent].children.iter().copied());
    }
    for (i, index) in order.iter().copied().enumerate().rev() {
        if cells[index].children.is_empty() {
            continue;
        }
        let inset = if i == 0 { 0.0 } else { gap };
        measure(cells, index, gap, inset)?;
    }
    cells[root].w = cells[root].nw;
    cells[root].h = cells[root].nh;
    cells[root].x = cells[root].w / 2.0;
    cells[root].y = cells[root].h / 2.0;
    for (i, index) in order.iter().copied().enumerate() {
        if cells[index].children.is_empty() {
            continue;
        }
        let inset = if i == 0 { 0.0 } else { gap };
        arrange(cells, index, gap, inset);
    }
    Ok((cells[root].w, cells[root].h))
}
