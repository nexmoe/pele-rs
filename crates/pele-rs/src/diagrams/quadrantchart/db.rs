//! The model of a quadrant chart.

use crate::diagrams::common::ordered::Ordered;
use crate::error::{Error, ErrorCode};
use crate::util::js_to_number;

/// A text of the chart. Mermaid has a second kind of text, a markdown string.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct QuadrantText {
    pub text: String,
    /// Whether the text is a markdown string, which Mermaid draws as markdown.
    pub markdown: bool,
}

/// The style of a point.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct PointStyle {
    pub radius: Option<f64>,
    pub color: Option<String>,
    pub stroke_color: Option<String>,
    pub stroke_width: Option<String>,
}

/// A point of the chart, with its place and its style.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct QuadrantPoint {
    pub x: f64,
    pub y: f64,
    pub text: QuadrantText,
    pub class_name: String,
    pub style: PointStyle,
}

/// The chart as Mermaid's parser builds it.
#[derive(Debug, Clone, Default)]
pub(crate) struct QuadrantDb {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// Quadrants 1 to 4: top right, top left, bottom left, bottom right.
    pub quadrants: [Option<QuadrantText>; 4],
    pub x_axis_left: Option<QuadrantText>,
    pub x_axis_right: Option<QuadrantText>,
    pub y_axis_bottom: Option<QuadrantText>,
    pub y_axis_top: Option<QuadrantText>,
    pub points: Vec<QuadrantPoint>,
    pub classes: Ordered<PointStyle>,
}

fn invalid(style: &str, value: &str, kind: &str) -> Error {
    Error::new(
        ErrorCode::Semantic,
        format!("value for {style} {value} is invalid, please use a valid {kind}"),
    )
}

/// Whether the value is a whole number, as Mermaid's `^\d+$` says.
fn is_number(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

/// Whether the value is a hex code of three or six digits, with or without its hash.
fn is_hex(value: &str) -> bool {
    let digits = value.strip_prefix('#').unwrap_or(value);
    (digits.len() == 6 || digits.len() == 3) && digits.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Whether the value is a number of pixels, as Mermaid's `^\d+px$` says.
fn is_pixels(value: &str) -> bool {
    match value.strip_suffix("px") {
        Some(digits) => is_number(digits),
        None => false,
    }
}

/// The first two parts of Mermaid's `style.trim().split(/\s*:\s*/)`, found in one pass.
fn key_value(style: &str) -> (String, String) {
    let s = style.trim();
    let Some(colon) = s.find(':') else {
        return (s.to_string(), "undefined".to_string());
    };
    let rest = s[colon + 1..].trim_start();
    match rest.find(':') {
        Some(next) => (
            s[..colon].trim_end().to_string(),
            rest[..next].trim_end().to_string(),
        ),
        None => (s[..colon].trim_end().to_string(), rest.to_string()),
    }
}

/// The style of a point, as Mermaid reads it from the statements that follow it.
pub(crate) fn parse_styles(styles: &[String]) -> Result<PointStyle, Error> {
    let mut out = PointStyle::default();
    for style in styles {
        let (key, value) = key_value(style);
        match key.as_str() {
            "radius" => {
                if !is_number(&value) {
                    return Err(invalid(&key, &value, "number"));
                }
                out.radius = Some(js_to_number(&value));
            }
            "color" => {
                if !is_hex(&value) {
                    return Err(invalid(&key, &value, "hex code"));
                }
                out.color = Some(value);
            }
            "stroke-color" => {
                if !is_hex(&value) {
                    return Err(invalid(&key, &value, "hex code"));
                }
                out.stroke_color = Some(value);
            }
            "stroke-width" => {
                if !is_pixels(&value) {
                    return Err(invalid(&key, &value, "number of pixels (eg. 10px)"));
                }
                out.stroke_width = Some(value);
            }
            _ => {
                return Err(Error::new(
                    ErrorCode::Semantic,
                    format!("style named {key} is not supported."),
                ));
            }
        }
    }
    Ok(out)
}

/// A text with the spaces around it taken away.
fn trimmed(label: &QuadrantText) -> QuadrantText {
    QuadrantText {
        text: label.text.trim().to_string(),
        markdown: label.markdown,
    }
}

impl QuadrantDb {
    pub(crate) fn set_quadrant_text(&mut self, quadrant: usize, label: &QuadrantText) {
        self.quadrants[quadrant] = Some(trimmed(label));
    }

    pub(crate) fn set_x_axis_left_text(&mut self, label: &QuadrantText) {
        self.x_axis_left = Some(trimmed(label));
    }

    pub(crate) fn set_x_axis_right_text(&mut self, label: &QuadrantText) {
        self.x_axis_right = Some(trimmed(label));
    }

    pub(crate) fn set_y_axis_bottom_text(&mut self, label: &QuadrantText) {
        self.y_axis_bottom = Some(trimmed(label));
    }

    pub(crate) fn set_y_axis_top_text(&mut self, label: &QuadrantText) {
        self.y_axis_top = Some(trimmed(label));
    }

    pub(crate) fn add_point(
        &mut self,
        label: &QuadrantText,
        class_name: &str,
        x: &str,
        y: &str,
        styles: &[String],
    ) -> Result<(), Error> {
        let style = parse_styles(styles)?;
        self.points.push(QuadrantPoint {
            x: js_to_number(x),
            y: js_to_number(y),
            text: trimmed(label),
            class_name: class_name.to_string(),
            style,
        });
        Ok(())
    }

    pub(crate) fn add_class(&mut self, class_name: &str, styles: &[String]) -> Result<(), Error> {
        let style = parse_styles(styles)?;
        self.classes.set(class_name, style);
        Ok(())
    }

    pub(crate) fn set_diagram_title(&mut self, title: &str) {
        self.title = Some(title.to_string());
    }

    pub(crate) fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(text.trim_start().to_string());
    }

    pub(crate) fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(collapse_lines(text));
    }
}

/// Takes the space after a line break away, as Mermaid does with an accessible description.
fn collapse_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let Some(at) = rest.find('\n') else {
            out.push_str(rest);
            break;
        };
        let (before, after) = rest.split_at(at);
        out.push_str(before);
        out.push('\n');
        rest = after.trim_start();
    }
    out
}
