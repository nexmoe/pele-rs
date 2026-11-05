//! An optional reusable SVG scene for native painters. The default core remains std-only.
//!
//! This adapter parses SVG once with usvg; it is not a layout-native scene IR. Hosts walk the
//! tree's groups, paths and text, preserving transforms, clipping, gradients and opacity.
//! Painting at a new zoom does not call the Mermaid parser, layout, or SVG parser again.
//! Fonts are supplied by the host. No system fonts, external images, files or URLs are loaded.

use crate::{Error, ErrorCode, Palette, ParsedDiagram, RenderOptions, Rendered};
use std::sync::Arc;

pub use usvg;

// Placeholder fonts claim broad Unicode coverage but do not contain the real glyphs.
// usvg may replace an entire mixed Latin/emoji run when LastResort claims every character.
// Keep measurement and scene shaping consistent without mutating the host's database.
fn usable_fonts(mut fonts: Arc<usvg::fontdb::Database>) -> Arc<usvg::fontdb::Database> {
    let placeholders: Vec<_> = fonts
        .faces()
        .filter(|face| {
            face.post_script_name == "LastResort"
                || face.families.iter().any(|(name, _)| name == "Last Resort")
        })
        .map(|face| face.id)
        .collect();
    if !placeholders.is_empty() {
        let db = Arc::make_mut(&mut fonts);
        for id in placeholders {
            db.remove_face(id);
        }
    }
    fonts
}

/// An owned vector tree and its SVG export. Safe to move to a background worker or share.
pub struct SvgScene {
    rendered: Rendered,
    tree: usvg::Tree,
}

/// Measures with the same shaping and fallback implementation as usvg/resvg. A unique text
/// run is shaped through a tiny SVG; wrap this in `CachingMeasurer` to reuse the result.
/// The host supplies font bytes and family names; no system font discovery is performed.
/// LastResort placeholder faces are excluded from both measurement and scene shaping.
pub struct FontMeasurer {
    fonts: Arc<usvg::fontdb::Database>,
    family: String,
    mono: String,
}

impl FontMeasurer {
    pub fn new(
        fonts: Arc<usvg::fontdb::Database>,
        family: impl Into<String>,
        mono: impl Into<String>,
    ) -> Result<Self, Error> {
        let fonts = usable_fonts(fonts);
        if fonts.faces().next().is_none() {
            return Err(Error::new(
                ErrorCode::Semantic,
                "Font measurement requires a host font database.",
            ));
        }
        Ok(Self {
            fonts,
            family: family.into(),
            mono: mono.into(),
        })
    }
}

impl crate::TextMeasurer for FontMeasurer {
    fn width(&self, text: &str, size: f64, style: crate::Style) -> f64 {
        if text.is_empty() {
            return 0.0;
        }
        if !size.is_finite() || size <= 0.0 {
            return f64::NAN;
        }
        if text.chars().all(char::is_whitespace) {
            // usvg omits a whitespace-only text node. A visible sentinel preserves the
            // advance, including leading spaces, without relying on glyph ink bounds.
            return self.width(&(text.to_string() + "A"), size, style)
                - self.width("A", size, style);
        }
        let family = if style.has(crate::Style::MONO) {
            &self.mono
        } else {
            &self.family
        };
        let size = if style.has(crate::Style::SUB | crate::Style::SUP) {
            size / 1.2
        } else {
            size
        };
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"><text xml:space=\"preserve\" font-family=\"{}\" font-size=\"{size}\" font-weight=\"{}\" font-style=\"{}\">{}</text></svg>",
            crate::svg::esc(family),
            if style.has(crate::Style::BOLD) {
                "bold"
            } else {
                "normal"
            },
            if style.has(crate::Style::ITALIC) {
                "italic"
            } else {
                "normal"
            },
            crate::svg::esc(text)
        );
        let options = usvg::Options {
            fontdb: self.fonts.clone(),
            ..Default::default()
        };
        let Ok(tree) = usvg::Tree::from_str(&svg, &options) else {
            return f64::NAN;
        };
        fn text_width(group: &usvg::Group) -> Option<f64> {
            for node in group.children() {
                match node {
                    usvg::Node::Text(text) => return Some(f64::from(text.bounding_box().width())),
                    usvg::Node::Group(group) => {
                        if let Some(width) = text_width(group) {
                            return Some(width);
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        text_width(tree.root()).unwrap_or(0.0)
    }
}

impl SvgScene {
    /// Convert a render to a scene. Supply the same font database/fallbacks as the measurer.
    /// A render with CSS variables is resolved to the light palette; use `palette` when
    /// rendering for another appearance. HTML math and images require a host renderer and
    /// are rejected instead of disappearing silently.
    pub fn new(mut rendered: Rendered, fonts: Arc<usvg::fontdb::Database>) -> Result<Self, Error> {
        if rendered.svg.contains("<foreignObject") || rendered.svg.contains("<image") {
            return Err(Error::new(
                ErrorCode::Unsupported,
                "SVG scenes with images or HTML math require a host renderer.",
            )
            .with_kind(rendered.kind));
        }
        let fonts = usable_fonts(fonts);
        if rendered.svg.contains("<text") && fonts.faces().next().is_none() {
            return Err(Error::new(
                ErrorCode::Semantic,
                "A scene containing text requires a host font database.",
            )
            .with_kind(rendered.kind));
        }
        rendered.svg = crate::svg::resolve_vars(&rendered.svg, &Palette::light(), None);
        let options = usvg::Options {
            fontdb: fonts,
            image_href_resolver: usvg::ImageHrefResolver {
                resolve_data: Box::new(|_, _, _| None),
                resolve_string: Box::new(|_, _| None),
            },
            ..Default::default()
        };
        let tree = usvg::Tree::from_str(&rendered.svg, &options).map_err(|e| {
            Error::new(
                ErrorCode::Semantic,
                format!("The rendered SVG cannot be read: {e}"),
            )
            .with_kind(rendered.kind)
        })?;
        Ok(Self { rendered, tree })
    }
    pub fn tree(&self) -> &usvg::Tree {
        &self.tree
    }
    pub fn rendered(&self) -> &Rendered {
        &self.rendered
    }
    pub fn into_rendered(self) -> Rendered {
        self.rendered
    }
}

impl ParsedDiagram {
    /// Render with concrete colors, then build a reusable tree for a native painter.
    pub fn scene(
        &self,
        options: &RenderOptions,
        fonts: Arc<usvg::fontdb::Database>,
    ) -> Result<SvgScene, Error> {
        let mut rendered = self.render(options)?;
        if options.palette.is_none() {
            rendered.svg = crate::svg::resolve_vars(
                &rendered.svg,
                &Palette::light(),
                options.font_family_mono.as_deref(),
            );
        }
        SvgScene::new(rendered, fonts)
    }
}
