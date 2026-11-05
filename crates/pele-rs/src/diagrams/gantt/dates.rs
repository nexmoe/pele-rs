//! JavaScript's `Date`, and the parts of dayjs 1.11 that Mermaid's gantt code relies on, with
//! the plugins it loads (custom parse format, advanced format, ISO week) and the English
//! locale: strict parsing against a format, formatting, and calendar arithmetic. Ported from
//! pele's `diagrams/gantt/dates.ts`.
//!
//! pele works in the local time of the machine it runs on. pele-rs has no time zone data, and
//! its output must not depend on where it runs, so local time here is UTC.

use crate::util::{is_space, js_number, js_to_number};

pub(crate) const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
pub(crate) const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

pub(super) const MS_DAY: f64 = 86_400_000.0;
const WEEK: f64 = 7.0 * MS_DAY;

/// `ToIntegerOrInfinity`.
fn int(x: f64) -> f64 {
    if x.is_nan() { 0.0 } else { x.trunc() + 0.0 }
}

/// Days from 1970-01-01 to the first of a month, for a proleptic Gregorian year and a month
/// from 0.
fn days_from_civil(year: f64, month: f64) -> f64 {
    let y = if month < 2.0 { year - 1.0 } else { year };
    let era = (y / 400.0).floor();
    let yoe = y - era * 400.0;
    let m = month + 1.0;
    let doy = ((153.0 * (if m > 2.0 { m - 3.0 } else { m + 9.0 }) + 2.0) / 5.0).floor();
    let doe = yoe * 365.0 + (yoe / 4.0).floor() - (yoe / 100.0).floor() + doy;
    era * 146_097.0 + doe - 719_468.0
}

/// The year, month from 0, and day of the month of a day number.
fn civil_from_days(z: f64) -> (f64, f64, f64) {
    let z = z + 719_468.0;
    let era = (z / 146_097.0).floor();
    let doe = z - era * 146_097.0;
    let yoe = ((doe - (doe / 1460.0).floor() + (doe / 36524.0).floor()
        - (doe / 146_096.0).floor())
        / 365.0)
        .floor();
    let doy = doe - (365.0 * yoe + (yoe / 4.0).floor() - (yoe / 100.0).floor());
    let mp = ((5.0 * doy + 2.0) / 153.0).floor();
    let d = doy - ((153.0 * mp + 2.0) / 5.0).floor() + 1.0;
    let m = if mp < 10.0 { mp + 3.0 } else { mp - 9.0 };
    let y = yoe + era * 400.0 + if m <= 2.0 { 1.0 } else { 0.0 };
    (y, m - 1.0, d)
}

/// `MakeDay`.
pub(super) fn make_day(year: f64, month: f64, date: f64) -> f64 {
    if !year.is_finite() || !month.is_finite() || !date.is_finite() {
        return f64::NAN;
    }
    let (y, m, dt) = (int(year), int(month), int(date));
    let ym = y + (m / 12.0).floor();
    if ym.abs() > 400_000.0 {
        return f64::NAN;
    }
    let mn = m.rem_euclid(12.0);
    days_from_civil(ym, mn) + dt - 1.0
}

/// `MakeTime`.
pub(super) fn make_time(h: f64, m: f64, s: f64, ms: f64) -> f64 {
    if !h.is_finite() || !m.is_finite() || !s.is_finite() || !ms.is_finite() {
        return f64::NAN;
    }
    int(h) * 3_600_000.0 + int(m) * 60_000.0 + int(s) * 1000.0 + int(ms)
}

/// `TimeClip`.
fn time_clip(t: f64) -> f64 {
    if !t.is_finite() || t.abs() > 8.64e15 {
        f64::NAN
    } else {
        int(t)
    }
}

/// A JavaScript `Date`: milliseconds since the epoch, or NaN for an invalid date.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Date(pub f64);

impl Date {
    pub(crate) fn new(t: f64) -> Date {
        Date(time_clip(t))
    }

    /// `new Date(year, month, day, hours, minutes, seconds, ms)`, which reads years 0 to 99
    /// as 1900 to 1999.
    pub(crate) fn local(y: f64, m: f64, d: f64, h: f64, mi: f64, s: f64, ms: f64) -> Date {
        Date(time_clip(Self::utc(y, m, d, h, mi, s, ms)))
    }

