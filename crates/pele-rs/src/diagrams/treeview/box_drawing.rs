//! The box-drawing preprocessor of a tree view.
//!
//! A tree may be written with box-drawing characters instead of an indentation. Mermaid turns
//! such a text into the indentation its grammar reads before the text reaches the parser:
//! `packages/mermaid/src/diagrams/treeView/boxDrawingPreprocessor.ts` in Mermaid 12.1.0. The
//! port here follows that file.

use crate::util::{is_space, round, trim};

/// The characters a tree may be drawn with.
const BOX_CHARS: [char; 8] = ['─', '━', '│', '┃', '└', '┗', '├', '┣'];
/// The characters a branch starts with.
const BRANCH_CHARS: [char; 4] = ['└', '┗', '├', '┣'];
/// The characters a branch is drawn with.
const DASH_CHARS: [char; 2] = ['─', '━'];

/// The text the grammar reads, and where its lines came from.
#[derive(Debug)]
pub(crate) struct BoxDrawing {
    /// The text the grammar reads.
    pub text: String,
    /// The line of the source each line of the text came from, counted from one. Empty when
    /// the text was not changed.
    pub lines: Vec<usize>,
}

/// A line of the source that cannot be read.
#[derive(Debug)]
pub(crate) struct BoxError {
    /// The line of the source that is refused, counted from one.
    pub line: usize,
    pub message: String,
}

/// Turns a tree that is drawn with box-drawing characters into one that is indented.
pub(crate) fn preprocess(source: &str) -> Result<BoxDrawing, BoxError> {
    let lines: Vec<&str> = source.split('\n').collect();
    let Some(keyword) = lines.iter().position(|line| trim(line) == "treeView-beta") else {
        // Without the keyword there is nothing to draw, and the grammar says so itself.
        return Ok(unchanged(source));
    };

    // The lines that hold the tree itself, with the tabs already turned into spaces.
    let mut content: Vec<String> = Vec::new();
    for line in lines.iter().skip(keyword + 1) {
        if trim(line).is_empty() || is_comment(line) || is_metadata(line) {
            continue;
        }
        if is_decoration(line) {
            continue;
        }
        content.push(line.replace('\t', "    "));
    }
    if !content.iter().any(|line| line.chars().any(is_box_char)) {
        return Ok(unchanged(source));
    }

    let segment = segment_width(&content);
    let mut out: Vec<String> = Vec::new();
    let mut map: Vec<usize> = Vec::new();
    for (index, line) in lines.iter().take(keyword + 1).enumerate() {
        out.push((*line).to_string());
        map.push(index + 1);
    }
    for (index, line) in lines.iter().enumerate().skip(keyword + 1) {
        if trim(line).is_empty() || is_comment(line) || is_metadata(line) {
            out.push((*line).to_string());
            map.push(index + 1);
            continue;
        }
        if is_decoration(line) {
            // A line that only carries the bars of the tree is not read.
            continue;
        }
        let normalized = line.replace('\t', "    ");
        let chars: Vec<char> = normalized.chars().collect();
        match chars.iter().position(|c| is_branch_char(*c)) {
            Some(at) => {
                // The depth of a branch is where it stands, in the width of a segment.
                let column = chars[..at].iter().map(|c| c.len_utf16()).sum::<usize>();
                let depth = round(column as f64 / segment as f64) as usize + 1;
                let mut pos = at + 1;
                while pos < chars.len() && is_dash_char(chars[pos]) {
                    pos += 1;
                }
                while pos < chars.len() && chars[pos] == ' ' {
                    pos += 1;
                }
                let text: String = chars[pos..].iter().collect();
                let text = text.trim_end();
                if text.is_empty() {
                    return Err(BoxError {
                        line: index + 1,
                        message: format!(
                            "Line {}: Empty node — expected a filename or directory name after the \
                             box-drawing prefix",
                            index + 1
                        ),
                    });
                }
                out.push("    ".repeat(depth) + text);
                map.push(index + 1);
            }
            None if is_box_only(&normalized) => {
                // A line that carries nothing but the drawing of the tree is not read.
            }
            None if chars.iter().any(|c| is_box_char(*c)) => {
                // A name that holds a box character, such as "Section ─ A.txt", is a node of
                // the top of the tree.
                out.push((*line).to_string());
                map.push(index + 1);
            }
            None if chars.first().copied().is_some_and(is_space) => {
                return Err(BoxError {
                    line: index + 1,
                    message: format!(
                        "Line {}: Unexpected indentation without box-drawing characters. In \
                         box-drawing format, use ├── or └── prefixes for indented nodes.",
                        index + 1
                    ),
                });
            }
            None => {
                out.push((*line).to_string());
                map.push(index + 1);
            }
        }
    }
    Ok(BoxDrawing {
        text: out.join("\n"),
        lines: map,
    })
}

