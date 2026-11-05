//! JavaScript's own date parser: what `new Date(text)` makes of a string.
//!
//! Mermaid reads a gantt date that the chart's `dateFormat` does not match with `new
//! Date(text)`. V8 parses such a string with the ECMAScript date time string format first, and
//! with a lenient legacy format after that, which reads a good deal more: "2014-1-12", a bare
//! "0", and text with something in front of the date, such as "done 2014-01-06". This is a port
//! of V8's `src/date/dateparser.cc`, so that pele-rs reads the dates Mermaid reads.
//!
//! V8 reads a string as UTF-16 code units, and so does this. Local time is UTC here, as in
//! [`super::dates`].

use super::dates::{Date, MS_DAY, make_day, make_time};

/// V8's `IsWhiteSpace`: the space separators, and the tab, the vertical tab and the form feed.
/// The line terminators are not white space to V8, though they end a word all the same, since
/// they are below 'A'.
fn is_white_space(c: u32) -> bool {
    matches!(
        c,
        0x09 | 0x0B | 0x0C | 0x20 | 0xA0 | 0x1680 | 0x202F | 0x205F | 0x3000 | 0xFEFF
    ) || (0x2000..=0x200A).contains(&c)
}

/// V8's `IsWhiteSpaceOrLineTerminator`: the line terminators are white space too.
fn is_white_space_or_line_terminator(c: u32) -> bool {
    is_white_space(c) || matches!(c, 0x0A | 0x0D | 0x2028 | 0x2029)
}

/// The kinds of word V8 knows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum KeywordType {
    Invalid,
    MonthName,
    TimeZoneName,
    TimeSeparator,
    AmPm,
}

/// V8's keyword table: the first three letters of a word, and what the word means. A word
/// shorter than three letters is padded with zeros, as in V8.
macro_rules! keywords {
    ($($prefix:literal, $kind:ident, $value:literal;)*) => {
        &[$(($prefix, KeywordType::$kind, $value)),*]
    };
}

const KEYWORDS: &[(&str, KeywordType, i64)] = keywords! {
    "jan", MonthName, 1;
    "feb", MonthName, 2;
    "mar", MonthName, 3;
    "apr", MonthName, 4;
    "may", MonthName, 5;
    "jun", MonthName, 6;
    "jul", MonthName, 7;
    "aug", MonthName, 8;
    "sep", MonthName, 9;
    "oct", MonthName, 10;
    "nov", MonthName, 11;
    "dec", MonthName, 12;
    "am\0", AmPm, 0;
    "pm\0", AmPm, 12;
    "ut\0", TimeZoneName, 0;
    "utc", TimeZoneName, 0;
    "z\0\0", TimeZoneName, 0;
    "gmt", TimeZoneName, 0;
    "cdt", TimeZoneName, -5;
    "cst", TimeZoneName, -6;
    "edt", TimeZoneName, -4;
    "est", TimeZoneName, -5;
    "mdt", TimeZoneName, -6;
    "mst", TimeZoneName, -7;
    "pdt", TimeZoneName, -7;
    "pst", TimeZoneName, -8;
    "t\0\0", TimeSeparator, 0;
};

/// The number of digits V8 keeps of a numeral.
const MAX_SIGNIFICANT_DIGITS: usize = 9;

/// V8's lookup of a word in the keyword table. A word longer than three letters only matches a
/// month name.
fn lookup(prefix: &[u32; 3], length: usize) -> (KeywordType, i64) {
    for (letters, kind, value) in KEYWORDS {
        let letters = letters.as_bytes();
        if prefix[0] == u32::from(letters[0])
            && prefix[1] == u32::from(letters[1])
            && prefix[2] == u32::from(letters[2])
            && (length <= 3 || *kind == KeywordType::MonthName)
        {
            return (*kind, *value);
        }
    }
    (KeywordType::Invalid, 0)
}

