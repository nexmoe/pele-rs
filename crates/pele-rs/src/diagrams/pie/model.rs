use std::collections::HashSet;

use super::parser::PieAst;
use crate::error::{Error, ErrorCode};
use crate::svg::Accessible;
use crate::util::{is_space, js_number, trim_start};

#[derive(Debug, PartialEq)]
pub(crate) struct PieModel {
    pub title: Option<String>,
    pub acc: Accessible,
    pub show_data: bool,
    /// Each label once, with the value it was first given, in the order written.
    pub sections: Vec<(String, f64)>,
}

/// `.replace(/\n\s+/g, '\n')`
fn unindent(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '\n' {
            while chars.next_if(|&c| is_space(c)).is_some() {}
        }
    }
    out
}

pub(crate) fn build_pie(ast: PieAst, title: Option<&str>) -> Result<PieModel, Error> {
    let mut seen = HashSet::new();
    let mut sections = Vec::new();
    for (label, value) in ast.sections {
        if value < 0.0 {
            return Err(Error::new(
                ErrorCode::Semantic,
                format!(
                    "\"{label}\" has invalid value: {}. Negative values are not allowed in pie \
                     charts. All slice values must be >= 0.",
                    js_number(value)
                ),
            ));
        }
        if seen.insert(label.clone()) {
            sections.push((label, value));
        }
    }
    Ok(PieModel {
        title: ast
            .title
            .filter(|t| !t.is_empty())
            .or_else(|| title.map(str::to_string)),
        acc: Accessible {
            title: ast.acc_title.map(|t| trim_start(&t).to_string()),
            descr: ast.acc_descr.map(|d| unindent(&d)),
        },
        show_data: ast.show_data,
        sections,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_first_value_of_a_label() {
        let ast = PieAst {
            sections: vec![
                ("A".to_string(), 1.0),
                ("A".to_string(), 2.0),
                ("B".to_string(), 3.0),
            ],
            ..PieAst::default()
        };
        let model = build_pie(ast, Some("front")).unwrap();
        assert_eq!(
            model.sections,
            vec![("A".to_string(), 1.0), ("B".to_string(), 3.0)]
        );
        assert_eq!(model.title.as_deref(), Some("front"));
    }

    #[test]
    fn unindents_descriptions() {
        assert_eq!(unindent("a\n  b\n\n c"), "a\nb\nc");
    }
}
