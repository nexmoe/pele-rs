//! The sankey renderer. Ported from pele's `diagrams/sankey/render.ts`.

use std::collections::HashMap;
use std::fmt::Write;

use super::db::SankeyDb;
use super::layout::{Alignment, LayoutOptions, layout_sankey};
use crate::diagram::{Input, Output};
use crate::diagrams::common::fit::{fit_width, title_room};
use crate::error::Error;
use crate::preprocess::{Config, Value};
use crate::svg::{
    Accessible, esc, esc_text, label_svg, num, resolve_style, series_color, svg_document,
    title_label,
};
use crate::text::Style;
use crate::text::entities::decode_entities;
use crate::util::round;

/// The room between a bar and its label.
const LABEL_GAP: f64 = 6.0;
/// The room a value needs next to a label.
const VALUE_GAP: f64 = 15.0;
/// How many lines of label a node keeps, when the column is crowded.
const LABEL_ROOM: f64 = 1.8;
/// Above this many nodes the labels are not moved out of each other's way.
const CAREFUL: usize = 300;

/// A number from the configuration, as pele reads it: a value under the minimum falls back to
/// the default, and a value over the maximum is cut down to it.
fn number_option(conf: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    match conf.get(key) {
        Some(Value::Number(n)) if *n >= min => n.min(max),
        _ => fallback,
    }
}

/// A text from the configuration; a number is read as its text.
fn text_option(conf: &Config, key: &str) -> String {
    match conf.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) if n.is_finite() => crate::util::js_number(*n),
        _ => String::new(),
    }
}

/// A colour from the configuration, as an inline style that overrides the series colour.
fn fill_style(color: &str) -> String {
    resolve_style(&[format!("fill:{color}")]).shape
}

/// Two 32-bit hashes, so different gradients practically never share an id.
fn content_id(text: &str) -> String {
    let mut a: u32 = 0x811c9dc5;
    let mut b: u32 = 5381;
    for unit in text.encode_utf16() {
        let c = u32::from(unit);
        a = (a ^ c).wrapping_mul(0x01000193);
        b = b.wrapping_mul(33).wrapping_add(c);
    }
    fn base36(mut n: u32) -> String {
        let mut out = Vec::new();
        loop {
            out.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[(n % 36) as usize]);
            n /= 36;
            if n == 0 {
                break;
            }
        }
        out.reverse();
        String::from_utf8(out).unwrap()
    }
    base36(a) + &base36(b)
}

/// The value as Mermaid shows it: two decimals, and numbers too large for that left as they
/// are.
fn rounded(value: f64) -> f64 {
    if value.abs() < 1e15 {
        round(value * 100.0) / 100.0
    } else {
        value
    }
}

/// A label beside a bar.
struct Label {
    /// The value text, empty when the values are not shown.
    value: String,
    width: f64,
    /// Whether the label sits after the bar.
    after: bool,
    left: f64,
    y: f64,
}

/// Whether a label at that place would run into one of the labels nearby.
fn overlaps_label(
    bands: &HashMap<i64, Vec<usize>>,
    labels: &[Label],
    index: usize,
    left: f64,
    y: f64,
    line_height: f64,
) -> bool {
    let band = (y / line_height).floor() as i64;
    let Some(nearby) = bands.get(&band) else {
        return false;
    };
    nearby.iter().any(|&other| {
        other != index
            && (labels[other].y - y).abs() < line_height
            && left < labels[other].left + labels[other].width + LABEL_GAP
            && left + labels[index].width > labels[other].left - LABEL_GAP
    })
}

pub(crate) fn render_sankey(db: &SankeyDb, input: &Input<'_>) -> Result<Output, Error> {
    let empty = Config::new();
    let conf = match input.config.get("sankey") {
        Some(Value::Map(m)) => m,
        _ => &empty,
    };
    let natural = number_option(conf, "width", 600.0, 1.0, 10000.0);
    // The layout can fail, and the fitting loop draws the chart more than once, so the
    // failure is kept aside and reported once the loop is done.
    let mut failure: Option<Error> = None;
    let out = fit_width(input.options, natural, 160.0, |chart_width, _| {
        match draw(db, conf, input, chart_width, natural) {
            Ok(out) => out,
            Err(e) => {
                failure = Some(e);
                Output {
                    svg: String::new(),
                    width: 0.0,
                    height: 0.0,
                    links: Vec::new(),
                }
            }
        }
    });
    match failure {
        Some(e) => Err(e),
        None => Ok(out),
    }
}