/// The text as it was, with no line to remember.
fn unchanged(source: &str) -> BoxDrawing {
    BoxDrawing {
        text: source.to_string(),
        lines: Vec::new(),
    }
}

/// The width of a segment of the tree, which is where the first branch that is not at the
/// left edge stands.
fn segment_width(content: &[String]) -> usize {
    for line in content {
        let chars: Vec<char> = line.chars().collect();
        if let Some(at) = chars.iter().position(|c| is_branch_char(*c)) {
            let column = chars[..at].iter().map(|c| c.len_utf16()).sum::<usize>();
            if column > 0 {
                return column;
            }
        }
    }
    4
}

/// Whether a character is one of the tree is drawn with.
fn is_box_char(c: char) -> bool {
    BOX_CHARS.contains(&c)
}

/// Whether a character starts a branch.
fn is_branch_char(c: char) -> bool {
    BRANCH_CHARS.contains(&c)
}

/// Whether a character is one of the ones a branch is drawn with.
fn is_dash_char(c: char) -> bool {
    DASH_CHARS.contains(&c)
}

/// Whether a line carries nothing but the bars of the tree.
fn is_decoration(line: &str) -> bool {
    !line.is_empty() && line.chars().all(|c| is_space(c) || c == '│' || c == '┃')
}

/// Whether a line carries nothing but the drawing of the tree.
fn is_box_only(line: &str) -> bool {
    !line.is_empty() && line.chars().all(|c| is_space(c) || is_box_char(c))
}

/// Whether a line is a comment.
fn is_comment(line: &str) -> bool {
    line.trim_start_matches(is_space).starts_with("%%")
}

/// Whether a line names the title of the diagram or its accessibility.
fn is_metadata(line: &str) -> bool {
    let rest = line.trim_start_matches(is_space);
    if let Some(rest) = rest.strip_prefix("title") {
        return rest.starts_with([' ', '\t']);
    }
    if let Some(rest) = rest.strip_prefix("accTitle") {
        return rest.trim_start_matches([' ', '\t']).starts_with(':');
    }
    if let Some(rest) = rest.strip_prefix("accDescr") {
        let rest = rest.trim_start_matches([' ', '\t']);
        return rest.starts_with(':') || rest.starts_with('{');
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(source: &str) -> String {
        preprocess(source).expect("the source reads").text
    }

    #[test]
    fn turns_a_branch_into_an_indentation() {
        assert_eq!(
            read("treeView-beta\n├── src\n│   └── index.js\n└── package.json\n"),
            "treeView-beta\n    src\n        index.js\n    package.json\n"
        );
    }

    #[test]
    fn leaves_an_indented_tree_alone() {
        let source = "treeView-beta\n    src\n        index.js\n";
        assert_eq!(read(source), source);
    }

    #[test]
    fn reads_the_depth_of_a_branch_from_the_width_of_a_segment() {
        // The segment is as wide as the place the first branch stands in, so a branch that
        // stands at the beginning of a segment is one level deeper than the root.
        assert_eq!(
            read("treeView-beta\n│   ├── a\n│   └── b\n"),
            "treeView-beta\n        a\n        b\n"
        );
    }

    #[test]
    fn refuses_a_branch_without_a_name() {
        let error = preprocess("treeView-beta\n├──\n").expect_err("the source is refused");
        assert_eq!(error.line, 2);
        assert!(error.message.contains("Empty node"), "{}", error.message);
    }

    #[test]
    fn refuses_an_indentation_without_a_branch() {
        let error = preprocess("treeView-beta\n├── a\n    b\n").expect_err("the source is refused");
        assert_eq!(error.line, 3);
        assert!(
            error.message.contains("Unexpected indentation"),
            "{}",
            error.message
        );
    }

    #[test]
    fn keeps_a_name_that_holds_a_box_character() {
        assert_eq!(
            read("treeView-beta\nSection ─ A.txt\n"),
            "treeView-beta\nSection ─ A.txt\n"
        );
    }
}
