//! Splits label text into styled, measured lines and wraps them to a width. Ported from pele's
//! `text/label.ts`. Icons and math are the host's: a label holds them only when the host of the
//! render has them (see [`Host`]).

use std::rc::Rc;

use super::entities::decode_entities;
use super::{Host, MathMarkup, Style, TextMeasurer};
use crate::util::{chars, is_space, is_word, round, trim, trim_end, trim_start};

/// An icon the host has, in the place of an icon reference such as `fa:fa-car`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Icon {
    pub name: String,
    pub markup: Rc<str>,
}

/// A run of text, or an icon, or typeset math. An icon and math have no text.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Span {
    pub text: String,
    pub style: Style,
    pub width: f64,
    pub icon: Option<Icon>,
    pub math: Option<Rc<MathMarkup>>,
}

impl Span {
    pub(crate) fn text(text: String, style: Style, width: f64) -> Span {
        Span {
            text,
            style,
            width,
            icon: None,
            math: None,
        }
    }

    /// Whether the span is text, which can take more text of the same style.
    pub(crate) fn is_plain(&self) -> bool {
        self.icon.is_none() && self.math.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Label {
    pub lines: Vec<Vec<Span>>,
    pub widths: Vec<f64>,
    pub width: f64,
    pub height: f64,
    pub size: f64,
    pub line_height: f64,
    /// The height of each line, present only when math needs more than `line_height` for some.
    pub heights: Option<Vec<f64>>,
}

/// An icon is drawn this much wider than the font size is.
const ICON_ADVANCE: f64 = 1.25;

/// Math is put in the text as one private-use character each, which keeps the TeX from being
/// read as markup. At most this many expressions of a label are typeset.
const MARK: u32 = 0xe000;
const MAX_MARKS: usize = 0x1900;

fn is_mark(c: char) -> bool {
    ('\u{e000}'..='\u{f8ff}').contains(&c)
}

/// The math of a label: the TeX of each expression, and its typeset form where the host has one.
struct Marks {
    tex: Vec<String>,
    math: Vec<Option<Rc<MathMarkup>>>,
}

/// What the layout of a label is given besides the text.
#[derive(Clone, Copy)]
struct Env<'a> {
    host: Option<&'a Host<'a>>,
    marks: Option<&'a Marks>,
}

pub(crate) const LINE_HEIGHT: f64 = 1.5;
/// Lines of one wrapped label sit closer, so the label reads as one block.
const WRAPPED_LINE_HEIGHT: f64 = 1.3;

const TAG_STYLES: &[(&str, Style)] = &[
    ("b", Style::BOLD),
    ("strong", Style::BOLD),
    ("i", Style::ITALIC),
    ("em", Style::ITALIC),
    ("cite", Style::ITALIC),
    ("dfn", Style::ITALIC),
    ("var", Style::ITALIC),
    ("code", Style::MONO),
    ("kbd", Style::MONO),
    ("samp", Style::MONO),
    ("tt", Style::MONO),
    ("sub", Style::SUB),
    ("sup", Style::SUP),
    ("u", Style::UNDERLINE),
    ("ins", Style::UNDERLINE),
];

const HTML_TAGS: &[&str] = &[
    "a",
    "abbr",
    "address",
    "article",
    "aside",
    "audio",
    "b",
    "bdi",
    "bdo",
    "big",
    "blockquote",
    "body",
    "br",
    "button",
    "caption",
    "center",
    "cite",
    "code",
    "col",
    "colgroup",
    "data",
    "dd",
    "del",
    "details",
    "dfn",
    "dialog",
    "div",
    "dl",
    "dt",
    "em",
    "embed",
    "fieldset",
    "figcaption",
    "figure",
    "font",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "i",
    "iframe",
    "img",
    "input",
    "ins",
    "kbd",
    "label",
    "legend",
    "li",
    "link",
    "main",
    "map",
    "mark",
    "meta",
    "meter",
    "nav",
    "noscript",
    "object",
    "ol",
    "optgroup",
    "option",
    "output",
    "p",
    "picture",
    "pre",
    "progress",
    "q",
    "rp",
    "rt",
    "ruby",
    "s",
    "samp",
    "script",
    "section",
    "select",
    "small",
    "source",
    "span",
    "strike",
    "strong",
    "style",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "time",
    "title",
    "tr",
    "track",
    "tt",
    "u",
    "ul",
    "var",
    "video",
    "wbr",
];

/// Break between any two Chinese or Japanese characters. Hangul is left out: Korean puts
/// spaces between words.
fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x2e80..=0x9fff | 0xf900..=0xfaff | 0xff00..=0xff60)
}