    /// `Date.UTC`.
    pub(crate) fn utc(y: f64, m: f64, d: f64, h: f64, mi: f64, s: f64, ms: f64) -> f64 {
        let y = if y.is_finite() && (0.0..=99.0).contains(&int(y)) {
            1900.0 + int(y)
        } else {
            y
        };
        time_clip(make_day(y, m, d) * MS_DAY + make_time(h, mi, s, ms))
    }

    pub(crate) fn time(self) -> f64 {
        self.0
    }

    pub(crate) fn valid(self) -> bool {
        !self.0.is_nan()
    }

    fn day_number(self) -> f64 {
        (self.0 / MS_DAY).floor()
    }

    fn within_day(self) -> f64 {
        self.0.rem_euclid(MS_DAY)
    }

    fn civil(self) -> (f64, f64, f64) {
        if self.0.is_nan() {
            (f64::NAN, f64::NAN, f64::NAN)
        } else {
            civil_from_days(self.day_number())
        }
    }

    pub(crate) fn year(self) -> f64 {
        self.civil().0
    }

    pub(crate) fn month(self) -> f64 {
        self.civil().1
    }

    pub(crate) fn date(self) -> f64 {
        self.civil().2
    }

    /// The day of the week, 0 for Sunday.
    pub(crate) fn weekday(self) -> f64 {
        (self.day_number() + 4.0).rem_euclid(7.0)
    }

    pub(crate) fn hours(self) -> f64 {
        (self.within_day() / 3_600_000.0).floor()
    }

    pub(crate) fn minutes(self) -> f64 {
        (self.within_day() / 60_000.0).floor() % 60.0
    }

    pub(crate) fn seconds(self) -> f64 {
        (self.within_day() / 1000.0).floor() % 60.0
    }

    pub(crate) fn millis(self) -> f64 {
        self.within_day() % 1000.0
    }

    pub(crate) fn set_time(&mut self, t: f64) {
        self.0 = time_clip(t);
    }

    pub(crate) fn set_date(&mut self, d: f64) {
        if self.valid() {
            let (y, m, _) = self.civil();
            self.0 = time_clip(make_day(y, m, d) * MS_DAY + self.within_day());
        }
    }

    pub(crate) fn set_month(&mut self, m: f64, d: Option<f64>) {
        if self.valid() {
            let (y, _, date) = self.civil();
            self.0 = time_clip(make_day(y, m, d.unwrap_or(date)) * MS_DAY + self.within_day());
        }
    }

    /// Sets the year; on an invalid date, of 1970-01-01.
    pub(crate) fn set_full_year(&mut self, y: f64) {
        let base = if self.valid() { *self } else { Date(0.0) };
        let (_, m, d) = base.civil();
        self.0 = time_clip(make_day(y, m, d) * MS_DAY + base.within_day());
    }

    pub(crate) fn set_hours(&mut self, h: f64, m: f64, s: f64, ms: f64) {
        if self.valid() {
            self.0 = time_clip(self.day_number() * MS_DAY + make_time(h, m, s, ms));
        }
    }
}

fn pad(value: f64, length: usize) -> String {
    let s = js_number(value);
    let n = s.encode_utf16().count();
    if n >= length {
        s
    } else {
        "0".repeat(length - n) + &s
    }
}

fn ordinal(n: f64) -> String {
    const S: [&str; 4] = ["th", "st", "nd", "rd"];
    let v = n % 100.0;
    let pick = |i: f64| ((0.0..4.0).contains(&i) && i.fract() == 0.0).then(|| S[i as usize]);
    let suffix = pick((v - 20.0) % 10.0).or_else(|| pick(v)).unwrap_or(S[0]);
    format!("{}{suffix}", js_number(n))
}

pub(crate) fn iso_weekday(date: Date) -> f64 {
    match date.weekday() {
        0.0 => 7.0,
        d => d,
    }
}

pub(crate) fn add_days(date: Date, days: f64) -> Date {
    let mut next = date;
    next.set_date(date.date() + days);
    next
}

fn days_in_month(date: Date) -> f64 {
    let mut last = Date::local(date.year(), date.month() + 1.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    last.set_hours(23.0, 59.0, 59.0, 999.0);
    last.date()
}

/// Sets the month or the year, keeping the day of the month unless the new month is shorter.
fn set_calendar(date: Date, apply: impl FnOnce(&mut Date)) -> Date {
    let mut next = date;
    next.set_date(1.0);
    apply(&mut next);
    next.set_date(date.date().min(days_in_month(next)));
    next
}

/// The units of a duration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Unit {
    Ms,
    S,
    M,
    H,
    D,
    W,
    Month,
    Y,
}