/// A token of the date string.
#[derive(Clone, Copy, Debug)]
enum Token {
    Invalid,
    Unknown,
    WhiteSpace,
    Number(i64, usize),
    Symbol(u32),
    EndOfInput,
    Keyword(KeywordType, i64, usize),
}

impl Token {
    fn is_invalid(self) -> bool {
        matches!(self, Token::Invalid)
    }

    fn is_number(self) -> bool {
        matches!(self, Token::Number(_, _))
    }

    fn is_symbol(self, symbol: u32) -> bool {
        matches!(self, Token::Symbol(s) if s == symbol)
    }

    fn is_white_space(self) -> bool {
        matches!(self, Token::WhiteSpace)
    }

    fn is_end_of_input(self) -> bool {
        matches!(self, Token::EndOfInput)
    }

    fn is_keyword(self) -> bool {
        matches!(self, Token::Keyword(_, _, _))
    }

    fn is_keyword_type(self, kind: KeywordType) -> bool {
        matches!(self, Token::Keyword(k, _, _) if k == kind)
    }

    fn is_keyword_z(self) -> bool {
        matches!(self, Token::Keyword(KeywordType::TimeZoneName, value, 1) if value == 0)
    }

    fn is_fixed_length_number(self, length: usize) -> bool {
        matches!(self, Token::Number(_, l) if l == length)
    }

    fn is_ascii_sign(self) -> bool {
        self.is_symbol(u32::from(b'+')) || self.is_symbol(u32::from(b'-'))
    }

    fn number(self) -> i64 {
        match self {
            Token::Number(value, _) => value,
            _ => 0,
        }
    }

    fn length(self) -> usize {
        match self {
            Token::Number(_, length) => length,
            Token::Keyword(_, _, length) => length,
            _ => 0,
        }
    }

    fn symbol(self) -> u32 {
        match self {
            Token::Symbol(symbol) => symbol,
            _ => 0,
        }
    }

    /// One for '+' and minus one for '-'.
    fn ascii_sign(self) -> i64 {
        if self.is_symbol(u32::from(b'+')) {
            1
        } else {
            -1
        }
    }

    fn keyword_type(self) -> KeywordType {
        match self {
            Token::Keyword(kind, _, _) => kind,
            _ => KeywordType::Invalid,
        }
    }

    fn keyword_value(self) -> i64 {
        match self {
            Token::Keyword(_, value, _) => value,
            _ => 0,
        }
    }
}

/// Reads the characters of the string.
struct InputReader<'a> {
    text: &'a [u16],
    index: usize,
    ch: u32,
}

impl<'a> InputReader<'a> {
    fn new(text: &'a [u16]) -> Self {
        let mut reader = InputReader {
            text,
            index: 0,
            ch: 0,
        };
        reader.next();
        reader
    }

    fn next(&mut self) {
        self.ch = self.text.get(self.index).map_or(0, |&u| u32::from(u));
        self.index += 1;
    }

    fn position(&self) -> usize {
        self.index
    }

    fn is_end(&self) -> bool {
        self.ch == 0
    }

    fn is_ascii_digit(&self) -> bool {
        (u32::from(b'0')..=u32::from(b'9')).contains(&self.ch)
    }

    /// The characters V8 reads as a word: everything at or above 'A'.
    fn is_ascii_alpha_or_above(&self) -> bool {
        self.ch >= u32::from(b'A')
    }

    fn is_white_space_char(&self) -> bool {
        is_white_space(self.ch)
    }

    fn skip(&mut self, c: u32) -> bool {
        if self.ch == c {
            self.next();
            true
        } else {
            false
        }
    }

    /// Reads a string of digits as an unsigned number, keeping at most
    /// [`MAX_SIGNIFICANT_DIGITS`] of them.
    fn read_unsigned_numeral(&mut self) -> i64 {
        let mut n = 0;
        let mut digits = 0;
        while self.ch == u32::from(b'0') {
            self.next();
        }
        while self.is_ascii_digit() {
            if digits < MAX_SIGNIFICANT_DIGITS {
                n = n * 10 + i64::from(self.ch - u32::from(b'0'));
            }
            digits += 1;
            self.next();
        }
        n
    }