/// Characters that stay with the character before them.
fn is_close(c: char) -> bool {
    "、。，．：；！？）］｝」』】〕〉》〙〗ー…‥・ゝゞぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮ,.:;!?)]}"
        .contains(c)
}

/// Characters that stay with the character after them.
fn is_open(c: char) -> bool {
    "（［｛「『【〔〈《〘〖([{".contains(c)
}

/// Break hints: a zero-width space, and a soft hyphen, which shows as '-' where a line breaks.
fn is_hint(c: char) -> bool {
    c == '\u{200b}' || c == '\u{ad}'
}

const SOFT_HYPHEN: char = '\u{ad}';

/// Finds an icon reference such as `fa:fa-car` at or after `from`. Returns its start and end.
fn find_icon(s: &[char], from: usize) -> Option<(usize, usize)> {
    (from..s.len()).find_map(|i| icon_at(s, i).map(|end| (i, end)))
}

fn icon_at(s: &[char], i: usize) -> Option<usize> {
    if !chars::starts_with(s, i, &['f', 'a']) {
        return None;
    }
    let mut k = i + 2;
    if s.get(k).is_some_and(|c| "bklrs".contains(*c)) {
        k += 1;
    }
    if !chars::starts_with(s, k, &[':', 'f', 'a', '-']) {
        return None;
    }
    let name = k + 4;
    let end = name
        + s[name..]
            .iter()
            .take_while(|&&c| is_word(c) || c == '-')
            .count();
    (end > name).then_some(end)
}

fn has_markup(raw: &str) -> bool {
    if raw.contains(['<', '\\', '\n', '*', '_']) {
        return true;
    }
    let s: Vec<char> = raw.chars().collect();
    find_icon(&s, 0).is_some()
}

fn push_plain(line: &mut Vec<Span>, text: &str, style: Style) {
    let decoded = decode_entities(text);
    match line.last_mut() {
        Some(last) if last.is_plain() && last.style == style => last.text.push_str(&decoded),
        _ => line.push(Span::text(decoded, style, 0.0)),
    }
}

/// Pushes text, with the math of the label put back where its marks are.
fn push_text(line: &mut Vec<Span>, text: &str, style: Style, env: Env<'_>) {
    if text.is_empty() {
        return;
    }
    let Some(found) = env.marks.filter(|_| text.chars().any(is_mark)) else {
        push_plain(line, text, style);
        return;
    };
    let mut plain = String::new();
    for c in text.chars() {
        if !is_mark(c) {
            plain.push(c);
            continue;
        }
        if !plain.is_empty() {
            push_plain(line, &plain, style);
            plain.clear();
        }
        let at = (c as u32 - MARK) as usize;
        match found.math.get(at).and_then(Option::as_ref) {
            Some(math) => line.push(Span {
                text: String::new(),
                style,
                width: math.width(),
                icon: None,
                math: Some(math.clone()),
            }),
            // Math the host does not typeset stays as it was written.
            None => push_plain(line, &format!("$${}$$", found.tex[at]), style),
        }
    }
    if !plain.is_empty() {
        push_plain(line, &plain, style);
    }
}

/// Pushes text with its icon references: an icon the host has, and for one it has not, nothing,
/// not even the gap, nor the space that set the reference off from the text.
fn push_icons(line: &mut Vec<Span>, text: &[char], style: Style, env: Env<'_>) {
    let mut last = 0;
    while let Some((start, end)) = find_icon(text, last) {
        push_text(line, &chars::string(&text[last..start]), style, env);
        last = end;
        let name = chars::string(&text[start..end]);
        if let Some(markup) = env.host.and_then(|host| host.icon(&name)) {
            line.push(Span {
                text: String::new(),
                style,
                width: 0.0,
                icon: Some(Icon { name, markup }),
                math: None,
            });
        } else if text.get(last) == Some(&' ') {
            last += 1;
        } else if last == text.len() {
            if let Some(before) = line.last_mut().filter(|before| before.is_plain()) {
                before.text = trim_end(&before.text).to_string();
            }
        }
    }
    push_text(line, &chars::string(&text[last..]), style, env);
}

struct Open {
    style: Style,
    /// Sub/sup stack; only the innermost tag shifts text.
    shifts: Vec<Style>,
}

/// Matches an HTML tag at `i`. Returns its lowercased name and length.
fn match_tag(s: &[char], i: usize) -> Option<(String, usize)> {
    let mut j = i + 1;
    if s.get(j) == Some(&'/') {
        j += 1;
    }
    let start = j;
    if !s.get(j).is_some_and(char::is_ascii_alphabetic) {
        return None;
    }
    j += 1;
    while s.get(j).is_some_and(|&c| is_word(c) || c == '-') {
        j += 1;
    }
    let name = chars::string(&s[start..j]).to_ascii_lowercase();
    if s.get(j).is_some_and(|&c| is_space(c)) {
        let k = j
            + 1
            + s[j + 1..]
                .iter()
                .take_while(|&&c| c != '<' && c != '>')
                .count();
        return (s.get(k) == Some(&'>')).then(|| (name, k + 1 - i));
    }
    if s.get(j) == Some(&'/') {
        j += 1;
    }
    (s.get(j) == Some(&'>')).then(|| (name, j + 1 - i))
}

