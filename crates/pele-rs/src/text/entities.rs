//! Mermaid's entity codes. `#name;` codes are hidden behind placeholder characters before
//! parsing, so that `#` and `;` survive the grammar, and turned into characters when text is
//! drawn. Ported from pele's `preprocess.ts` and `text/entities.ts`.

use crate::util::{is_line_end, is_space, is_word};

/// Opens a hidden code; a second `°` marks a numeric one.
const OPEN: &str = "ﬂ°";
const CLOSE: &str = "¶ß";

const NAMED: &[(&str, u32)] = &[
    ("amp", 38),
    ("lt", 60),
    ("gt", 62),
    ("quot", 34),
    ("apos", 39),
    ("nbsp", 160),
    ("shy", 173),
    ("iexcl", 161),
    ("cent", 162),
    ("pound", 163),
    ("curren", 164),
    ("yen", 165),
    ("sect", 167),
    ("copy", 169),
    ("laquo", 171),
    ("not", 172),
    ("reg", 174),
    ("deg", 176),
    ("plusmn", 177),
    ("sup2", 178),
    ("sup3", 179),
    ("micro", 181),
    ("para", 182),
    ("middot", 183),
    ("sup1", 185),
    ("raquo", 187),
    ("frac14", 188),
    ("frac12", 189),
    ("frac34", 190),
    ("iquest", 191),
    ("times", 215),
    ("divide", 247),
    ("ndash", 8211),
    ("mdash", 8212),
    ("lsquo", 8216),
    ("rsquo", 8217),
    ("sbquo", 8218),
    ("ldquo", 8220),
    ("rdquo", 8221),
    ("bdquo", 8222),
    ("dagger", 8224),
    ("Dagger", 8225),
    ("bull", 8226),
    ("hellip", 8230),
    ("permil", 8240),
    ("prime", 8242),
    ("Prime", 8243),
    ("lsaquo", 8249),
    ("rsaquo", 8250),
    ("euro", 8364),
    ("trade", 8482),
    ("larr", 8592),
    ("uarr", 8593),
    ("rarr", 8594),
    ("darr", 8595),
    ("harr", 8596),
    ("lArr", 8656),
    ("uArr", 8657),
    ("rArr", 8658),
    ("dArr", 8659),
    ("hArr", 8660),
    ("forall", 8704),
    ("part", 8706),
    ("exist", 8707),
    ("empty", 8709),
    ("nabla", 8711),
    ("isin", 8712),
    ("notin", 8713),
    ("ni", 8715),
    ("prod", 8719),
    ("sum", 8721),
    ("minus", 8722),
    ("radic", 8730),
    ("infin", 8734),
    ("and", 8743),
    ("or", 8744),
    ("cap", 8745),
    ("cup", 8746),
    ("int", 8747),
    ("there4", 8756),
    ("sim", 8764),
    ("cong", 8773),
    ("asymp", 8776),
    ("ne", 8800),
    ("equiv", 8801),
    ("le", 8804),
    ("ge", 8805),
    ("sub", 8834),
    ("sup", 8835),
    ("sube", 8838),
    ("supe", 8839),
    ("oplus", 8853),
    ("otimes", 8855),
    ("perp", 8869),
    ("sdot", 8901),
    ("loz", 9674),
    ("spades", 9824),
    ("clubs", 9827),
    ("hearts", 9829),
    ("diams", 9830),
    ("check", 10003),
    ("cross", 10007),
    ("AMP", 38),
    ("LT", 60),
    ("GT", 62),
    ("QUOT", 34),
    ("lsqb", 91),
    ("rsqb", 93),
    ("lcub", 123),
    ("rcub", 125),
    ("verbar", 124),
    ("VerticalLine", 124),
    ("midast", 42),
    ("DiacriticalGrave", 96),
    ("Hat", 94),
    ("UnderBar", 95),
    ("equals", 61),
    ("colon", 58),
    ("semi", 59),
    ("num", 35),
    ("percnt", 37),
    ("lpar", 40),
    ("rpar", 41),
    ("lbrack", 91),
    ("rbrack", 93),
    ("lbrace", 123),
    ("rbrace", 125),
    ("vert", 124),
    ("sol", 47),
    ("bsol", 92),
    ("quest", 63),
    ("excl", 33),
    ("ast", 42),
    ("plus", 43),
    ("comma", 44),
    ("period", 46),
    ("dollar", 36),
    ("commat", 64),
    ("grave", 96),
    ("hat", 94),
    ("lowbar", 95),
    ("tilde", 126),
    ("Alpha", 913),
    ("Beta", 914),
    ("Gamma", 915),
    ("Delta", 916),
    ("Epsilon", 917),
    ("Zeta", 918),
    ("Eta", 919),
    ("Theta", 920),
    ("Iota", 921),
    ("Kappa", 922),
    ("Lambda", 923),
    ("Mu", 924),
    ("Nu", 925),
    ("Xi", 926),
    ("Omicron", 927),
    ("Pi", 928),
    ("Rho", 929),
    ("Sigma", 931),
    ("Tau", 932),
    ("Upsilon", 933),
    ("Phi", 934),
    ("Chi", 935),
    ("Psi", 936),
    ("Omega", 937),
    ("alpha", 945),
    ("beta", 946),
    ("gamma", 947),
    ("delta", 948),
    ("epsilon", 949),
    ("zeta", 950),
    ("eta", 951),
    ("theta", 952),
    ("iota", 953),
    ("kappa", 954),
    ("lambda", 955),
    ("mu", 956),
    ("nu", 957),
    ("xi", 958),
    ("omicron", 959),
    ("pi", 960),
    ("rho", 961),
    ("sigmaf", 962),
    ("sigma", 963),
    ("tau", 964),
    ("upsilon", 965),
    ("phi", 966),
    ("chi", 967),
    ("psi", 968),
    ("omega", 969),
];

