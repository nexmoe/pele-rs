//! The drawing of a cynefin diagram: a frame of four regions with an ellipse in the middle, the
//! domains in them, and the transitions between them. The layout is pele's own, ported from
//! `diagrams/cynefin/render.ts`.

use super::boundaries::{
    FOLD, HORIZON, MARKER_TRIM, WaveOptions, arrow_head, backward, cliff, curve_path, ellipse_path,
    forward, resolve_seed, wave,
};
use super::db::{CynefinModel, Domain};
use crate::diagram::{Input, Output};
use crate::error::Error;
use crate::preprocess::{Config, Value};
use crate::svg::{Accessible, ROUNDED, esc_text, label_svg, num, series_color, svg_document};
use crate::text::Style;
use crate::text::label::{Label, layout_label};

/// The four regions of the frame, with the column and the row they stand in and the color of
/// the series they are filled with.
const QUADRANTS: [(Domain, usize, usize, usize); 4] = [
    (Domain::Complex, 0, 0, 0),
    (Domain::Complicated, 1, 0, 1),
    (Domain::Chaotic, 0, 1, 3),
    (Domain::Clear, 1, 1, 2),
];

/// The color of the confusion region.
const CONFUSION_COLOR: usize = 4;

/// The name of a domain, the model of its decisions and the kind of practice it holds. The
/// order is the one of [`slot`].
const META: [(Domain, &str, &str, &str); 5] = [
    (
        Domain::Complex,
        "Complex",
        "Probe \u{2192} Sense \u{2192} Respond",
        "Emergent Practices",
    ),
    (
        Domain::Complicated,
        "Complicated",
        "Sense \u{2192} Analyse \u{2192} Respond",
        "Good Practices",
    ),
    (
        Domain::Chaotic,
        "Chaotic",
        "Act \u{2192} Sense \u{2192} Respond",
        "Novel Practices",
    ),
    (
        Domain::Clear,
        "Clear",
        "Sense \u{2192} Categorise \u{2192} Respond",
        "Best Practices",
    ),
    (Domain::Confusion, "Confusion", "", "Disorder"),
];

/// The most items the confusion region shows; the rest are counted in one badge.
const MAX_CONFUSION_ITEMS: usize = 3;
/// The width an item is wrapped at.
const ITEM_MAX_WIDTH: f64 = 200.0;
const ITEM_PAD_X: f64 = 10.0;
const ITEM_PAD_Y: f64 = 4.0;
const ITEM_GAP: f64 = 6.0;
const HEAD_GAP: f64 = 10.0;
const MARGIN: f64 = 20.0;
const CLEARANCE: f64 = 10.0;

/// An item of a domain, in a badge.
struct Badge {
    text: String,
    label: Label,
    w: f64,
    h: f64,
    overflow: bool,
}

/// A domain, with the room it takes.
struct Block {
    name: Domain,
    title: Label,
    subtitles: Vec<Label>,
    badges: Vec<Badge>,
    w: f64,
    h: f64,
    cx: f64,
    cy: f64,
}

/// The room a value of the settings takes.
fn setting(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    match config.get("cynefin") {
        Some(Value::Map(map)) => match map.get(key) {
            Some(Value::Number(value)) if *value >= min && *value <= max => *value,
            _ => fallback,
        },
        _ => fallback,
    }
}

/// Whether the domains tell what their decisions are.
fn show_descriptions(config: &Config) -> bool {
    match config.get("cynefin") {
        Some(Value::Map(map)) => {
            !matches!(map.get("showDomainDescriptions"), Some(Value::Bool(false)))
        }
        _ => true,
    }
}