fn tag_style(tag: &str) -> Style {
    TAG_STYLES
        .iter()
        .find(|&&(name, _)| name == tag)
        .map_or(Style::NONE, |&(_, style)| style)
}

/// Splits inline HTML and, for markdown labels, emphasis markers into styled spans.
fn inline(
    line: &mut Vec<Span>,
    source: &[char],
    markdown: bool,
    base: Style,
    open: &mut Open,
    env: Env<'_>,
) {
    let shift = Style::SUB | Style::SUP;
    let mut style = open.style;
    let mut marked = Style::NONE;
    let mut text: Vec<char> = Vec::new();
    let n = source.len();
    let mut i = 0;
    let flush = |line: &mut Vec<Span>, text: &mut Vec<char>, style: Style, open: &Open| {
        let shifted = style | open.shifts.last().copied().unwrap_or(Style::NONE);
        push_icons(line, text, shifted, env);
        text.clear();
    };
    while i < n {
        let ch = source[i];
        if ch == '<' {
            if let Some((tag, len)) = match_tag(source, i) {
                let closing = source[i + 1] == '/';
                let bit = tag_style(&tag);
                if bit.has(shift) {
                    flush(line, &mut text, style, open);
                    let at = open.shifts.iter().rposition(|&s| s == bit);
                    if !closing {
                        open.shifts.push(bit);
                    } else if let Some(at) = at {
                        open.shifts.remove(at);
                    }
                } else if !bit.is_empty() {
                    flush(line, &mut text, style, open);
                    if closing {
                        open.style = (open.style & !bit) | (base & bit);
                        style = (style & !bit) | (base & bit);
                    } else {
                        open.style |= bit;
                        style |= bit;
                    }
                }
                // Other HTML tags are dropped and their text kept. Anything else shaped like a
                // tag, such as the type in List<T>, is text.
                if !bit.is_empty() || HTML_TAGS.contains(&tag.as_str()) {
                    i += len;
                    continue;
                }
            }
        } else if markdown && (ch == '*' || ch == '_') {
            let double = source.get(i + 1) == Some(&ch);
            let (bit, len) = if double {
                (Style::BOLD, 2)
            } else {
                (Style::ITALIC, 1)
            };
            let opening = !marked.has(bit);
            let prev = if i > 0 { source[i - 1] } else { ' ' };
            let next = source.get(i + len).copied().unwrap_or(' ');
            let marker: &[char] = if double { &[ch, ch] } else { &[ch] };
            let can_open =
                opening && !is_space(next) && chars::find(source, marker, i + len).is_some();
            let can_close = !opening && !is_space(prev);
            let wordy = ch == '_' && is_word(prev) && is_word(next);
            if (can_open || can_close) && !wordy {
                flush(line, &mut text, style, open);
                if can_open {
                    marked |= bit;
                    style |= bit;
                } else {
                    marked = marked & !bit;
                    style = (style & !bit) | (base & bit);
                }
                i += len;
                continue;
            }
        }
        text.push(ch);
        i += 1;
    }
    flush(line, &mut text, style, open);
}

fn measure_spans(line: &mut [Span], measurer: &dyn TextMeasurer, size: f64) -> f64 {
    let mut total = 0.0;
    for span in line {
        span.width = if span.icon.is_some() {
            size * ICON_ADVANCE
        } else if let Some(math) = &span.math {
            math.width()
        } else {
            measurer.width(&span.text, size, span.style)
        };
        total += span.width;
    }
    total
}

/// The end of the word-like piece that starts at `q`, which is not whitespace.
fn piece_at(s: &[char], q: usize) -> Option<usize> {
    let len = s.len();
    if q >= len {
        return None;
    }
    // Opening punctuation, a Chinese or Japanese character, and closing punctuation.
    let opens = s[q..].iter().take_while(|&&c| is_open(c)).count();
    for j in (0..=opens).rev() {
        if q + j < len && is_cjk(s[q + j]) {
            let end = q + j + 1;
            return Some(end + s[end..].iter().take_while(|&&c| is_close(c)).count());
        }
    }
    // A word, with any break hints and closing punctuation after it.
    let word = s[q..]
        .iter()
        .take_while(|&&c| !is_space(c) && !is_cjk(c) && !is_hint(c))
        .count();
    if word > 0 {
        let mut end = q + word;
        end += s[end..].iter().take_while(|&&c| is_hint(c)).count();
        end += s[end..].iter().take_while(|&&c| is_close(c)).count();
        return Some(end);
    }
    // A run of break hints.
    let hints = s[q..].iter().take_while(|&&c| is_hint(c)).count();
    (hints > 0).then_some(q + hints)
}

