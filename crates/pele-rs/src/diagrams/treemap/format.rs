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

/// The subset of d3 format specifiers supported by Pele.
pub(crate) fn format_spec(value: f64, spec: &str) -> String {
    let spec = if spec == "$0,0" { "$," } else { spec };
    let chars: Vec<char> = spec.chars().collect();
    let mut i = 0;
    if chars.get(1).is_some_and(|c| "<>=^".contains(*c)) {
        i = 2;
    } else if chars.first().is_some_and(|c| "<>=^".contains(*c)) {
        i = 1;
    }
    let plus = chars.get(i) == Some(&'+');
    if chars.get(i).is_some_and(|c| "-+( ".contains(*c)) {
        i += 1;
    }
    let dollar = chars.get(i) == Some(&'$');
    if dollar {
        i += 1;
    }
    while chars.get(i).is_some_and(char::is_ascii_digit) {
        i += 1;
    }
    let comma = chars.get(i) == Some(&',');
    if comma {
        i += 1;
    }
    let mut precision = None;
    if chars.get(i) == Some(&'.') {
        i += 1;
        let from = i;
        let mut n = 0usize;
        while chars.get(i).is_some_and(char::is_ascii_digit) {
            n = (n * 10 + chars[i].to_digit(10).unwrap() as usize).min(20);
            i += 1;
        }
        if i == from {
            return format_value(value);
        }
        precision = Some(n);
    }
    let trim = chars.get(i) == Some(&'~');
    if trim {
        i += 1;
    }
    let kind = chars.get(i).copied().unwrap_or('\0');
    if kind != '\0' {
        if !kind.is_ascii_alphabetic() && kind != '%' {
            return format_value(value);
        }
        i += 1;
    }
    if i != chars.len() {
        return format_value(value);
    }
    let abs = value.abs();
    let mut suffix = "";
    let mut body = match kind {
        'f' => fixed(abs, precision.unwrap_or(6)),
        '%' => {
            suffix = "%";
            fixed(abs * 100.0, precision.unwrap_or(6))
        }
        'd' => fixed(crate::util::round(abs), 0),
        'e' => {
            let body = format!("{:.*e}", precision.unwrap_or(6), abs);
            let (mantissa, exp) = body.split_once('e').unwrap();
            let exp: i32 = exp.parse().unwrap();
            format!("{mantissa}e{}{exp}", if exp >= 0 { "+" } else { "" })
        }
        _ => js_number(significant(abs, precision.unwrap_or(12).max(1))),
    };
    if trim && body.contains('.') && !body.contains('e') {
        while body.ends_with('0') {
            body.pop();
        }
        if body.ends_with('.') {
            body.pop();
        }
    }
    if comma && !body.contains('e') {
        body = match body.split_once('.') {
            Some((whole, rest)) => format!("{}.{rest}", group(whole)),
            None => group(&body),
        };
    }
    format!(
        "{}{}{body}{suffix}",
        if value < 0.0 {
            "-"
        } else if plus {
            "+"
        } else {
            ""
        },
        if dollar { "$" } else { "" }
    )
}

// JavaScript toFixed rounds the exact binary value, with ties away from zero.
fn fixed(value: f64, places: usize) -> String {
    if !value.is_finite() || value >= 1e21 {
        return js_number(value);
    }
    let bits = value.to_bits();
    let e = ((bits >> 52) & 2047) as i32;
    let mantissa = if e == 0 {
        bits & ((1 << 52) - 1)
    } else {
        (bits & ((1 << 52) - 1)) | (1 << 52)
    } as u128;
    let exponent = if e == 0 { -1074 } else { e - 1023 - 52 } + places as i32;
    let scaled = mantissa * 5u128.pow(places as u32);
    let rounded = if exponent >= 0 {
        if scaled.checked_shl(exponent as u32).is_none()
            || (exponent as u32) < 128 && scaled > u128::MAX >> (exponent as u32)
        {
            return format!("{value:.places$}");
        }
        scaled << exponent
    } else {
        let shift = (-exponent) as u32;
        if shift >= 128 {
            0
        } else {
            (scaled >> shift) + u128::from(scaled & ((1u128 << shift) - 1) >= 1u128 << (shift - 1))
        }
    };
    let mut body = rounded.to_string();
    if places > 0 {
        if body.len() <= places {
            body = format!("{}{}", "0".repeat(places + 1 - body.len()), body);
        }
        body.insert(body.len() - places, '.');
    }
    body
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
