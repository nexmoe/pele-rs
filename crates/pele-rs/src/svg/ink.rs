//! Contrast selection from Pele 1.13's svg/ink.ts (MIT, Obsidian).
use super::ResolvedStyle;

pub(crate) const ON_LIGHT: &str = "var(--pele-fg-on-light,#1f1f1f)";
pub(crate) const ON_DARK: &str = "var(--pele-fg-on-dark,#fff)";
const DARK: &str = "black blue blueviolet brown crimson darkblue darkcyan darkgreen darkmagenta darkolivegreen darkorchid darkred darkslateblue darkslategray darkviolet dimgray firebrick forestgreen green indigo maroon mediumblue mediumslateblue mediumvioletred midnightblue navy olive purple rebeccapurple royalblue saddlebrown seagreen sienna slateblue steelblue teal";
const LIGHT: &str = "aliceblue antiquewhite aqua aquamarine azure beige bisque blanchedalmond burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk cyan darkgoldenrod darkgray darkkhaki darkorange darksalmon darkseagreen darkturquoise deeppink deepskyblue dodgerblue floralwhite fuchsia gainsboro ghostwhite gold goldenrod gray greenyellow honeydew hotpink indianred ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightpink lightsalmon lightseagreen lightskyblue lightslategray lightsteelblue lightyellow lime limegreen linen magenta mediumaquamarine mediumorchid mediumpurple mediumseagreen mediumspringgreen mediumturquoise mintcream mistyrose moccasin navajowhite oldlace olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue red rosybrown salmon sandybrown seashell silver skyblue slategray snow springgreen tan thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen";

