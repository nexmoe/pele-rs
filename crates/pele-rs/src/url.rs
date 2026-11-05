//! Link and image addresses from diagram text. Ported from pele's `util/url.ts`, which follows
//! `@braintree/sanitize-url` as Mermaid does, and then keeps only the schemes the host allows.

use crate::svg::esc;
use crate::text::entities::decode_entities;
use crate::util::{trim, trim_start};
use crate::{Link, RenderOptions};

const BLANK: &str = "about:blank";

fn is_ctrl(c: char) -> bool {
    matches!(c, '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}' | '\u{2000}'..='\u{200d}' | '\u{feff}')
}

/// `decodeURIComponent`, or the text as it is when it is not valid percent-encoding.
fn decode(uri: &str) -> String {
    if !uri.contains('%') {
        return uri.to_string();
    }
    let b = uri.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            // `from_str_radix` would take a sign, which `decodeURIComponent` does not.
            let hex = b
                .get(i + 1..i + 3)
                .filter(|h| h.iter().all(u8::is_ascii_hexdigit))
                .and_then(|h| std::str::from_utf8(h).ok());
            match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                Some(v) => {
                    out.push(v);
                    i += 3;
                    continue;
                }
                None => return uri.to_string(),
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| uri.to_string())
}

/// `&#(\w+)(^\w|;)?` replaced by `String.fromCharCode(digits)`.
fn html_entities(s: &str) -> (String, bool) {
    let mut out = String::with_capacity(s.len());
    let mut found = false;
    let mut rest = s;
    while let Some(at) = rest.find("&#") {
        let after = &rest[at + 2..];
        let len = after
            .find(|c: char| !crate::util::is_word(c))
            .unwrap_or(after.len());
        if len == 0 {
            out.push_str(&rest[..at + 2]);
            rest = after;
            continue;
        }
        found = true;
        out.push_str(&rest[..at]);
        let code = after[..len].parse::<u32>().map_or(0, |n| n & 0xffff);
        out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
        rest = &after[len..];
        if let Some(r) = rest.strip_prefix(';') {
            rest = r;
        }
    }
    out.push_str(rest);
    (out, found)
}

/// Removes `&newline;` and `&tab;`, ignoring case.
fn html_ctrl_entities(s: &str) -> (String, bool) {
    let lower = s.to_ascii_lowercase();
    let mut out = String::with_capacity(s.len());
    let mut found = false;
    let mut i = 0;
    while i < s.len() {
        if lower[i..].starts_with("&newline;") {
            i += 9;
            found = true;
        } else if lower[i..].starts_with("&tab;") {
            i += 5;
            found = true;
        } else {
            let c = s[i..].chars().next().unwrap_or(' ');
            out.push(c);
            i += c.len_utf8();
        }
    }
    (out, found)
}

/// Removes `(\\|%5[cC])((%(6[eE]|72|74))|[nrt])`.
fn ws_escapes(s: &str) -> (String, bool) {
    let mut out = String::with_capacity(s.len());
    let mut found = false;
    let mut i = 0;
    let lower = s.to_ascii_lowercase();
    while i < s.len() {
        let rest = &lower[i..];
        let lead = if rest.starts_with('\\') {
            1
        } else if rest.starts_with("%5c") {
            3
        } else {
            0
        };
        if lead > 0 {
            let tail = &rest[lead..];
            let skip =
                if tail.starts_with("%6e") || tail.starts_with("%72") || tail.starts_with("%74") {
                    3
                } else if tail.starts_with(['n', 'r', 't'])
                    && s[i + lead..].starts_with(['n', 'r', 't'])
                {
                    1
                } else {
                    0
                };
            if skip > 0 {
                i += lead + skip;
                found = true;
                continue;
            }
        }
        let c = s[i..].chars().next().unwrap_or(' ');
        out.push(c);
        i += c.len_utf8();
    }
    (out, found)
}

