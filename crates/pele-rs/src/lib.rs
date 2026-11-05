//! Renders [Mermaid](https://mermaid.js.org) diagram text to SVG, in pure Rust.
//!
//! ```
//! let err = pele_rs::render("not a diagram", &Default::default()).unwrap_err();
//! assert_eq!(err.code, pele_rs::ErrorCode::Unsupported);
//! ```
//!
//! The output takes its colors and fonts from CSS custom properties (`--pele-bg`,
//! `--pele-fg`, `--pele-line`, `--pele-accent`, `--pele-series-1` to `-8`, …), so
//! a page can re-theme a diagram without rendering it again.
//!
//! Behavior, layout and limits follow [pele](https://github.com/obsidianmd/pele).

mod detect;
mod diagram;
mod diagrams;
mod error;
#[cfg(any(
    feature = "flowchart",
    feature = "state",
    feature = "class",
    feature = "er",
    feature = "requirement",
    feature = "block",
    feature = "usecase",
    feature = "agentflow",
    feature = "c4",
))]
#[allow(
    dead_code,
    reason = "parts are used by diagram types that are not written yet"
)]
mod layout;
mod options;
mod preprocess;
#[allow(
    dead_code,
    unused_imports,
    reason = "parts are used only by diagram types that are not compiled in or not written yet"
)]
mod svg;
mod text;
#[cfg(any(feature = "flowchart", feature = "class", feature = "agentflow",))]
mod unicode;
#[cfg(any(
    feature = "flowchart",
    feature = "state",
    feature = "class",
    feature = "er",
    feature = "kanban",
    feature = "gantt",
    feature = "usecase",
    feature = "agentflow",
    feature = "c4",
    feature = "wardley",
))]
#[allow(
    dead_code,
    reason = "parts are used by diagram types that are not written yet"
)]
mod url;
mod util;

pub use detect::DiagramKind;
pub use error::{Error, ErrorCode};
pub use options::{Limits, Link, MonospacePart, Palette, RenderOptions, Rendered};
pub use text::{
    CachingMeasurer, IconResolver, MathMarkup, MathResolver, MetricsMeasurer, Style, TextMeasurer,
};

mod palette;
mod parsed;
#[cfg(feature = "scene")]
pub mod scene;
mod source_map;
pub use parsed::{ParseOptions, ParsedDiagram, parse};

use preprocess::{Preprocessed, preprocess};

/// Detects the type of a diagram without parsing it.
pub fn detect_type(src: &str) -> Option<DiagramKind> {
    detect::detect(&preprocess(src).ok()?.text)
}

/// Renders diagram text to SVG.
pub fn render(src: &str, options: &RenderOptions) -> Result<Rendered, Error> {
    parse(
        src,
        &ParseOptions {
            limits: options.limits,
            config: options.config.clone(),
        },
    )?
    .render(options)
}

/// Points an error at the first line of the diagram itself, past front matter, directives and
/// comments. The error keeps no position when that line cannot be found in the source.
fn at_diagram_start(err: Error, src: &str, pre: &Preprocessed) -> Error {
    let first = pre.text.lines().next().unwrap_or("");
    if first.is_empty() {
        return err;
    }
    let body = match pre.line_offset {
        0 => 0,
        n => src
            .match_indices('\n')
            .nth(n - 1)
            .map_or(src.len(), |(i, _)| i + 1),
    };
    match src[body..].find(first) {
        Some(at) => err.at(src, body + at),
        None => err,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_unknown_types_with_a_position() {
        let src = "---\ntitle: x\n---\n%% note\n\n  hello world\n";
        let err = render(src, &RenderOptions::default()).unwrap_err();
        assert_eq!(err.code, ErrorCode::Unsupported);
        assert_eq!(err.kind, None);
        assert_eq!((err.line, err.column), (6, 3));
        assert_eq!(err.snippet, "  hello world\n--^");
    }

    #[test]
    // The type has to be out of this build for the test to have a subject: a build with every
    // feature draws every type, so there is nothing left to report as not supported.
    #[cfg(not(feature = "treeview"))]
    fn reports_types_not_compiled_in() {
        // A type whose feature is off reports that it is not supported.
        let err = render("treeView-beta\n", &RenderOptions::default()).unwrap_err();
        assert_eq!(err.code, ErrorCode::Unsupported);
        assert_eq!(err.kind, Some(DiagramKind::TreeView));
        assert_eq!(
            err.message,
            "Diagram type \"treeView\" is not supported yet."
        );
        assert_eq!((err.line, err.column), (1, 1));
    }

    #[test]
    fn reports_empty_text_without_a_position() {
        let err = render("  \n%% only a comment\n", &RenderOptions::default()).unwrap_err();
        assert_eq!(err.code, ErrorCode::Unsupported);
        assert_eq!((err.line, err.column), (0, 0));
    }

    #[test]
    fn enforces_the_source_limit() {
        let options = RenderOptions {
            limits: Limits {
                source: 3,
                ..Limits::default()
            },
            ..RenderOptions::default()
        };
        assert_eq!(
            render("abc", &options).unwrap_err().code,
            ErrorCode::Unsupported
        );
        assert_eq!(render("abcd", &options).unwrap_err().code, ErrorCode::Limit);
        assert_eq!(
            render("圓餅圖", &options).unwrap_err().code,
            ErrorCode::Unsupported
        );
    }

    #[test]
    fn detects_types() {
        assert_eq!(
            detect_type("%%{init: {}}%%\ngraph TD"),
            Some(DiagramKind::Flowchart)
        );
        assert_eq!(
            detect_type("---\ntitle: t\n---\nsequenceDiagram"),
            Some(DiagramKind::Sequence)
        );
        assert_eq!(detect_type("nothing"), None);
    }
}

#[cfg(all(test, feature = "all"))]
mod official_units;
