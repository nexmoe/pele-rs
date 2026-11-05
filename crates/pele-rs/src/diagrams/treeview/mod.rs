//! Tree views: a tree of files, written as an indented outline or drawn with box-drawing
//! characters.

mod box_drawing;
mod db;
mod icons;
mod lexer;
mod parser;
mod render;

#[cfg(test)]
mod compat;

use crate::diagram::{Diagram, Input, Parsed};
use crate::error::{Error, ErrorCode};

/// The treeView diagram type.
pub(crate) struct TreeView;

impl Diagram for TreeView {
    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error> {
        let source = &input.source;
        // A tree may be drawn with box-drawing characters; Mermaid turns such a text into the
        // indentation its grammar reads.
        let drawn = box_drawing::preprocess(source).map_err(|error| {
            Error::new(ErrorCode::Syntax, error.message).at(source, line_offset(source, error.line))
        })?;
        let chars: Vec<char> = drawn.text.chars().collect();
        let ast = parser::parse(&chars).map_err(|at| {
            // The grammar reads a text the box-drawing characters may have changed, so the
            // place of the error is the line it stands in.
            let line = line_of(&chars, at);
            let original = drawn.lines.get(line - 1).copied().unwrap_or(line);
            let offset = if drawn.lines.is_empty() {
                // The text was not changed, so the character keeps its place.
                char_offset(source, at)
            } else {
                line_offset(source, original)
            };
            Error::new(ErrorCode::Syntax, "The tree view could not be read.").at(source, offset)
        })?;
        let model = db::build(&ast, input.title);
        Ok(Box::new(move |input: &Input<'_>| {
            Ok(render::render_treeview(&model, input))
        }))
    }
}

/// The line a character stands in, counted from one.
fn line_of(chars: &[char], at: usize) -> usize {
    chars[..at.min(chars.len())]
        .iter()
        .filter(|&&c| c == '\n')
        .count()
        + 1
}

/// The place in the source a character stands at.
fn char_offset(source: &str, at: usize) -> usize {
    source
        .char_indices()
        .nth(at)
        .map(|(offset, _)| offset)
        .unwrap_or(source.len())
}

/// The place in the source a line starts at.
fn line_offset(source: &str, line: usize) -> usize {
    if line <= 1 {
        return 0;
    }
    let mut offset = 0;
    for _ in 1..line {
        match source[offset..].find('\n') {
            Some(at) => offset += at + 1,
            None => return source.len(),
        }
    }
    offset
}
