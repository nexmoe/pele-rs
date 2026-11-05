//! Writes the CSS variables of a finished SVG out as plain values, for viewers that ignore
//! `var()` (resvg, librsvg), so the SVG needs no stylesheet to be drawn in the right colors.

use super::esc;
use super::theme::families;
use crate::Palette;

/// Whether `color` is safe to write into an attribute: letters, digits and the punctuation of
/// CSS colors, and nothing that could end the attribute or the declaration.
fn is_safe_color(color: &str) -> bool {
    !color.trim().is_empty()
        && color
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " #%.,()-".contains(c))
}

/// The palette with every unsafe color replaced by the light one.
fn sanitized(palette: &Palette) -> Palette {
    let light = Palette::light();
    let pick = |mine: &str, base: &str| {
        if is_safe_color(mine) { mine } else { base }
            .trim()
            .to_string()
    };
    Palette {
        bg: pick(&palette.bg, &light.bg),
        fg: pick(&palette.fg, &light.fg),
        muted: pick(&palette.muted, &light.muted),
        line: pick(&palette.line, &light.line),
        surface: pick(&palette.surface, &light.surface),
        surface_alt: pick(&palette.surface_alt, &light.surface_alt),
        border: pick(&palette.border, &light.border),
        accent: pick(&palette.accent, &light.accent),
        critical: pick(&palette.critical, &light.critical),
        series: std::array::from_fn(|i| pick(&palette.series[i], &light.series[i])),
    }
}

/// What a variable stands for, or `None` when the SVG does not define it. `fallback` is the
/// text after the comma of `var(--name, fallback)`.
fn value(name: &str, fallback: Option<&str>, palette: &Palette, mono: &str) -> Option<String> {
    if let Some(alias) = name.strip_prefix("_") {
        // The private aliases of the root element.
        return Some(
            match alias {
                "bg" => &palette.bg,
                "fg" => &palette.fg,
                "m" => &palette.muted,
                "l" => &palette.line,
                "s" => &palette.surface,
                "a" => &palette.surface_alt,
                "b" => &palette.border,
                "c" => &palette.accent,
                "r" => "4px",
                "tw" | "hw" | "w" => "bold",
                "fm" => mono,
                _ => return None,
            }
            .to_string(),
        );
    }
    let token = name.strip_prefix("pele-rs-")?;
    match palette.token(token) {
        Some(color) => Some(color.to_string()),
        None => fallback.map(str::to_string),
    }
}

/// Replaces the `var()` of one tag. A variable that is not the library's own and has no
/// fallback is left as it is.
fn resolve_tag(tag: &str, palette: &Palette, mono: &str) -> String {
    let mut out = String::with_capacity(tag.len());
    let mut rest = tag;
    while let Some(at) = rest.find("var(--") {
        out.push_str(&rest[..at]);
        let body = &rest[at + 4..];
        // Find the closing parenthesis, counting the ones inside a fallback.
        let mut depth = 1;
        let mut end = None;
        let mut comma = None;
        for (i, c) in body.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i);
                        break;
                    }
                }
                ',' if depth == 1 && comma.is_none() => comma = Some(i),
                _ => {}
            }
        }
        let Some(end) = end else {
            // Unbalanced: nothing more can be read as a variable.
            out.push_str(&rest[at..]);
            return out;
        };
        let fallback = comma.map(|c| body[c + 1..end].trim());
        let resolved_fallback = fallback.map(|f| resolve_tag(f, palette, mono));
        // `body` starts at the dashes of the name.
        let name = body[..comma.unwrap_or(end)].trim().trim_start_matches("--");
        match value(name, resolved_fallback.as_deref(), palette, mono) {
            Some(v) => out.push_str(&v),
            None => match resolved_fallback {
                Some(f) => out.push_str(&f),
                None => out.push_str(&rest[at..at + 4 + end + 1]),
            },
        }
        rest = &rest[at + 4 + end + 1..];
    }
    out.push_str(rest);
    out
}