    /// Reads a word and fills the prefix with its first three letters, in lower case.
    fn read_word(&mut self, prefix: &mut [u32; 3]) -> usize {
        let mut length = 0;
        while self.is_ascii_alpha_or_above() && !self.is_white_space_char() {
            if length < prefix.len() {
                prefix[length] = match self.ch {
                    c @ 65..=90 => c + 32,
                    c => c,
                };
            }
            self.next();
            length += 1;
        }
        length
    }

    fn skip_white_space(&mut self) -> bool {
        if is_white_space_or_line_terminator(self.ch) {
            self.next();
            true
        } else {
            false
        }
    }

    /// Skips a parenthesized text, which V8 ignores.
    fn skip_parentheses(&mut self) -> bool {
        if self.ch != u32::from(b'(') {
            return false;
        }
        let mut balance = 0;
        loop {
            if self.ch == u32::from(b')') {
                balance -= 1;
            } else if self.ch == u32::from(b'(') {
                balance += 1;
            }
            self.next();
            if balance <= 0 || self.is_end() {
                break;
            }
        }
        true
    }
}

/// Turns the string into tokens.
struct Scanner<'a> {
    input: InputReader<'a>,
    next: Token,
}

impl<'a> Scanner<'a> {
    fn new(text: &'a [u16]) -> Self {
        let mut scanner = Scanner {
            input: InputReader::new(text),
            next: Token::Invalid,
        };
        scanner.next = scanner.scan();
        scanner
    }

    fn next(&mut self) -> Token {
        let token = self.next;
        self.next = self.scan();
        token
    }

    fn peek(&self) -> Token {
        self.next
    }

    fn skip_symbol(&mut self, symbol: u32) -> bool {
        if self.next.is_symbol(symbol) {
            self.next = self.scan();
            true
        } else {
            false
        }
    }

    fn scan(&mut self) -> Token {
        let before = self.input.position();
        if self.input.is_end() {
            return Token::EndOfInput;
        }
        if self.input.is_ascii_digit() {
            let n = self.input.read_unsigned_numeral();
            let length = self.input.position() - before;
            return Token::Number(n, length);
        }
        if self.input.skip(u32::from(b':')) {
            return Token::Symbol(u32::from(b':'));
        }
        if self.input.skip(u32::from(b'-')) {
            return Token::Symbol(u32::from(b'-'));
        }
        if self.input.skip(u32::from(b'+')) {
            return Token::Symbol(u32::from(b'+'));
        }
        if self.input.skip(u32::from(b'.')) {
            return Token::Symbol(u32::from(b'.'));
        }
        if self.input.skip(u32::from(b')')) {
            return Token::Symbol(u32::from(b')'));
        }
        if self.input.is_ascii_alpha_or_above() && !self.input.is_white_space_char() {
            let mut prefix = [0; 3];
            let length = self.input.read_word(&mut prefix);
            let (kind, value) = lookup(&prefix, length);
            return Token::Keyword(kind, value, length);
        }
        if self.input.skip_white_space() {
            return Token::WhiteSpace;
        }
        if self.input.skip_parentheses() {
            return Token::Unknown;
        }
        self.input.next();
        Token::Unknown
    }
}

/// The year, the month and the day of a date.
struct DayComposer {
    comp: [i64; 3],
    index: usize,
    named_month: Option<i64>,
    iso_date: bool,
}

impl DayComposer {
    fn new() -> Self {
        DayComposer {
            comp: [0; 3],
            index: 0,
            named_month: None,
            iso_date: false,
        }
    }

    fn is_empty(&self) -> bool {
        self.index == 0
    }