pub(crate) fn add(date: Date, amount: f64, unit: Unit) -> Date {
    match unit {
        Unit::Month => set_calendar(date, |d| d.set_month(date.month() + amount, None)),
        Unit::Y => set_calendar(date, |d| d.set_full_year(date.year() + amount)),
        Unit::D => add_days(date, crate::util::round(amount)),
        Unit::W => add_days(date, crate::util::round(7.0 * amount)),
        Unit::H => Date::new(date.0 + amount * 3_600_000.0),
        Unit::M => Date::new(date.0 + amount * 60_000.0),
        Unit::S => Date::new(date.0 + amount * 1000.0),
        Unit::Ms => Date::new(date.0 + amount),
    }
}

fn week_thursday(date: Date) -> Date {
    add_days(date, 4.0 - iso_weekday(date))
}

fn iso_week(date: Date) -> f64 {
    let thursday = week_thursday(date);
    let jan1 = Date::local(thursday.year(), 0.0, 1.0, 0.0, 0.0, 0.0, 0.0);
    let weekday = iso_weekday(jan1);
    let first = add_days(jan1, 4.0 - weekday + if weekday > 4.0 { 7.0 } else { 0.0 });
    let weeks = (thursday.0 - first.0) / WEEK;
    let whole = if weeks < 0.0 {
        let c = weeks.ceil();
        if c == 0.0 || c.is_nan() { 0.0 } else { c }
    } else {
        weeks.floor()
    };
    whole + 1.0
}

/// Finds the first of `alternatives` that `text` has at `i`.
fn alternative<'a>(text: &[char], i: usize, alternatives: &[&'a str]) -> Option<&'a str> {
    alternatives.iter().copied().find(|a| {
        let mut k = i;
        a.chars().all(|c| {
            let ok = text.get(k) == Some(&c);
            k += 1;
            ok
        })
    })
}

/// `\[([^\]]+)]` at `i`: the length of the match.
fn bracket(text: &[char], i: usize) -> Option<usize> {
    if text.get(i) != Some(&'[') {
        return None;
    }
    let close = text[i + 1..].iter().position(|&c| c == ']')?;
    (close > 0).then_some(close + 2)
}

/// Replaces every leftmost match of `matcher`, as a global `String.prototype.replace` does.
fn replace_all(
    text: &str,
    mut matcher: impl FnMut(&[char], usize) -> Option<(usize, String)>,
) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        match matcher(&chars, i) {
            Some((len, with)) if len > 0 => {
                out += &with;
                i += len;
            }
            _ => {
                out.push(chars[i]);
                i += 1;
            }
        }
    }
    out
}

const ADVANCED: &[&str] = &[
    "Q", "wo", "ww", "w", "WW", "W", "zzz", "z", "gggg", "GGGG", "Do", "X", "x", "kk", "k", "S",
];
const FORMAT: &[&str] = &[
    "YYYY", "YY", "MMMM", "MMM", "MM", "M", "DD", "D", "dddd", "ddd", "dd", "d", "HH", "H", "hh",
    "h", "a", "A", "mm", "m", "ss", "s", "ZZ", "Z", "SSS",
];
const DEFAULT_FORMAT: &str = "YYYY-MM-DDTHH:mm:ssZ";

