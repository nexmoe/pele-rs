//! The layout and the SVG of a tree view.
//!
//! The layout is pele's own: `src/diagrams/treeview/render.ts` in pele 1.9.0. Mermaid draws a
//! tree in another way, but the tree is drawn the way pele draws it.

use std::fmt::Write;

use super::db::{Model, NodeType};
use super::icons::Icons;
use crate::DiagramKind;
use crate::diagram::{Input, Output};
use crate::diagrams::common::literal::literal_label;
use crate::preprocess::Value;
use crate::svg::{Accessible, ROUNDED, class_names, esc, esc_text, label_svg, num, svg_document};
use crate::text::Style;
use crate::text::entities::decode_entities;
use crate::text::label::{Label, layout_label};
use crate::util::round;

/// The room between a name and the description beside it.
const DESC_GAP: f64 = 16.0;
/// How far the lines of the tree stand from the names they join.
const GUIDE_INSET: f64 = 7.0;
/// The bend of the line that joins a name to the one below it.
const ELBOW: f64 = 4.0;
/// How far a highlighted name reaches over the edge of the tree.
const HIGHLIGHT_OVERHANG: f64 = 4.0;
/// The room a name has before it is broken over several lines.
const MIN_ROOM: f64 = 40.0;

/// The settings of the diagram.
#[derive(Debug, Clone, Copy)]
struct Settings {
    padding_x: f64,
    padding_y: f64,
    row_indent: f64,
    line_thickness: f64,
}

impl Settings {
    /// Reads the settings of the diagram. Mermaid's own defaults, with pele's.
    fn from_config(config: &crate::preprocess::Config) -> Settings {
        let mut settings = Settings {
            padding_x: 5.0,
            padding_y: 2.0,
            row_indent: 15.0,
            line_thickness: 1.0,
        };
        let Some(Value::Map(tree)) = config.get("treeView") else {
            return settings;
        };
        if let Some(Value::Number(value)) = tree.get("paddingX") {
            settings.padding_x = number(*value, 0.0, 100.0, settings.padding_x);
        }
        if let Some(Value::Number(value)) = tree.get("paddingY") {
            settings.padding_y = number(*value, 0.0, 100.0, settings.padding_y);
        }
        if let Some(Value::Number(value)) = tree.get("rowIndent") {
            settings.row_indent = number(*value, 0.0, 400.0, settings.row_indent);
        }
        if let Some(Value::Number(value)) = tree.get("lineThickness") {
            settings.line_thickness = number(*value, 0.25, 12.0, settings.line_thickness);
        }
        settings
    }
}

/// A number of the configuration, when it is one the diagram accepts.
fn number(value: f64, least: f64, most: f64, fallback: f64) -> f64 {
    if value.is_finite() && value >= least && value <= most {
        value
    } else {
        fallback
    }
}

/// A row of the tree.
struct Row {
    /// The node the row holds.
    node: usize,
    /// Where the row starts, measured from the left of the tree.
    x: f64,
    label_x: f64,
    /// The middle of the row, measured from the top of the rows.
    cy: f64,
    /// Where the row starts, measured from the top of the rows.
    top: f64,
    /// The height of the row.
    height: f64,
    /// The name of the icon of the row, as it is drawn.
    icon_label: Option<String>,
    /// The room the icon takes.
    /// The name of the node.
    text: String,
    /// The name, measured and broken over several lines when the tree is drawn tight.
    label: Option<Label>,
    /// The description of the node, measured and broken over several lines when the tree is
    /// drawn tight.
    description: Option<Label>,
    /// The description of the node, as it is written.
    description_text: Option<String>,
    /// Whether the node stands for a folder.
    directory: bool,
    /// Whether the node is highlighted.
    highlighted: bool,
}

/// Draws the tree.
pub(crate) fn render_treeview(model: &Model, input: &Input<'_>) -> Output {
    let loose = draw(model, input, false);
    let Some(available) = input.options.max_width.filter(|&w| w > 0.0) else {
        return loose;
    };
    if loose.width <= available {
        return loose;
    }
    // A tree that does not fit is drawn tight, with the names broken over several lines.
    let tight = draw(model, input, true);
    if tight.width < loose.width {
        tight
    } else {
        loose
    }
}