    fn add(&mut self, n: i64) -> bool {
        if self.index < self.comp.len() {
            self.comp[self.index] = n;
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn set_named_month(&mut self, n: i64) {
        self.named_month = Some(n);
    }

    /// Says that the numbers are in year, month, day order.
    fn set_iso_date(&mut self) {
        self.iso_date = true;
    }

    /// The year, the month from 0, and the day of the month, or `None` when the numbers do not
    /// make a date.
    fn write(&mut self) -> Option<(f64, f64, f64)> {
        if self.index < 1 {
            return None;
        }
        // The day and the month default to 1.
        while self.index < self.comp.len() {
            self.comp[self.index] = 1;
            self.index += 1;
        }
        let year;
        let month;
        let day;
        match self.named_month {
            None => {
                if self.iso_date || !is_day(self.comp[0]) {
                    // Year, month, day.
                    year = self.comp[0];
                    month = self.comp[1];
                    day = self.comp[2];
                } else {
                    // Month, day, and the year when there is one.
                    month = self.comp[0];
                    day = self.comp[1];
                    year = self.comp[2];
                }
            }
            Some(named) => {
                month = named;
                if !is_day(self.comp[0]) {
                    // Year, month, day.
                    year = self.comp[0];
                    day = self.comp[1];
                } else {
                    // Day, month, year.
                    day = self.comp[0];
                    year = self.comp[1];
                }
            }
        }
        let mut year = year;
        if !self.iso_date {
            if (0..=49).contains(&year) {
                year += 2000;
            } else if (50..=99).contains(&year) {
                year += 1900;
            }
        }
        if !is_month(month) || !is_day(day) {
            return None;
        }
        Some((year as f64, (month - 1) as f64, day as f64))
    }
}

/// The hour, the minute, the second and the millisecond of a time.
struct TimeComposer {
    comp: [i64; 4],
    index: usize,
    hour_offset: Option<i64>,
}

impl TimeComposer {
    fn new() -> Self {
        TimeComposer {
            comp: [0; 4],
            index: 0,
            hour_offset: None,
        }
    }

    fn is_empty(&self) -> bool {
        self.index == 0
    }

    fn is_expecting(&self, n: i64) -> bool {
        (self.index == 1 && is_minute(n))
            || (self.index == 2 && is_second(n))
            || (self.index == 3 && is_millisecond(n))
    }

    fn add(&mut self, n: i64) -> bool {
        if self.index < self.comp.len() {
            self.comp[self.index] = n;
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn add_final(&mut self, n: i64) -> bool {
        if !self.add(n) {
            return false;
        }
        while self.index < self.comp.len() {
            self.comp[self.index] = 0;
            self.index += 1;
        }
        true
    }

    /// Sets the hour that "am" or "pm" makes of it.
    fn set_hour_offset(&mut self, n: i64) {
        self.hour_offset = Some(n);
    }

    fn write(&mut self) -> Option<(f64, f64, f64, f64)> {
        while self.index < self.comp.len() {
            self.comp[self.index] = 0;
            self.index += 1;
        }
        let mut hour = self.comp[0];
        let minute = self.comp[1];
        let second = self.comp[2];
        let millisecond = self.comp[3];
        if let Some(offset) = self.hour_offset {
            if !(0..=12).contains(&hour) {
                return None;
            }
            hour %= 12;
            hour += offset;
        }
        if !is_hour(hour)
            || !is_minute(minute)
            || !is_second(second)
            || !is_millisecond(millisecond)
        {
            // A 24th hour is allowed when the rest of the time is zero.
            if hour != 24 || minute != 0 || second != 0 || millisecond != 0 {
                return None;
            }
        }
        Some((
            hour as f64,
            minute as f64,
            second as f64,
            millisecond as f64,
        ))
    }
}

/// The offset from UTC of a time zone.
struct TimeZoneComposer {
    sign: Option<i64>,
    hour: Option<i64>,
    minute: Option<i64>,
}

impl TimeZoneComposer {
    fn new() -> Self {
        TimeZoneComposer {
            sign: None,
            hour: None,
            minute: None,
        }
    }

    fn is_empty(&self) -> bool {
        self.hour.is_none()
    }

    fn is_utc(&self) -> bool {
        self.hour == Some(0) && self.minute == Some(0)
    }

    fn set(&mut self, offset_in_hours: i64) {
        let sign = if offset_in_hours < 0 { -1 } else { 1 };
        self.sign = Some(sign);
        self.hour = Some(offset_in_hours * sign);
        self.minute = Some(0);
    }

    fn set_sign(&mut self, sign: i64) {
        self.sign = Some(if sign < 0 { -1 } else { 1 });
    }

    fn set_absolute_hour(&mut self, hour: i64) {
        self.hour = Some(hour);
    }

    fn set_absolute_minute(&mut self, minute: Option<i64>) {
        self.minute = minute;
    }

    fn is_expecting(&self, n: i64) -> bool {
        self.hour.is_some() && self.minute.is_none() && is_minute(n)
    }

    /// The offset in seconds, or NaN when there is none, which leaves the time in local time.
    fn write(&mut self) -> f64 {
        match self.sign {
            None => f64::NAN,
            Some(sign) => {
                let hour = self.hour.unwrap_or(0);
                let minute = self.minute.unwrap_or(0);
                let total = hour * 3600 + minute * 60;
                if sign < 0 {
                    -total as f64
                } else {
                    total as f64
                }
            }
        }
    }
}

fn is_month(n: i64) -> bool {
    (1..=12).contains(&n)
}

fn is_day(n: i64) -> bool {
    (1..=31).contains(&n)
}

fn is_hour(n: i64) -> bool {
    (0..=23).contains(&n)
}

fn is_minute(n: i64) -> bool {
    (0..=59).contains(&n)
}

fn is_second(n: i64) -> bool {
    (0..=59).contains(&n)
}

fn is_millisecond(n: i64) -> bool {
    (0..=999).contains(&n)
}

/// The first three digits of a numeral, as V8 reads the milliseconds of a time.
fn read_milliseconds(token: Token) -> i64 {
    let mut number = token.number();
    let mut length = token.length();
    match length.cmp(&3) {
        // Fewer than three digits: put the most significant one in the hundreds.
        std::cmp::Ordering::Less => {
            if length == 1 {
                number *= 100;
            } else if length == 2 {
                number *= 10;
            }
        }
        std::cmp::Ordering::Equal => {}
        std::cmp::Ordering::Greater => {
            if length > MAX_SIGNIFICANT_DIGITS {
                length = MAX_SIGNIFICANT_DIGITS;
            }
            // More than three digits: keep the three most significant ones.
            let mut factor = 1;
            loop {
                factor *= 10;
                length -= 1;
                if length <= 3 {
                    break;
                }
            }
            number /= factor;
        }
    }
    number
}

/// Parses an ECMAScript date time string, and returns the token the legacy parser continues
/// with. A date without a time is read as UTC, as the specification says.
fn parse_es5_date_time(
    scanner: &mut Scanner<'_>,
    day: &mut DayComposer,
    time: &mut TimeComposer,
    tz: &mut TimeZoneComposer,
) -> Token {
    // The date: an optional sign and six digits, or four digits.
    if scanner.peek().is_ascii_sign() {
        // The sign is kept, so that the legacy parser can have it.
        let sign_token = scanner.next();
        if !scanner.peek().is_fixed_length_number(6) {
            return sign_token;
        }
        let sign = sign_token.ascii_sign();
        let year = scanner.next().number();
        if sign < 0 && year == 0 {
            return sign_token;
        }
        let _ = day.add(sign * year);
    } else if scanner.peek().is_fixed_length_number(4) {
        let _ = day.add(scanner.next().number());
    } else {
        return scanner.next();
    }
    if scanner.skip_symbol(u32::from(b'-')) {
        if !scanner.peek().is_fixed_length_number(2) || !is_month(scanner.peek().number()) {
            return scanner.next();
        }
        let _ = day.add(scanner.next().number());
        if scanner.skip_symbol(u32::from(b'-')) {
            if !scanner.peek().is_fixed_length_number(2) || !is_day(scanner.peek().number()) {
                return scanner.next();
            }
            let _ = day.add(scanner.next().number());
        }
    }
    // The time, after a 'T'.
    if !scanner.peek().is_keyword_type(KeywordType::TimeSeparator) {
        if !scanner.peek().is_end_of_input() {
            return scanner.next();
        }
    } else {
        scanner.next();
        if !scanner.peek().is_fixed_length_number(2) || !(0..=24).contains(&scanner.peek().number())
        {
            return Token::Invalid;
        }
        // A 24th hour is allowed, but no other time may start with 24.
        let hour_is_24 = scanner.peek().number() == 24;
        let _ = time.add(scanner.next().number());
        if !scanner.skip_symbol(u32::from(b':')) {
            return Token::Invalid;
        }
        if !scanner.peek().is_fixed_length_number(2)
            || !is_minute(scanner.peek().number())
            || (hour_is_24 && scanner.peek().number() > 0)
        {
            return Token::Invalid;
        }
        let _ = time.add(scanner.next().number());
        if scanner.skip_symbol(u32::from(b':')) {
            if !scanner.peek().is_fixed_length_number(2)
                || !is_second(scanner.peek().number())
                || (hour_is_24 && scanner.peek().number() > 0)
            {
                return Token::Invalid;
            }
            let _ = time.add(scanner.next().number());
            if scanner.skip_symbol(u32::from(b'.')) {
                if !scanner.peek().is_number() || (hour_is_24 && scanner.peek().number() > 0) {
                    return Token::Invalid;
                }
                // More or fewer than three digits are allowed.
                let _ = time.add(read_milliseconds(scanner.next()));
            }
        }
        // The time zone.
        if scanner.peek().is_keyword_z() {
            scanner.next();
            tz.set(0);
        } else if scanner.peek().is_symbol(u32::from(b'+'))
            || scanner.peek().is_symbol(u32::from(b'-'))
        {
            let sign = if scanner.next().symbol() == u32::from(b'+') {
                1
            } else {
                -1
            };
            tz.set_sign(sign);
            if scanner.peek().is_fixed_length_number(4) {
                // The hhmm form.
                let hourmin = scanner.next().number();
                let hour = hourmin / 100;
                let minute = hourmin % 100;
                if !is_hour(hour) || !is_minute(minute) {
                    return Token::Invalid;
                }
                tz.set_absolute_hour(hour);
                tz.set_absolute_minute(Some(minute));
            } else {
                // The hh:mm form.
                if !scanner.peek().is_fixed_length_number(2) || !is_hour(scanner.peek().number()) {
                    return Token::Invalid;
                }
                tz.set_absolute_hour(scanner.next().number());
                if !scanner.skip_symbol(u32::from(b':')) {
                    return Token::Invalid;
                }
                if !scanner.peek().is_fixed_length_number(2) || !is_minute(scanner.peek().number())
                {
                    return Token::Invalid;
                }
                tz.set_absolute_minute(Some(scanner.next().number()));
            }
        }
        if !scanner.peek().is_end_of_input() {
            return Token::Invalid;
        }
    }
    // A date without a time is read as UTC.
    if tz.is_empty() && time.is_empty() {
        tz.set(0);
    }
    day.set_iso_date();
    Token::EndOfInput
}

/// What `new Date(text)` makes of a string.
pub(crate) fn parse(text: &str) -> Date {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut scanner = Scanner::new(&units);
    let mut day = DayComposer::new();
    let mut time = TimeComposer::new();
    let mut tz = TimeZoneComposer::new();

    let mut token = parse_es5_date_time(&mut scanner, &mut day, &mut time, &mut tz);
    if token.is_invalid() {
        return Date::new(f64::NAN);
    }
    let mut has_read_number = !day.is_empty();
    while !token.is_end_of_input() {
        if token.is_number() {
            has_read_number = true;
            let n = token.number();
            if scanner.skip_symbol(u32::from(b':')) {
                if scanner.skip_symbol(u32::from(b':')) {
                    // A number and "::".
                    if !time.is_empty() {
                        return Date::new(f64::NAN);
                    }
                    let _ = time.add(n);
                    let _ = time.add(0);
                } else {
                    // A number and ":".
                    if !time.add(n) {
                        return Date::new(f64::NAN);
                    }
                    if scanner.peek().is_symbol(u32::from(b'.')) {
                        scanner.next();
                    }
                }
            } else if scanner.skip_symbol(u32::from(b'.')) && time.is_expecting(n) {
                let _ = time.add(n);
                if !scanner.peek().is_number() {
                    return Date::new(f64::NAN);
                }
                let ms = read_milliseconds(scanner.next());
                if ms < 0 {
                    return Date::new(f64::NAN);
                }
                let _ = time.add_final(ms);
            } else if tz.is_expecting(n) {
                tz.set_absolute_minute(Some(n));
            } else if time.is_expecting(n) {
                let _ = time.add_final(n);
                // The end of the string, a space, "Z", "+" or "-" has to follow.
                let peek = scanner.peek();
                if !peek.is_end_of_input()
                    && !peek.is_white_space()
                    && !peek.is_keyword_z()
                    && !peek.is_ascii_sign()
                {
                    return Date::new(f64::NAN);
                }
            } else {
                // A date component.
                if !day.add(n) {
                    return Date::new(f64::NAN);
                }
                scanner.skip_symbol(u32::from(b'-'));
            }
        } else if token.is_keyword() {
            let kind = token.keyword_type();
            let value = token.keyword_value();
            if kind == KeywordType::AmPm && !time.is_empty() {
                time.set_hour_offset(value);
            } else if kind == KeywordType::MonthName {
                day.set_named_month(value);
                scanner.skip_symbol(u32::from(b'-'));
            } else if kind == KeywordType::TimeZoneName && has_read_number {
                tz.set(value);
            } else {
                // A word V8 does not know is only allowed before the first number.
                if has_read_number {
                    return Date::new(f64::NAN);
                }
                // The first number has to be separated from a word by a space or another sign.
                if scanner.peek().is_number() {
                    return Date::new(f64::NAN);
                }
            }
        } else if token.is_ascii_sign() && (tz.is_utc() || !time.is_empty()) {
            // A time zone offset, after a time or after UTC.
            tz.set_sign(token.ascii_sign());
            // The number that follows may be missing.
            let mut n = 0;
            let mut length = 0;
            if scanner.peek().is_number() {
                let next = scanner.next();
                length = next.length();
                n = next.number();
            }
            has_read_number = true;
            if scanner.peek().is_symbol(u32::from(b':')) {
                tz.set_absolute_hour(n);
                tz.set_absolute_minute(None);
            } else if length == 2 || length == 1 {
                // A time zone like GMT-8.
                tz.set_absolute_hour(n);
                tz.set_absolute_minute(Some(0));
            } else if length == 4 || length == 3 {
                // The hhmm form.
                tz.set_absolute_hour(n / 100);
                tz.set_absolute_minute(Some(n % 100));
            } else {
                // A time zone like GMT-12345 is not accepted.
                return Date::new(f64::NAN);
            }
        } else if (token.is_ascii_sign() || token.is_symbol(u32::from(b')'))) && has_read_number {
            // A sign or a bracket after a number is not allowed.
            return Date::new(f64::NAN);
        }
        // Other characters and spaces are ignored.
        token = scanner.next();
    }

    let Some((year, month, day_of_month)) = day.write() else {
        return Date::new(f64::NAN);
    };
    let Some((hour, minute, second, millisecond)) = time.write() else {
        return Date::new(f64::NAN);
    };
    let offset = tz.write();
    let time =
        make_day(year, month, day_of_month) * MS_DAY + make_time(hour, minute, second, millisecond);
    // Without a time zone the time is local, which is UTC here.
    let time = if offset.is_nan() {
        time
    } else {
        time - offset * 1000.0
    };
    Date::new(time)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The time of a date, in the way `Date::local` reads it.
    fn at(y: f64, m: f64, d: f64, h: f64, mi: f64, s: f64, ms: f64) -> f64 {
        Date::local(y, m, d, h, mi, s, ms).time()
    }

    #[test]
    fn reads_the_date_time_string_format() {
        assert_eq!(
            parse("2020-01-05").time(),
            at(2020.0, 0.0, 5.0, 0.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(
            parse("2020-01-05T10:00:00+02:00").time(),
            at(2020.0, 0.0, 5.0, 8.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(
            parse("2020-01-05T10:00:00.500Z").time(),
            at(2020.0, 0.0, 5.0, 10.0, 0.0, 0.0, 500.0)
        );
    }

    #[test]
    fn reads_the_legacy_formats() {
        // A date the format does not match.
        assert_eq!(
            parse("2020-1-5").time(),
            at(2020.0, 0.0, 5.0, 0.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(
            parse("Jan 5 2020").time(),
            at(2020.0, 0.0, 5.0, 0.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(
            parse("5 Jan 2020").time(),
            at(2020.0, 0.0, 5.0, 0.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(
            parse("March 5, 2020").time(),
            at(2020.0, 2.0, 5.0, 0.0, 0.0, 0.0, 0.0)
        );
        // A year on its own is 2001, as V8 reads it.
        assert_eq!(
            parse("Jan 5").time(),
            at(2001.0, 0.0, 5.0, 0.0, 0.0, 0.0, 0.0)
        );
        // A number V8 reads as a year.
        assert_eq!(parse("0").time(), at(2000.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0));
        assert_eq!(parse("49").time(), at(2049.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0));
        // A number V8 reads as a month.
        assert_eq!(parse("5").time(), at(2001.0, 4.0, 1.0, 0.0, 0.0, 0.0, 0.0));
        // A word before the date is ignored.
        assert_eq!(
            parse("done 2014-01-06").time(),
            at(2014.0, 0.0, 6.0, 0.0, 0.0, 0.0, 0.0)
        );
        // A time.
        assert_eq!(
            parse("1 17:30").time(),
            at(2001.0, 0.0, 1.0, 17.0, 30.0, 0.0, 0.0)
        );
        assert_eq!(
            parse("Jan 5 2020 10:00 pm").time(),
            at(2020.0, 0.0, 5.0, 22.0, 0.0, 0.0, 0.0)
        );
        // A time zone.
        assert_eq!(
            parse("2020-01-05 GMT").time(),
            at(2020.0, 0.0, 5.0, 0.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(
            parse("1 Jan 2020 GMT+0300").time(),
            at(2020.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0) - 3.0 * 3_600_000.0
        );
    }

    #[test]
    fn reads_the_dates_v8_reads() {
        for (text, expected) in super::super::date_cases::CASES {
            let date = parse(text);
            match expected {
                Some(time) => assert_eq!(date.time(), *time, "{text:?}"),
                None => assert!(!date.valid(), "{text:?}"),
            }
        }
    }

    #[test]
    fn refuses_what_v8_refuses() {
        assert!(!parse("2020-13-05").valid());
        assert!(!parse("May").valid());
        assert!(!parse("(2020-01-05)").valid());
        assert!(!parse("").valid());
        assert!(!parse("not a date").valid());
    }
}
