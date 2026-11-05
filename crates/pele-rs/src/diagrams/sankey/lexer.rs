//! The sankey lexer: Mermaid's jison lexer for `sankey.jison`, as pele's `lexer.ts` emulates
//! it. It reads UTF-16 code units, as the jison lexer does, so that what counts as one
//! character is the same.

/// The tokens of the sankey grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum T {
    End,
    /// Not a jison token: the reference lexer would return the same empty token forever from
    /// here.
    Stuck,
    Sankey,
    Newline,
    Comma,
    Dquote,
    NonEscapedText,
    EscapedText,
    Eof,
}

impl T {
    /// The token's name, as the error messages of the parser name it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            T::End => "$end",
            T::Stuck => "STUCK",
            T::Sankey => "SANKEY",
            T::Newline => "NEWLINE",
            T::Comma => "COMMA",
            T::Dquote => "DQUOTE",
            T::NonEscapedText => "NON_ESCAPED_TEXT",
            T::EscapedText => "ESCAPED_TEXT",
            T::Eof => "EOF",
        }
    }
}

/// The tokens of a source, and where each of them starts.
pub(crate) struct Tokens {
    pub types: Vec<T>,
    pub texts: Vec<String>,
    /// The start of each token, in UTF-16 code units.
    pub starts: Vec<usize>,
}

/// The state the lexer is in. The jison lexer uses start conditions; the ones that matter here
/// are the text outside a quoted field, the text inside one, and the text before the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Csv,
    EscapedText,
}

/// RFC 4180's TEXTDATA, as Mermaid's grammar lists it: the printable ASCII characters, except
/// the quote and the comma. Mermaid's grammar has no rule for a character outside that range,
/// so a field that holds one is a parse error there, and pele-rs reads it the same way.
fn is_text_data(c: u32) -> bool {
    (32..=126).contains(&c) && c != 34 && c != 44
}

/// The characters a word is made of, as the jison lexer's word rule counts them.
fn is_word(c: u16) -> bool {
    (0x30..=0x39).contains(&c)
        || (0x41..=0x5a).contains(&c)
        || (0x61..=0x7a).contains(&c)
        || c == 95
}

/// The code point at `i`, and how many UTF-16 code units it takes. A surrogate that is not a
/// pair stands for itself, as `codePointAt` reads it.
fn code_point_at(src: &[u16], i: usize) -> (u32, usize) {
    let c = src[i];
    if (0xd800..0xdc00).contains(&c) {
        if let Some(&low) = src.get(i + 1) {
            if (0xdc00..0xe000).contains(&low) {
                let cp = 0x10000 + ((c as u32 - 0xd800) << 10) + (low as u32 - 0xdc00);
                return (cp, 2);
            }
        }
    }
    (c as u32, 1)
}

/// Whether the text at `p` is the keyword, in any case. The jison lexer matches it with
/// `%options case-insensitive`, which lowercases the text and compares it with the keyword.
fn keyword(src: &[u16], p: usize, word: &str) -> bool {
    let w: Vec<u16> = word.encode_utf16().collect();
    if p + w.len() > src.len() {
        return false;
    }
    for (i, &expected) in w.iter().enumerate() {
        let c = src[p + i];
        let lower = if (0x41..=0x5a).contains(&c) {
            c + 32
        } else if c == 0x212a {
            // The Kelvin sign lowercases to a plain k.
            b'k' as u16
        } else {
            c
        };
        if lower != expected {
            return false;
        }
    }
    // The keyword must not be followed by a word character.
    match src.get(p + w.len()) {
        Some(&c) => !is_word(c),
        None => true,
    }
}

/// Reads the source as the jison lexer for `sankey.jison` reads it.
pub(crate) fn tokenize(src: &[u16]) -> Tokens {
    let n = src.len();
    let mut types = Vec::new();
    let mut texts = Vec::new();
    let mut starts = Vec::new();
    let mut stack: Vec<S> = Vec::new();
    let mut state = S::Initial;
    let mut p = 0;

    while p < n {
        let c = src[p];
        let start = p;
        let mut q = p;
        if state == S::EscapedText {
            if c == 34 && src.get(p + 1).copied() != Some(34) {
                if let Some(s) = stack.pop() {
                    state = s;
                }
                types.push(T::Dquote);
                texts.push(String::from_utf16_lossy(&src[p..p + 1]));
                starts.push(p);
                p += 1;
                continue;
            }
            while q < n {
                let (d, len) = code_point_at(src, q);
                if is_text_data(d) || d == 44 || d == 13 || d == 10 {
                    q += len;
                } else if d == 34 && src.get(q + 1).copied() == Some(34) {
                    q += 2;
                } else {
                    break;
                }
            }
            types.push(T::EscapedText);
            texts.push(String::from_utf16_lossy(&src[p..q]));
            starts.push(p);
            p = q;
        } else {
            if state == S::Initial && (c | 32) == 115 {
                let length = if keyword(src, p, "sankey-beta") {
                    11
                } else if keyword(src, p, "sankey") {
                    6
                } else {
                    0
                };
                if length > 0 {
                    stack.push(state);
                    state = S::Csv;
                    types.push(T::Sankey);
                    texts.push(String::from_utf16_lossy(&src[p..p + length]));
                    starts.push(p);
                    p += length;
                    continue;
                }
            }
            if c == 10 || (c == 13 && src.get(p + 1).copied() == Some(10)) {
                let end = if c == 10 { p + 1 } else { p + 2 };
                types.push(T::Newline);
                texts.push(String::from_utf16_lossy(&src[p..end]));
                starts.push(p);
                p = end;
                continue;
            }
            if c == 44 {
                types.push(T::Comma);
                texts.push(String::from_utf16_lossy(&src[p..p + 1]));
                starts.push(p);
                p += 1;
                continue;
            }
            if c == 34 {
                stack.push(state);
                state = S::EscapedText;
                types.push(T::Dquote);
                texts.push(String::from_utf16_lossy(&src[p..p + 1]));
                starts.push(p);
                p += 1;
                continue;
            }
            while q < n {
                let (d, len) = code_point_at(src, q);
                if !is_text_data(d) {
                    break;
                }
                q += len;
            }
            types.push(T::NonEscapedText);
            texts.push(String::from_utf16_lossy(&src[p..q]));
            starts.push(p);
            p = q;
        }
        // A character outside the CSV alphabet matches as empty text and is never consumed.
        if q == start {
            types.push(T::Stuck);
            texts.push(String::new());
            starts.push(p);
            return Tokens {
                types,
                texts,
                starts,
            };
        }
    }

    types.push(if state == S::EscapedText {
        T::EscapedText
    } else {
        T::Eof
    });
    texts.push(String::new());
    starts.push(n);
    types.push(T::End);
    texts.push(String::new());
    starts.push(n);
    Tokens {
        types,
        texts,
        starts,
    }
}