/// Formats a date as dayjs does. Returns `None` for the week-of-year and time zone name
/// tokens, which need dayjs plugins Mermaid does not load and so fail there.
pub(crate) fn format(date: Date, template: &str) -> Option<String> {
    if !date.valid() {
        return Some("Invalid Date".to_string());
    }
    let s = if template.is_empty() {
        DEFAULT_FORMAT
    } else {
        template
    };
    if s == "YYYY-MM-DD" {
        return Some(iso_date(date));
    }
    let mut unsupported = false;
    let advanced = replace_all(s, |text, i| {
        if let Some(len) = bracket(text, i) {
            return Some((len, text[i..i + len].iter().collect()));
        }
        let token = alternative(text, i, ADVANCED)?;
        let with = match token {
            "Q" => js_number(((date.month() + 1.0) / 3.0).ceil()),
            "Do" => format!("[{}]", ordinal(date.date())),
            "GGGG" => js_number(week_thursday(date).year()),
            "W" => js_number(iso_week(date)),
            "WW" => pad(iso_week(date), 2),
            "k" => js_number(if date.hours() == 0.0 {
                24.0
            } else {
                date.hours()
            }),
            "kk" => pad(
                if date.hours() == 0.0 {
                    24.0
                } else {
                    date.hours()
                },
                2,
            ),
            "X" => js_number((date.0 / 1000.0).floor()),
            "x" => js_number(date.0),
            "gggg" | "wo" | "w" | "ww" | "z" | "zzz" => {
                unsupported = true;
                token.to_string()
            }
            _ => token.to_string(),
        };
        Some((token.len(), with))
    });
    if unsupported {
        return None;
    }
    let hours = date.hours();
    let h12 = if hours % 12.0 == 0.0 {
        12.0
    } else {
        hours % 12.0
    };
    Some(replace_all(&advanced, |text, i| {
        if let Some(len) = bracket(text, i) {
            return Some((len, text[i + 1..i + len - 1].iter().collect()));
        }
        let token = alternative(text, i, FORMAT)?;
        let with = match token {
            "YY" => {
                let y: Vec<char> = js_number(date.year()).chars().collect();
                y[y.len().saturating_sub(2)..].iter().collect()
            }
            "YYYY" => pad(date.year(), 4),
            "M" => js_number(date.month() + 1.0),
            "MM" => pad(date.month() + 1.0, 2),
            "MMM" => MONTHS[date.month() as usize][..3].to_string(),
            "MMMM" => MONTHS[date.month() as usize].to_string(),
            "D" => js_number(date.date()),
            "DD" => pad(date.date(), 2),
            "d" => js_number(date.weekday()),
            "dd" => WEEKDAYS[date.weekday() as usize][..2].to_string(),
            "ddd" => WEEKDAYS[date.weekday() as usize][..3].to_string(),
            "dddd" => WEEKDAYS[date.weekday() as usize].to_string(),
            "H" => js_number(hours),
            "HH" => pad(hours, 2),
            "h" => js_number(h12),
            "hh" => pad(h12, 2),
            "a" => (if hours < 12.0 { "am" } else { "pm" }).to_string(),
            "A" => (if hours < 12.0 { "AM" } else { "PM" }).to_string(),
            "m" => js_number(date.minutes()),
            "mm" => pad(date.minutes(), 2),
            "s" => js_number(date.seconds()),
            "ss" => pad(date.seconds(), 2),
            "SSS" => pad(date.millis(), 3),
            "Z" => "+00:00".to_string(),
            _ => "+0000".to_string(),
        };
        Some((token.len(), with))
    }))
}

pub(crate) fn iso_date(date: Date) -> String {
    let year = date.year();
    let month = date.month() + 1.0;
    let day = date.date();
    let y = if (1000.0..=9999.0).contains(&year) {
        js_number(year)
    } else {
        pad(year, 4)
    };
    format!(
        "{y}{}{}{}{}",
        if month < 10.0 { "-0" } else { "-" },
        js_number(month),
        if day < 10.0 { "-0" } else { "-" },
        js_number(day)
    )
}

#[derive(Debug, Default)]
struct Fields {
    year: Option<f64>,
    month: Option<f64>,
    day: Option<f64>,
    day_given: bool,
    hours: Option<f64>,
    minutes: Option<f64>,
    seconds: Option<f64>,
    milliseconds: Option<f64>,
    week: Option<f64>,
    offset: Option<f64>,
    afternoon: Option<bool>,
}

/// Whether JavaScript would treat an optional number as true.
fn truthy(v: Option<f64>) -> bool {
    v.is_some_and(|v| v != 0.0 && !v.is_nan())
}

/// The patterns dayjs matches a format token's input with.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Pattern {
    /// Exactly this many digits.
    Digits(usize),
    /// One or two digits.
    OneOrTwo,
    Signed,
    Offset,
    Word,
}

fn digit(c: u16) -> bool {
    (48..=57).contains(&c)
}

fn digits_from(s: &[u16], i: usize) -> usize {
    s[i..].iter().take_while(|&&c| digit(c)).count()
}