fn word_len(s: &str) -> usize {
    s.bytes().take_while(|&b| is_word(b as char)).count()
}

/// Does what Mermaid's `text.replace(/keyword.*:\S*#.*;/g, drop the last character)` does: on a
/// line where the keyword is followed by a colon, a color-like `#`, and later a semicolon, the
/// last semicolon goes. Mermaid's regex backtracks without bound; this is one pass.
fn drop_color_semicolons(text: &str, keyword: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    let mut from = text.find(keyword);
    while let Some(start) = from {
        let (line_end, next) = match text[start..].char_indices().find(|&(_, c)| is_line_end(c)) {
            Some((i, c)) => (start + i, start + i + c.len_utf8()),
            None => (text.len(), text.len()),
        };
        let mut after_colon = false;
        let mut hash = None;
        for (j, c) in text[start + keyword.len()..line_end].char_indices() {
            if c == ':' {
                after_colon = true;
            } else if c == '#' {
                if after_colon {
                    hash = Some(start + keyword.len() + j);
                    break;
                }
            } else if is_space(c) {
                after_colon = false;
            }
        }
        if let Some(hash) = hash {
            if let Some(semi) = text[..line_end].rfind(';').filter(|&s| s > hash) {
                out.push_str(&text[last..semi]);
                last = semi + 1;
            }
        }
        from = if line_end >= text.len() {
            None
        } else {
            text[next..].find(keyword).map(|i| next + i)
        };
    }
    if last == 0 {
        return text.to_string();
    }
    out.push_str(&text[last..]);
    out
}

