//! The CSS custom properties a diagram is themed with.

/// Private aliases set once on the root element, so each use is short and the public
/// `--pele-rs-*` tokens keep their fallbacks. The text color is set as well: an icon is drawn
/// in currentColor, which would otherwise be whatever color the page gives the container.
pub(crate) const ROOT_STYLE: &str = concat!(
    "--_bg:var(--pele-rs-bg,#fff);--_fg:var(--pele-rs-fg,#1f1f1f);--_m:var(--pele-rs-muted,#6e6e6e);",
    "--_l:var(--pele-rs-line,#1f1f1f);--_s:var(--pele-rs-surface,#f3f3f3);--_a:var(--pele-rs-surface-alt,#e6e6e6);",
    "--_b:var(--pele-rs-border,#8a8a8a);--_c:var(--pele-rs-accent,#5b7bd5);color:var(--_fg)",
);

const SERIES: [&str; 8] = [
    "#4c78a8", "#f58518", "#54a24b", "#e45756", "#72b7b2", "#eeca3b", "#b279a2", "#9d755d",
];

/// The color of the nth data series in a chart, cycling through eight tokens.
pub(crate) fn series_color(index: usize) -> String {
    let k = index % 8;
    format!("var(--pele-rs-series-{},{})", k + 1, SERIES[k])
}

/// Corners are rounded through CSS, which older browsers support for `rx`. The default goes
/// first so that a diagram's own style can override it.
pub(crate) const ROUNDED: &str = " style=\"rx:var(--_r)\"";

pub(crate) fn rounded(attrs: &str) -> String {
    match attrs.find(" style=\"") {
        None => format!("{attrs}{ROUNDED}"),
        Some(at) => format!("{}rx:var(--_r);{}", &attrs[..at + 8], &attrs[at + 8..]),
    }
}

/// The color of what a diagram marks as critical.
pub(crate) const CRITICAL: &str = "var(--pele-rs-critical,#e45756)";

const SHAPE_PROPS: &[&str] = &[
    "fill",
    "fill-opacity",
    "stroke",
    "stroke-width",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-opacity",
    "opacity",
    "rx",
    "ry",
];

const TEXT_PROPS: &[&str] = &[
    "color",
    "font-size",
    "font-family",
    "font-weight",
    "font-style",
    "text-decoration",
    "letter-spacing",
    "word-spacing",
    "opacity",
];

/// `url\s*\(|expression\s*\(|javascript:|[<>{};\\]|@import|image-set|\/\*`, ignoring case.
fn unsafe_value(value: &str) -> bool {
    let v = value.to_ascii_lowercase();
    let call = |name: &str| {
        v.match_indices(name)
            .any(|(i, _)| crate::util::trim_start(&v[i + name.len()..]).starts_with('('))
    };
    call("url")
        || call("expression")
        || v.contains("javascript:")
        || v.contains(['<', '>', '{', '}', ';', '\\'])
        || v.contains("@import")
        || v.contains("image-set")
        || v.contains("/*")
}

/// Mermaid style declarations split into what applies to a shape, to its outline, and to its
/// text, each written as a `style` attribute.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ResolvedStyle {
    pub shape: String,
    pub line: String,
    pub text: String,
    pub stroke: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub font_size: Option<f64>,
}

fn set(map: &mut Vec<(String, String)>, prop: &str, value: &str) {
    match map.iter_mut().find(|(p, _)| p == prop) {
        Some(entry) => entry.1 = value.to_string(),
        None => map.push((prop.to_string(), value.to_string())),
    }
}

fn get<'a>(map: &'a [(String, String)], prop: &str) -> Option<&'a str> {
    map.iter().find(|(p, _)| p == prop).map(|(_, v)| v.as_str())
}

fn remove_important(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let mut out = String::with_capacity(value.len());
    let mut i = 0;
    while let Some(at) = lower[i..].find("!important") {
        out.push_str(&value[i..i + at]);
        i += at + "!important".len();
    }
    out.push_str(&value[i..]);
    out
}