/// Splits text into the pieces a line may break between, each with the whitespace around it.
fn pieces(text: &str) -> Vec<String> {
    let s: Vec<char> = text.chars().collect();
    let spaces = |from: usize| s[from..].iter().take_while(|&&c| is_space(c)).count();
    let mut out = Vec::new();
    let mut p = 0;
    while p < s.len() {
        let q = p + spaces(p);
        let end = match piece_at(&s, q) {
            Some(end) => end + spaces(end),
            None if q > p => q,
            None => p + 1,
        };
        out.push(chars::string(&s[p..end]));
        p = end;
    }
    out
}

struct Wrapper<'a> {
    measurer: &'a dyn TextMeasurer,
    size: f64,
    current: Vec<Span>,
    width: f64,
    out: &'a mut Vec<Vec<Span>>,
    widths: &'a mut Vec<f64>,
}

impl Wrapper<'_> {
    fn measure(&self, text: &str, style: Style) -> f64 {
        self.measurer.width(text, self.size, style)
    }

    fn commit(&mut self, hyphen: bool) {
        if let Some(last) = self.current.last().filter(|last| last.is_plain()) {
            let mut trimmed = trim_end(&last.text).to_string();
            if hyphen && last.text.ends_with(SOFT_HYPHEN) {
                trimmed.pop();
                trimmed.push('-');
            }
            if trimmed != last.text {
                let w = self.measure(&trimmed, last.style);
                let last = self.current.last_mut().expect("checked above");
                self.width += w - last.width;
                last.width = w;
                last.text = trimmed;
            }
        }
        self.out.push(std::mem::take(&mut self.current));
        self.widths.push(self.width);
        self.width = 0.0;
    }
}

fn wrap(
    line: &[Span],
    measurer: &dyn TextMeasurer,
    size: f64,
    max: f64,
    out: &mut Vec<Vec<Span>>,
    widths: &mut Vec<f64>,
) {
    let mut w = Wrapper {
        measurer,
        size,
        current: Vec::new(),
        width: 0.0,
        out,
        widths,
    };
    for (index, span) in line.iter().enumerate() {
        if !span.is_plain() {
            // An icon or math is not broken, and starts a line if it does not fit the one it is on.
            let advance = span
                .math
                .as_ref()
                .map_or(size * ICON_ADVANCE, |math| math.width());
            if w.width > 0.0 && w.width + advance > max {
                w.commit(false);
            }
            w.current.push(Span {
                width: advance,
                ..span.clone()
            });
            w.width += advance;
            continue;
        }
        let pieces = pieces(&span.text);
        // A piece ending in a soft hyphen fits if its '-' does, or if the rest of the word
        // does, which hides the hyphen. The line only grows along a word, so no later hyphen in
        // it fits either, and the remaining width of each word makes this one check.
        let soft = span.text.contains(SOFT_HYPHEN);
        let n = if soft { pieces.len() } else { 0 };
        let mut own = vec![0.0; n];
        let mut rest = vec![0.0; n + 1];
        for q in (0..n).rev() {
            // A soft hyphen before a space is no break of its own: the space is.
            let shy = pieces[q].ends_with(SOFT_HYPHEN);
            let end = trim_end(&pieces[q]);
            let end = if shy {
                &end[..end.len() - SOFT_HYPHEN.len_utf8()]
            } else {
                end
            };
            own[q] = w.measure(end, span.style);
            // Word width beyond this span is unknown.
            rest[q] = if !shy {
                own[q]
            } else if q == n - 1 {
                if index == line.len() - 1 {
                    own[q]
                } else {
                    f64::INFINITY
                }
            } else {
                own[q] + rest[q + 1]
            };
        }
        for (p, piece) in pieces.iter().enumerate() {
            let mut part = piece.as_str();
            let part_width = if soft {
                own[p]
            } else {
                w.measure(trim_end(part), span.style)
            };
            let fits = if !soft || !part.ends_with(SOFT_HYPHEN) {
                w.width + part_width <= max
            } else {
                let hyphenated = format!("{}-", &part[..part.len() - SOFT_HYPHEN.len_utf8()]);
                w.width + w.measure(&hyphenated, span.style) <= max || w.width + rest[p] <= max
            };
            if w.width > 0.0 && !fits {
                // Breaking at whitespace leaves a preceding soft hyphen hidden.
                w.commit(trim_start(part) == part);
                part = trim_start(part);
            }
            match w.current.last() {
                Some(last) if last.is_plain() && last.style == span.style => {
                    let mut text = last.text.clone();
                    text.push_str(part);
                    // Remeasuring for each zero-width piece would take quadratic time.
                    let width = if part_width > 0.0 || trim_end(part) != part {
                        Some(w.measure(&text, span.style))
                    } else {
                        None
                    };
                    let last = w.current.last_mut().expect("matched above");
                    last.text = text;
                    if let Some(width) = width {
                        w.width += width - last.width;
                        last.width = width;
                    }
                }
                _ => {
                    let width = w.measure(part, span.style);
                    w.current
                        .push(Span::text(part.to_string(), span.style, width));
                    w.width += width;
                }
            }
        }
    }
    if !w.current.is_empty() || w.out.is_empty() {
        w.commit(false);
    }
}

