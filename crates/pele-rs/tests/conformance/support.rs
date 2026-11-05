//! Replay the callbacks and options observed in official Pele tests.
use pele_rs::{ErrorCode, MathMarkup, RenderOptions, Style, TextMeasurer};
use serde_json::{Value, json};

fn config_json(value: &Value) -> String {
    match value {
        Value::Object(map) if map.len() == 1 && map.contains_key("$number") => {
            match map["$number"].as_str() {
                Some("Infinity") => "1e400".into(),
                Some("-Infinity") => "-1e400".into(),
                _ => "null".into(),
            }
        }
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!("{}:{}", serde_json::to_string(k).unwrap(), config_json(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Array(values) => format!(
            "[{}]",
            values.iter().map(config_json).collect::<Vec<_>>().join(",")
        ),
        value => value.to_string(),
    }
}
fn style(bits: u64) -> Style {
    [
        Style::BOLD,
        Style::ITALIC,
        Style::MONO,
        Style::SUB,
        Style::SUP,
        Style::UNDERLINE,
    ]
    .into_iter()
    .enumerate()
    .fold(
        Style::NONE,
        |s, (i, flag)| if bits & (1 << i) != 0 { s | flag } else { s },
    )
}
struct RecordedWidths(Vec<Value>);
impl TextMeasurer for RecordedWidths {
    fn width(&self, text: &str, size: f64, emphasis: Style) -> f64 {
        self.0
            .iter()
            .find(|v| {
                v[0].as_str() == Some(text)
                    && v[1].as_f64() == Some(size)
                    && style(v[2].as_u64().unwrap()) == emphasis
            })
            .and_then(|v| v[3].as_f64())
            .unwrap_or(f64::NAN)
    }
}
pub fn render(entry: &Value) -> Value {
    let v = &entry["options"];
    let mut options = RenderOptions::default();
    // Pele has no nesting cap. Use an explicit native limit for its deep stress suites;
    // the safer native default is independently exercised by tests/options.rs.
    options.limits.max_depth = 30_000;
    macro_rules! number {
        ($field:ident, $key:literal) => {
            options.$field = v[$key].as_f64();
        };
    }
    macro_rules! string {
        ($field:ident, $key:literal) => {
            options.$field = v[$key].as_str().map(str::to_string);
        };
    }
    number!(font_size, "fontSize");
    number!(max_width, "maxWidth");
    number!(padding, "padding");
    number!(direction_breakpoint, "directionBreakpoint");
    number!(now, "now");
    string!(font_family, "fontFamily");
    string!(font_family_mono, "fontFamilyMono");
    string!(link_rel, "linkRel");
    if let Some(parts) = v["monospace"].as_array() {
        options.monospace = parts
            .iter()
            .filter_map(|v| match v.as_str()? {
                "members" => Some(pele_rs::MonospacePart::Members),
                "attributes" => Some(pele_rs::MonospacePart::Attributes),
                "files" => Some(pele_rs::MonospacePart::Files),
                "messages" => Some(pele_rs::MonospacePart::Messages),
                _ => None,
            })
            .collect();
    }
    if let Some(s) = v["idPrefix"].as_str() {
        options.id_prefix = Some(s.to_string());
    }
    if let Some(b) = v["responsive"].as_bool() {
        options.responsive = Some(b);
    }
    if let Some(b) = v["autoDirection"].as_bool() {
        options.auto_direction = b;
    }
    if let Some(b) = v["links"].as_bool() {
        options.links = b;
    }
    if let Some(b) = v["images"].as_bool() {
        options.images = b;
    }
    for (key, field) in [
        ("limit", &mut options.limits.source),
        ("outputLimit", &mut options.limits.output),
        ("maxEdges", &mut options.limits.max_edges),
    ] {
        if v[key]["$number"] == "Infinity" {
            *field = usize::MAX;
        } else if let Some(n) = v[key].as_u64() {
            *field = n as usize;
        }
    }
    for (key, field) in [
        ("linkSchemes", &mut options.link_schemes),
        ("imageSchemes", &mut options.image_schemes),
    ] {
        if let Some(a) = v[key].as_array() {
            *field = Some(
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect(),
            );
        }
    }
    if !v["config"].is_null() {
        options.config = Some(config_json(&v["config"]));
    }
    if let Some(widths) = entry["widths"].as_array() {
        options.measurer = Some(Box::new(RecordedWidths(widths.clone())));
    }
    if let Some(icons) = entry["icons"].as_array() {
        let icons = icons.clone();
        options.icons = Some(Box::new(move |name| {
            icons
                .iter()
                .find(|v| v[0].as_str() == Some(name))
                .and_then(|v| v[1].as_str())
                .map(str::to_string)
        }));
    }
    if let Some(math) = entry["math"].as_array() {
        let math = math.clone();
        options.math = Some(Box::new(move |tex, size| {
            let found = math
                .iter()
                .find(|v| v[0].as_str() == Some(tex) && v[1].as_f64() == Some(size))?;
            let m = &found[2];
            let width = m["width"].as_f64()?;
            let height = m["height"].as_f64()?;
            if let Some(svg) = m["svg"].as_str() {
                Some(MathMarkup::Svg {
                    svg: svg.to_string(),
                    width,
                    height,
                })
            } else {
                Some(MathMarkup::Html {
                    html: m["html"].as_str()?.to_string(),
                    width,
                    height,
                })
            }
        }));
    }
    let source = entry["source"].as_str().unwrap();
    let rendered = if entry["parseOptions"].is_object() {
        let p = &entry["parseOptions"];
        let mut parse_options = pele_rs::ParseOptions::default();
        parse_options.limits.max_depth = options.limits.max_depth;
        parse_options.config = p.get("config").map(config_json);
        if p["limit"]["$number"] == "Infinity" {
            parse_options.limits.source = usize::MAX;
        } else if let Some(n) = p["limit"].as_u64() {
            parse_options.limits.source = n as usize;
        }
        if p["maxEdges"]["$number"] == "Infinity" {
            parse_options.limits.max_edges = usize::MAX;
        } else if let Some(n) = p["maxEdges"].as_u64() {
            parse_options.limits.max_edges = n as usize;
        }
        pele_rs::parse(source, &parse_options).and_then(|model| model.render(&options))
    } else {
        pele_rs::render(source, &options)
    };
    match rendered {
        Ok(result) => {
            json!({ "result": { "type": result.kind.as_str(), "svg": result.svg, "width": result.width, "height": result.height,
            "links": result.links.iter().map(|l| json!({ "id": l.id, "href": l.href, "internal": l.internal })).collect::<Vec<_>>() } })
        }
        Err(e) => {
            json!({ "error": { "code": match e.code { ErrorCode::Unsupported => "unsupported-diagram", ErrorCode::Syntax => "syntax", ErrorCode::Semantic => "semantic", ErrorCode::Limit => "limit" }, "message": e.message, "line": e.line, "column": e.column, "snippet": e.snippet } })
        }
    }
}

fn close(a: f64, b: f64) -> bool {
    a.is_finite() && b.is_finite() && (a - b).abs() <= 0.011_f64.max(1e-14 * a.abs().max(b.abs()))
}
fn tokens(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        if b[i].is_ascii_whitespace() || b[i] == b',' {
            i += 1;
            continue;
        }
        let start = i;
        if b[i].is_ascii_digit() || matches!(b[i], b'+' | b'-' | b'.') {
            i += 1;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            if matches!(b.get(i), Some(b'e' | b'E')) {
                i += 1;
                if matches!(b.get(i), Some(b'+' | b'-')) {
                    i += 1;
                }
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
        } else if b[i].is_ascii_alphabetic() {
            i += 1;
            while i < b.len() && b[i].is_ascii_alphabetic() {
                i += 1;
            }
        } else {
            i += s[i..].chars().next().unwrap().len_utf8();
        }
        out.push(&s[start..i]);
    }
    out
}
fn attribute_equal(name: &str, a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    if !matches!(
        name,
        "x" | "y"
            | "x1"
            | "y1"
            | "x2"
            | "y2"
            | "cx"
            | "cy"
            | "r"
            | "rx"
            | "ry"
            | "width"
            | "height"
            | "viewBox"
            | "d"
            | "points"
            | "transform"
            | "stroke-width"
            | "font-size"
            | "dx"
            | "dy"
            | "stroke-dasharray"
            | "stroke-dashoffset"
    ) {
        return false;
    }
    let (a, b) = (tokens(a), tokens(b));
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            *a == b
                || a.parse::<f64>()
                    .ok()
                    .zip(b.parse::<f64>().ok())
                    .is_some_and(|(a, b)| close(a, b))
        })
}
pub fn difference(expected: &Value, actual: &Value) -> Option<String> {
    if let Some(e) = expected.get("error") {
        return match actual.get("error") {
            None => Some("official rejected, Rust accepted".to_string()),
            Some(a) if e["code"] != a["code"] => {
                Some(format!("error code {} != {}", e["code"], a["code"]))
            }
            _ => None,
        };
    }
    if let Some(e) = actual.get("error") {
        return Some(format!("Rust rejected: {e}"));
    }
    let (e, a) = (&expected["result"], &actual["result"]);
    for key in ["type", "links"] {
        if e[key] != a[key] {
            return Some(format!("{key}: {} != {}", e[key], a[key]));
        }
    }
    for key in ["width", "height"] {
        if !e[key]
            .as_f64()
            .zip(a[key].as_f64())
            .is_some_and(|(e, a)| close(e, a))
        {
            return Some(format!("{key}: {} != {}", e[key], a[key]));
        }
    }
    let (Some(es), Some(as_)) = (e["svg"].as_str(), a["svg"].as_str()) else {
        return Some("missing SVG".into());
    };
    if es == as_ {
        return None;
    }
    let ep = match roxmltree::Document::parse(es) {
        Ok(v) => v,
        Err(err) => return Some(format!("official XML: {err}")),
    };
    let ap = match roxmltree::Document::parse(as_) {
        Ok(v) => v,
        Err(err) => return Some(format!("Rust XML: {err}")),
    };
    let en: Vec<_> = ep
        .descendants()
        .filter(|n| n.is_element() || n.is_text())
        .collect();
    let an: Vec<_> = ap
        .descendants()
        .filter(|n| n.is_element() || n.is_text())
        .collect();
    for (i, (e, a)) in en.iter().zip(&an).enumerate() {
        if e.tag_name() != a.tag_name() || e.is_text() != a.is_text() {
            return Some(format!(
                "node {i}: {:?} != {:?}",
                e.tag_name(),
                a.tag_name()
            ));
        }
        if e.is_text() && e.text() != a.text() {
            return Some(format!("node {i} text: {:?} != {:?}", e.text(), a.text()));
        }
        if e.attributes().len() != a.attributes().len() {
            return Some(format!(
                "node {i} attribute count {} != {}",
                e.attributes().len(),
                a.attributes().len()
            ));
        }
        for attr in e.attributes() {
            let Some(other) = a
                .attributes()
                .find(|other| other.name() == attr.name() && other.namespace() == attr.namespace())
                .map(|other| other.value())
            else {
                return Some(format!("node {i} missing @{}", attr.name()));
            };
            if !attribute_equal(attr.name(), attr.value(), other) {
                return Some(format!(
                    "node {i} @{}: {:?} != {:?}",
                    attr.name(),
                    attr.value(),
                    other
                ));
            }
        }
    }
    if en.len() != an.len() {
        return Some(format!("node count {} != {}", en.len(), an.len()));
    }
    None
}
