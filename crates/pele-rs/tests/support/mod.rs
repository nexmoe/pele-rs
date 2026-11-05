//! Shared by the integration tests: a check that output is inert, hostile payloads, and a
//! mutator for corpus inputs. Ported from pele's `tests/support/inert.ts`, `xml.ts` and
//! `corpus.ts`.
#![allow(dead_code, reason = "each test file uses the parts it needs")]

use std::collections::HashSet;

/// pele-rs's output is inserted into pages as markup, so it must be inert whatever the diagram
/// says: only drawing elements, only presentation attributes, no script-capable URLs, no way to
/// load a stylesheet. A new diagram type that needs another element or attribute adds it here,
/// with care. The `defs`, `linearGradient` and `stop` elements are the gradient of the bands of
/// a sankey chart.
const ELEMENTS: &[&str] = &[
    "svg",
    "g",
    "rect",
    "circle",
    "ellipse",
    "polygon",
    "path",
    "text",
    "tspan",
    "a",
    "title",
    "desc",
    "image",
    "defs",
    "linearGradient",
    "stop",
];
const ATTRIBUTES: &[&str] = &[
    "xmlns",
    "class",
    "viewBox",
    "width",
    "height",
    "style",
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "fill",
    "fill-opacity",
    "opacity",
    "stroke",
    "stroke-width",
    "stroke-dasharray",
    "stroke-linecap",
    "stroke-linejoin",
    "role",
    "aria-roledescription",
    "aria-labelledby",
    "aria-describedby",
    "id",
    "data-id",
    "data-icon",
    "data-href",
    "transform",
    "x",
    "y",
    "rx",
    "ry",
    "r",
    "cx",
    "cy",
    "d",
    "points",
    "text-anchor",
    "xml:space",
    "href",
    "target",
    "rel",
    "preserveAspectRatio",
    "baseline-shift",
    "text-decoration",
    "gradientUnits",
    "x1",
    "x2",
    "offset",
    "stop-color",
    "overflow",
    "color",
];
const STYLE_PROPERTIES: &[&str] = &[
    "fill",
    "fill-opacity",
    "stroke",
    "stroke-width",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-opacity",
    "opacity",
    "rx",
    "ry",
    "font-size",
    "font-family",
    "font-weight",
    "font-style",
    "text-decoration",
    "letter-spacing",
    "word-spacing",
    "max-width",
];
const ROOT_STYLES: &[&str] = &[
    "height:auto",
    "color:var(--_fg)",
    "text-transform:none",
    "direction:ltr",
];
const NUMERIC: &[&str] = &[
    "x", "y", "width", "height", "r", "cx", "cy", "rx", "ry", "x1", "x2", "offset",
];

/// Text an attacker might put anywhere a diagram accepts a string.
pub const PAYLOADS: &[&str] = &[
    "<script>alert(1)</script>",
    "\"><script>alert(1)</script>",
    "<img src=x onerror=alert(1)>",
    "<svg onload=alert(1)>",
    "<a href=\"javascript:alert(1)\">x</a>",
    "<foreignObject><iframe src=javascript:alert(1)></iframe></foreignObject>",
    "</text></g></svg><script>alert(1)</script>",
    "javascript:alert(1)",
    "JaVaScRiPt:alert(1)",
    " javascript:alert(1)",
    "jav&#x09;ascript:alert(1)",
    "data:text/html,<script>alert(1)</script>",
    "vbscript:msgbox(1)",
    "' onload='alert(1)",
    "\" onmouseover=\"alert(1)",
    "x\" onclick=\"alert(1)\" y=\"",
    "#60;script#62;alert(1)#60;/script#62;",
    "#106;avascript:alert(1)",
    "#34; onload=#34;alert(1)",
    "url#40;javascript:alert#40;1#41;#41;",
    "red#59;background:url#40;//evil.example/x#41;",
    "&lt;script&gt;alert(1)&lt;/script&gt;",
    "&#60;script&#62;",
    "url(javascript:alert(1))",
    "expression(alert(1))",
    "red;background:url(//evil.example/x)",
    "red\" onload=\"alert(1)",
    "]]><script>alert(1)</script>",
    "--><script>alert(1)</script>",
    "\u{0}\u{1}\u{8}\u{b}\u{1f}",
    "__proto__",
    "constructor",
    "toString",
    "hasOwnProperty",
    "{{7*7}}",
    "${alert(1)}",
    "%0ajavascript:alert(1)",
    "file:///etc/passwd",
    "//evil.example/share",
    "\\\\evil.example\\share",
    "smb://evil.example/share",
    "obsidian://open?vault=x",
    "\\\"><script>",
];

pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '.' | '-')
}

/// Whether every `&` in `text` starts a character reference XML knows.
fn references_ok(text: &str) -> bool {
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        rest = &rest[at + 1..];
        let Some(end) = rest.find(';') else {
            return false;
        };
        let name = &rest[..end];
        let ok = matches!(name, "amp" | "lt" | "gt" | "quot" | "apos")
            || name
                .strip_prefix("#x")
                .is_some_and(|h| !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit()))
            || name
                .strip_prefix('#')
                .is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()));
        if !ok {
            return false;
        }
    }
    true
}

/// An attribute value as a browser reads it, with character references turned into characters.
fn decode(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let end = rest.find(';').unwrap_or(0);
        let name = &rest[1..end.max(1)];
        let c = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => name
                .strip_prefix("#x")
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                .map(|n: u32| char::from_u32(n.min(0x10ffff)).unwrap_or('\u{fffd}')),
        };
        match c {
            Some(c) if end > 0 => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Parses the markup, checking that it is well-formed XML: balanced tags, quoted attributes,
/// escaped text, no attribute twice.
pub fn elements(xml: &str) -> Result<Vec<Element>, String> {
    let mut out = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut rest = xml;
    loop {
        let lt = rest.find('<').unwrap_or(rest.len());
        let text = &rest[..lt];
        if text.contains('>') {
            return Err(format!(
                "unescaped \">\" in text near {:?}",
                &text[..text.len().min(40)]
            ));
        }
        if !references_ok(text) {
            return Err(format!(
                "unescaped \"&\" in text near {:?}",
                &text[..text.len().min(40)]
            ));
        }
        if lt == rest.len() {
            break;
        }
        rest = &rest[lt + 1..];
        let closing = rest.starts_with('/');
        if closing {
            rest = &rest[1..];
        }
        let name_len = rest.find(|c: char| !is_name_char(c)).unwrap_or(rest.len());
        if name_len == 0 || !rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
            return Err(format!(
                "malformed tag near {:?}",
                &rest[..rest.len().min(60)]
            ));
        }
        let name = rest[..name_len].to_string();
        rest = &rest[name_len..];
        let mut attrs = Vec::new();
        let mut seen = HashSet::new();
        loop {
            let trimmed = rest.trim_start();
            let had_space = trimmed.len() < rest.len();
            rest = trimmed;
            if let Some(after) = rest.strip_prefix("/>") {
                if closing {
                    return Err(format!("malformed closing tag </{name}/>"));
                }
                rest = after;
                break;
            }
            if let Some(after) = rest.strip_prefix('>') {
                rest = after;
                if closing {
                    let open = stack.pop();
                    if open.as_deref() != Some(name.as_str()) {
                        return Err(format!("</{name}> closes <{}>", open.unwrap_or_default()));
                    }
                } else {
                    stack.push(name.clone());
                }
                break;
            }
            if !had_space || closing {
                return Err(format!("malformed attributes in <{name}>"));
            }
            let len = rest.find(|c: char| !is_name_char(c)).unwrap_or(rest.len());
            let attr = &rest[..len];
            let Some(value) = rest[len..].strip_prefix("=\"") else {
                return Err(format!("unquoted attribute {attr} in <{name}>"));
            };
            let end = value
                .find(['"', '<'])
                .ok_or(format!("unclosed attribute in <{name}>"))?;
            if value.as_bytes()[end] == b'<' {
                return Err(format!("\"<\" in attribute {attr} of <{name}>"));
            }
            let raw = &value[..end];
            if !references_ok(raw) {
                return Err(format!("unescaped \"&\" in attributes of <{name}>"));
            }
            if !seen.insert(attr.to_string()) {
                return Err(format!("<{name}> has {attr} twice"));
            }
            attrs.push((attr.to_string(), decode(raw)));
            rest = &value[end + 1..];
        }
        if !closing {
            out.push(Element { name, attrs });
        }
    }
    if let Some(open) = stack.pop() {
        return Err(format!("unclosed <{open}>"));
    }
    Ok(out)
}

