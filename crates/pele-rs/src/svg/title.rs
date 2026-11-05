use super::{label_svg, num};
use crate::text::label::{Label, layout_label};
use crate::text::{PlainMeasurer, Style, TextMeasurer};

/// Lays out a diagram title. Mermaid draws titles as plain text, whatever they hold.
pub(crate) fn title_label(
    title: Option<&str>,
    measurer: &dyn TextMeasurer,
    size: f64,
    max_width: f64,
) -> Label {
    // Math stays out of a title: it is plain text, as in Mermaid.
    let plain = PlainMeasurer::new(measurer);
    layout_label(title, false, &plain, size, max_width, Style::BOLD)
}

pub(crate) struct Titled {
    pub width: f64,
    pub height: f64,
    pub content: String,
}

/// Puts a diagram title above content that was laid out without one.
pub(crate) fn with_title(
    title: Option<&str>,
    measurer: &dyn TextMeasurer,
    size: f64,
    pad: f64,
    width: f64,
    height: f64,
    content: String,
) -> Titled {
    let label = title_label(title, measurer, size, 4000.0);
    if label.height == 0.0 {
        return Titled {
            width,
            height,
            content,
        };
    }
    let shift = label.height + 12.0;
    let full = width.max((label.width + 2.0 * pad).ceil());
    Titled {
        width: full,
        height: (height + shift).ceil(),
        content: format!(
            "{}<g transform=\"translate({},{})\">{content}</g>",
            label_svg(
                &label,
                full / 2.0,
                pad + label.height / 2.0,
                " class=\"pele-rs-title\" font-weight=\"var(--_tw)\""
            ),
            num((full - width) / 2.0),
            num(shift)
        ),
    }
}
