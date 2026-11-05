//! The characters of a source, as the jison lexers read them: on UTF-16 code units, so that
//! what counts as one character is the same as in the grammar.

/// Whitespace as `\s` sees it, on a UTF-16 code unit.
pub(crate) fn is_space(c: u16) -> bool {
    if c < 128 {
        return c == 32 || (9..=13).contains(&c);
    }
    c == 0xa0
        || c == 0x1680
        || (0x2000..=0x200a).contains(&c)
        || c == 0x2028
        || c == 0x2029
        || c == 0x202f
        || c == 0x205f
        || c == 0x3000
        || c == 0xfeff
}

/// A digit, on a UTF-16 code unit.
pub(crate) fn is_digit(c: u16) -> bool {
    (48..=57).contains(&c)
}

/// An ASCII letter, on a UTF-16 code unit.
pub(crate) fn is_letter(c: u16) -> bool {
    (65..=90).contains(&c) || (97..=122).contains(&c)
}

/// A character of `\w`, on a UTF-16 code unit.
pub(crate) fn is_word(c: u16) -> bool {
    is_digit(c) || is_letter(c) || c == 95
}

/// A line terminator: what `.` does not match.
#[allow(dead_code, reason = "only the quadrant chart needs it")]
pub(crate) fn is_line_end(c: u16) -> bool {
    c == 10 || c == 13 || c == 0x2028 || c == 0x2029
}

/// Whether `word`, written in lower case, is at `at`. Only ASCII letters match in either case,
/// as with the `i` flag on a regular expression without the `u` flag.
#[allow(dead_code, reason = "only the git graph needs it")]
pub(crate) fn word_at(units: &[u16], at: usize, word: &str) -> bool {
    for (k, w) in word.bytes().enumerate() {
        let Some(&c) = units.get(at + k) else {
            return false;
        };
        if c != u16::from(w) && !(w.is_ascii_lowercase() && c == u16::from(w) - 32) {
            return false;
        }
    }
    true
}

/// The byte offset of each UTF-16 code unit of a text, and one for the end of the text.
#[allow(dead_code, reason = "only the quadrant chart needs it")]
pub(crate) fn byte_offsets(src: &str) -> Vec<usize> {
    let mut out = Vec::with_capacity(src.len() + 1);
    let mut at = 0;
    for c in src.chars() {
        for _ in 0..c.len_utf16() {
            out.push(at);
        }
        at += c.len_utf8();
    }
    out.push(at);
    out
}
