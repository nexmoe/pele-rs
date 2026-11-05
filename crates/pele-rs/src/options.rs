use crate::DiagramKind;
use crate::text::{IconResolver, MathResolver, TextMeasurer};

/// Caps that protect a host from untrusted diagram text. A diagram's own configuration cannot
/// raise them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The longest source accepted, in characters. Mermaid's default `maxTextSize`.
    pub source: usize,
    /// The longest SVG returned, in characters. A source within its limit can still describe
    /// a drawing of many megabytes, which would stall the page it is put into.
    pub output: usize,
    /// The most edges a flowchart may have. `A & B --> C & D` multiplies, so a few kilobytes of
    /// source can ask for millions. The other diagram types have no edge limit of their own, as
    /// pele has none for them either.
    pub max_edges: usize,
    /// The deepest nesting accepted, such as subgraphs within subgraphs. A stack overflow
    /// aborts the process in Rust, so recursion is bounded explicitly.
    pub max_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            source: 50_000,
            output: 4_000_000,
            max_edges: 5_000,
            max_depth: 64,
        }
    }
}

/// The colors of a diagram, for a viewer that does not read CSS custom properties.
///
/// By default the SVG takes its colors from `var(--pele-*, fallback)` so that the page can
/// theme it. Renderers such as resvg ignore `var()`, so a host that cannot theme the SVG
/// through CSS sets [`RenderOptions::palette`]: every color is then written out as a plain
/// value and the SVG holds no `var()` of its own.
///
/// A color is any CSS color made of letters, digits and `# % . , ( ) -` and spaces, such as
/// `#fff`, `rgb(0, 0, 0)` or `steelblue`. Anything else is replaced by the matching color of
/// [`Palette::light`], so that a color can never carry markup into the SVG.
///
/// ```
/// // The pie chart is only there when its feature is on.
/// # #[cfg(feature = "pie")]
/// # {
/// let options = pele_rs::RenderOptions {
///     palette: Some(pele_rs::Palette::dark()),
///     ..Default::default()
/// };
/// let svg = pele_rs::render("pie\n\"a\": 1\n\"b\": 2", &options).unwrap().svg;
/// assert!(!svg.contains("var("));
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    /// The page behind the diagram (`--pele-bg`).
    pub bg: String,
    /// Text (`--pele-fg`).
    pub fg: String,
    /// Secondary text and axes (`--pele-muted`).
    pub muted: String,
    /// Lines and arrows (`--pele-line`).
    pub line: String,
    /// The fill of nodes (`--pele-surface`).
    pub surface: String,
    /// The fill of nested or alternate areas (`--pele-surface-alt`).
    pub surface_alt: String,
    /// Outlines (`--pele-border`).
    pub border: String,
    /// The accent color (`--pele-accent`).
    pub accent: String,
    /// What a diagram marks as critical (`--pele-critical`).
    pub critical: String,
    /// The colors of data series, cycling (`--pele-series-1` to `-8`).
    pub series: [String; 8],
}

impl Palette {
    /// The colors the SVG falls back to on a light page.
    pub fn light() -> Self {
        let s = |c: &str| c.to_string();
        Self {
            bg: s("#fff"),
            fg: s("#1f1f1f"),
            muted: s("#6e6e6e"),
            line: s("#1f1f1f"),
            surface: s("#f3f3f3"),
            surface_alt: s("#e6e6e6"),
            border: s("#8a8a8a"),
            accent: s("#5b7bd5"),
            critical: s("#e45756"),
            series: [
                "#4c78a8", "#f58518", "#54a24b", "#e45756", "#72b7b2", "#eeca3b", "#b279a2",
                "#9d755d",
            ]
            .map(s),
        }
    }

    /// Colors for a dark page.
    pub fn dark() -> Self {
        let s = |c: &str| c.to_string();
        Self {
            bg: s("#1e1e1e"),
            fg: s("#e6e6e6"),
            muted: s("#a0a0a0"),
            line: s("#c8c8c8"),
            surface: s("#2d2d2d"),
            surface_alt: s("#3a3a3a"),
            border: s("#6e6e6e"),
            accent: s("#7f9be8"),
            critical: s("#ff6b6b"),
            series: [
                "#6a9bd1", "#ff9f40", "#6fbf66", "#ff7070", "#8fd0cb", "#f2d65c", "#c796bd",
                "#b9917a",
            ]
            .map(s),
        }
    }

    /// The color of a `--pele-*` token by its suffix, if the palette has one.
    pub(crate) fn token(&self, name: &str) -> Option<&str> {
        Some(match name {
            "bg" => &self.bg,
            "fg" => &self.fg,
            "muted" => &self.muted,
            "line" => &self.line,
            "surface" => &self.surface,
            "surface-alt" => &self.surface_alt,
            "border" => &self.border,
            "accent" => &self.accent,
            "critical" => &self.critical,
            _ => {
                let n: usize = name.strip_prefix("series-")?.parse().ok()?;
                return self.series.get(n.checked_sub(1)?).map(String::as_str);
            }
        })
    }
}