fn is_number(value: &str) -> bool {
    if value == "100%" {
        return true;
    }
    let v = value.strip_prefix('-').unwrap_or(value);
    let (mantissa, exp) = match v.split_once('e') {
        Some((m, e)) => (m, Some(e.strip_prefix(['-', '+']).unwrap_or(e))),
        None => (v, None),
    };
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let mantissa_ok = match mantissa.split_once('.') {
        Some((i, f)) => digits(i) && digits(f),
        None => digits(mantissa),
    };
    mantissa_ok && exp.is_none_or(digits)
}

fn lower(value: &str) -> String {
    value.to_ascii_lowercase()
}

/// `url\s*\(` that points at something outside the document. A fragment reference points at an
/// element of the same document, which can neither load nor run anything.
fn has_url(value: &str) -> bool {
    let v = lower(value);
    v.match_indices("url").any(|(i, _)| {
        let rest = v[i + 3..].trim_start();
        let Some(rest) = rest.strip_prefix('(') else {
            return false;
        };
        let end = rest.find(')').unwrap_or(rest.len());
        !is_fragment(&rest[..end])
    })
}

/// A local reference: `#` and an id.
fn is_fragment(target: &str) -> bool {
    let Some(id) = target.trim().strip_prefix('#') else {
        return false;
    };
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn unsafe_style(value: &str) -> bool {
    let v = lower(value);
    has_url(value)
        || v.contains("expression")
        || v.contains("@import")
        || v.contains("javascript:")
        || value.contains(['<', '>', '{', '}', '\\'])
}

fn unsafe_paint(value: &str) -> bool {
    has_url(value) || lower(value).contains("javascript:")
}

/// `^[\s\u0000-\u001f]*(?:javascript|vbscript|data)\s*:`, ignoring case.
fn script_url(value: &str) -> bool {
    let v = lower(value.trim_start_matches(|c: char| c.is_whitespace() || c <= '\u{1f}'));
    ["javascript", "vbscript", "data"].iter().any(|scheme| {
        v.strip_prefix(scheme)
            .is_some_and(|rest| rest.trim_start().starts_with(':'))
    })
}

/// `^(?:(?:https?|mailto|tel):|(?![\\/]{2}|[a-z][a-z0-9+.-]*:))`, ignoring case.
fn allowed_url(value: &str) -> bool {
    let v = lower(value);
    if ["http:", "https:", "mailto:", "tel:"]
        .iter()
        .any(|p| v.starts_with(p))
    {
        return true;
    }
    let b = v.as_bytes();
    if b.len() >= 2 && matches!(b[0], b'\\' | b'/') && matches!(b[1], b'\\' | b'/') {
        return false;
    }
    let scheme = v
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')))
        .unwrap_or(v.len());
    !(scheme > 0 && b[0].is_ascii_alphabetic() && v[scheme..].starts_with(':'))
}

fn root_font(decl: &str) -> bool {
    decl.strip_prefix("font:")
        .and_then(|d| d.strip_suffix("px sans-serif"))
        .is_some_and(is_number)
}

/// The first thing about the markup that is not inert, or `None` when there is none.
pub fn inert_problem(svg: &str) -> Option<String> {
    let elements = match elements(svg) {
        Ok(elements) => elements,
        Err(problem) => return Some(problem),
    };
    for el in &elements {
        if !ELEMENTS.contains(&el.name.as_str()) {
            return Some(format!("element <{}>", el.name));
        }
        for (name, value) in &el.attrs {
            let name = name.as_str();
            if !ATTRIBUTES.contains(&name) {
                return Some(format!("attribute {name} on <{}>", el.name));
            }
            if NUMERIC.contains(&name) && !is_number(value) {
                return Some(format!("{name}=\"{value}\" is not a number"));
            }
            match name {
                "href" => {
                    if el.name != "a" && el.name != "image" {
                        return Some(format!("href on <{}>", el.name));
                    }
                    if script_url(value) {
                        return Some(format!("href=\"{value}\" can run script"));
                    }
                    let seen: String = value
                        .chars()
                        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
                        .collect();
                    let seen = seen.trim_start_matches(|c: char| c <= ' ');
                    if !allowed_url(seen) {
                        return Some(format!("href=\"{value}\" has a scheme that is not allowed"));
                    }
                }
                "target" => {
                    if !matches!(value.as_str(), "_self" | "_blank" | "_parent" | "_top") {
                        return Some(format!("target=\"{value}\""));
                    }
                }
                "style" => {
                    if unsafe_style(value) {
                        return Some(format!("style=\"{value}\""));
                    }
                    for decl in value.split(';') {
                        let trimmed = decl.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        let prop = decl[..decl.find(':').unwrap_or(0)].trim();
                        let root = el.name == "svg"
                            && (ROOT_STYLES.contains(&trimmed) || root_font(trimmed));
                        if !root && !STYLE_PROPERTIES.contains(&prop) && !prop.starts_with("--_") {
                            return Some(format!("style property {prop}"));
                        }
                    }
                }
                "fill" | "stroke" | "font-family" | "rx" if unsafe_paint(value) => {
                    return Some(format!("{name}=\"{value}\""));
                }
                _ => {}
            }
        }
    }
    None
}

#[track_caller]
pub fn assert_inert(svg: &str, src: &str) {
    if let Some(problem) = inert_problem(svg) {
        panic!("{src:?}: {problem}");
    }
}

/// A small deterministic random number generator, as in pele's tests.
pub struct Random(u32);

impl Random {
    pub fn new(seed: u32) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(self.0) / 4_294_967_296.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() * n as f64) as usize
    }

    fn pick<'a>(&mut self, list: &[&'a str]) -> &'a str {
        list[self.below(list.len())]
    }
}

