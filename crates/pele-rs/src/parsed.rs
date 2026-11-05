//! Reusable, immutable diagram models. Device scale and zoom belong to the host's painter.

use std::sync::Arc;

use crate::diagram::{Input, Parsed};
use crate::preprocess::{Preprocessed, preprocess, with_host_config};
use crate::text::entities::encode_entities;
use crate::text::{Host, HostMeasurer};
use crate::{DiagramKind, Error, ErrorCode, Limits, MetricsMeasurer, RenderOptions, Rendered};

struct CheckedMeasurer<'a> {
    inner: &'a dyn crate::TextMeasurer,
    invalid: std::cell::Cell<bool>,
}
impl crate::TextMeasurer for CheckedMeasurer<'_> {
    fn width(&self, text: &str, size: f64, style: crate::Style) -> f64 {
        let width = self.inner.width(text, size, style);
        if width.is_finite() && width >= 0.0 {
            width
        } else {
            self.invalid.set(true);
            0.0
        }
    }
    fn host(&self) -> Option<&Host<'_>> {
        self.inner.host()
    }
}

/// Settings that affect parsing. These stay fixed for the lifetime of a parsed diagram.
#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    pub limits: Limits,
    /// Host configuration as JSON, underneath the diagram's own configuration.
    pub config: Option<String>,
}

/// An owned model that can be cloned cheaply, shared between workers, and rendered repeatedly.
/// No font callbacks, drawing backend, or global state are retained here.
#[derive(Clone)]
pub struct ParsedDiagram {
    source: Arc<str>,
    parser_source: Arc<str>,
    pre: Arc<Preprocessed>,
    model: Arc<dyn Parsed>,
    kind: DiagramKind,
    keep_comments: bool,
    math: bool,
}

impl std::fmt::Debug for ParsedDiagram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedDiagram")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

/// Parse once, then call [`ParsedDiagram::render`] for different widths or fonts.
pub fn parse(src: &str, options: &ParseOptions) -> Result<ParsedDiagram, Error> {
    if src.encode_utf16().count() > options.limits.source {
        return Err(Error::new(
            ErrorCode::Limit,
            format!(
                "Diagram source is longer than the limit of {} characters.",
                options.limits.source
            ),
        ));
    }
    let mut pre = preprocess(src)?;
    let kind = crate::detect::detect(&pre.text).ok_or_else(|| {
        crate::at_diagram_start(
            Error::new(
                ErrorCode::Unsupported,
                "No diagram type detected for the given text.",
            ),
            src,
            &pre,
        )
    })?;
    let diagram = crate::diagram::lookup(kind).ok_or_else(|| {
        crate::at_diagram_start(
            Error::new(
                ErrorCode::Unsupported,
                format!("Diagram type \"{}\" is not supported yet.", kind.as_str()),
            )
            .with_kind(kind),
            src,
            &pre,
        )
    })?;
    if let Some(config) = &options.config {
        pre.config = with_host_config(config, &pre.config).map_err(|e| e.with_kind(kind))?;
    }
    let source = if diagram.keep_comments() {
        &pre.with_comments
    } else {
        &pre.text
    };
    let parser_source = encode_entities(source) + "\n";
    let render_options = RenderOptions {
        limits: options.limits,
        ..Default::default()
    };
    let input = Input {
        source: parser_source.clone(),
        config: &pre.config,
        title: pre.title.as_deref(),
        line_offset: pre.line_offset,
        options: &render_options,
        measurer: &MetricsMeasurer,
    };
    let model = diagram.parse(&input).map_err(|e| {
        crate::source_map::remap(
            e.with_kind(kind),
            src,
            &parser_source,
            diagram.keep_comments(),
        )
    })?;
    Ok(ParsedDiagram {
        source: Arc::from(src),
        parser_source: Arc::from(parser_source),
        pre: Arc::new(pre),
        model: Arc::from(model),
        kind,
        keep_comments: diagram.keep_comments(),
        math: diagram.math(),
    })
}

impl ParsedDiagram {
    pub fn kind(&self) -> DiagramKind {
        self.kind
    }
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Render without parsing again. Width, fonts, palette, links, and time may change.
    /// Parsing config and structural limits are retained by the model; render options do not reparse it.
    /// Cache the returned SVG separately from device-dependent raster images.
    pub fn render(&self, options: &RenderOptions) -> Result<Rendered, Error> {
        let measured = options.measurer.as_deref().unwrap_or(&MetricsMeasurer);
        let math = options.math.as_ref().filter(|_| self.math);
        let hosted;
        let measurer = if options.icons.is_some() || math.is_some() {
            hosted = HostMeasurer::new(measured, Host::new(options.icons.as_ref(), math));
            &hosted as &dyn crate::TextMeasurer
        } else {
            measured
        };
        let checked = CheckedMeasurer {
            inner: measurer,
            invalid: std::cell::Cell::new(false),
        };
        let input = Input {
            source: self.parser_source.to_string(),
            config: &self.pre.config,
            title: self.pre.title.as_deref(),
            line_offset: self.pre.line_offset,
            options,
            measurer: &checked,
        };
        let _palette = crate::palette::enter(self.kind, &self.pre.config);
        let numeric = crate::svg::NumericGuard::new();
        let mut out = self.model.render(&input).map_err(|e| {
            crate::source_map::remap(
                e.with_kind(self.kind),
                &self.source,
                &self.parser_source,
                self.keep_comments,
            )
        })?;
        if checked.invalid.get()
            || numeric.invalid()
            || !out.width.is_finite()
            || !out.height.is_finite()
        {
            return Err(Error::new(
                ErrorCode::Limit,
                "Diagram cannot be laid out in finite numbers.",
            )
            .with_kind(self.kind));
        }
        if let Some(palette) = &options.palette {
            out.svg =
                crate::svg::resolve_vars(&out.svg, palette, options.font_family_mono.as_deref());
        }
        if out.svg.encode_utf16().count() > options.limits.output {
            return Err(Error::new(
                ErrorCode::Limit,
                format!(
                    "Diagram output is longer than the limit of {} characters.",
                    options.limits.output
                ),
            )
            .with_kind(self.kind));
        }
        for link in &mut out.links {
            link.id = crate::text::entities::decode_entities(&link.id);
        }
        Ok(Rendered {
            svg: out.svg,
            width: out.width,
            height: out.height,
            links: out.links,
            kind: self.kind,
        })
    }
}