pub(crate) fn resolve_style<S: AsRef<str>>(declarations: &[S]) -> ResolvedStyle {
    let mut shape = Vec::new();
    let mut text = Vec::new();
    for decl in declarations {
        let decl = decl.as_ref();
        let Some(at) = decl.find(':').filter(|&at| at > 0) else {
            continue;
        };
        let prop = crate::util::trim(&decl[..at]).to_lowercase();
        let value = remove_important(&decl[at + 1..]);
        let value = crate::util::trim(&value);
        if value.is_empty() || unsafe_value(value) {
            continue;
        }
        if SHAPE_PROPS.contains(&prop.as_str()) {
            set(&mut shape, &prop, value);
        }
        if TEXT_PROPS.contains(&prop.as_str()) {
            set(
                &mut text,
                if prop == "color" { "fill" } else { &prop },
                value,
            );
        }
    }
    if shape.is_empty() && text.is_empty() {
        return ResolvedStyle::default();
    }
    let mut shape_css = String::new();
    let mut line_css = String::new();
    for (prop, value) in &shape {
        shape_css += &format!("{prop}:{value};");
        if prop != "fill" && prop != "fill-opacity" {
            line_css += &format!("{prop}:{value};");
        }
    }
    let mut text_css = String::new();
    for (prop, value) in &text {
        text_css += &format!("{prop}:{value};");
    }
    let weight = get(&text, "font-weight");
    let size = get(&text, "font-size");
    let px = size
        .filter(|s| {
            let n = s.strip_suffix("px").unwrap_or(s);
            !n.is_empty() && n.chars().all(|c| c.is_ascii_digit() || c == '.')
        })
        .and_then(parse_float);
    let attr = |css: &str| {
        if css.is_empty() {
            String::new()
        } else {
            format!(" style=\"{}\"", super::esc(css))
        }
    };
    ResolvedStyle {
        shape: attr(&shape_css),
        line: attr(&line_css),
        text: attr(&text_css),
        stroke: get(&shape, "stroke").map(str::to_string),
        bold: weight.is_some_and(|w| {
            w == "bold" || w == "bolder" || parse_float(w).is_some_and(|n| n >= 600.0)
        }),
        italic: get(&text, "font-style") == Some("italic"),
        font_size: px.filter(|&p| p > 0.0 && p <= 200.0),
    }
}

/// JavaScript's `parseFloat` for the leading decimal number of `s`.
pub(crate) fn parse_float(s: &str) -> Option<f64> {
    let s = crate::util::trim_start(s);
    let mut end = 0;
    let mut dot = false;
    let mut digits = false;
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() {
            digits = true;
            end = i + 1;
        } else if c == '.' && !dot {
            dot = true;
            if digits {
                end = i + 1;
            }
        } else if (c == '-' || c == '+') && i == 0 {
        } else {
            break;
        }
    }
    if !digits {
        return None;
    }
    s[..end].trim_end_matches('.').parse().ok()
}

