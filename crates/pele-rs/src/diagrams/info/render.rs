//! The drawing of an information diagram: the title of the diagram, and the version of the
//! library under it. Ported from pele's `diagrams/info/render.ts`, whose layout pele-rs keeps.
//!
//! Mermaid's own renderer draws the version alone, in a box of 100 by 400 pixels, and reads no
//! title at all. pele draws the title of the diagram over the version, and pele-rs keeps that:
//! the layout of a diagram is pele's, not Mermaid's.

use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::diagrams::info::db::InfoModel;
use crate::svg::{Accessible, label_svg, svg_document};
use crate::text::Style;
use crate::text::label::layout_label;

/// The version of the library, as the diagram shows it.
const VERSION: &str = "1.13.0";

/// The space between the title and the version under it.
const TITLE_GAP: f64 = 8.0;

/// The width the title and the version are laid out in, as pele lays them out: a title wider
/// than this is wrapped.
const TEXT_WIDTH: f64 = 4000.0;

/// Draws an information diagram.
pub(crate) fn render_info(model: &InfoModel, input: &Input<'_>) -> Output {
    let size = input.options.font_size.unwrap_or(16.0);
    let pad = input.options.padding.unwrap_or(8.0);
    let measurer = input.measurer;

    let title = layout_label(
        model.title.as_deref(),
        false,
        measurer,
        size,
        TEXT_WIDTH,
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + TITLE_GAP
    } else {
        0.0
    };
    let version_text = format!("Pele v{VERSION}");
    let version = layout_label(
        Some(&version_text),
        false,
        measurer,
        size,
        TEXT_WIDTH,
        Style::NONE,
    );

    let width = (title.width.max(version.width) + 2.0 * pad).ceil();
    let height = (title_height + version.height + 2.0 * pad).ceil();
    let content = format!(
        "{}{}",
        label_svg(
            &title,
            width / 2.0,
            pad + title.height / 2.0,
            " class=\"pele-title\" font-weight=\"var(--_tw)\""
        ),
        label_svg(
            &version,
            width / 2.0,
            pad + title_height + version.height / 2.0,
            " class=\"pele-version\" fill=\"var(--_m)\""
        )
    );

    let acc = Accessible {
        title: model.acc_title.clone(),
        descr: model.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            DiagramKind::Info,
            width,
            height,
            size,
            input.options,
            &acc,
            &content,
        ),
        width,
        height,
        links: Vec::new(),
    }
}