fn finish(lines: Vec<Vec<Span>>, widths: Vec<f64>, size: f64, line_height: f64) -> Label {
    let width = widths.iter().copied().fold(0.0, f64::max);
    let snug = if lines.len() > 1 {
        round(size * WRAPPED_LINE_HEIGHT)
    } else {
        line_height
    };
    Label {
        height: lines.len() as f64 * snug,
        lines,
        widths,
        width,
        size,
        line_height: snug,
        heights: None,
    }
}

/// Lays out label text: splits it at explicit breaks, applies inline HTML emphasis (and
/// markdown emphasis when `markdown` is set), and wraps lines wider than `max_width`.
pub(crate) fn layout_label(
    raw: Option<&str>,
    markdown: bool,
    measurer: &dyn TextMeasurer,
    size: f64,
    max_width: f64,
    base: Style,
) -> Label {
    let Some(raw) = raw.filter(|raw| !raw.is_empty()) else {
        return Label::default();
    };
    let line_height = round(size * LINE_HEIGHT);
    let host = measurer.host();

    if let Some(host) = host.filter(|host| host.math_active()) {
        if raw.contains("$$") && !raw.chars().any(is_mark) {
            if let Some((marked, found)) = find_math(raw, host, size) {
                let env = Env {
                    host: Some(host),
                    marks: Some(&found),
                };
                let label = layout_lines(&marked, markdown, measurer, size, max_width, base, env);
                return with_heights(label);
            }
        }
    }

    if !has_markup(raw) && !raw.contains('&') && !raw.contains('ﬂ') {
        let text = trim(raw);
        let width = measurer.width(text, size, base);
        let breakable = text.contains(' ') || text.contains(is_cjk) || text.contains(is_hint);
        let span = Span::text(text.to_string(), base, width);
        if width <= max_width || !breakable {
            return Label {
                lines: vec![vec![span]],
                widths: vec![width],
                width,
                height: line_height,
                size,
                line_height,
                heights: None,
            };
        }
        let (mut lines, mut widths) = (Vec::new(), Vec::new());
        wrap(&[span], measurer, size, max_width, &mut lines, &mut widths);
        return finish(lines, widths, size, line_height);
    }
    let env = Env { host, marks: None };
    layout_lines(raw, markdown, measurer, size, max_width, base, env)
}

/// Finds the `$$…$$` of a label, as the pattern `\$\$(.+?)\$\$` does, and types them with the
/// host. Returns the text with each expression replaced by a mark.
fn find_math(raw: &str, host: &Host<'_>, size: f64) -> Option<(String, Marks)> {
    let s: Vec<char> = raw.chars().collect();
    let is_break = |c: char| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}');
    let dollars = |i: usize| s.get(i) == Some(&'$') && s.get(i + 1) == Some(&'$');
    let mut out = String::new();
    let mut found = Marks {
        tex: Vec::new(),
        math: Vec::new(),
    };
    let mut i = 0;
    let mut copied = 0;
    while i < s.len() {
        if !dollars(i) {
            i += 1;
            continue;
        }
        // The shortest run of one or more characters on the line, up to the next `$$`.
        let mut k = i + 3;
        while k <= s.len() && !dollars(k) && !is_break(s[k - 1]) {
            k += 1;
        }
        if k > s.len() || is_break(s[k - 1]) || !dollars(k) {
            i += 1;
            continue;
        }
        let end = k + 2;
        if found.tex.len() < MAX_MARKS {
            out.push_str(&chars::string(&s[copied..i]));
            out.push(
                char::from_u32(MARK + found.tex.len() as u32).expect("in the private use area"),
            );
            found.tex.push(chars::string(&s[i + 2..k]));
            copied = end;
        }
        i = end;
    }
    if found.tex.is_empty() {
        return None;
    }
    out.push_str(&chars::string(&s[copied..]));
    for tex in &found.tex {
        found.math.push(host.typeset(&decode_entities(tex), size));
    }
    Some((out, found))
}