/// Class names from a diagram, as a string to append after other classes.
pub(crate) fn class_names(raw: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in raw.chars() {
        if crate::util::is_word(c) || c == '-' {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    if out.is_empty() {
        out
    } else {
        format!(" {out}")
    }
}

pub(crate) const FONT_MONO: &str = "var(--_fm)";
pub(crate) const RADIUS: &str = "var(--pele-rs-radius,4px)";

fn is_family_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// Matches one font family name at the start of `s`: quoted, or a run of letters, digits,
/// `_`, `-`, `.` and spaces. Returns the rest of `s`.
fn family(s: &str) -> Option<&str> {
    let first = s.chars().next()?;
    if first == '"' || first == '\'' {
        let body = &s[1..];
        let end = body.find(|c: char| "\"'\\\n<>{}();:@".contains(c))?;
        return body[end..].strip_prefix(first);
    }
    if !is_family_char(first) {
        return None;
    }
    Some(s.trim_start_matches(|c: char| is_family_char(c) || c == ' ' || c == '.'))
}

/// Whether `list` is a list of font names, quoted or not, with nothing that could end the
/// declaration it is put in.
fn is_family_list(list: &str) -> bool {
    let mut rest = crate::util::trim_start(list);
    loop {
        let Some(after) = family(rest) else {
            return false;
        };
        rest = crate::util::trim_start(after);
        match rest.strip_prefix(',') {
            Some(next) => rest = crate::util::trim_start(next),
            None => return rest.is_empty(),
        }
    }
}

pub(crate) fn families<'a>(list: Option<&'a str>, generic: &'a str) -> &'a str {
    list.filter(|l| is_family_list(l)).unwrap_or(generic)
}

/// Sans-serif CJK fonts, named for the viewers that do not pick one for CJK text themselves
/// (resvg draws it in a serif font). They come after the generic family, so a viewer tries
/// them for the characters the first font lacks. Traditional Chinese goes first, then
/// Simplified Chinese, Japanese and Korean.
const CJK_SANS: &str = "'Noto Sans CJK TC','Noto Sans TC','PingFang TC','Microsoft JhengHei',\
    'Noto Sans CJK SC','PingFang SC','Microsoft YaHei','Noto Sans CJK JP','Hiragino Sans',\
    'Yu Gothic','Meiryo','Noto Sans CJK KR','Malgun Gothic','WenQuanYi Micro Hei'";

/// `list` followed by the CJK fonts, when `compat` is set.
fn with_cjk(list: &str, compat: bool) -> String {
    if compat {
        format!("{list},{CJK_SANS}")
    } else {
        list.to_string()
    }
}

/// The fonts the labels were measured in are the ones they are drawn in, unless the page names
/// others. With `compat`, the style also suits viewers that read neither CSS variables nor the
/// `font` shorthand: it gains the font size on its own and fonts for CJK text. The rest of what
/// text inherits is pinned, as none of it was measured.
pub(crate) fn font_style(
    size: f64,
    family: Option<&str>,
    mono: Option<&str>,
    compat: bool,
) -> String {
    let size = super::num(size);
    // resvg does not read the `font` shorthand and would draw 12 pixel text.
    let longhand = if compat {
        format!("font-size:{size}px;")
    } else {
        String::new()
    };
    // resvg drops the whole `style` attribute from the first custom property on, so the
    // private ones (`--_fm` here) are left out and `var(--_fm)` is resolved from the options.
    let mono = if compat {
        String::new()
    } else {
        format!(
            "--_fm:var(--pele-rs-font-mono,{});",
            families(mono, "monospace")
        )
    };
    format!(
        ";{mono}font:{size}px sans-serif;{longhand}\
         letter-spacing:normal;word-spacing:normal;text-transform:none;direction:ltr;\
         font-family:var(--pele-rs-font,{})",
        with_cjk(families(family, "sans-serif"), compat),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_font_lists() {
        assert!(is_family_list("Inter"));
        assert!(is_family_list(
            " \"Noto Sans TC\", 'PingFang TC' , sans-serif "
        ));
        assert!(is_family_list("Font 1.5, x"));
        assert!(is_family_list("\"a, b\""));
    }

    #[test]
    fn rejects_unsafe_font_lists() {
        assert!(!is_family_list(""));
        assert!(!is_family_list("Inter;color:red"));
        assert!(!is_family_list("x, "));
        assert!(!is_family_list("\"x\\\""));
        assert!(!is_family_list("url(x)"));
        assert!(!is_family_list("'a\"b'"));
    }

    #[test]
    fn resolves_styles() {
        let style = resolve_style(&[
            "fill:#f9f",
            "stroke:#333 !important",
            "color:red",
            "font-weight:700",
            "x:url(y)",
        ]);
        assert_eq!(style.shape, " style=\"fill:#f9f;stroke:#333;\"");
        assert_eq!(style.line, " style=\"stroke:#333;\"");
        assert_eq!(style.text, " style=\"fill:red;font-weight:700;\"");
        assert!(style.bold);
        assert_eq!(resolve_style(&["fill:url(#x)"]), ResolvedStyle::default());
        assert_eq!(resolve_style(&["font-size:14px"]).font_size, Some(14.0));
        assert_eq!(class_names("a b$c "), " a b c");
        assert_eq!(class_names("  "), "");
        assert_eq!(rounded(" fill=\"x\""), " fill=\"x\" style=\"rx:var(--_r)\"");
        assert_eq!(rounded(" style=\"a:b\""), " style=\"rx:var(--_r);a:b\"");
    }

    #[test]
    fn adds_cjk_fonts_after_the_family() {
        let style = font_style(16.0, Some("Inter"), None, true);
        assert!(style.contains("font-family:var(--pele-rs-font,Inter,'Noto Sans CJK TC',"));
        assert!(font_style(16.0, None, None, true).contains("sans-serif,'Noto Sans CJK TC'"));
        assert!(!font_style(16.0, None, None, false).contains("CJK"));
    }

    #[test]
    fn writes_font_style() {
        assert_eq!(
            font_style(16.0, Some("x;y"), None, false),
            ";--_fm:var(--pele-rs-font-mono,monospace);font:16px sans-serif;letter-spacing:normal;\
             word-spacing:normal;text-transform:none;direction:ltr;font-family:var(--pele-rs-font,sans-serif)"
        );
    }
}