/// Turns the corner radius the library puts in a `style` attribute into an `rx` attribute,
/// which resvg draws and the `style` property it does not. A tag that has an `rx` already keeps
/// it, as the two never meet in a viewer that reads both.
fn radius_attribute(tag: &str) -> String {
    const ONLY: &str = " style=\"rx:var(--_r)\"";
    const FIRST: &str = "style=\"rx:var(--_r);";
    let has_rx = tag.contains(" rx=\"");
    if let Some(at) = tag.find(ONLY) {
        let rx = if has_rx { "" } else { " rx=\"4\"" };
        return format!("{}{rx}{}", &tag[..at], &tag[at + ONLY.len()..]);
    }
    if let Some(at) = tag.find(FIRST) {
        let rx = if has_rx { "" } else { "rx=\"4\" " };
        return format!("{}{rx}style=\"{}", &tag[..at], &tag[at + FIRST.len()..]);
    }
    tag.to_string()
}

/// Replaces every `var()` in the tags of `svg` with its value. Text between tags is never
/// touched, so a label that reads `var(--x)` stays as it was written.
pub(crate) fn resolve_vars(svg: &str, palette: &Palette, mono: Option<&str>) -> String {
    let palette = sanitized(palette);
    let mono = esc(families(mono, "monospace"));
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let tag = &rest[start..];
        // A tag ends at the first `>` outside a double-quoted attribute value.
        let mut quoted = false;
        let mut len = tag.len();
        for (i, c) in tag.char_indices() {
            match c {
                '"' => quoted = !quoted,
                '>' if !quoted => {
                    len = i + 1;
                    break;
                }
                _ => {}
            }
        }
        out.push_str(&resolve_tag(
            &radius_attribute(&tag[..len]),
            &palette,
            &mono,
        ));
        rest = &tag[len..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(svg: &str) -> String {
        resolve_vars(svg, &Palette::dark(), None)
    }

    #[test]
    fn resolves_tokens_aliases_and_fallbacks() {
        let p = Palette::dark();
        assert_eq!(
            run("<g fill=\"var(--_bg)\" stroke=\"var(--pele-rs-line,#000)\"/>"),
            format!("<g fill=\"{}\" stroke=\"{}\"/>", p.bg, p.line)
        );
        assert_eq!(
            run("<g style=\"rx:var(--_r);fill:var(--pele-rs-series-3,#123)\"/>"),
            format!("<g rx=\"4\" style=\"fill:{}\"/>", p.series[2])
        );
        // Not a color token: the fallback is the value.
        assert_eq!(
            run("<g x=\"var(--pele-rs-radius,4px)\" y=\"var(--other,red)\"/>"),
            "<g x=\"4px\" y=\"red\"/>"
        );
        assert_eq!(run("<g x=\"var(--other)\"/>"), "<g x=\"var(--other)\"/>");
    }

    #[test]
    fn writes_the_radius_as_an_attribute() {
        assert_eq!(
            run("<rect x=\"1\" style=\"rx:var(--_r)\"/>"),
            "<rect x=\"1\" rx=\"4\"/>"
        );
        assert_eq!(
            run("<rect style=\"rx:var(--_r);fill:red\"/>"),
            "<rect rx=\"4\" style=\"fill:red\"/>"
        );
        assert_eq!(
            run("<rect rx=\"3\" style=\"rx:var(--_r)\"/>"),
            "<rect rx=\"3\"/>"
        );
    }

    #[test]
    fn leaves_text_alone() {
        assert_eq!(
            run("<text fill=\"var(--_fg)\">var(--_fg) a > b</text>"),
            format!(
                "<text fill=\"{}\">var(--_fg) a > b</text>",
                Palette::dark().fg
            )
        );
    }

    #[test]
    fn rejects_unsafe_colors() {
        let mut p = Palette::light();
        p.fg = "red\" onload=\"x".to_string();
        let svg = resolve_vars("<g fill=\"var(--_fg)\"/>", &p, None);
        assert_eq!(svg, format!("<g fill=\"{}\"/>", Palette::light().fg));
    }
}
