use std::fmt;

use crate::DiagramKind;

/// What kind of failure an [`Error`] reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// No diagram type was detected, or the detected type is not compiled in.
    Unsupported,
    /// The text does not follow the diagram's grammar.
    Syntax,
    /// The text parses but describes something invalid, such as an edge to a missing node.
    Semantic,
    /// The source, the output or the diagram's structure exceeds a [`Limits`](crate::Limits) value.
    Limit,
}

/// Why a diagram could not be rendered.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    /// The detected diagram type, when detection got that far.
    pub kind: Option<DiagramKind>,
    /// 1-based line of the error in the source, or 0 when it has no position.
    pub line: u32,
    /// 1-based column of the error, counted in characters, or 0 when it has no position.
    pub column: u32,
    pub message: String,
    /// The source line at the error, followed by a line with a caret under the error position.
    /// Empty when the error has no position.
    pub snippet: String,
}

impl Error {
    pub(crate) fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            kind: None,
            line: 0,
            column: 0,
            message: message.into(),
            snippet: String::new(),
        }
    }

    pub(crate) fn with_kind(mut self, kind: DiagramKind) -> Self {
        self.kind.get_or_insert(kind);
        self
    }

    pub(crate) fn at(mut self, src: &str, offset: usize) -> Self {
        let loc = locate(src, offset);
        self.line = loc.line;
        self.column = loc.column;
        self.snippet = loc.snippet;
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

pub(crate) struct Location {
    pub line: u32,
    pub column: u32,
    pub snippet: String,
}

/// The line, column and snippet of a byte offset in `src`.
pub(crate) fn locate(src: &str, offset: usize) -> Location {
    let offset = offset.min(src.len());
    let line_start = src[..offset].rfind('\n').map_or(0, |i| i + 1);
    let line_end = src[offset..].find('\n').map_or(src.len(), |i| offset + i);
    let line = src[..line_start].matches('\n').count() + 1;
    let column = src[line_start..offset].chars().count() + 1;
    let snippet = format!(
        "{}\n{}^",
        &src[line_start..line_end],
        "-".repeat(column - 1)
    );
    Location {
        line: u32::try_from(line).unwrap_or(u32::MAX),
        column: u32::try_from(column).unwrap_or(u32::MAX),
        snippet,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locates_offsets() {
        let loc = locate("pie\n  bad line\n", 6);
        assert_eq!((loc.line, loc.column), (2, 3));
        assert_eq!(loc.snippet, "  bad line\n--^");
    }

    #[test]
    fn counts_columns_in_characters() {
        let src = "標題 x";
        let loc = locate(src, src.find('x').unwrap());
        assert_eq!(loc.column, 4);
        assert_eq!(loc.snippet, "標題 x\n---^");
    }
}
