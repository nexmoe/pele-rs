//! The model of an information diagram: the title that is drawn, and the accessible text that
//! names the drawing.

use super::parser::InfoAst;

/// The model of an information diagram.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct InfoModel {
    /// The title of the diagram, from the `title` directive or from the front matter.
    pub title: Option<String>,
    /// The accessible title, from the `accTitle` directive.
    pub acc_title: Option<String>,
    /// The accessible description, from the `accDescr` directive.
    pub acc_descr: Option<String>,
}

/// Builds the model of an information diagram.
///
/// Mermaid's own grammar reads a `title` directive into the tree, and keeps it even when it is
/// empty. pele drops an empty title and reads the front matter title only when the diagram has
/// none. Mermaid is the specification, so an empty title stays a title here: nothing is drawn of
/// it, but it is one.
pub(crate) fn build_info(ast: &InfoAst, title: Option<&str>) -> InfoModel {
    InfoModel {
        title: ast.title.clone().or_else(|| title.map(str::to_string)),
        acc_title: ast.acc_title.clone(),
        acc_descr: ast.acc_descr.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_title_of_the_diagram() {
        let ast = InfoAst {
            title: Some("A title".to_string()),
            ..InfoAst::default()
        };
        let model = build_info(&ast, Some("From the front matter"));
        assert_eq!(model.title.as_deref(), Some("A title"));
    }

    #[test]
    fn falls_back_to_the_front_matter_title() {
        let model = build_info(&InfoAst::default(), Some("From the front matter"));
        assert_eq!(model.title.as_deref(), Some("From the front matter"));

        let model = build_info(&InfoAst::default(), None);
        assert_eq!(model.title, None);
    }

    #[test]
    fn keeps_an_empty_title() {
        let ast = InfoAst {
            title: Some(String::new()),
            ..InfoAst::default()
        };
        let model = build_info(&ast, Some("From the front matter"));
        assert_eq!(model.title.as_deref(), Some(""));
    }
}