/// Hides `#name;` entity codes behind placeholder characters, as Mermaid does before parsing.
pub(crate) fn encode_entities(text: &str) -> String {
    if !text.contains('#') {
        return text.to_string();
    }
    let text = drop_color_semicolons(&drop_color_semicolons(text, "style"), "classDef");
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(at) = rest.find('#') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let n = word_len(after);
        if n > 0 && after[n..].starts_with(';') {
            let inner = &after[..n];
            out.push_str(OPEN);
            if inner.bytes().all(|b| b.is_ascii_digit()) {
                out.push('°');
            }
            out.push_str(inner);
            out.push_str(CLOSE);
            rest = &after[n + 1..];
        } else {
            out.push('#');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

pub(crate) fn encode_mapped(mapped: &mut crate::source_map::Mapped) {
    // The style pass deletes only semicolons. Preserve the offsets of every retained byte.
    for keyword in ["style", "classDef"] {
        let text = drop_color_semicolons(&mapped.text, keyword);
        if text != mapped.text {
            let mut next = crate::source_map::Mapped {
                text: String::new(),
                origins: Vec::new(),
            };
            let mut chars = mapped.text.char_indices();
            for ch in text.chars() {
                if let Some((at, _)) = chars.find(|(_, c)| *c == ch) {
                    next.push(ch, mapped.origins[at]);
                }
            }
            *mapped = next;
        }
    }
    let mut out = crate::source_map::Mapped {
        text: String::new(),
        origins: Vec::new(),
    };
    let mut at = 0;
    while at < mapped.text.len() {
        let rest = &mapped.text[at..];
        if let Some(after) = rest.strip_prefix('#') {
            let n = word_len(after);
            if n > 0 && after[n..].starts_with(';') {
                let inner = &after[..n];
                for ch in OPEN.chars() {
                    out.push(ch, mapped.origins[at]);
                }
                if inner.bytes().all(|b| b.is_ascii_digit()) {
                    out.push('°', mapped.origins[at]);
                }
                for (i, ch) in inner.char_indices() {
                    out.push(ch, mapped.origins[at + 1 + i]);
                }
                for ch in CLOSE.chars() {
                    out.push(ch, mapped.origins[at + 1 + n]);
                }
                at += n + 2;
                continue;
            }
        }
        let ch = rest.chars().next().unwrap();
        out.push(ch, mapped.origins[at]);
        at += ch.len_utf8();
    }
    *mapped = out;
}

/// Turns hidden codes back into entities, so that each code is decoded once.
fn restore_placeholders(text: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    let mut from = text.find(OPEN);
    while let Some(start) = from {
        let inner = start + OPEN.len();
        let Some(stop) = text[inner..].find('¶').map(|i| inner + i) else {
            break;
        };
        let after_stop = stop + '¶'.len_utf8();
        if text[after_stop..].starts_with('ß') {
            let numeric = text[inner..].starts_with('°');
            out.push_str(&text[last..start]);
            out.push_str(if numeric { "&#" } else { "&" });
            out.push_str(
                &text[if numeric {
                    inner + '°'.len_utf8()
                } else {
                    inner
                }..stop],
            );
            out.push(';');
            last = after_stop + 'ß'.len_utf8();
        }
        from = text[after_stop..].find(OPEN).map(|i| after_stop + i);
    }
    if last == 0 {
        return text.to_string();
    }
    out.push_str(&text[last..]);
    out
}

fn code_point(n: Option<u32>) -> Option<char> {
    n.filter(|&n| n > 0).and_then(char::from_u32)
}

/// Turns Mermaid's `#35;` / `#quot;` codes (as hidden by [`encode_entities`]) and HTML entities
/// into characters.
pub(crate) fn decode_entities(text: &str) -> String {
    let restored;
    let text = if text.contains(CLOSE) {
        restored = restore_placeholders(text);
        restored.as_str()
    } else {
        text
    };
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        match decode_one(after) {
            Some((c, used)) => {
                out.push(c);
                rest = &after[used..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Decodes the entity whose body starts `s` (after the `&`). Returns the character and the
/// length of the body up to and including the `;`.
fn decode_one(s: &str) -> Option<(char, usize)> {
    if let Some(num) = s.strip_prefix('#') {
        let hex = num.starts_with(['x', 'X']);
        let digits = if hex { &num[1..] } else { num };
        let n = digits
            .bytes()
            .take_while(|b| {
                if hex {
                    b.is_ascii_hexdigit()
                } else {
                    b.is_ascii_digit()
                }
            })
            .count();
        if n > 0 && digits[n..].starts_with(';') {
            let value = u32::from_str_radix(&digits[..n], if hex { 16 } else { 10 }).ok();
            let used = s.len() - digits.len() + n + 1;
            return code_point(value).map(|c| (c, used));
        }
        return None;
    }
    let n = word_len(s);
    if n == 0 || !s[n..].starts_with(';') {
        return None;
    }
    let name = &s[..n];
    NAMED
        .iter()
        .find(|&&(k, _)| k == name)
        .and_then(|&(_, v)| char::from_u32(v))
        .map(|c| (c, n + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_codes() {
        assert_eq!(
            encode_entities("a #35; b #quot; c #x"),
            "a ﬂ°°35¶ß b ﬂ°quot¶ß c #x"
        );
        assert_eq!(encode_entities("no codes"), "no codes");
    }

    #[test]
    fn drops_semicolons_after_colors() {
        assert_eq!(encode_entities("style A fill:#f9f;"), "style A fill:#f9f");
        assert_eq!(
            encode_entities("classDef c fill:#f00;\nA;"),
            "classDef c fill:#f00\nA;"
        );
        // Only a color glued to the colon is protected, as in pele.
        assert_eq!(
            encode_entities("style A fill: #f9f;"),
            "style A fill: ﬂ°f9f¶ß"
        );
    }

    #[test]
    fn decodes_codes_and_entities() {
        assert_eq!(decode_entities(&encode_entities("#35; #quot;")), "# \"");
        assert_eq!(
            decode_entities("&lt;b&gt; &#65;&#x42; &bogus; &#0; &"),
            "<b> AB &bogus; &#0; &"
        );
        assert_eq!(decode_entities("&#xD800;"), "&#xD800;");
    }
}