impl Pattern {
    /// The length of a match at `i`.
    fn at(self, s: &[u16], i: usize) -> Option<usize> {
        match self {
            Pattern::Digits(n) => {
                (s.len() - i >= n && s[i..i + n].iter().all(|&c| digit(c))).then_some(n)
            }
            Pattern::OneOrTwo => match digits_from(s, i) {
                0 => None,
                1 => Some(1),
                _ => Some(2),
            },
            Pattern::Signed => {
                let sign = usize::from(matches!(s[i], 43 | 45));
                match digits_from(s, i + sign) {
                    0 => None,
                    n => Some(sign + n),
                }
            }
            Pattern::Offset => {
                if s[i] == 90 {
                    return Some(1);
                }
                if !matches!(s[i], 43 | 45) || digits_from(s, i + 1) < 2 {
                    return None;
                }
                let mut k = i + 3;
                if s.get(k) == Some(&58) {
                    k += 1;
                }
                if s.len() - k >= 2 && digit(s[k]) && digit(s[k + 1]) {
                    k += 2;
                }
                Some(k - i)
            }
            Pattern::Word => {
                let d = digits_from(s, i);
                let word = s[i + d..]
                    .iter()
                    .take_while(|&&c| {
                        !digit(c)
                            && !matches!(c, 45 | 95 | 58 | 47 | 44 | 40 | 41)
                            && !char::from_u32(u32::from(c)).is_some_and(is_space)
                    })
                    .count();
                (word > 0).then_some(d + word)
            }
        }
    }