/// Gives the lines that hold math the height the math needs.
pub(crate) fn with_heights(label: Label) -> Label {
    let mut heights: Option<Vec<f64>> = None;
    for (i, line) in label.lines.iter().enumerate() {
        let mut h = label.line_height;
        for span in line {
            if let Some(math) = &span.math {
                h = h.max((math.height() + label.size / 4.0).ceil());
            }
        }
        if h != label.line_height && heights.is_none() {
            heights = Some(vec![label.line_height; label.lines.len()]);
        }
        if let Some(heights) = heights.as_mut() {
            heights[i] = h;
        }
    }
    match heights {
        None => Label {
            height: label.lines.len() as f64 * label.line_height,
            heights: None,
            ..label
        },
        Some(heights) => Label {
            height: heights.iter().sum(),
            heights: Some(heights),
            ..label
        },
    }
}

pub(crate) fn has_math(line: &[Span]) -> bool {
    line.iter().any(|span| span.math.is_some())
}

/// The distance from the top of a label to the middle of its line `i`.
pub(crate) fn line_middle(label: &Label, i: usize) -> f64 {
    match &label.heights {
        None => (i as f64 + 0.5) * label.line_height,
        Some(heights) => heights[..i].iter().sum::<f64>() + heights[i] / 2.0,
    }
}

/// Line `i` of a label, as a label of its own.
pub(crate) fn label_line(label: &Label, i: usize) -> Label {
    let width = label.widths[i];
    let height = label.heights.as_ref().map_or(label.line_height, |h| h[i]);
    Label {
        lines: vec![label.lines[i].clone()],
        widths: vec![width],
        width,
        height,
        size: label.size,
        line_height: label.line_height,
        heights: (height != label.line_height).then(|| vec![height]),
    }
}