/// Draws the tree, either as it is or as narrow as it may be.
fn draw(model: &Model, input: &Input<'_>, tight: bool) -> Output {
    let options = input.options;
    let measurer = input.measurer;
    let size = options.font_size.unwrap_or(16.0);
    let pad = options.padding.unwrap_or(8.0);
    let settings = Settings::from_config(input.config);
    let icons = Icons::from_config(input.config);

    let (rows, step, row_of) = layout(model, input, &settings, &icons, size, pad, tight);

    let title = layout_label(
        model.title.as_deref(),
        false,
        measurer,
        size,
        if tight {
            title_room(options, pad)
        } else {
            4000.0
        },
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };

    // The names stand in a column, and the descriptions in a column of their own to the right.
    let label_right = rows.iter().fold(0.0f64, |most, row| {
        let width = match &row.label {
            Some(label) => label.width,
            None => measurer.width(
                &row.text,
                size,
                row.style()
                    | if options.monospace.contains(&crate::MonospacePart::Files) {
                        Style::MONO
                    } else {
                        Style::NONE
                    },
            ),
        };
        most.max(row.label_x + width)
    });
    let desc_x = label_right + DESC_GAP;
    let mut width = label_right;
    for row in &rows {
        let Some(description) = &row.description_text else {
            continue;
        };
        let w = match &row.description {
            Some(label) => label.width,
            None => measurer.width(description, size, Style::ITALIC),
        };
        width = width.max(if tight { row.label_x } else { desc_x } + w);
    }
    width = width.max(title.width);

    let any_highlight = rows.iter().any(|row| row.highlighted);
    let inset = pad
        + if any_highlight {
            HIGHLIGHT_OVERHANG
        } else {
            0.0
        };
    let top = pad + title_height;
    let rows_height: f64 = rows.iter().map(|row| row.height).sum();
    let total_width = (width + 2.0 * inset).ceil();
    let total_height = (top + rows_height + pad).ceil();

    // The lines that join a node to the ones below it.
    let mut guides = String::new();
    for row in &rows {
        let children = &model.nodes[row.node].children;
        if children.is_empty() {
            continue;
        }
        let gx = inset + row.x + GUIDE_INSET;
        let tick = gx.max(inset + row.x + step - settings.padding_x);
        let last = children
            .last()
            .map(|&c| top + rows[row_of[c]].cy)
            .unwrap_or(row.cy);
        let r = ELBOW.min(tick - gx);
        let mut d = format!(
            "M{},{}V{}",
            num(gx),
            num(top + row.top + row.height - 2.0),
            num(last - r)
        );
        if r > 0.0 {
            let _ = write!(
                d,
                "Q{},{} {},{}H{}",
                num(gx),
                num(last),
                num(gx + r),
                num(last),
                num(tick)
            );
        }
        for child in children.iter().take(children.len() - 1) {
            if tick <= gx {
                break;
            }
            let cy = top + rows[row_of[*child]].cy;
            let _ = write!(d, "M{},{}H{}", num(gx), num(cy), num(tick));
        }
        guides += &format!("<path d=\"{d}\"/>");
    }

    let baseline = size * 0.35;
    let family = if options.monospace.contains(&crate::MonospacePart::Files) {
        format!(" font-family=\"{}\"", crate::svg::FONT_MONO)
    } else {
        String::new()
    };
    let mut body = String::new();
    for row in &rows {
        let node = &model.nodes[row.node];
        let cy = top + row.cy;
        let mut inner = String::new();
        if row.highlighted {
            let x = inset + row.x - HIGHLIGHT_OVERHANG;
            let _ = write!(
                inner,
                "<rect class=\"pele-tree-highlight\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" \
                 fill=\"var(--_c)\" fill-opacity=\"0.14\"{ROUNDED}/>",
                num(x),
                num(top + row.top + 1.0),
                num(total_width - inset + HIGHLIGHT_OVERHANG - x),
                num(row.height - 2.0)
            );
        }
        if let Some(icon) = &row.icon_label {
            let glyph = if icon == "mermaid-treeview:folder"
                || (icon != "mermaid-treeview:file" && row.directory)
            {
                "M3 6.5A1.5 1.5 0 0 1 4.5 5h4.6l2.4 2.5h8A1.5 1.5 0 0 1 21 9v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18z"
            } else {
                "M6.5 3h7L19 8.5v11a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 5 19.5v-15A1.5 1.5 0 0 1 6.5 3zM13.5 3v5.5H19"
            };
            let markup = measurer.host().and_then(|h| h.icon(icon)).map_or_else(
                || format!("<path d=\"{glyph}\" stroke=\"var(--_m)\" stroke-width=\"1.5\"/>"),
                |s| s.to_string(),
            );
            inner += &crate::svg::icon_svg(icon, inset + row.x, cy - size / 2.0, size, &markup);
        }
        match &row.label {
            Some(label) => {
                inner += &left_label(
                    label,
                    inset + row.label_x,
                    top + row.top + settings.padding_y,
                    " class=\"pele-label\"",
                    row.directory,
                );
            }
            None => {
                if !row.text.is_empty() {
                    let _ = write!(
                        inner,
                        "<text class=\"pele-label\" x=\"{}\" y=\"{}\"{}{family} xml:space=\"preserve\">{}</text>",
                        num(inset + row.label_x),
                        num(cy + baseline),
                        if row.directory {
                            " font-weight=\"var(--_hw)\""
                        } else {
                            ""
                        },
                        esc(&row.text)
                    );
                }
            }
        }
        if let Some(description) = &row.description_text {
            match &row.description {
                Some(label) => {
                    inner += &left_label(
                        label,
                        inset + row.label_x,
                        top + row.top
                            + settings.padding_y
                            + row.label.as_ref().map_or(0.0, |l| l.height),
                        " class=\"pele-tree-description\" fill=\"var(--_m)\"",
                        false,
                    );
                }
                None => {
                    let _ = write!(
                        inner,
                        "<text class=\"pele-tree-description\" x=\"{}\" y=\"{}\" \
                         font-style=\"italic\" fill=\"var(--_m)\" xml:space=\"preserve\">{}</text>",
                        num(inset + desc_x),
                        num(cy + baseline),
                        esc(description)
                    );
                }
            }
        }
        let _ = write!(
            body,
            "<g class=\"pele-node pele-tree-{}{}\" data-id=\"{}\">{}</g>",
            if row.directory { "dir" } else { "file" },
            class_names(node.css_class.as_deref().unwrap_or("")),
            esc_text(&node.name),
            inner
        );
    }

    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &if guides.is_empty() {
        String::new()
    } else {
        format!(
            "<g class=\"pele-tree-guides\" fill=\"none\" stroke=\"var(--_b)\" \
             stroke-width=\"{}\">{guides}</g>",
            num(settings.line_thickness)
        )
    } + &body;

    let acc = Accessible {
        title: model.acc_title.clone(),
        descr: model.acc_descr.clone(),
    };
    Output {
        svg: svg_document(
            DiagramKind::TreeView,
            total_width,
            total_height,
            size,
            options,
            &acc,
            &content,
        ),
        width: total_width,
        height: total_height,
        links: Vec::new(),
    }
}

