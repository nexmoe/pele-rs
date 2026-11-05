//! The format of the values of a treemap.
//! Ported from pele's `diagrams/treemap/format.ts`.
//!
//! The type reads no configuration, so only Mermaid's default is ever used: a number with
//! thousands separators.

use crate::util::js_number;

/// Formats a value the way Mermaid's default `,` format does.
pub(crate) fn format_value(value: f64) -> String {
    let abs = value.abs();
    // The value is written with twelve significant digits, as d3-format does.
    let body = js_number(significant(abs, 12));
    let sign = if value < 0.0 { "-" } else { "" };
    if body.contains('e') {
        return format!("{sign}{body}");
    }
    let (whole, rest) = match body.find('.') {
        Some(at) => (&body[..at], &body[at..]),
        None => (&body[..], ""),
    };
    format!("{sign}{}{rest}", group(whole))
}

/// The number, rounded to the given count of significant digits.
fn significant(value: f64, digits: usize) -> f64 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    format!("{:.*e}", digits.saturating_sub(1), value)
        .parse()
        .unwrap_or(value)
}

/// Puts a comma every three digits of a number.
fn group(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    let count = digits.len();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (count - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_number_with_thousands_separators() {
        assert_eq!(format_value(0.0), "0");
        assert_eq!(format_value(10.0), "10");
        assert_eq!(format_value(700000.0), "700,000");
        assert_eq!(format_value(1234567.0), "1,234,567");
        assert_eq!(format_value(123.45), "123.45");
        assert_eq!(format_value(0.35), "0.35");
    }

    #[test]
    fn writes_a_negative_number() {
        assert_eq!(format_value(-1000.0), "-1,000");
    }
}
