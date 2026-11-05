use super::theme::{RADIUS, ROOT_STYLE, font_style};
use super::{esc, esc_text, num};
use crate::DiagramKind;
use crate::RenderOptions;

/// The accessible name and description a diagram declares with `accTitle` and `accDescr`.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Accessible {
    pub title: Option<String>,
    pub descr: Option<String>,
}

/// The weights of bold text, each set only in a diagram that has such text.
const WEIGHTS: &[(&str, &str)] = &[("tw", "title"), ("hw", "heading"), ("w", "bold")];

/// Wraps diagram content in the root `<svg>` element.
pub(crate) fn svg_document(
    kind: DiagramKind,
    width: f64,
    height: f64,
    font_size: f64,
    options: &RenderOptions,
    acc: &Accessible,
    content: &str,
) -> String {
    let prefix = esc(options.id_prefix.as_deref().unwrap_or("pele"));
    let mut head = String::new();
    let mut aria = String::new();
    if let Some(title) = acc.title.as_deref().filter(|t| !t.is_empty()) {
        head += &format!("<title id=\"{prefix}-title\">{}</title>", esc_text(title));
        aria += &format!(" aria-labelledby=\"{prefix}-title\"");
    }
    if let Some(descr) = acc.descr.as_deref().filter(|d| !d.is_empty()) {
        head += &format!("<desc id=\"{prefix}-desc\">{}</desc>", esc_text(descr));
        aria += &format!(" aria-describedby=\"{prefix}-desc\"");
    }
    let fit = if options
        .responsive
        .unwrap_or_else(crate::palette::responsive)
    {
        "max-width:100%;height:auto;"
    } else {
        ""
    };
    let compat = options.palette.is_some();
    // resvg drops the whole `style` attribute from the first custom property on, so a
    // viewer that needs a palette gets none of the private aliases: `var(--_x)` is resolved
    // in the content and the root keeps only its text color.
    let root = if compat {
        "color:var(--_fg)"
    } else {
        ROOT_STYLE
    };
    let mut weights = String::new();
    if !compat && content.contains("var(--_r)") {
        weights += &format!(";--_r:{RADIUS}");
    }
    for (alias, name) in WEIGHTS {
        if !compat && content.contains(&format!("var(--_{alias})")) {
            weights += &format!(";--_{alias}:var(--pele-{name}-weight,bold)");
        }
    }
    let font = font_style(
        font_size,
        options.font_family.as_deref(),
        options.font_family_mono.as_deref(),
        options.palette.is_some(),
    );
    let (w, h) = (num(width), num(height));
    let kind = kind.as_str();
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"pele pele-{kind}\" \
         viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" style=\"{fit}{root}{weights}{}{}\" \
         fill=\"var(--_fg)\" role=\"graphics-document document\" aria-roledescription=\"{kind}\"{aria}>\
         {head}{content}</svg>",
        esc(&font),
        crate::palette::style()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_the_root_element() {
        let options = RenderOptions {
            id_prefix: Some("d\"1".to_string()),
            ..RenderOptions::default()
        };
        let acc = Accessible {
            title: Some("A & B".to_string()),
            descr: None,
        };
        let svg = svg_document(
            DiagramKind::Pie,
            100.0,
            50.5,
            16.0,
            &options,
            &acc,
            "<g font-weight=\"var(--_tw)\"/>",
        );
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"pele pele-pie\" viewBox=\"0 0 100 50.5\""));
        assert!(svg.contains("<title id=\"d&quot;1-title\">A &amp; B</title><g"));
        assert!(svg.contains(" aria-labelledby=\"d&quot;1-title\">"));
        assert!(svg.contains(";--_tw:var(--pele-title-weight,bold);--_fm:"));
        assert!(!svg.contains("--_r:"));
        assert!(svg.ends_with("</svg>"));
    }
}