/// The rows of the tree, and the room between two levels of it.
fn layout(
    model: &Model,
    input: &Input<'_>,
    settings: &Settings,
    icons: &Icons,
    size: f64,
    pad: f64,
    tight: bool,
) -> (Vec<Row>, f64, Vec<usize>) {
    let measurer = input.measurer;
    let options = input.options;
    let step = if tight {
        let available = options.max_width.unwrap_or(320.0);
        (settings.row_indent + settings.padding_x)
            .min(((available - 120.0) / depth_of(model).max(1) as f64).max(2.0))
    } else {
        settings.row_indent + settings.padding_x
    };
    let row_height = round(size * 1.5) + 2.0 * settings.padding_y;

    let mut rows: Vec<Row> = Vec::new();
    // The row every node is drawn in, so that a node can be found without a walk.
    let mut row_of: Vec<usize> = vec![0; model.nodes.len()];
    let mut pending: Vec<(usize, usize)> = Vec::new();
    push(&model.nodes[0].children, 0, &mut pending);
    let mut rows_height = 0.0;
    while let Some((node, depth)) = pending.pop() {
        let x = depth as f64 * step;
        let item = &model.nodes[node];
        let directory = item.node_type == NodeType::Directory;
        let text = decode_entities(&item.name);
        let siblings = &model.nodes[item.parent.unwrap_or(0)].children;
        let slot = siblings
            .iter()
            .any(|&i| icons.of(&model.nodes[i]).is_some());
        let label_x = x + if slot { size + 6.0 } else { 0.0 };
        let room = MIN_ROOM
            .max(options.max_width.unwrap_or(320.0) - 2.0 * pad - 2.0 * HIGHLIGHT_OVERHANG - x);
        let style = (if directory { Style::BOLD } else { Style::NONE })
            | if options.monospace.contains(&crate::MonospacePart::Files) {
                Style::MONO
            } else {
                Style::NONE
            };
        let icon_label = icons.of(item);
        let label = tight.then(|| literal_label(&text, measurer, size, room, style));
        let description_text = item.description.as_deref().map(decode_entities);
        let description = match &description_text {
            Some(text) if tight => Some(literal_label(text, measurer, size, room, Style::ITALIC)),
            _ => None,
        };
        let height = if tight {
            row_height.max(
                label.as_ref().map_or(0.0, |l| l.height)
                    + description.as_ref().map_or(0.0, |l| l.height)
                    + 2.0 * settings.padding_y,
            )
        } else {
            row_height
        };
        let cy = rows_height
            + if tight {
                settings.padding_y + size.max(label.as_ref().map_or(0.0, |l| l.height)) / 2.0
            } else {
                height / 2.0
            };
        row_of[node] = rows.len();
        rows.push(Row {
            node,
            x,
            label_x,
            cy,
            top: rows_height,
            height,
            icon_label,
            text,
            label,
            description,
            description_text,
            directory,
            highlighted: item
                .css_class
                .as_deref()
                .is_some_and(|classes| classes.split_whitespace().any(|c| c == "highlight")),
        });
        rows_height += height;
        let mut children = Vec::new();
        push(&item.children, depth + 1, &mut children);
        pending.extend(children);
    }
    (rows, step, row_of)
}

