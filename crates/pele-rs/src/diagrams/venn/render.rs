//! The drawing of a venn diagram: the circles of the sets, the overlaps that have a look of
//! their own, and the text of every region. The layout is pele's own, ported from its
//! `diagrams/venn/render.ts`.

use std::collections::HashMap;

use super::db::{VennModel, VennSubset};
use super::layout::{Circle, Spot, find_spot, layout_circles, region_path};
use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::preprocess::{Config, Value};
use crate::svg::{
    Accessible, ResolvedStyle, esc_text, label_svg, num, resolve_style, series_color, svg_document,
};
use crate::text::Style;
use crate::text::label::{Label, layout_label};

/// The radius the largest circle of the diagram gets.
const RADIUS: f64 = 100.0;
/// How much the diagram may grow to hold what is written in it.
const MAX_GROWTH: f64 = 1.6;

/// A number from the configuration of the diagram, as Mermaid's `venn` section holds it.
fn setting(config: &Config, key: &str, min: f64) -> Option<f64> {
    let venn = match config.get("venn") {
        Some(Value::Map(venn)) => venn,
        _ => return None,
    };
    match venn.get(key) {
        Some(Value::Number(value)) if value.is_finite() && *value >= min => Some(*value),
        _ => None,
    }
}

/// A line of text in a region.
struct Line {
    /// The id of a text node, or nothing for the label of the region itself.
    id: Option<String>,
    raw: String,
    size: f64,
    base: Style,
    label: Label,
}

/// A region of the diagram: a set, or an overlap of sets.
struct Region {
    sets: Vec<String>,
    /// The circles the region is inside of.
    inside: Vec<usize>,
    spot: Spot,
    /// What is written in the region: its label first, then its texts.
    lines: Vec<Line>,
    style: ResolvedStyle,
}

/// One key for a list of set ids. Ids may hold any character, so they are not simply joined.
fn key_of(ids: &[String]) -> String {
    if ids.len() == 1 {
        return ids[0].clone();
    }
    let mut out = String::from("[");
    for (i, id) in ids.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&super::db::json_string(id));
    }
    out.push(']');
    out
}