/// Follows the algorithm of `@braintree/sanitize-url`, which Mermaid applies to click targets.
pub(crate) fn sanitize_url(url: &str) -> String {
    if url.is_empty() {
        return BLANK.to_string();
    }
    let mut decoded = decode(trim(url));
    let mut rounds = 0;
    loop {
        rounds += 1;
        if rounds > 32 {
            return BLANK.to_string();
        }
        let s: String = decoded.chars().filter(|&c| !is_ctrl(c)).collect();
        let (s, _) = html_entities(&s);
        let (s, _) = html_ctrl_entities(&s);
        let s: String = s.chars().filter(|&c| !is_ctrl(c)).collect();
        let (s, _) = ws_escapes(&s);
        decoded = decode(trim(&s));
        let more = decoded.chars().any(is_ctrl)
            || html_entities(&decoded).1
            || html_ctrl_entities(&decoded).1
            || ws_escapes(&decoded).1;
        if !more {
            break;
        }
    }
    if decoded.is_empty() {
        return BLANK.to_string();
    }
    if decoded.starts_with(['.', '/']) {
        return decoded;
    }
    let trimmed = trim_start(&decoded);
    // `^.+(:|&colon;)` with the `m` flag: the first line that has a colon after its first
    // character, up to its last one. `.` stops at every JavaScript line terminator.
    let scheme = trimmed
        .split(['\n', '\r', '\u{2028}', '\u{2029}'])
        .find_map(|line| {
            let colon = line.rfind(':').filter(|&i| i > 0).map(|i| i + 1);
            let entity = line.rfind("&colon;").filter(|&i| i > 0).map(|i| i + 7);
            colon.max(entity).map(|end| &line[..end])
        });
    let Some(scheme) = scheme else {
        return decoded;
    };
    let protocol = trim(scheme).to_lowercase();
    let bare = protocol.trim_start_matches(|c: char| !crate::util::is_word(c));
    if ["javascript", "data", "vbscript"]
        .iter()
        .any(|p| bare.starts_with(p))
    {
        return BLANK.to_string();
    }
    let normalized = trimmed.replace('\\', "/");
    if protocol == "mailto:" || protocol.contains("://") {
        return normalized;
    }
    if protocol == "http:" || protocol == "https:" {
        return http_url(&protocol, &normalized[protocol.len()..])
            .unwrap_or_else(|| BLANK.to_string());
    }
    normalized
}

/// What `new URL(…).toString()` makes of an `http:` or `https:` address whose only colon is
/// the scheme's (anything else has returned before), or `None` where `new URL` throws. The
/// steps are those of the WHATWG URL parser for special schemes, less what a colon would start
/// (a port or a password) and the Unicode mapping of IDNA, which only lowercases here.
fn http_url(scheme: &str, rest: &str) -> Option<String> {
    let rest = rest.trim_start_matches('/');
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(authority_end);
    let (user, host) = match authority.rfind('@') {
        Some(at) => (Some(&authority[..at]), &authority[at + 1..]),
        None => (None, authority),
    };
    let host = parse_host(host)?;
    let mut out = format!("{scheme}//");
    if let Some(user) = user.filter(|u| !u.is_empty()) {
        out += &percent_encode(user, |c| {
            path_char(c) || matches!(c, '/' | ':' | ';' | '=' | '@' | '['..='^' | '|')
        });
        out.push('@');
    }
    out += &host;
    let (before_fragment, fragment) = match tail.find('#') {
        Some(i) => (&tail[..i], Some(&tail[i + 1..])),
        None => (tail, None),
    };
    let (path, query) = match before_fragment.find('?') {
        Some(i) => (&before_fragment[..i], Some(&before_fragment[i + 1..])),
        None => (before_fragment, None),
    };
    out += &normalize_path(path);
    if let Some(query) = query {
        out.push('?');
        out += &percent_encode(query, |c| matches!(c, ' ' | '"' | '#' | '<' | '>' | '\''));
    }
    if let Some(fragment) = fragment {
        out.push('#');
        out += &percent_encode(fragment, |c| matches!(c, ' ' | '"' | '<' | '>' | '`'));
    }
    Some(out)
}

/// Characters the path percent-encode set holds, besides controls and non-ASCII.
fn path_char(c: char) -> bool {
    matches!(c, ' ' | '"' | '#' | '<' | '>' | '?' | '`' | '{' | '}')
}