fn named(names: &str, value: &str) -> bool {
    names.split(' ').any(|n| n == value)
}
fn hex(value: &str) -> bool {
    value.strip_prefix('#').is_some_and(|s| {
        matches!(s.len(), 3 | 4 | 6 | 8) && s.bytes().all(|b| b.is_ascii_hexdigit())
    })
}
pub(crate) fn plain_color(raw: &str) -> bool {
    let value = raw.trim().to_ascii_lowercase();
    let name = value.replacen("grey", "gray", 1);
    hex(&value)
        || named(DARK, &name)
        || named(LIGHT, &name)
        || matches!(value.as_str(), "transparent" | "currentcolor")
}
fn linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
fn luminance(r: f64, g: f64, b: f64) -> f64 {
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

// CSS's decimal grammar excludes trailing decimal points, infinity and nonnumeric units.
fn number(raw: &str) -> Option<(f64, &str)> {
    let bytes = raw.as_bytes();
    let mut i = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let start = i;
    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    let digits = i - start;
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return None;
        }
    } else if digits == 0 {
        return None;
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        i += 1;
        i += usize::from(matches!(bytes.get(i), Some(b'+' | b'-')));
        let start = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    Some((raw[..i].parse().ok()?, &raw[i..]))
}
fn amount(raw: &str, full: f64) -> f64 {
    match number(raw) {
        Some((n, unit @ ("" | "%"))) => {
            (n / if unit == "%" { 100.0 } else { full }).clamp(0.0, 1.0)
        }
        _ => f64::NAN,
    }
}
fn hue(raw: &str) -> f64 {
    match number(raw) {
        Some((n, "" | "deg")) => n,
        Some((n, "turn")) => n * 360.0,
        Some((n, "rad")) => n * 180.0 / std::f64::consts::PI,
        Some((n, "grad")) => n * 0.9,
        _ => f64::NAN,
    }
}
fn arguments(body: &str, hsl: bool) -> Option<Vec<&str>> {
    if body.contains(',') {
        let list: Vec<_> = body.split(',').map(str::trim).collect();
        if !(3..=4).contains(&list.len()) {
            return None;
        }
        let percent = list[usize::from(hsl)..3]
            .iter()
            .filter(|s| s.ends_with('%'))
            .count();
        if percent == if hsl { 2 } else { 3 } || (!hsl && percent == 0) {
            Some(list)
        } else {
            None
        }
    } else {
        let mut parts = body.split('/');
        let mut list: Vec<_> = parts.next()?.split_whitespace().collect();
        if list.len() != 3 {
            return None;
        }
        if let Some(alpha) = parts.next() {
            list.push(alpha.trim());
        }
        if parts.next().is_some() {
            return None;
        }
        Some(
            list.into_iter()
                .map(|s| if s == "none" { "0" } else { s })
                .collect(),
        )
    }
}
fn color(raw: &str) -> Option<(f64, f64)> {
    let value = raw.trim().to_ascii_lowercase();
    let name = value.replacen("grey", "gray", 1);
    if named(DARK, &name) {
        return Some((0.0, 1.0));
    }
    if named(LIGHT, &name) {
        return Some((1.0, 1.0));
    }
    if hex(&value) {
        let digits = if value.len() < 6 {
            value[1..].chars().flat_map(|c| [c, c]).collect::<String>()
        } else {
            value[1..].to_string()
        };
        let n =
            |i: usize| u8::from_str_radix(&digits[2 * i..2 * i + 2], 16).unwrap() as f64 / 255.0;
        return Some((
            luminance(n(0), n(1), n(2)),
            if digits.len() == 8 { n(3) } else { 1.0 },
        ));
    }
    let (function, body) = value.split_once('(')?;
    if !matches!(function, "rgb" | "rgba" | "hsl" | "hsla") {
        return None;
    }
    let list = arguments(body.strip_suffix(')')?, function.starts_with("hsl"))?;
    let alpha = if list.len() == 4 {
        amount(list[3], 1.0)
    } else {
        1.0
    };
    let rgb = if function.starts_with("hsl") {
        let h = ((hue(list[0]) % 360.0 + 360.0) % 360.0) / 30.0;
        let s = amount(list[1], 100.0);
        let l = amount(list[2], 100.0);
        let a = s * l.min(1.0 - l);
        [0.0, 8.0, 4.0].map(|k| {
            let t = (k + h) % 12.0;
            if t.is_nan() {
                f64::NAN
            } else {
                l - a * (t - 3.0).min(9.0 - t).clamp(-1.0, 1.0)
            }
        })
    } else {
        [0, 1, 2].map(|i| amount(list[i], 255.0))
    };
    let lum = luminance(rgb[0], rgb[1], rgb[2]);
    if (lum + alpha).is_nan() {
        None
    } else {
        Some((lum, alpha))
    }
}
fn declarations(shape: &str) -> impl Iterator<Item = (&str, &str)> {
    shape
        .split(['"', ';'])
        .filter_map(|part| part.split_once(':'))
}
fn fill(shape: &str) -> Option<&str> {
    declarations(shape).find_map(|(key, value)| (key == "fill").then_some(value))
}
fn opacity(shape: &str) -> f64 {
    declarations(shape)
        .filter(|(key, _)| matches!(*key, "opacity" | "fill-opacity"))
        .fold(1.0, |p, (_, v)| p * amount(v.trim(), 1.0))
}
pub(crate) fn ink(shape: &str, text: &str) -> Option<&'static str> {
    if fill(text).is_some() {
        return None;
    }
    let (lum, alpha) = color(fill(shape)?)?;
    if !matches!(
        (alpha * opacity(shape)).partial_cmp(&0.5),
        Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
    ) {
        return None;
    }
    let even = (1.05 * (luminance(31.0 / 255.0, 31.0 / 255.0, 31.0 / 255.0) + 0.05)).sqrt() - 0.05;
    Some(if lum > even { ON_LIGHT } else { ON_DARK })
}
pub(crate) fn paints(shape: &str) -> bool {
    let Some(fill) = fill(shape) else {
        return false;
    };
    if matches!(
        fill.trim().to_ascii_lowercase().as_str(),
        "none" | "transparent"
    ) {
        return false;
    }
    let scale = opacity(shape);
    color(fill).map_or(1.0, |(_, a)| a) * if scale.is_nan() { 1.0 } else { scale } > 0.0
}
pub(crate) fn ink_fill(style: &ResolvedStyle, fallback: Option<&str>) -> String {
    ink(&style.shape, &style.text)
        .or(fallback)
        .map_or_else(String::new, |c| format!(" fill=\"{c}\""))
}