/// Draws a cynefin diagram.
pub(crate) fn render_cynefin(model: &CynefinModel, input: &Input<'_>) -> Result<Output, Error> {
    let options = input.options;
    let size = options.font_size.unwrap_or(16.0);
    let small = (size * 0.8125).round();
    let item_size = (size * 0.875).round();
    let measurer = input.measurer;
    // The padding of the frame is the padding of the drawing, as pele's layout has it. Mermaid's
    // schema has a `padding` of its own for the type, which this layout does not read.
    let pad = options.padding.unwrap_or(8.0);
    let show = show_descriptions(input.config);
    let amplitude = setting(input.config, "boundaryAmplitude", 8.0, 0.0, 50.0);
    let seed = resolve_seed(
        setting(input.config, "seed", 0.0, -1e15, 1e15),
        &options.id_prefix,
    );
    let item_line = (item_size * 1.5).round();

    let badge = |text: &str, overflow: bool| -> Badge {
        let label = layout_label(
            Some(text),
            false,
            measurer,
            item_size,
            ITEM_MAX_WIDTH,
            Style::NONE,
        );
        let w = label.width + 2.0 * ITEM_PAD_X;
        let h = label.height.max(item_line) + 2.0 * ITEM_PAD_Y;
        Badge {
            text: text.to_string(),
            label,
            w,
            h,
            overflow,
        }
    };

    let block = |name: Domain| -> Block {
        let meta = META[name.slot()];
        let title = layout_label(Some(meta.1), false, measurer, size, 4000.0, Style::BOLD);
        let mut subtitles: Vec<Label> = Vec::new();
        if show {
            for text in [meta.2, meta.3] {
                if !text.is_empty() {
                    subtitles.push(layout_label(
                        Some(text),
                        false,
                        measurer,
                        small,
                        4000.0,
                        Style::ITALIC,
                    ));
                }
            }
        }
        let items = model.items(name);
        let shown = if name == Domain::Confusion && items.len() > MAX_CONFUSION_ITEMS {
            MAX_CONFUSION_ITEMS
        } else {
            items.len()
        };
        let mut badges: Vec<Badge> = Vec::new();
        for item in items.iter().take(shown) {
            badges.push(badge(item, false));
        }
        if shown < items.len() {
            badges.push(badge(&format!("+{} more", items.len() - shown), true));
        }
        let mut w = title.width;
        let mut h = title.height;
        for subtitle in &subtitles {
            w = w.max(subtitle.width);
            h += subtitle.height;
        }
        if !badges.is_empty() {
            h += HEAD_GAP - ITEM_GAP;
        }
        for item in &badges {
            w = w.max(item.w);
            h += ITEM_GAP + item.h;
        }
        Block {
            name,
            title,
            subtitles,
            badges,
            w,
            h,
            cx: 0.0,
            cy: 0.0,
        }
    };

    let mut blocks: Vec<Block> = QUADRANTS.iter().map(|(name, ..)| block(*name)).collect();
    let mut confusion = block(Domain::Confusion);
    let rx = (confusion.w * 0.71 + 8.0).max(88.0);
    let ry = (confusion.h * 0.71 + 6.0).max(64.0);

    // The size of the frame. It is pele's own: the widths and the heights Mermaid's schema
    // gives are the ones the layout of Mermaid's renderer draws, not the ones here.
    let mut quad_w = setting(input.config, "width", 640.0, 100.0, 10000.0) / 2.0;
    let mut quad_h = setting(input.config, "height", 440.0, 100.0, 10000.0) / 2.0;
    for b in &blocks {
        quad_w = quad_w.max(b.w + 2.0 * MARGIN);
        quad_h = quad_h.max(b.h + 2.0 * MARGIN);
    }
    // A block sits in the middle of its quadrant, so its inner corner must stay clear of the
    // ellipse.
    for b in &blocks {
        let dx = (quad_w - b.w) / 2.0;
        let dy = (quad_h - b.h) / 2.0;
        if (dx / (rx + CLEARANCE)).powi(2) + (dy / (ry + CLEARANCE)).powi(2) >= 1.0 {
            continue;
        }
        let wider = b.w + 2.0 * (rx + CLEARANCE);
        let taller = b.h + 2.0 * (ry + CLEARANCE);
        if wider * quad_h <= quad_w * taller {
            quad_w = quad_w.max(wider);
        } else {
            quad_h = quad_h.max(taller);
        }
    }

    let title = layout_label(
        model.title.as_deref(),
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
    let width = 2.0 * quad_w;
    let height = 2.0 * quad_h;
    let x0 = pad;
    let y0 = pad + title_height;
    let cx = x0 + quad_w;
    let cy = y0 + quad_h;
    let right = x0 + width;
    let bottom = y0 + height;

    let fold = wave(
        y0,
        cy,
        cx,
        seed,
        amplitude,
        &WaveOptions {
            segments: 4,
            pin_end: true,
            ..FOLD
        },
    );
    let west = wave(
        x0,
        cx,
        cy,
        seed + 100.0,
        amplitude,
        &WaveOptions {
            segments: 4,
            pin_end: true,
            ..HORIZON
        },
    );
    let east = wave(
        cx,
        right,
        cy,
        seed + 200.0,
        amplitude,
        &WaveOptions {
            segments: 4,
            pin_start: true,
            ..HORIZON
        },
    );
    let drop = cliff(cx, cy, bottom, width * 0.03);

    let at = |curve: &[f64], end: bool| -> String {
        if end {
            format!(
                "{},{}",
                num(curve[curve.len() - 2]),
                num(curve[curve.len() - 1])
            )
        } else {
            format!("{},{}", num(curve[0]), num(curve[1]))
        }
    };

    let regions = [
        (
            Domain::Complex,
            format!(
                "M{},{}L{}{}{}Z",
                num(x0),
                num(y0),
                at(&fold, false),
                forward(&fold),
                backward(&west)
            ),
        ),
        (
            Domain::Complicated,
            format!(
                "M{}L{},{}L{}{}{}Z",
                at(&fold, false),
                num(right),
                num(y0),
                at(&east, true),
                backward(&east),
                backward(&fold)
            ),
        ),
        (
            Domain::Chaotic,
            format!(
                "M{}{}{}L{},{}Z",
                at(&west, false),
                forward(&west),
                forward(&drop),
                num(x0),
                num(bottom)
            ),
        ),
        (
            Domain::Clear,
            format!(
                "M{}{}L{},{}L{}{}Z",
                at(&east, false),
                forward(&east),
                num(right),
                num(bottom),
                at(&drop, true),
                backward(&drop)
            ),
        ),
    ];

    let mut fills = String::new();
    for (name, _, _, color) in QUADRANTS {
        let path = regions
            .iter()
            .find(|(domain, _)| *domain == name)
            .map_or("", |(_, path)| path.as_str());
        fills.push_str(&format!(
            "<path class=\"pele-rs-cynefin-region\" data-id=\"{}\" d=\"{path}\" fill=\"{}\" fill-opacity=\"0.14\"/>",
            name.as_str(),
            series_color(color)
        ));
    }
    let boundaries = format!(
        "<g class=\"pele-rs-cynefin-boundaries\" fill=\"none\" stroke=\"var(--_b)\" stroke-dasharray=\"6 3\">\
         <path d=\"{}\"/><path d=\"{}\"/><path d=\"{}\"/>\
         <path class=\"pele-rs-cynefin-cliff\" d=\"{}\" stroke=\"var(--_l)\" stroke-width=\"2\" \
         stroke-dasharray=\"none\" stroke-linecap=\"round\"/></g>",
        curve_path(&fold),
        curve_path(&west),
        curve_path(&east),
        curve_path(&drop),
    );
    let oval = ellipse_path(cx, cy, rx, ry);
    let ellipse = format!(
        "<path d=\"{oval}\" fill=\"var(--_bg)\"/>\
         <path class=\"pele-rs-cynefin-region\" data-id=\"confusion\" d=\"{oval}\" fill=\"{}\" \
         fill-opacity=\"0.18\" stroke=\"var(--_b)\" stroke-dasharray=\"4 2\"/>",
        series_color(CONFUSION_COLOR)
    );

    // The five blocks, in the places they stand in.
    for (index, (_, column, row, _)) in QUADRANTS.iter().enumerate() {
        blocks[index].cx = x0 + quad_w * (*column as f64 + 0.5);
        blocks[index].cy = y0 + quad_h * (*row as f64 + 0.5);
    }
    confusion.cx = cx;
    confusion.cy = cy;
    blocks.push(confusion);

    let mut domains = String::new();
    for b in &blocks {
        domains.push_str(&draw_block(b, small, item_size));
    }

    let inside = |b: &Block, x: f64, y: f64| -> bool {
        if b.name == Domain::Confusion {
            ((x - cx) / (rx + 4.0)).powi(2) + ((y - cy) / (ry + 4.0)).powi(2) <= 1.0
        } else {
            (x - b.cx).abs() <= b.w / 2.0 + 8.0 && (y - b.cy).abs() <= b.h / 2.0 + 8.0
        }
    };

    let mut edges = String::new();
    let mut seen: Vec<((Domain, Domain), usize)> = Vec::new();
    for transition in &model.transitions {
        let from = &blocks[transition.from.slot()];
        let to = &blocks[transition.to.slot()];
        if from.name == to.name {
            continue;
        }
        let key = if transition.from.as_str() < transition.to.as_str() {
            (transition.from, transition.to)
        } else {
            (transition.to, transition.from)
        };
        let repeat = match seen.iter_mut().find(|(pair, _)| *pair == key) {
            Some((_, count)) => {
                *count += 1;
                *count - 1
            }
            None => {
                seen.push((key, 1));
                0
            }
        };

        let dx = to.cx - from.cx;
        let dy = to.cy - from.cy;
        let len = dx.hypot(dy).max(f64::MIN_POSITIVE);
        let mx = (from.cx + to.cx) / 2.0;
        let my = (from.cy + to.cy) / 2.0;
        // Curves bow away from the middle of the diagram, which keeps them off the ellipse.
        let outward = if -dy * (mx - cx) + dx * (my - cy) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let nx = (-dy / len) * outward;
        let ny = (dx / len) * outward;
        // A diagonal runs through the middle, so it bows far enough to pass the ellipse.
        let diagonal = from.name != Domain::Confusion
            && to.name != Domain::Confusion
            && dx.abs() > 1.0
            && dy.abs() > 1.0;
        let bow = (if diagonal {
            2.0 * ((rx * nx).hypot(ry * ny) + 16.0)
        } else {
            len * 0.15
        }) + repeat as f64 * 56.0;
        let px = mx + nx * bow;
        let py = my + ny * bow;
        let point = |t: f64| -> (f64, f64) {
            let u = 1.0 - t;
            (
                u * u * from.cx + 2.0 * u * t * px + t * t * to.cx,
                u * u * from.cy + 2.0 * u * t * py + t * t * to.cy,
            )
        };
        let crossing = |b: &Block, mut lo: f64, mut hi: f64, leaving: bool| -> f64 {
            for _ in 0..16 {
                let mid = (lo + hi) / 2.0;
                let (x, y) = point(mid);
                if inside(b, x, y) == leaving {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            (lo + hi) / 2.0
        };
        const STEPS: usize = 48;
        let mut i = 1;
        while i <= STEPS {
            let (x, y) = point(i as f64 / STEPS as f64);
            if !inside(from, x, y) {
                break;
            }
            i += 1;
        }
        if i > STEPS {
            continue;
        }
        let t0 = crossing(
            from,
            (i - 1) as f64 / STEPS as f64,
            i as f64 / STEPS as f64,
            true,
        );
        while i <= STEPS {
            let (x, y) = point(i as f64 / STEPS as f64);
            if inside(to, x, y) {
                break;
            }
            i += 1;
        }
        let t1 = if i > STEPS {
            1.0
        } else {
            crossing(
                to,
                (i - 1) as f64 / STEPS as f64,
                i as f64 / STEPS as f64,
                false,
            )
        };

        let (sx, sy) = point(t0);
        let (ex, ey) = point(t1);
        let blossom = |a: f64, b: f64, c: f64| -> f64 {
            (1.0 - t0) * (1.0 - t1) * a + ((1.0 - t0) * t1 + t0 * (1.0 - t1)) * b + t0 * t1 * c
        };
        let qx = blossom(from.cx, px, to.cx);
        let qy = blossom(from.cy, py, to.cy);
        let el = (ex - qx).hypot(ey - qy).max(f64::MIN_POSITIVE);
        let ux = (ex - qx) / el;
        let uy = (ey - qy) / el;
        let head = arrow_head(ex, ey, ux, uy, "var(--_l)");
        let ex = ex - ux * MARKER_TRIM;
        let ey = ey - uy * MARKER_TRIM;

        let mut text = String::new();
        if let Some(label) = transition.label.as_deref() {
            let label = layout_label(
                Some(label),
                false,
                measurer,
                small,
                ITEM_MAX_WIDTH,
                Style::NONE,
            );
            let (lx, ly) = point((t0 + t1) / 2.0);
            let away = (nx.abs() * label.width) / 2.0 + (ny.abs() * label.height) / 2.0 + 6.0;
            let half = label.width / 2.0 + 4.0;
            let tx = (lx + nx * away)
                .max(x0 + half)
                .min((right - half).max(x0 + half));
            text.push_str(&label_svg(
                &label,
                tx,
                ly + ny * away,
                &format!(" class=\"pele-rs-edge-label\" font-size=\"{}\"", num(small)),
            ));
        }
        edges.push_str(&format!(
            "<g class=\"pele-rs-edge\" data-id=\"{}-{}\">\
             <path d=\"M{},{}Q{},{} {},{}\" fill=\"none\" stroke=\"var(--_l)\"/>{head}{text}</g>",
            transition.from.as_str(),
            transition.to.as_str(),
            num(sx),
            num(sy),
            num(qx),
            num(qy),
            num(ex),
            num(ey),
        ));
    }

    let total_width = (width.max(title.width) + 2.0 * pad).ceil();
    let total_height = (height + title_height + 2.0 * pad).ceil();
    let content = format!(
        "{}<g class=\"pele-rs-cynefin-regions\">{fills}</g>{boundaries}{ellipse}{domains}{}",
        label_svg(
            &title,
            x0 + width / 2.0,
            pad + title.height / 2.0,
            " class=\"pele-rs-title\" font-weight=\"var(--_tw)\""
        ),
        if edges.is_empty() {
            String::new()
        } else {
            format!("<g class=\"pele-rs-edges\">{edges}</g>")
        },
    );

    let acc = Accessible {
        title: model.acc_title.clone(),
        descr: model.acc_descr.clone(),
    };
    Ok(Output {
        svg: svg_document(
            crate::DiagramKind::Cynefin,
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
    })
}

/// Draws a domain: its name, what it tells of its decisions, and its items.
fn draw_block(b: &Block, small: f64, item_size: f64) -> String {
    let mut y = b.cy - b.h / 2.0;
    let mut out = label_svg(
        &b.title,
        b.cx,
        y + b.title.height / 2.0,
        " class=\"pele-rs-label\" font-weight=\"var(--_hw)\"",
    );
    y += b.title.height;
    for subtitle in &b.subtitles {
        out.push_str(&label_svg(
            subtitle,
            b.cx,
            y + subtitle.height / 2.0,
            &format!(
                " class=\"pele-rs-cynefin-subtitle\" font-size=\"{}\" font-style=\"italic\" fill=\"var(--_m)\"",
                num(small)
            ),
        ));
        y += subtitle.height;
    }
    y += HEAD_GAP - ITEM_GAP;
    for item in &b.badges {
        y += ITEM_GAP;
        let class = if item.overflow {
            "pele-rs-node pele-rs-cynefin-overflow"
        } else {
            "pele-rs-node"
        };
        let id = if item.overflow {
            String::new()
        } else {
            format!(" data-id=\"{}\"", esc_text(&item.text))
        };
        out.push_str(&format!(
            "<g class=\"{class}\"{id}>\
             <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{}{ROUNDED}/>\
             {}</g>",
            num(b.cx - item.w / 2.0),
            num(y),
            num(item.w),
            num(item.h),
            if item.overflow {
                " stroke-dasharray=\"3 2\""
            } else {
                ""
            },
            label_svg(
                &item.label,
                b.cx,
                y + item.h / 2.0,
                &format!(
                    " class=\"pele-rs-label\" font-size=\"{}\"{}",
                    num(item_size),
                    if item.overflow {
                        " fill=\"var(--_m)\""
                    } else {
                        ""
                    }
                ),
            ),
        ));
        y += item.h;
    }
    format!(
        "<g class=\"pele-rs-cynefin-domain\" data-id=\"{}\">{out}</g>",
        b.name.as_str()
    )
}
