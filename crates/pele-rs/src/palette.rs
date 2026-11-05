//! Mermaid's chart palettes are author colors, applied before the host's theme.

use crate::{
    DiagramKind,
    preprocess::{Config, Value},
};

pub(crate) fn apply(svg: &mut String, kind: DiagramKind, config: &Config) {
    let Some(Value::Map(vars)) = config.get("themeVariables") else {
        return;
    };
    let (prefix, first) = match kind {
        DiagramKind::Pie => ("pie", 1),
        DiagramKind::GitGraph => ("git", 0),
        DiagramKind::Mindmap => ("cScale", 1),
        DiagramKind::Timeline | DiagramKind::Radar | DiagramKind::Treemap => ("cScale", 0),
        DiagramKind::Venn => ("venn", 1),
        _ => return,
    };
    // Only substitute inside markup, never text nodes that happen to contain token strings.
    let mut out = String::with_capacity(svg.len());
    for part in svg.split_inclusive('>') {
        let Some(start) = part.find('<') else {
            out.push_str(part);
            continue;
        };
        out.push_str(&part[..start]);
        let mut tag = part[start..].to_string();
        for i in 0..8 {
            let Some(Value::String(color)) = vars.get(&format!("{prefix}{}", first + i)) else {
                continue;
            };
            let color = color.trim();
            if !safe_color(color) {
                continue;
            }
            let token = format!("var(--pele-rs-series-{},", i + 1);
            while let Some(at) = tag.find(&token) {
                let Some(end) = tag[at..].find(')') else {
                    break;
                };
                tag.replace_range(at..at + end + 1, color);
            }
        }
        out.push_str(&tag);
    }
    *svg = out;
}

fn safe_color(color: &str) -> bool {
    !color.is_empty()
        && color
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " #%.,()-".contains(c))
        && !color.to_ascii_lowercase().contains("url")
        && !color.to_ascii_lowercase().contains("expression")
        && !color.to_ascii_lowercase().contains("var(")
}