fn draw(
    db: &SankeyDb,
    conf: &Config,
    input: &Input<'_>,
    chart_width: f64,
    natural: f64,
) -> Result<Output, Error> {
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let least = round(size * 0.625);
    // Labels shrink with a chart narrowed to fit, as they would if the whole picture were
    // scaled down.
    let mut small = least.max(round(size * 0.875 * (chart_width / natural).min(1.0)));
    let pad = options.padding.unwrap_or(8.0);

    let chart_height = number_option(conf, "height", 400.0, 1.0, 10000.0);
    let node_width = number_option(conf, "nodeWidth", 10.0, 1.0, 200.0).min(chart_width);
    let show_values = !matches!(conf.get("showValues"), Some(Value::Bool(false)));
    let prefix = crate::text::entities::decode_entities(&text_option(conf, "prefix"));
    let suffix = crate::text::entities::decode_entities(&text_option(conf, "suffix"));
    let outlined = matches!(conf.get("labelStyle"), Some(Value::String(s)) if s == "outlined");
    let alignment = match conf.get("nodeAlignment") {
        Some(Value::String(s)) if s == "left" => Alignment::Left,
        Some(Value::String(s)) if s == "right" => Alignment::Right,
        Some(Value::String(s)) if s == "center" => Alignment::Center,
        _ => Alignment::Justify,
    };
    let link_color = match conf.get("linkColor") {
        Some(Value::String(s)) => s.clone(),
        _ => "gradient".to_string(),
    };

    let lay = |label_size: f64| {
        layout_sankey(
            db,
            &LayoutOptions {
                width: chart_width,
                height: chart_height,
                node_width,
                node_padding: number_option(conf, "nodePadding", 12.0, 0.0, 200.0)
                    + if show_values { VALUE_GAP } else { 0.0 },
                label_height: label_size * 1.15,
                alignment,
            },
        )
    };
    let mut layout = lay(small)?;
    // Labels also shrink with the most crowded column, so each keeps about LABEL_ROOM lines.
    let mut rows: HashMap<usize, usize> = HashMap::new();
    let mut fullest = 1usize;
    for node in &layout.nodes {
        let count = rows.entry(node.layer).or_insert(0);
        *count += 1;
        if *count > fullest {
            fullest = *count;
        }
    }
    let crowded = least.max(round(chart_height / fullest as f64 / LABEL_ROOM));
    if crowded < small {
        small = crowded;
        layout = lay(small)?;
    }
    let nodes = &layout.nodes;
    let links = &layout.links;
    let line_height = small * 1.5;

    let mut custom: HashMap<String, String> = HashMap::new();
    if let Some(Value::Map(colors)) = conf.get("nodeColors") {
        for (id, color) in colors {
            if let Value::String(color) = color {
                let style = fill_style(color);
                if !style.is_empty() {
                    custom.insert(id.clone(), style);
                }
            }
        }
    }
    let node_style = |i: usize| custom.get(&db.nodes[i].id).cloned().unwrap_or_default();
    let node_color = |i: usize| -> String {
        let style = node_style(i);
        // Match the whole fill to preserve semicolons in escaped entities.
        if let Some(rest) = style.strip_prefix(" style=\"fill:") {
            if let Some(fill) = rest.strip_suffix(";\"") {
                return fill.to_string();
            }
        }
        series_color(i)
    };

    // Labels sit beside the bars, on the side that faces the middle of the chart unless that
    // side is crowded.
    let mut central = 0usize;
    let mut most = 0.0f64;
    for node in nodes {
        if node.value > most {
            most = node.value;
            central = node.layer;
        }
    }
    let mut min_x = 0.0f64;
    let mut max_x = chart_width;
    let mut min_y = 0.0f64;
    let mut max_y = chart_height;
    let mut labels: Vec<Label> = Vec::with_capacity(nodes.len());
    for (i, node) in nodes.iter().enumerate() {
        let name = decode_entities(&db.nodes[i].id);
        let value = if show_values {
            format!(
                "{prefix}{}{suffix}",
                crate::util::js_number(rounded(node.value))
            )
        } else {
            String::new()
        };
        let measured = if value.is_empty() {
            name.clone()
        } else {
            format!("{name} {value}")
        };
        let width = measurer.width(&measured, small, Style::NONE);
        let after = if outlined {
            node.layer >= central
        } else {
            node.x < chart_width / 2.0
        };
        let x = if after {
            node.x + node_width + LABEL_GAP
        } else {
            node.x - LABEL_GAP
        };
        let y = node.y + node.height / 2.0;
        min_x = min_x.min(if after { x } else { x - width });
        max_x = max_x.max(if after { x + width } else { x });
        min_y = min_y.min(y - small * 0.75);
        max_y = max_y.max(y + small * 0.75);
        labels.push(Label {
            value,
            width,
            after,
            left: if after { x } else { x - width },
            y,
        });
    }

    // A label moves at most one line; retain two bands on each side.
    let mut bands: HashMap<i64, Vec<usize>> = HashMap::new();
    if nodes.len() <= CAREFUL {
        for (i, label) in labels.iter().enumerate() {
            let band = (label.y / line_height).floor() as i64;
            for key in [
                band.saturating_sub(2),
                band.saturating_sub(1),
                band,
                band.saturating_add(1),
                band.saturating_add(2),
            ] {
                bands.entry(key).or_default().push(i);
            }
        }
    }

    let mut labels_out = String::new();
    let mut nodes_out = String::new();
    for (i, node) in nodes.iter().enumerate() {
        let id = &db.nodes[i].id;
        let name = decode_entities(id);
        let height = node.height.max(1.0);
        let _ = write!(
            nodes_out,
            "<rect class=\"pele-sankey-node\" data-id=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" \
             height=\"{}\" rx=\"{}\" fill=\"{}\"{}/>",
            esc_text(id),
            num(node.x),
            num(node.y),
            num(node_width),
            num(height),
            num(2.0f64.min(height / 2.0)),
            series_color(i),
            node_style(i)
        );

        if nodes.len() <= CAREFUL
            && overlaps_label(&bands, &labels, i, labels[i].left, labels[i].y, line_height)
        {
            // Try the other side, then an adjacent line when both sides are crowded.
            'place: for dy in [0.0, -line_height, line_height] {
                for after in [labels[i].after, !labels[i].after] {
                    let left = if after {
                        node.x + node_width + LABEL_GAP
                    } else {
                        node.x - LABEL_GAP - labels[i].width
                    };
                    let y = labels[i].y + dy;
                    if overlaps_label(&bands, &labels, i, left, y, line_height)
                        || left < min_x
                        || left + labels[i].width > max_x
                        || y - small * 0.75 < min_y
                        || y + small * 0.75 > max_y
                    {
                        continue;
                    }
                    if nodes.iter().enumerate().any(|(j, bar)| {
                        j != i
                            && left < bar.x + node_width + LABEL_GAP
                            && left + labels[i].width > bar.x - LABEL_GAP
                            && y - small * 0.75 < bar.y + bar.height
                            && y + small * 0.75 > bar.y
                    }) {
                        continue;
                    }
                    // A label stays nearer its own bar than any other in the column, so it
                    // cannot name that one.
                    let own = (node.y - y).max(0.0).max(y - node.y - node.height);
                    if nodes.iter().enumerate().any(|(j, bar)| {
                        j != i
                            && bar.x == node.x
                            && (bar.y - y).max(0.0).max(y - bar.y - bar.height) <= own
                    }) {
                        continue;
                    }
                    labels[i].left = left;
                    labels[i].y = y;
                    labels[i].after = after;
                    break 'place;
                }
            }
        }

        let label = &labels[i];
        let x = if label.after {
            node.x + node_width + LABEL_GAP
        } else {
            node.x - LABEL_GAP
        };
        let position = format!(
            " x=\"{}\" y=\"{}\" text-anchor=\"{}\"",
            num(x),
            num(label.y + small * 0.35),
            if label.after { "start" } else { "end" }
        );
        let mut text = esc(&name);
        if !label.value.is_empty() {
            let _ = write!(
                text,
                " <tspan class=\"pele-sankey-value\" fill=\"var(--_m)\">{}</tspan>",
                esc(&label.value)
            );
        }
        let _ = write!(
            labels_out,
            "<g class=\"pele-label\" data-id=\"{}\">",
            esc_text(id)
        );
        if outlined {
            let _ = write!(
                labels_out,
                "<text{position} fill=\"none\" stroke=\"var(--_bg)\" stroke-width=\"3\" \
                 stroke-linejoin=\"round\">{}{}</text>",
                esc(&name),
                if label.value.is_empty() {
                    String::new()
                } else {
                    format!(" {}", esc(&label.value))
                }
            );
        }
        let _ = write!(labels_out, "<text{position}>{text}</text></g>");
    }

    let fixed = if link_color == "gradient" || link_color == "source" || link_color == "target" {
        String::new()
    } else {
        fill_style(&link_color)
    };
    // Without an idPrefix diagrams share gradient ids, and Chrome then uses a hidden or
    // differently themed copy's gradient. Other characters would end the url() and void the
    // whole fill.
    let id_prefix = options.id_prefix.as_deref().unwrap_or("");
    let gradient = link_color == "gradient"
        && !id_prefix.is_empty()
        && id_prefix
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    let mut gradients: Vec<String> = Vec::new();
    let mut links_out = String::new();
    for link in links {
        let end = if link_color == "target" {
            link.target
        } else {
            link.source
        };
        let half = link.thickness.max(1.0) / 2.0;
        let x0 = nodes[link.source].x + node_width;
        let x1 = nodes[link.target].x;
        let mut paint = format!(
            "fill=\"{}\"{}",
            series_color(end),
            if fixed.is_empty() {
                node_style(end)
            } else {
                fixed.clone()
            }
        );
        if !fixed.is_empty() {
            paint = format!("fill=\"{}\"{}", series_color(end), fixed);
        }
        if gradient {
            // Ids come from the gradient itself, so diagrams sharing a prefix agree on them.
            // The colour after the url() is the fallback.
            let stops = format!(
                "x1=\"{}\" x2=\"{}\"><stop offset=\"0\" stop-color=\"{}\"/>\
                 <stop offset=\"1\" stop-color=\"{}\"/>",
                num(x0),
                num(x1),
                node_color(link.source),
                node_color(link.target)
            );
            let id = format!("{id_prefix}-sankey-{}", content_id(&stops));
            if !gradients
                .iter()
                .any(|g| g.contains(&format!("id=\"{id}\"")))
            {
                gradients.push(format!(
                    "<linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" {stops}\
                     </linearGradient>"
                ));
            }
            paint = format!("fill=\"url(#{id}) {}\"", node_color(link.source));
        }
        let mid = num((x0 + x1) / 2.0);
        let from = esc_text(&db.nodes[link.source].id);
        let to = esc_text(&db.nodes[link.target].id);
        let _ = write!(
            links_out,
            "<path class=\"pele-sankey-link\" data-id=\"{from}-&gt;{to}\" \
             d=\"M{},{}C{},{} {},{} {},{}L{},{}C{},{} {},{} {},{}Z\" {paint}>",
            num(x0),
            num(link.source_y - half),
            mid,
            num(link.source_y - half),
            mid,
            num(link.target_y - half),
            num(x1),
            num(link.target_y - half),
            num(x1),
            num(link.target_y + half),
            mid,
            num(link.target_y + half),
            mid,
            num(link.source_y + half),
            num(x0),
            num(link.source_y + half)
        );
        let _ = write!(
            links_out,
            "<title>{from} → {to}: {}{}{}</title></path>",
            crate::svg::esc(&prefix),
            crate::util::js_number(rounded(link.value)),
            crate::svg::esc(&suffix)
        );
    }

    let title = title_label(db.title.as_deref(), measurer, size, title_room(options));
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let inner = (max_x - min_x).max(title.width);
    let ox = pad - min_x + (inner - (max_x - min_x)) / 2.0;
    let oy = pad + title_height - min_y;
    let width = (inner + 2.0 * pad).ceil();
    let height = (max_y - min_y + title_height + 2.0 * pad).ceil();

    let svg = svg_document(
        crate::DiagramKind::Sankey,
        width,
        height,
        size,
        options,
        &Accessible::default(),
        &format!(
            "{}<g transform=\"translate({},{})\">{}<g class=\"pele-sankey-links\" \
             fill-opacity=\"0.4\">{links_out}</g><g class=\"pele-sankey-nodes\">{nodes_out}</g>\
             <g class=\"pele-labels\" font-size=\"{}\">{labels_out}</g></g>",
            label_svg(
                &title,
                width / 2.0,
                pad + title.height / 2.0,
                " class=\"pele-title\" font-weight=\"var(--_tw)\""
            ),
            num(ox),
            num(oy),
            if gradients.is_empty() {
                String::new()
            } else {
                format!("<defs>{}</defs>", gradients.join(""))
            },
            num(small)
        ),
    );

    Ok(Output {
        svg,
        width,
        height,
        links: Vec::new(),
    })
}