/// Percent-encodes controls, non-ASCII characters and those `extra` names, as UTF-8.
fn percent_encode(s: &str, extra: impl Fn(char) -> bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if !(' '..='~').contains(&c) || extra(c) {
            let mut buf = [0; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out += &format!("%{b:02X}");
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The path with `.` and `..` segments resolved and its characters encoded; `/` when empty.
fn normalize_path(path: &str) -> String {
    let path = path.strip_prefix('/').unwrap_or(path);
    let segments: Vec<&str> = path.split('/').collect();
    let mut out: Vec<String> = Vec::new();
    for (i, segment) in segments.iter().enumerate() {
        let last = i + 1 == segments.len();
        let lower = segment.to_ascii_lowercase();
        if matches!(lower.as_str(), ".." | ".%2e" | "%2e." | "%2e%2e") {
            out.pop();
            if last {
                out.push(String::new());
            }
        } else if matches!(lower.as_str(), "." | "%2e") {
            if last {
                out.push(String::new());
            }
        } else {
            out.push(percent_encode(segment, path_char));
        }
    }
    format!("/{}", out.join("/"))
}

/// A host as the URL parser reads it: percent-decoded, lowercased, non-ASCII labels in
/// Punycode, and a number in any of the forms IPv4 allows written as four decimal parts.
fn parse_host(host: &str) -> Option<String> {
    if host.is_empty() {
        return None;
    }
    let decoded = decode_bytes(host)?;
    let mut ascii = String::with_capacity(decoded.len());
    for (i, label) in decoded.split('.').enumerate() {
        if i > 0 {
            ascii.push('.');
        }
        if label.is_ascii() {
            let label = label.to_ascii_lowercase();
            if let Some(code) = label.strip_prefix("xn--") {
                // A label that decodes to something IDNA would refuse, such as a control, fails.
                if punydecode(code)?
                    .chars()
                    .any(|c| c.is_ascii() || refused(c))
                {
                    return None;
                }
            }
            ascii += &label;
        } else {
            if label.chars().any(refused) {
                return None;
            }
            ascii += "xn--";
            ascii += &punycode(&label.to_lowercase())?;
        }
    }
    let forbidden = |c: char| {
        c <= ' '
            || matches!(
                c,
                '#' | '/'
                    | ':'
                    | '<'
                    | '>'
                    | '?'
                    | '@'
                    | '['
                    | '\\'
                    | ']'
                    | '^'
                    | '|'
                    | '%'
                    | '\u{7f}'
            )
    };
    if ascii.is_empty() || ascii.contains(forbidden) {
        return None;
    }
    let mut labels: Vec<&str> = ascii.split('.').collect();
    if labels.len() > 1 && labels.last() == Some(&"") {
        labels.pop();
    }
    let last = labels.last().copied().unwrap_or("");
    let hex = last.strip_prefix("0x").or_else(|| last.strip_prefix("0X"));
    let numeric = (!last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()))
        || hex.is_some_and(|h| h.bytes().all(|b| b.is_ascii_hexdigit()));
    if !numeric {
        return Some(ascii);
    }
    ipv4(&labels)
}

/// Characters IDNA refuses in a label: spaces, line separators, controls and invisible format
/// characters.
fn refused(c: char) -> bool {
    c.is_whitespace()
        || c.is_control()
        || matches!(c, '\u{200b}'..='\u{200f}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
}

/// Percent-decodes into UTF-8, as the host parser does before anything else.
fn decode_bytes(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = b
            .get(i + 1..i + 3)
            .filter(|h| b[i] == b'%' && h.iter().all(u8::is_ascii_hexdigit));
        match hex {
            Some(h) => {
                out.push(u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()?);
                i += 3;
            }
            None => {
                out.push(b[i]);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn ipv4(labels: &[&str]) -> Option<String> {
    if labels.len() > 4 {
        return None;
    }
    let mut numbers = Vec::with_capacity(labels.len());
    for label in labels {
        if label.is_empty() {
            return None;
        }
        let (digits, radix) = if let Some(h) = label
            .strip_prefix("0x")
            .or_else(|| label.strip_prefix("0X"))
        {
            (h, 16)
        } else if label.len() > 1 && label.starts_with('0') {
            (&label[1..], 8)
        } else {
            (*label, 10)
        };
        let n = if digits.is_empty() {
            0
        } else {
            u64::from_str_radix(digits, radix).ok()?
        };
        numbers.push(n);
    }
    let (last, rest) = numbers.split_last()?;
    if rest.iter().any(|&n| n > 255) || *last >= 256u64.pow(5 - numbers.len() as u32) {
        return None;
    }
    let mut value = *last;
    for (i, &n) in rest.iter().enumerate() {
        value += n << (8 * (3 - i));
    }
    Some(format!(
        "{}.{}.{}.{}",
        value >> 24,
        (value >> 16) & 255,
        (value >> 8) & 255,
        value & 255
    ))
}

/// The label a Punycode string (RFC 3492, without `xn--`) encodes, or `None` when it is not
/// valid Punycode or encodes nothing.
fn punydecode(code: &str) -> Option<String> {
    const BASE: u32 = 36;
    let (basic, digits) = match code.rfind('-') {
        Some(i) => (&code[..i], &code[i + 1..]),
        None => ("", code),
    };
    let mut out: Vec<char> = basic.chars().collect();
    let (mut n, mut i, mut bias) = (128u32, 0u32, 72u32);
    let mut it = digits.bytes().peekable();
    while it.peek().is_some() {
        let old = i;
        let mut w = 1u32;
        let mut k = BASE;
        loop {
            let d = match it.next()? {
                b @ b'a'..=b'z' => u32::from(b - b'a'),
                b @ b'A'..=b'Z' => u32::from(b - b'A'),
                b @ b'0'..=b'9' => u32::from(b - b'0') + 26,
                _ => return None,
            };
            i = i.checked_add(d.checked_mul(w)?)?;
            let t = if k <= bias {
                1
            } else if k >= bias + 26 {
                26
            } else {
                k - bias
            };
            if d < t {
                break;
            }
            w = w.checked_mul(BASE - t)?;
            k += BASE;
        }
        let points = out.len() as u32 + 1;
        let mut delta = (i - old) / if old == 0 { 700 } else { 2 };
        delta += delta / points;
        let mut k = 0;
        while delta > ((BASE - 1) * 26) / 2 {
            delta /= BASE - 1;
            k += BASE;
        }
        bias = k + (BASE * delta) / (delta + 38);
        n = n.checked_add(i / points)?;
        i %= points;
        out.insert(i as usize, char::from_u32(n)?);
        i += 1;
    }
    (!out.is_empty()).then(|| out.into_iter().collect())
}

/// The Punycode form of a label (RFC 3492), without the `xn--` prefix.
fn punycode(label: &str) -> Option<String> {
    const BASE: u32 = 36;
    let digit = |d: u32| {
        char::from(if d < 26 {
            b'a' + d as u8
        } else {
            b'0' + (d - 26) as u8
        })
    };
    let adapt = |mut delta: u32, points: u32, first: bool| {
        delta /= if first { 700 } else { 2 };
        delta += delta / points;
        let mut k = 0;
        while delta > ((BASE - 1) * 26) / 2 {
            delta /= BASE - 1;
            k += BASE;
        }
        k + (BASE * delta) / (delta + 38)
    };
    let input: Vec<u32> = label.chars().map(u32::from).collect();
    let mut out: String = label.chars().filter(char::is_ascii).collect();
    let basic = out.len() as u32;
    let mut handled = basic;
    if basic > 0 {
        out.push('-');
    }
    let (mut n, mut delta, mut bias) = (128u32, 0u32, 72u32);
    while (handled as usize) < input.len() {
        let m = input.iter().copied().filter(|&c| c >= n).min()?;
        delta = delta.checked_add((m - n).checked_mul(handled + 1)?)?;
        n = m;
        for &c in &input {
            if c < n {
                delta = delta.checked_add(1)?;
            }
            if c == n {
                let mut q = delta;
                let mut k = BASE;
                loop {
                    let t = if k <= bias {
                        1
                    } else if k >= bias + 26 {
                        26
                    } else {
                        k - bias
                    };
                    if q < t {
                        break;
                    }
                    out.push(digit(t + (q - t) % (BASE - t)));
                    q = (q - t) / (BASE - t);
                    k += BASE;
                }
                out.push(digit(q));
                bias = adapt(delta, handled + 1, handled == basic);
                delta = 0;
                handled += 1;
            }
        }
        delta += 1;
        n += 1;
    }
    Some(out)
}

/// A URL as written in a diagram: entity codes are read first, so they cannot hide a scheme.
pub(crate) fn safe_url(url: &str) -> String {
    sanitize_url(&decode_entities(url))
}

const LINK_SCHEMES: &[&str] = &["http", "https", "mailto", "tel"];
const IMAGE_SCHEMES: &[&str] = &["http", "https"];

/// Keeps a URL only when it is relative or its scheme is one of `schemes`.
fn allow(url: &str, schemes: &[&str]) -> Option<String> {
    let b = url.as_bytes();
    if b.len() >= 2 && matches!(b[0], b'\\' | b'/') && matches!(b[1], b'\\' | b'/') {
        return None;
    }
    let scheme_len = url
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')))
        .unwrap_or(url.len());
    let has_scheme =
        scheme_len > 0 && b[0].is_ascii_alphabetic() && url[scheme_len..].starts_with(':');
    if !has_scheme {
        return Some(url.to_string());
    }
    let scheme = url[..scheme_len].to_ascii_lowercase();
    schemes
        .iter()
        .any(|s| s.eq_ignore_ascii_case(&scheme))
        .then(|| url.to_string())
}

fn schemes(list: &Option<Vec<String>>, default: &'static [&'static str]) -> Vec<String> {
    list.clone()
        .unwrap_or_else(|| default.iter().map(|s| (*s).to_string()).collect())
}

/// A link address the host allows, or `None`. A refused link is left out of the drawing.
pub(crate) fn link_url(url: &str, options: &RenderOptions) -> Option<String> {
    if !options.links {
        return None;
    }
    let list = schemes(&options.link_schemes, LINK_SCHEMES);
    allow(url, &list.iter().map(String::as_str).collect::<Vec<_>>())
}

pub(crate) fn image_url(url: &str, options: &RenderOptions) -> Option<String> {
    if !options.images {
        return None;
    }
    let list = schemes(&options.image_schemes, IMAGE_SCHEMES);
    allow(url, &list.iter().map(String::as_str).collect::<Vec<_>>())
}

pub(crate) fn rel_attr(options: &RenderOptions) -> String {
    match options.link_rel.as_deref() {
        Some("") => String::new(),
        Some(rel) => format!(" rel=\"{}\"", esc(rel)),
        None => " rel=\"noopener\"".to_string(),
    }
}

/// Wraps `body` in a link to `url`, when the host allows it, and records the link.
pub(crate) fn linked(
    body: &str,
    url: &str,
    id: &str,
    links: &mut Vec<Link>,
    options: &RenderOptions,
    attrs: &str,
) -> String {
    let Some(href) = link_url(url, options) else {
        return body.to_string();
    };
    let out = format!(
        "<a href=\"{}\"{attrs}{}>{body}</a>",
        esc(&href),
        rel_attr(options)
    );
    links.push(Link {
        id: id.to_string(),
        href,
        internal: false,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes the sanitized form of the inputs of a differential run; see `tools/oracle/diff.mjs`.
    #[test]
    #[ignore = "run by tools/oracle/diff.mjs"]
    fn oracle_dump() {
        crate::diagrams::common::golden::dump("url", sanitize_url);
    }

    #[test]
    fn sanitizes_urls() {
        assert_eq!(sanitize_url("javascript:alert(1)"), BLANK);
        assert_eq!(sanitize_url(" JaVaScRiPt:alert(1)"), BLANK);
        assert_eq!(sanitize_url("jav&#x09;ascript:alert(1)"), BLANK);
        assert_eq!(sanitize_url("%0ajavascript:alert(1)"), BLANK);
        assert_eq!(sanitize_url("data:text/html,x"), BLANK);
        assert_eq!(
            sanitize_url("https://Example.com/a"),
            "https://example.com/a"
        );
        assert_eq!(sanitize_url("http://x"), "http://x/");
        assert_eq!(sanitize_url("/relative"), "/relative");
        assert_eq!(sanitize_url("page.html"), "page.html");
        assert_eq!(sanitize_url("mailto:a@b.c"), "mailto:a@b.c");
        // As `new URL` parses http and https addresses.
        assert_eq!(sanitize_url("https://www.</text>github.com"), BLANK);
        assert_eq!(sanitize_url("https://a b.com"), BLANK);
        assert_eq!(sanitize_url("https://0x7f.1/"), "https://127.0.0.1/");
        assert_eq!(sanitize_url("https://0x7f..1/"), BLANK);
        assert_eq!(sanitize_url("https://1.2.3.4.5/"), BLANK);
        assert_eq!(sanitize_url("https://xn--/"), BLANK);
        assert_eq!(
            sanitize_url("https://bücher.de/"),
            "https://xn--bcher-kva.de/"
        );
        assert_eq!(sanitize_url("http://a.b/c/../d/./e"), "http://a.b/d/e");
        assert_eq!(
            sanitize_url("https://a.com/a b?c d#e f"),
            "https://a.com/a%20b?c%20d#e%20f"
        );
        assert_eq!(
            sanitize_url("https://a.com/\u{2028}x:y"),
            "https://a.com/%E2%80%A8x:y"
        );
        assert_eq!(sanitize_url("tel:%25+1"), "tel:%+1");
        let encoded = crate::text::entities::encode_entities("#106;avascript:alert(1)");
        assert_eq!(safe_url(&encoded), BLANK);
    }

    #[test]
    fn allows_only_listed_schemes() {
        let options = RenderOptions::default();
        assert_eq!(
            link_url("https://x/", &options).as_deref(),
            Some("https://x/")
        );
        assert_eq!(link_url("page", &options).as_deref(), Some("page"));
        assert_eq!(link_url("file:///etc/passwd", &options), None);
        assert_eq!(link_url("//evil/share", &options), None);
        assert_eq!(link_url("obsidian://open", &options), None);
        assert_eq!(image_url("mailto:x", &options), None);
    }
}
