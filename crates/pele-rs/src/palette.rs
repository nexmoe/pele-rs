//! Author palettes from Pele's svg/palette.ts. Scoped per render and per thread.
use crate::svg::ink::{ink, plain_color};
use crate::{
    DiagramKind,
    preprocess::{Config, Value},
};

#[derive(Default)]
struct Palette {
    responsive: bool,
    style: String,
    inks: [Option<&'static str>; 8],
    colors: [Option<String>; 8],
}
thread_local! { static CURRENT: std::cell::RefCell<Vec<Palette>> = const { std::cell::RefCell::new(Vec::new()) }; }
pub(crate) struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        CURRENT.with(|v| {
            v.borrow_mut().pop();
        });
    }
}

pub(crate) fn responsive() -> bool {
    CURRENT.with(|v| v.borrow().last().is_none_or(|p| p.responsive))
}
pub(crate) fn style() -> String {
    CURRENT.with(|v| {
        v.borrow()
            .last()
            .map_or_else(String::new, |p| p.style.clone())
    })
}
#[cfg(any(
    feature = "pie",
    feature = "git",
    feature = "timeline",
    feature = "treemap"
))]
pub(crate) fn series_ink_fill(index: usize) -> String {
    CURRENT.with(|v| {
        v.borrow()
            .last()
            .and_then(|p| p.inks[index % 8])
            .map_or_else(String::new, |c| format!(" fill=\"{c}\""))
    })
}
pub(crate) fn colors() -> [Option<String>; 8] {
    CURRENT.with(|v| {
        v.borrow()
            .last()
            .map_or_else(Default::default, |p| p.colors.clone())
    })
}

fn safe_color(value: &str) -> Option<&str> {
    let color = value.trim();
    if color.is_empty()
        || color.len() > 200
        || !color
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c.is_whitespace() || "_#%.,+-/()".contains(c))
    {
        return None;
    }
    if !color.contains('(') {
        return plain_color(color).then_some(color);
    }
    let mut depth = 0i32;
    let mut word_start = 0;
    for (at, c) in color.char_indices() {
        if c == '(' {
            depth += 1;
            let name = color[word_start..at].trim().to_ascii_lowercase();
            if !matches!(
                name.as_str(),
                "rgb"
                    | "rgba"
                    | "hsl"
                    | "hsla"
                    | "hwb"
                    | "lab"
                    | "lch"
                    | "oklab"
                    | "oklch"
                    | "color"
                    | "color-mix"
                    | "light-dark"
                    | "var"
            ) {
                return None;
            }
            if name == "var" && !color[at + 1..].trim_start().starts_with("--pele-") {
                return None;
            }
            word_start = at + 1;
        } else if c == ')' {
            depth -= 1;
            if depth < 0 {
                return None;
            }
            word_start = at + 1;
        } else if !c.is_ascii_alphanumeric() && c != '_' && c != '-' && !c.is_whitespace() {
            word_start = at + c.len_utf8();
        } else if c.is_whitespace() {
            // A following word starts a new function name, while `rgb (` keeps rgb.
            if color[at..]
                .trim_start()
                .chars()
                .next()
                .is_some_and(|n| n.is_ascii_alphanumeric() || n == '-')
            {
                word_start = at + c.len_utf8();
            }
        }
    }
    (depth == 0).then_some(color)
}
fn split_list(value: &str) -> Vec<&str> {
    let mut depth = 0i32;
    let mut start = 0;
    let mut parts = Vec::new();
    for (at, c) in value.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(value[start..at].trim());
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(value[start..].trim());
    parts
}
pub(crate) fn enter(kind: DiagramKind, config: &Config) -> Guard {
    let section = match kind {
        DiagramKind::XyChart => "xyChart",
        DiagramKind::Wardley => "wardley-beta",
        _ => kind.as_str(),
    };
    let fixed = matches!(config.get(section), Some(Value::Map(section)) if section.get("useMaxWidth") == Some(&Value::Bool(false)));
    let mut palette = Palette {
        responsive: !fixed,
        ..Default::default()
    };
    if let Some(Value::Map(vars)) = config.get("themeVariables") {
        let (list, repeat): (Vec<Option<&str>>, bool) = if kind == DiagramKind::XyChart {
            let list = match vars.get("xyChart") {
                Some(Value::Map(v)) => match v.get("plotColorPalette") {
                    Some(Value::String(v)) => split_list(v).into_iter().map(Some).collect(),
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };
            (list, true)
        } else {
            let mapping = match kind {
                DiagramKind::Pie => Some(("pie", 1)),
                DiagramKind::GitGraph => Some(("git", 0)),
                DiagramKind::Mindmap => Some(("cScale", 1)),
                DiagramKind::Timeline | DiagramKind::Radar | DiagramKind::Treemap => {
                    Some(("cScale", 0))
                }
                DiagramKind::Venn => Some(("venn", 1)),
                _ => None,
            };
            let repeat = kind == DiagramKind::Venn;
            let list = mapping.map_or_else(Vec::new, |(prefix, first)| {
                (0..8)
                    .filter_map(|i| {
                        let value = vars.get(&format!("{prefix}{}", first + i));
                        if repeat && matches!(value, None | Some(Value::Null)) {
                            return None;
                        }
                        if repeat && matches!(value, Some(Value::String(v)) if v.is_empty()) {
                            return None;
                        }
                        Some(match value {
                            Some(Value::String(v)) => Some(v.as_str()),
                            _ => None,
                        })
                    })
                    .collect()
            });
            (list, repeat)
        };
        if !list.is_empty() {
            for i in 0..8 {
                if !repeat && i >= list.len() {
                    break;
                }
                if let Some(color) = list[i % list.len()].and_then(safe_color) {
                    palette.style += &format!(";--pele-series-{}:{color}", i + 1);
                    palette.inks[i] = ink(&format!(";fill:{color}"), "");
                    palette.colors[i] = Some(color.to_string());
                }
            }
        }
    }
    palette.style = crate::svg::esc(&palette.style);
    CURRENT.with(|v| v.borrow_mut().push(palette));
    Guard
}