/// Puts the children of a node on the stack, the last one first, so that they are read in the
/// order they were written.
fn push(children: &[usize], depth: usize, pending: &mut Vec<(usize, usize)>) {
    for &child in children.iter().rev() {
        pending.push((child, depth));
    }
}

/// How deep the tree goes.
fn depth_of(model: &Model) -> usize {
    let mut depth = 0;
    let mut pending: Vec<(usize, usize)> =
        model.nodes[0].children.iter().map(|&c| (c, 0)).collect();
    while let Some((node, at)) = pending.pop() {
        depth = depth.max(at);
        for &child in &model.nodes[node].children {
            pending.push((child, at + 1));
        }
    }
    depth
}

/// The room a title takes before it is broken over several lines.
fn title_room(options: &crate::RenderOptions, pad: f64) -> f64 {
    options
        .max_width
        .filter(|&w| w > 0.0)
        .map_or(4000.0, |w| 80f64.max(w - 2.0 * pad))
}

impl Row {
    /// The emphasis the name of the row is drawn with.
    fn style(&self) -> Style {
        if self.directory {
            Style::BOLD
        } else {
            Style::NONE
        }
    }
}

/// A label with each line starting at `left`, for a list of names, where centered lines would
/// look ragged.
fn left_label(label: &Label, left: f64, top: f64, attrs: &str, bold: bool) -> String {
    let attrs = if bold {
        format!("{attrs} font-weight=\"var(--_hw)\"")
    } else {
        attrs.to_string()
    };
    let attrs = crate::svg::text_style(&attrs, crate::svg::shared_style(label));
    let mut out = String::new();
    for (k, line) in label.lines.iter().enumerate() {
        let y = top + (k as f64 + 0.5) * label.line_height + label.size * 0.35;
        let mut body = String::new();
        for span in line {
            let shared = crate::svg::shared_style(label);
            let own = crate::svg::span_style(span.style & !shared);
            if own.is_empty() {
                body += &esc(&span.text);
            } else {
                let _ = write!(body, "<tspan{own}>{}</tspan>", esc(&span.text));
            }
        }
        let _ = write!(
            out,
            "<text{attrs} x=\"{}\" y=\"{}\" text-anchor=\"start\" xml:space=\"preserve\">{body}</text>",
            num(left),
            num(y)
        );
    }
    out
}
