//! The model of an Ishikawa diagram: the effect and the causes under it, as a tree.

use super::parser::IshikawaAst;
use crate::error::{Error, ErrorCode};

/// A cause of the effect, with the causes under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IshikawaNode {
    /// The text of the cause.
    pub text: String,
    /// The causes under this one, as indexes into the nodes of the model.
    pub children: Vec<usize>,
}

/// The model of an Ishikawa diagram.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct IshikawaModel {
    /// The title from the front matter.
    pub title: Option<String>,
    /// The effect, the cause of all the others.
    pub root: Option<usize>,
    /// Every node of the tree, the effect first.
    pub nodes: Vec<IshikawaNode>,
}

impl IshikawaModel {
    /// The text of a node.
    pub(crate) fn text(&self, index: usize) -> &str {
        &self.nodes[index].text
    }
}

/// Builds the model of an Ishikawa diagram.
///
/// The first line is the effect. Every line after it is a cause, and the level it is indented by
/// says under which cause it hangs: the level of the first cause is the baseline, and a cause one
/// level deeper hangs under the cause before it, a cause at the same level beside it.
///
/// Mermaid's own database cleans the text of every cause with DOMPurify and makes the effect the
/// title of the diagram. pele keeps the text as it is and takes the title from the front matter,
/// and so does pele-rs: the text is escaped when it is drawn, and the title is drawn as one.
pub(crate) fn build_ishikawa(
    ast: &IshikawaAst,
    title: Option<String>,
    max_depth: usize,
) -> Result<IshikawaModel, Error> {
    let mut model = IshikawaModel {
        title,
        root: None,
        nodes: Vec::new(),
    };
    // The level of each cause on the stack, and the node it stands for.
    let mut stack: Vec<(usize, usize)> = Vec::new();
    // The level of the first cause, the baseline the others are measured from.
    let mut base_level: Option<usize> = None;
    for line in &ast.lines {
        let index = model.nodes.len();
        model.nodes.push(IshikawaNode {
            text: line.text.clone(),
            children: Vec::new(),
        });
        if model.root.is_none() {
            model.root = Some(index);
            stack.push((0, index));
            continue;
        }
        let base = *base_level.get_or_insert(line.level);
        let level = (line.level as i64 - base as i64 + 1).max(1) as usize;
        while stack.len() > 1 && stack.last().is_some_and(|&(l, _)| l >= level) {
            stack.pop();
        }
        let parent = stack.last().map(|&(_, node)| node).unwrap_or(index);
        model.nodes[parent].children.push(index);
        stack.push((level, index));
        if stack.len() > max_depth {
            return Err(Error::new(
                ErrorCode::Limit,
                format!("The diagram nests deeper than the limit of {max_depth}."),
            ));
        }
    }
    Ok(model)
}