    /// The first match in `s`, as an unanchored `exec` finds it.
    fn find(self, s: &[u16]) -> Option<(usize, usize)> {
        (0..s.len()).find_map(|i| self.at(s, i).map(|len| (i, len)))
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Step {
    Literal(usize),
    Read(&'static str, Pattern),
}

fn reader(token: &str) -> Option<(&'static str, Pattern)> {
    use Pattern::*;
    Some(match token {
        "A" => ("A", Word),
        "a" => ("a", Word),
        "Q" => ("Q", Digits(1)),
        "S" => ("S", Digits(1)),
        "SS" => ("SS", Digits(2)),
        "SSS" => ("SSS", Digits(3)),
        "s" | "ss" => ("s", OneOrTwo),
        "m" | "mm" => ("m", OneOrTwo),
        "H" | "h" | "HH" | "hh" => ("H", OneOrTwo),
        "D" => ("D", OneOrTwo),
        "DD" => ("D", Digits(2)),
        "Do" => ("Do", Word),
        "w" => ("w", OneOrTwo),
        "ww" => ("w", Digits(2)),
        "M" => ("M", OneOrTwo),
        "MM" => ("M", Digits(2)),
        "MMM" => ("MMM", Word),
        "MMMM" => ("MMMM", Word),
        "Y" => ("Y", Signed),
        "YY" => ("YY", Digits(2)),
        "YYYY" => ("Y", Digits(4)),
        "Z" | "ZZ" => ("Z", Offset),
        _ => return None,
    })
}

/// Splits a format into steps as dayjs does, or `None` when dayjs fails on it.
fn compile(template: &str) -> Option<Vec<Step>> {
    let t: Vec<char> = template.chars().collect();
    // dayjs expands localized tokens through a locale table the English locale does not have,
    // and fails.
    let mut i = 0;
    while i < t.len() {
        if let Some(len) = bracket(&t, i) {
            i += len;
            continue;
        }
        if matches!(t[i], 'L' | 'l') {
            return None;
        }
        i += 1;
    }
    const TOKENS: &[&str] = &[
        "A", "a", "Q", "YYYY", "YY", "Y", "ww", "w", "MMMM", "MMM", "MM", "M", "Do", "DD", "D",
        "hh", "h", "HH", "H", "mm", "m", "ss", "s", "SSS", "SS", "S", "z", "ZZ", "Z",
    ];
    let separator = |c: char| "-_:/.,()".contains(c) || is_space(c);
    let mut steps = Vec::new();
    let mut i = 0;
    while i < t.len() {
        if t[i] == '[' {
            // `\[[^[]*\]`: up to the last `]` before the next `[`.
            let stop = t[i + 1..]
                .iter()
                .position(|&c| c == '[')
                .map_or(t.len(), |k| i + 1 + k);
            if let Some(close) = t[i + 1..stop].iter().rposition(|&c| c == ']') {
                let len = close + 2;
                let token: String = t[i..i + len].iter().collect();
                let inner = token.strip_prefix('[').unwrap_or(&token);
                let inner = inner.strip_suffix(']').unwrap_or(inner);
                steps.push(Step::Literal(inner.encode_utf16().count()));
                i += len;
                continue;
            }
        }
        if separator(t[i]) {
            let len = t[i..].iter().take_while(|&&c| separator(c)).count();
            let units: usize = t[i..i + len].iter().map(|c| c.len_utf16()).sum();
            steps.push(Step::Literal(units));
            i += len;
            continue;
        }
        if let Some(token) = alternative(&t, i, TOKENS) {
            steps.push(match reader(token) {
                Some((name, pattern)) => Step::Read(name, pattern),
                None => Step::Literal(token.len()),
            });
            i += token.len();
            continue;
        }
        i += 1;
    }
    (!steps.is_empty()).then_some(steps)
}

fn number(s: &[u16]) -> f64 {
    js_to_number(&String::from_utf16_lossy(s))
}

/// Whether `read` took the input.
fn read(f: &mut Fields, name: &str, input: &[u16]) -> bool {
    let value = || number(input);
    match name {
        "A" => f.afternoon = Some(input == "PM".encode_utf16().collect::<Vec<_>>()),
        "a" => f.afternoon = Some(input == "pm".encode_utf16().collect::<Vec<_>>()),
        "Q" => f.month = Some((value() - 1.0) * 3.0 + 1.0),
        "S" => f.milliseconds = Some(value() * 100.0),
        "SS" => f.milliseconds = Some(value() * 10.0),
        "SSS" => f.milliseconds = Some(value()),
        "s" => f.seconds = Some(value()),
        "m" => f.minutes = Some(value()),
        "H" => f.hours = Some(value()),
        "D" => f.day = Some(value()),
        "w" => f.week = Some(value()),
        "M" => f.month = Some(value()),
        "Y" => f.year = Some(value()),
        "YY" => {
            let v = value();
            f.year = Some(v + if v > 68.0 { 1900.0 } else { 2000.0 });
        }
        "Do" => {
            let Some(from) = input.iter().position(|&c| digit(c)) else {
                return false;
            };
            // dayjs keeps the digits as a string here, so even "0" counts as a day having been
            // given.
            f.day = Some(number(&input[from..from + digits_from(input, from)]));
            f.day_given = true;
            let text = String::from_utf16_lossy(input);
            for i in 1..=31 {
                if ordinal(f64::from(i)) == text {
                    f.day = Some(f64::from(i));
                }
            }
        }
        "MMM" | "MMMM" => {
            let text = String::from_utf16_lossy(input);
            let Some(index) = MONTHS.iter().position(|m| {
                if name == "MMM" {
                    m[..3] == text
                } else {
                    *m == text
                }
            }) else {
                return false;
            };
            f.month = Some(index as f64 + 1.0);
        }
        _ => {
            // Z or ZZ.
            if input == [90] {
                f.offset = Some(0.0);
                return true;
            }
            // `input.match(/([+-]|\d\d)/g)`: the sign, then pairs of digits.
            let mut parts = Vec::new();
            let mut k = 0;
            while k < input.len() {
                if matches!(input[k], 43 | 45) {
                    parts.push(&input[k..=k]);
                    k += 1;
                } else if k + 1 < input.len() && digit(input[k]) && digit(input[k + 1]) {
                    parts.push(&input[k..k + 2]);
                    k += 2;
                } else {
                    k += 1;
                }
            }
            let second = parts.get(2).map_or(0.0, |p| number(p));
            let minutes = number(parts[1]) * 60.0 + if truthy(Some(second)) { second } else { 0.0 };
            f.offset = Some(if minutes == 0.0 {
                0.0
            } else if parts[0] == [43] {
                -minutes
            } else {
                minutes
            });
        }
    }
    true
}

fn loose(input: &str, template: &str, now: Date) -> Option<Date> {
    if template == "x" || template == "X" {
        let k = if template == "X" { 1000.0 } else { 1.0 };
        return Some(Date::new(k * js_to_number(input)));
    }
    let steps = compile(template)?;
    let mut f = Fields::default();
    let mut rest: Vec<u16> = input.encode_utf16().collect();
    let mut start = 0;
    for step in steps {
        let (name, pattern) = match step {
            Step::Literal(len) => {
                start += len;
                continue;
            }
            Step::Read(name, pattern) => (name, pattern),
        };
        let from = start.min(rest.len());
        let (at, len) = pattern.find(&rest[from..])?;
        let matched: Vec<u16> = rest[from + at..from + at + len].to_vec();
        if !read(&mut f, name, &matched) {
            return None;
        }
        // dayjs drops the first occurrence of the matched text, wherever it is, and does not
        // move `start`.
        let first = rest
            .windows(len)
            .position(|w| w == matched.as_slice())
            .expect("the match is in the text");
        rest.drain(first..first + len);
    }
    if let (Some(afternoon), Some(hours)) = (f.afternoon, f.hours) {
        if afternoon {
            if hours < 12.0 {
                f.hours = Some(hours + 12.0);
            }
        } else if hours == 12.0 {
            f.hours = Some(0.0);
        }
    }
    let day = if f.day_given || truthy(f.day) {
        f.day.unwrap_or(f64::NAN)
    } else if !truthy(f.year) && !truthy(f.month) {
        now.date()
    } else {
        1.0
    };
    let year = if truthy(f.year) {
        f.year.unwrap_or(0.0)
    } else {
        now.year()
    };
    let mut month = 0.0;
    if !truthy(f.year) || truthy(f.month) {
        month = match f.month {
            Some(m) if m > 0.0 => m - 1.0,
            _ => now.month(),
        };
    }
    let or0 = |v: Option<f64>| v.filter(|v| truthy(Some(*v))).unwrap_or(0.0);
    let (hours, minutes, seconds, ms) = (
        or0(f.hours),
        or0(f.minutes),
        or0(f.seconds),
        or0(f.milliseconds),
    );
    if let Some(offset) = f.offset {
        return Some(Date(Date::utc(
            year,
            month,
            day,
            hours,
            minutes,
            seconds,
            ms + offset * 60_000.0,
        )));
    }
    // Applying a week number needs a dayjs plugin Mermaid does not load.
    if truthy(f.week) {
        return None;
    }
    Some(Date::local(year, month, day, hours, minutes, seconds, ms))
}

/// Parses the way `dayjs(input, format, true)` does. `now` supplies the fields the format
/// leaves out.
pub(crate) fn parse_strict(input: &str, template: &str, now: Date) -> Option<Date> {
    let date = loose(input, template, now)?;
    (date.valid() && format(date, template).as_deref() == Some(input)).then_some(date)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: f64, m: f64, d: f64) -> Date {
        Date::local(y, m, d, 0.0, 0.0, 0.0, 0.0)
    }

    #[test]
    fn keeps_javascript_calendar_rules() {
        let d = at(2024.0, 1.0, 29.0);
        assert_eq!(
            (d.year(), d.month(), d.date(), d.weekday()),
            (2024.0, 1.0, 29.0, 4.0)
        );
        assert_eq!(at(50.0, 0.0, 1.0).year(), 1950.0);
        assert_eq!(
            add(at(2024.0, 0.0, 31.0), 1.0, Unit::Month),
            at(2024.0, 1.0, 29.0)
        );
        assert_eq!(add_days(at(2023.0, 11.0, 31.0), 1.0), at(2024.0, 0.0, 1.0));
        let mut d = Date(f64::NAN);
        d.set_full_year(2000.0);
        assert_eq!(d, at(2000.0, 0.0, 1.0));
    }

    #[test]
    fn parses_and_formats_as_dayjs() {
        let now = at(2024.0, 4.0, 17.0);
        let parse = |s: &str, f: &str| parse_strict(s, f, now).map(|d| d.time());
        assert_eq!(
            parse("2014-01-06", "YYYY-MM-DD"),
            Some(at(2014.0, 0.0, 6.0).time())
        );
        assert_eq!(parse("2014-1-06", "YYYY-MM-DD"), None);
        assert_eq!(parse("2014-02-30", "YYYY-MM-DD"), None);
        assert_eq!(
            parse("08:30", "HH:mm"),
            Some(Date::local(2024.0, 4.0, 17.0, 8.0, 30.0, 0.0, 0.0).time())
        );
        assert_eq!(
            parse("6th Jan 2014", "Do MMM YYYY"),
            Some(at(2014.0, 0.0, 6.0).time())
        );
        assert_eq!(parse("1388966400", "X"), Some(1388966400000.0));
        assert_eq!(
            format(at(2014.0, 0.0, 6.0), "dddd, MMMM Do YYYY [w]W").as_deref(),
            Some("Monday, January 6th 2014 w2")
        );
        assert_eq!(format(at(2014.0, 0.0, 6.0), "w"), None);
        assert_eq!(iso_date(at(-5.0, 0.0, 1.0)), "00-5-01-01");
    }
}