/// The ids of a set, without the ones that are named twice.
fn unique(ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(ids.len());
    for id in ids {
        if !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}

/// Draws a venn diagram.
pub(crate) fn render_venn(model: &VennModel, input: &Input<'_>) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let small = (size * 0.875).round();
    let tiny = (size * 0.8125).round();
    // The padding of the diagram, and the canvas it is drawn in when the configuration names one.
    let pad = setting(input.config, "padding", 0.0)
        .or(options.padding)
        .unwrap_or(8.0);
    let canvas_width = setting(input.config, "width", 1.0);
    let canvas_height = setting(input.config, "height", 1.0);

    let circles = layout_circles(&model.subsets);
    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, circle) in circles.iter().enumerate() {
        index.insert(circle.id.as_str(), i);
    }

    // Styles for the same targets add up, later declarations winning.
    let mut styles: Vec<(String, Vec<(String, String)>)> = Vec::new();
    let mut style_index: HashMap<String, usize> = HashMap::new();
    for entry in &model.styles {
        let key = key_of(&entry.targets);
        let at = match style_index.get(&key).copied() {
            Some(at) => at,
            None => {
                let at = styles.len();
                styles.push((key.clone(), Vec::new()));
                style_index.insert(key, at);
                at
            }
        };
        for (property, value) in &entry.styles {
            match styles[at].1.iter_mut().find(|(p, _)| p == property) {
                Some(slot) => slot.1 = value.clone(),
                None => styles[at].1.push((property.clone(), value.clone())),
            }
        }
    }
    let style_of = |key: &str| -> ResolvedStyle {
        let Some((_, declared)) = styles.iter().find(|(k, _)| k == key) else {
            return ResolvedStyle::default();
        };
        let list: Vec<String> = declared
            .iter()
            .map(|(property, value)| format!("{property}:{value}"))
            .collect();
        resolve_style(&list)
    };

    // Circles that touch each circle; only those can cut into a region of it.
    let mut neighbours: Vec<Vec<usize>> = vec![Vec::new(); circles.len()];
    for a in 0..circles.len() {
        for b in (a + 1)..circles.len() {
            let apart = (circles[a].x - circles[b].x).hypot(circles[a].y - circles[b].y);
            if apart < circles[a].r + circles[b].r {
                neighbours[a].push(b);
                neighbours[b].push(a);
            }
        }
    }

    // One region for each set and each stated overlap; a later statement about the same sets
    // replaces an earlier one.
    let mut stated: Vec<(String, &VennSubset)> = Vec::new();
    let mut stated_index: HashMap<String, usize> = HashMap::new();
    for subset in &model.subsets {
        let key = key_of(&subset.sets);
        match stated_index.get(&key).copied() {
            Some(at) => stated[at].1 = subset,
            None => {
                stated_index.insert(key.clone(), stated.len());
                stated.push((key, subset));
            }
        }
    }

    let mut regions: Vec<Region> = Vec::new();
    let mut region_index: HashMap<String, usize> = HashMap::new();
    for (key, subset) in stated {
        let sets = unique(&subset.sets);
        let inside: Vec<usize> = sets
            .iter()
            .filter_map(|id| index.get(id.as_str()).copied())
            .collect();
        let Some((&first, rest)) = inside.split_first() else {
            continue;
        };
        let mut smallest = first;
        for &i in rest {
            if circles[i].r < circles[smallest].r {
                smallest = i;
            }
        }
        let style = style_of(&key);
        let text = match &subset.label {
            // An empty label writes nothing, and does not fall back to the name of the set.
            Some(label) if !label.is_empty() => Some(label.clone()),
            Some(_) => None,
            None => {
                if subset.sets.len() == 1 {
                    Some(subset.sets[0].clone())
                } else {
                    None
                }
            }
        };
        let mut base = Style::NONE;
        if style.bold {
            base |= Style::BOLD;
        }
        if style.italic {
            base |= Style::ITALIC;
        }
        let outside: Vec<usize> = neighbours[smallest]
            .iter()
            .copied()
            .filter(|i| !inside.contains(i))
            .collect();
        let mut spot = find_spot(&circles, &inside, &outside);
        // A region that other circles cover, or all but cover, is written across them instead.
        if spot.margin < circles[smallest].r * 0.2 {
            spot = find_spot(&circles, &inside, &[]);
        }
        let mut lines: Vec<Line> = Vec::new();
        if let Some(text) = text {
            let font_size = if sets.len() == 1 { size } else { small };
            lines.push(Line {
                id: None,
                raw: text.clone(),
                size: font_size,
                base,
                label: layout_label(Some(&text), false, measurer, font_size, 4000.0, base),
            });
        }
        let at = regions.len();
        region_index.insert(key, at);
        regions.push(Region {
            sets,
            inside,
            spot,
            lines,
            style,
        });
    }
    for node in &model.text_nodes {
        let key = key_of(&node.sets);
        let Some(&at) = region_index.get(&key) else {
            continue;
        };
        let raw = node.label.clone().unwrap_or_else(|| node.id.clone());
        regions[at].lines.push(Line {
            id: Some(node.id.clone()),
            raw: raw.clone(),
            size: tiny,
            base: Style::NONE,
            label: layout_label(Some(&raw), false, measurer, tiny, 4000.0, Style::NONE),
        });
    }

    // Lengths so far are in units of the layout. The scale gives the largest circle a set radius,
    // and grows, within limits, until every region holds what is written in it.
    let mut largest = 0.0f64;
    for circle in &circles {
        if circle.r > largest {
            largest = circle.r;
        }
    }
    let mut scale: f64 = if largest > 0.0 { RADIUS / largest } else { 1.0 };
    let mut growth = 1.0f64;
    for region in &regions {
        if region.spot.margin <= 0.0 {
            continue;
        }
        let mut width = 0.0f64;
        let mut height = 0.0f64;
        for line in &region.lines {
            width = width.max(line.label.width);
            height += line.label.height;
        }
        let need = width.hypot(height) / 2.0 + 4.0;
        growth = growth.max((need / (region.spot.margin * scale)).min(MAX_GROWTH));
    }
    scale *= growth;

    // What is still too wide for its region is wrapped to it.
    for region in &mut regions {
        let room = (region.spot.margin * scale * 1.8).max(72.0);
        for line in &mut region.lines {
            if line.label.width > room {
                line.label =
                    layout_label(Some(&line.raw), false, measurer, line.size, room, line.base);
            }
        }
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for circle in &circles {
        min_x = min_x.min((circle.x - circle.r) * scale);
        min_y = min_y.min((circle.y - circle.r) * scale);
        max_x = max_x.max((circle.x + circle.r) * scale);
        max_y = max_y.max((circle.y + circle.r) * scale);
    }
    for region in &regions {
        let mut wide = 0.0f64;
        let mut tall = 0.0f64;
        for line in &region.lines {
            wide = wide.max(line.label.width);
            tall += line.label.height;
        }
        min_x = min_x.min(region.spot.x * scale - wide / 2.0);
        max_x = max_x.max(region.spot.x * scale + wide / 2.0);
        min_y = min_y.min(region.spot.y * scale - tall / 2.0);
        max_y = max_y.max(region.spot.y * scale + tall / 2.0);
    }
    if circles.is_empty() {
        min_x = 0.0;
        min_y = 0.0;
        max_x = 0.0;
        max_y = 0.0;
    }

    let title_text = model.title.as_deref().map(super::db::normalize_text);
    let title = layout_label(
        title_text.as_deref(),
        false,
        measurer,
        size,
        4000.0,
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let inner = (max_x - min_x).max(title.width);
    // The diagram is as big as its content, or as big as the canvas the configuration names, and
    // is centred in it.
    let content_width = max_x - min_x;
    let content_height = max_y - min_y;
    let natural_width = (inner + 2.0 * pad).ceil();
    let natural_height = (content_height + title_height + 2.0 * pad).ceil();
    let width = canvas_width
        .map_or(natural_width, |w| w.max(natural_width))
        .ceil();
    let height = canvas_height
        .map_or(natural_height, |h| h.max(natural_height))
        .ceil();
    let ox = pad + (width - 2.0 * pad - content_width) / 2.0 - min_x;
    let oy =
        pad + title_height + (height - title_height - 2.0 * pad - content_height) / 2.0 - min_y;
    let placed: Vec<Circle> = circles
        .iter()
        .map(|circle| Circle {
            id: circle.id.clone(),
            x: circle.x * scale + ox,
            y: circle.y * scale + oy,
            r: circle.r * scale,
        })
        .collect();

    let mut sets = String::new();
    for (i, circle) in placed.iter().enumerate() {
        let color = series_color(i);
        let style = regions
            .iter()
            .find(|region| region.sets.len() == 1 && region.sets[0] == circle.id)
            .map_or_else(ResolvedStyle::default, |region| region.style.clone());
        sets += &format!(
            "<circle class=\"pele-rs-venn-set\" data-id=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{}\" \
             fill=\"{color}\" fill-opacity=\"0.2\" stroke=\"{color}\"{}/>",
            esc_text(&circle.id),
            num(circle.x),
            num(circle.y),
            num(circle.r),
            style.shape
        );
    }

    let mut overlaps = String::new();
    let mut labels = String::new();
    for region in &regions {
        let id = esc_text(&region.sets.join("|"));
        // An overlap is only drawn when a style gives it a look of its own; otherwise the circles
        // show it.
        if region.sets.len() > 1 && !region.style.shape.is_empty() {
            let own: Vec<Circle> = region.inside.iter().map(|&i| placed[i].clone()).collect();
            let d = region_path(&own, num);
            if !d.is_empty() {
                overlaps += &format!(
                    "<path class=\"pele-rs-venn-overlap\" data-id=\"{id}\" d=\"{d}\" \
                     fill=\"none\"{}/>",
                    region.style.shape
                );
            }
        }
        let mut total = 0.0f64;
        for line in &region.lines {
            total += line.label.height;
        }
        if total == 0.0 {
            continue;
        }
        let x = region.spot.x * scale + ox;
        let mut y = region.spot.y * scale + oy - total / 2.0;
        let mut content = String::new();
        for line in &region.lines {
            let attrs = match &line.id {
                None => format!(
                    " class=\"pele-rs-label\"{}{}",
                    if line.size == size {
                        String::new()
                    } else {
                        format!(" font-size=\"{}\"", num(line.size))
                    },
                    region.style.text
                ),
                Some(id) => format!(
                    " class=\"pele-rs-venn-text\" data-id=\"{}\" font-size=\"{}\" \
                     fill=\"var(--_m)\"{}",
                    esc_text(id),
                    num(tiny),
                    style_of(id).text
                ),
            };
            content += &label_svg(&line.label, x, y + line.label.height / 2.0, &attrs);
            y += line.label.height;
        }
        labels += &format!("<g class=\"pele-rs-venn-region\" data-id=\"{id}\">{content}</g>");
    }

    let mut body = String::new();
    body += &label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-rs-title\" font-weight=\"var(--_tw)\"",
    );
    body += &format!("<g class=\"pele-rs-venn-sets\">{sets}</g>");
    if !overlaps.is_empty() {
        body += &format!("<g class=\"pele-rs-venn-overlaps\">{overlaps}</g>");
    }
    if !labels.is_empty() {
        body += &format!("<g class=\"pele-rs-venn-labels\">{labels}</g>");
    }
    let svg = svg_document(
        DiagramKind::Venn,
        width,
        height,
        size,
        options,
        &Accessible::default(),
        &body,
    );
    Output {
        svg,
        width,
        height,
        links: Vec::new(),
    }
}