fn char_slice(s: &str, from: usize, to: usize) -> String {
    s.chars().skip(from).take(to.saturating_sub(from)).collect()
}

/// Edits a random corpus entry one to three times: inserting a fragment or a piece of another
/// entry, deleting or repeating a few characters, or replacing one with a fragment.
pub fn mutate(corpus: &[&str], fragments: &[&str], rnd: &mut Random) -> String {
    let mut s = rnd.pick(corpus).to_string();
    let edits = 1 + rnd.below(3);
    for _ in 0..edits {
        let len = s.chars().count();
        let pos = rnd.below(len + 1);
        let r = rnd.next();
        let head = char_slice(&s, 0, pos);
        s = if r < 0.35 {
            head + rnd.pick(fragments) + &char_slice(&s, pos, len)
        } else if r < 0.6 {
            let n = 1 + rnd.below(3);
            head + &char_slice(&s, pos + n, len)
        } else if r < 0.75 {
            let other = rnd.pick(corpus);
            let from = rnd.below(other.chars().count());
            let n = rnd.below(20);
            head + &char_slice(other, from, from + n) + &char_slice(&s, pos, len)
        } else if r < 0.9 {
            let n = rnd.below(6);
            head + &char_slice(&s, pos, pos + n) + &char_slice(&s, pos, len)
        } else {
            head + rnd.pick(fragments) + &char_slice(&s, pos + 1, len)
        };
    }
    s
}

#[test]
fn finds_problems() {
    assert_eq!(inert_problem("<svg><g/></svg>"), None);
    assert!(inert_problem("<svg><script/></svg>").is_some());
    assert!(inert_problem("<svg onload=\"x\"/>").is_some());
    assert!(inert_problem("<svg><g></svg>").is_some());
    assert!(inert_problem("<svg>a & b</svg>").is_some());
    assert!(inert_problem("<svg><rect x=\"1px\"/></svg>").is_some());
    assert!(inert_problem("<svg><a href=\" javascript:x\"/></svg>").is_some());
    assert!(inert_problem("<svg><a href=\"//evil\"/></svg>").is_some());
    assert_eq!(inert_problem("<svg><a href=\"https://x\"/></svg>"), None);
    assert!(inert_problem("<svg><g style=\"background:url (x)\"/></svg>").is_some());
    // A fragment reference points at an element of the same document, which is inert.
    assert_eq!(inert_problem("<svg><g fill=\"url(#x)\"/></svg>"), None);
    assert!(inert_problem("<svg><g fill=\"url(https://evil.example/x)\"/></svg>").is_some());
    assert!(inert_problem("<svg><g fill=\"url(#x y)\"/></svg>").is_some());
    assert_eq!(
        inert_problem("<svg style=\"font:16px sans-serif;--_x:1\"><text>&amp;&#35;</text></svg>"),
        None
    );
}