/// Label categories the host can choose to draw in a monospace font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonospacePart {
    Members,
    Attributes,
    Files,
    Messages,
}

/// How a diagram is rendered. Every field has a default, so set only what you need:
///
/// ```
/// let options = pele_rs::RenderOptions {
///     font_size: Some(14.0),
///     ..Default::default()
/// };
/// ```
pub struct RenderOptions {
    /// The fonts labels are drawn in where the page sets no `--pele-font`. Text is measured
    /// with `measurer` regardless.
    pub font_family: Option<String>,
    /// Like `font_family`, for monospaced text and `--pele-font-mono`.
    pub font_family_mono: Option<String>,
    pub monospace: Vec<MonospacePart>,
    /// Font size in pixels. Each diagram type has its own default, usually 16.
    pub font_size: Option<f64>,
    /// Prefix of the ids in the SVG, so that several diagrams can share a page.
    pub id_prefix: Option<String>,
    /// The width the host has for the diagram, in pixels. A type that can draw itself narrower
    /// does, so that its text is not shrunk with it.
    pub max_width: Option<f64>,
    /// Whether a diagram that runs across may be drawn running down when it does not fit
    /// `max_width`, which is done under `direction_breakpoint` pixels (640 when `None`).
    pub auto_direction: bool,
    pub direction_breakpoint: Option<f64>,
    /// Space around the drawing in pixels. Each diagram type has its own default.
    pub padding: Option<f64>,
    /// Whether the SVG scales down to fit its container.
    pub responsive: Option<bool>,
    pub limits: Limits,
    /// Schemes a link may use besides a relative address. `None` allows `http`, `https`,
    /// `mailto` and `tel`; an empty list allows only relative links.
    pub link_schemes: Option<Vec<String>>,
    /// Schemes an image may use besides a relative address. `None` allows `http` and `https`.
    pub image_schemes: Option<Vec<String>>,
    /// The `rel` attribute of links. `None` writes `noopener`; an empty string writes none.
    pub link_rel: Option<String>,
    /// Measures text; [`MetricsMeasurer`](crate::MetricsMeasurer) when `None`.
    pub measurer: Option<Box<dyn TextMeasurer>>,
    /// Gives the icons of the host. An icon reference in a label, such as `fa:fa-car`, is drawn
    /// when the resolver has an icon of that name, and left out when it has not (or when there
    /// is no resolver). `None` has no icons.
    pub icons: Option<IconResolver>,
    /// Typesets the TeX between `$$` in labels, in the diagram types that read it. Without one,
    /// the TeX stays in the text as it was written.
    pub math: Option<MathResolver>,
    /// Configuration in the form of `%%{init}%%`, as a JSON object such as
    /// `{"theme": "dark", "flowchart": {"nodeSpacing": 30}}`. What the diagram sets itself
    /// wins, and no key of it can raise a limit. A text that is not a JSON object is an error.
    pub config: Option<String>,
    /// Whether links are drawn. When `false`, every link is left out and `Rendered::links` is
    /// empty, whatever `link_schemes` says.
    pub links: bool,
    /// Whether images are drawn. When `false`, every image is left out.
    pub images: bool,
    /// The current time, in milliseconds since 1970-01-01 UTC, for what a gantt chart dates
    /// from today. `None` is 1970-01-01, so that output never depends on the clock.
    pub now: Option<f64>,
    /// Writes every color out as a plain value instead of a CSS variable, for viewers that
    /// ignore `var()` (resvg, librsvg). `None`, the default, keeps the variables, so the page
    /// can theme the SVG. With a palette, fonts also gain fallbacks for Chinese, Japanese and
    /// Korean text, as such viewers would otherwise draw it in a serif font.
    pub palette: Option<Palette>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            font_family: None,
            font_family_mono: None,
            monospace: Vec::new(),
            font_size: None,
            id_prefix: None,
            max_width: None,
            auto_direction: true,
            direction_breakpoint: None,
            padding: None,
            responsive: None,
            limits: Limits::default(),
            link_schemes: None,
            image_schemes: None,
            link_rel: None,
            measurer: None,
            icons: None,
            math: None,
            config: None,
            links: true,
            images: true,
            now: None,
            palette: None,
        }
    }
}

/// A link a diagram's `click` statements or link syntax point to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The id of the element the link is on, as written in the diagram.
    pub id: String,
    pub href: String,
    /// Whether the link points into the page rather than away from it.
    pub internal: bool,
}

/// A rendered diagram.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub svg: String,
    pub width: f64,
    pub height: f64,
    pub links: Vec<Link>,
    pub kind: DiagramKind,
}