/// Removes the indentation all non-blank lines share.
fn dedent(text: &str) -> String {
    let indent = |line: &str| line.len() - trim_start(line).len();
    let min = text
        .split('\n')
        .filter(|line| !trim(line).is_empty())
        .map(indent)
        .min()
        .unwrap_or(0);
    if min == 0 {
        return text.to_string();
    }
    text.split('\n')
        .map(|line| {
            let cut = min.min(indent(line));
            // Indents are counted in bytes; a shared width may end inside a wide space.
            let cut = (0..=cut)
                .rev()
                .find(|&i| line.is_char_boundary(i))
                .unwrap_or(0);
            &line[cut..]
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Splits text at `<br>`, `<br/>`, `\n` written out, and line breaks.
fn split_breaks(source: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut i = 0;
    let bytes = source.as_bytes();
    while i < bytes.len() {
        let len = match bytes[i] {
            b'\n' => 1,
            b'\\' if bytes.get(i + 1) == Some(&b'n') => 2,
            b'<' if source[i + 1..]
                .get(..2)
                .is_some_and(|s| s.eq_ignore_ascii_case("br")) =>
            {
                let after = trim_start(&source[i + 3..]);
                let after = after.strip_prefix('/').unwrap_or(after);
                if after.starts_with('>') {
                    source.len() - after.len() + 1 - i
                } else {
                    0
                }
            }
            _ => 0,
        };
        if len > 0 {
            parts.push(&source[start..i]);
            i += len;
            start = i;
        } else {
            i += 1;
        }
    }
    parts.push(&source[start..]);
    parts
}

fn layout_lines(
    raw: &str,
    markdown: bool,
    measurer: &dyn TextMeasurer,
    size: f64,
    max_width: f64,
    base: Style,
    env: Env<'_>,
) -> Label {
    let line_height = round(size * LINE_HEIGHT);
    let normalized;
    let source = if markdown {
        normalized = dedent(&raw.replace("\r\n", "\n").replace('\r', "\n"));
        trim(&normalized)
    } else {
        trim(raw)
    };
    let mut lines = Vec::new();
    let mut widths = Vec::new();
    // HTML tags can span explicit breaks; Markdown emphasis stays local to each line.
    let mut open = Open {
        style: base,
        shifts: Vec::new(),
    };
    for part in split_breaks(source) {
        let part: Vec<char> = trim(part).chars().collect();
        let mut line = Vec::new();
        inline(&mut line, &part, markdown, base, &mut open, env);
        let w = measure_spans(&mut line, measurer, size);
        if w > max_width {
            wrap(&line, measurer, size, max_width, &mut lines, &mut widths);
        } else {
            lines.push(line);
            widths.push(w);
        }
    }
    finish(lines, widths, size, line_height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::MetricsMeasurer;

    fn texts(label: &Label) -> Vec<Vec<(String, Style)>> {
        label
            .lines
            .iter()
            .map(|line| line.iter().map(|s| (s.text.clone(), s.style)).collect())
            .collect()
    }

    fn layout(raw: &str, markdown: bool, max: f64) -> Label {
        layout_label(
            Some(raw),
            markdown,
            &MetricsMeasurer,
            16.0,
            max,
            Style::NONE,
        )
    }

    /// What `find_math` finds in a text: the marked text, and the TeX of each expression.
    fn math_of(raw: &str) -> Option<(String, Vec<String>)> {
        let resolver: crate::text::MathResolver = Box::new(|_, _| None);
        let host = Host::new(None, Some(&resolver));
        find_math(raw, &host, 16.0).map(|(text, found)| (text, found.tex))
    }

    #[test]
    fn finds_math_as_the_pattern_does() {
        let mark = |n: u32| char::from_u32(MARK + n).unwrap();
        let (text, tex) = math_of("a $$x^2$$ b $$y$$ c").unwrap();
        assert_eq!(text, format!("a {} b {} c", mark(0), mark(1)));
        assert_eq!(tex, ["x^2", "y"]);
        // The shortest match, of at least one character.
        let (_, tex) = math_of("$$$$$").unwrap();
        assert_eq!(tex, ["$"]);
        assert!(math_of("$$$$").is_none());
        // A match stays on its line.
        assert!(math_of("$$a\nb$$").is_none());
        assert!(math_of("$$ open").is_none());
        let (_, tex) = math_of("$$ a\n$$b$$").unwrap();
        assert_eq!(tex, ["b"]);
    }

    #[test]
    fn splits_pieces() {
        assert_eq!(pieces("ab cd  ef"), ["ab ", "cd  ", "ef"]);
        assert_eq!(pieces("中文，字(測"), ["中", "文，", "字", "(測"]);
        assert_eq!(pieces(" x "), [" x "]);
        assert_eq!(pieces("   "), ["   "]);
        assert_eq!(pieces("co\u{ad}op"), ["co\u{ad}", "op"]);
    }

    #[test]
    fn keeps_a_short_label_on_one_line() {
        let label = layout("  Hello world ", false, 200.0);
        assert_eq!(texts(&label), [[("Hello world".to_string(), Style::NONE)]]);
        assert_eq!(label.height, 24.0);
    }

    #[test]
    fn wraps_at_spaces() {
        let label = layout("alpha beta gamma delta", false, 60.0);
        let lines: Vec<&str> = label.lines.iter().map(|l| l[0].text.as_str()).collect();
        assert_eq!(lines, ["alpha", "beta", "gamma", "delta"]);
        assert_eq!(label.widths, [39.136, 31.136, 53.344, 34.688]);
        assert_eq!(label.line_height, 21.0);
        assert_eq!(label.height, 84.0);
    }

    #[test]
    fn splits_breaks_and_styles() {
        let label = layout("<b>bold</b> and <i>it</i><br/>next\\nlast", false, 1000.0);
        assert_eq!(
            texts(&label),
            vec![
                vec![
                    ("bold".to_string(), Style::BOLD),
                    (" and ".to_string(), Style::NONE),
                    ("it".to_string(), Style::ITALIC),
                ],
                vec![("next".to_string(), Style::NONE)],
                vec![("last".to_string(), Style::NONE)],
            ]
        );
    }

    #[test]
    fn keeps_unknown_tags_as_text() {
        let label = layout("List<T> <span>x</span>", false, 1000.0);
        assert_eq!(label.lines[0][0].text, "List<T> x");
    }

    #[test]
    fn applies_markdown_emphasis() {
        let label = layout("**bold** and _it_ snake_case", true, 1000.0);
        assert_eq!(
            texts(&label),
            [[
                ("bold".to_string(), Style::BOLD),
                (" and ".to_string(), Style::NONE),
                ("it".to_string(), Style::ITALIC),
                (" snake_case".to_string(), Style::NONE),
            ]]
        );
    }

    #[test]
    fn drops_icon_references() {
        let label = layout("fa:fa-car Car", false, 1000.0);
        assert_eq!(label.lines[0][0].text, "Car");
    }

    #[test]
    fn decodes_entities() {
        let label = layout("a &amp; b", false, 1000.0);
        assert_eq!(label.lines[0][0].text, "a & b");
    }

    #[test]
    fn shows_a_hyphen_at_a_soft_break() {
        let label = layout("extra\u{ad}ordinary", false, 50.0);
        assert_eq!(label.lines[0][0].text, "extra-");
        assert_eq!(label.lines[1][0].text, "ordinary");
    }

    #[test]
    fn dedents_markdown() {
        assert_eq!(dedent("  a\n    b\n\n  c"), "a\n  b\n\nc");
    }

    /// Expected values come from pele's `layoutLabel` with `metricsMeasurer` at e7f092a.
    #[test]
    fn matches_pele() {
        let cases: &[(&str, bool, f64, f64, u8, &str)] = &[
            (
                "The quick brown fox jumps over the lazy dog",
                false,
                16.0,
                100.0,
                0,
                "[The quick]0:69.36 | [brown fox]0:69.36 | [jumps over]0:78.24 | [the lazy dog]0:86.272 w=86.272 h=84 lh=21",
            ),
            (
                "Supercalifragilisticexpialidocious",
                false,
                16.0,
                80.0,
                0,
                "[Supercalifragilisticexpialidocious]0:229.392 w=229.392 h=24 lh=24",
            ),
            (
                "這是一個很長的中文標籤，需要換行顯示。",
                false,
                16.0,
                80.0,
                0,
                "[這是一個很]0:80 | [長的中文標]0:80 | [籤，需要換]0:80 | [行顯示。]0:64 w=80 h=84 lh=21",
            ),
            (
                "Rust 語言渲染 Mermaid 圖表",
                false,
                16.0,
                70.0,
                0,
                "[Rust 語言]0:69.344 | [渲染]0:32 | [Mermaid]0:62.224 | [圖表]0:32 w=69.344 h=84 lh=21",
            ),
            (
                "「引號」測試（括號）文字",
                false,
                16.0,
                40.0,
                0,
                "[「引]0:32 | [號」]0:32 | [測試]0:32 | [（括]0:32 | [號）]0:32 | [文字]0:32 w=32 h=126 lh=21",
            ),
            (
                "inter\u{ad}national\u{ad}ization",
                false,
                16.0,
                70.0,
                0,
                "[inter-]0:36.448 | [national-]0:61.36 | [ization]0:46.24 w=61.36 h=63 lh=21",
            ),
            (
                "**Bold** text with `code` and _italic_ words",
                true,
                16.0,
                120.0,
                0,
                "[Bold]1:35.552 [ text with]0:63.136 | [`code` and ]0:80.928 [italic]2:32 | [words]0:42.672 w=112.928 h=63 lh=21",
            ),
            (
                "Line one\nline *two* is longer than the first",
                true,
                14.0,
                100.0,
                0,
                "[Line one]0:53.704 | [line ]0:25.676 [two]2:21.784 [ is]0:14 | [longer than the]0:93.394 | [first]0:22.554 w=93.394 h=72 lh=18",
            ),
            (
                "<b>bold</b> <i>it</i> <u>u</u> H<sub>2</sub>O x<sup>2</sup>",
                false,
                16.0,
                1000.0,
                0,
                "[bold]1:33.776 [ ]0:4.448 [it]2:8 [ ]0:4.448 [u]32:8.896 [ H]0:16 [2]8:7.413333333333334 [O x]0:24.896 [2]16:7.413333333333334 w=115.29066666666668 h=24 lh=24",
            ),
            (
                "Header title text",
                false,
                16.0,
                60.0,
                1,
                "[Header]1:54.24 | [title]1:28.448 | [text]1:28.448 w=54.24 h=63 lh=21",
            ),
            (
                "a   b    c",
                false,
                16.0,
                1000.0,
                0,
                "[a   b    c]0:56.928 w=56.928 h=24 lh=24",
            ),
            (
                "path/to/some-long-file_name.rs",
                false,
                12.0,
                60.0,
                0,
                "[path/to/some-long-file_name.rs]0:165.396 w=165.396 h=18 lh=18",
            ),
        ];
        for &(raw, markdown, size, max, base, expected) in cases {
            let label = layout_label(
                Some(raw),
                markdown,
                &MetricsMeasurer,
                size,
                max,
                Style(base),
            );
            let lines: Vec<String> = label
                .lines
                .iter()
                .map(|line| {
                    let spans: Vec<String> = line
                        .iter()
                        .map(|s| format!("[{}]{}:{}", s.text, s.style.0, s.width))
                        .collect();
                    spans.join(" ")
                })
                .collect();
            let got = format!(
                "{} w={} h={} lh={}",
                lines.join(" | "),
                label.width,
                label.height,
                label.line_height
            );
            assert_eq!(got, expected, "{raw:?}");
        }
    }

    #[test]
    fn empty_label() {
        assert_eq!(
            layout_label(None, false, &MetricsMeasurer, 16.0, 100.0, Style::NONE),
            Label::default()
        );
    }
}
