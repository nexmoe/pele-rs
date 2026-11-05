//! Time axis support, following d3-time and d3-time-format (English locale, local time),
//! which Mermaid uses: the `axisFormat` specifiers, tick intervals with d3's `every` rule, and
//! its choice of automatic ticks. Ported from pele's `diagrams/gantt/axis.ts`; local time is
//! UTC here.

use super::dates::{Date, MONTHS, WEEKDAYS};
use crate::util::js_number;

const SECOND: f64 = 1000.0;
const MINUTE: f64 = 60.0 * SECOND;
const HOUR: f64 = 60.0 * MINUTE;
const DAY: f64 = 24.0 * HOUR;
const WEEK: f64 = 7.0 * DAY;
const MONTH: f64 = 30.0 * DAY;
const YEAR: f64 = 365.0 * DAY;

#[derive(Debug, Clone, Copy, PartialEq)]
enum TickUnit {
    Millisecond,
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

/// d3's `unit.every(step)`, or the unit itself for a step of 1.
#[derive(Debug, Clone, Copy)]
struct Calendar {
    unit: TickUnit,
    step: f64,
    /// The day weeks start on, 0 for Sunday.
    week_start: f64,
}

fn start_of_day(d: &mut Date) {
    d.set_hours(0.0, 0.0, 0.0, 0.0);
}

impl Calendar {
    fn new(unit: TickUnit, step: f64, week_start: f64) -> Calendar {
        Calendar {
            unit,
            step,
            week_start,
        }
    }

    fn base_floor(self, d: &mut Date) {
        let t = d.time();
        match self.unit {
            TickUnit::Millisecond => {}
            TickUnit::Second => d.set_time(t - d.millis()),
            TickUnit::Minute => d.set_time(t - d.millis() - d.seconds() * SECOND),
            TickUnit::Hour => {
                d.set_time(t - d.millis() - d.seconds() * SECOND - d.minutes() * MINUTE)
            }
            TickUnit::Day => start_of_day(d),
            TickUnit::Week => {
                d.set_date(d.date() - (d.weekday() + 7.0 - self.week_start) % 7.0);
                start_of_day(d);
            }
            TickUnit::Month => {
                d.set_date(1.0);
                start_of_day(d);
            }
            TickUnit::Year => {
                d.set_month(0.0, Some(1.0));
                start_of_day(d);
            }
        }
    }

    fn base_offset(self, d: &mut Date, step: f64) {
        let t = d.time();
        match self.unit {
            TickUnit::Millisecond => d.set_time(t + step),
            TickUnit::Second => d.set_time(t + step * SECOND),
            TickUnit::Minute => d.set_time(t + step * MINUTE),
            TickUnit::Hour => d.set_time(t + step * HOUR),
            TickUnit::Day => d.set_date(d.date() + step),
            TickUnit::Week => d.set_date(d.date() + step * 7.0),
            TickUnit::Month => d.set_month(d.month() + step, None),
            TickUnit::Year => d.set_full_year(d.year() + step),
        }
    }

    /// The value d3 tests against a step: a tick falls where it is a multiple of the step.
    fn field(self, d: Date) -> f64 {
        match self.unit {
            TickUnit::Second => d.seconds(),
            TickUnit::Minute => d.minutes(),
            TickUnit::Hour => d.hours(),
            TickUnit::Day => d.date() - 1.0,
            TickUnit::Month => d.month(),
            _ => f64::NAN,
        }
    }

    /// How many values `field` cycles through, which bounds how far apart filtered ticks can
    /// be.
    fn cycle(self) -> f64 {
        match self.unit {
            TickUnit::Second | TickUnit::Minute => 60.0,
            TickUnit::Hour => 24.0,
            TickUnit::Day => 31.0,
            TickUnit::Month => 12.0,
            _ => f64::INFINITY,
        }
    }

    fn ms(self) -> f64 {
        match self.unit {
            TickUnit::Millisecond => 1.0,
            TickUnit::Second => SECOND,
            TickUnit::Minute => MINUTE,
            TickUnit::Hour => HOUR,
            TickUnit::Day => DAY,
            TickUnit::Week => WEEK,
            TickUnit::Month => MONTH,
            TickUnit::Year => YEAR,
        }
    }

    fn test(self, d: Date) -> bool {
        self.field(d) % self.step == 0.0
    }

    fn floor(self, d: &mut Date) {
        let step = self.step;
        if step <= 1.0 {
            return self.base_floor(d);
        }
        match self.unit {
            TickUnit::Millisecond => d.set_time((d.time() / step).floor() * step),
            TickUnit::Year => {
                d.set_full_year((d.year() / step).floor() * step);
                self.base_floor(d);
            }
            TickUnit::Week => {
                let mut epoch = Date(0.0);
                self.base_floor(&mut epoch);
                self.base_floor(d);
                let weeks = span(epoch, *d, WEEK);
                self.base_offset(d, -weeks.rem_euclid(step));
            }
            _ => {
                // Date overflow makes the field NaN, which can never match the interval.
                self.base_floor(d);
                while d.valid() && !self.test(*d) {
                    d.set_time(d.time() - 1.0);
                    self.base_floor(d);
                }
            }
        }
    }

    fn offset(self, d: &mut Date, n: f64) {
        let step = self.step;
        if step <= 1.0 {
            return self.base_offset(d, n);
        }
        match self.unit {
            TickUnit::Millisecond => d.set_time(d.time() + n * step),
            TickUnit::Year => d.set_full_year(d.year() + n * step),
            TickUnit::Week => self.base_offset(d, n * step),
            _ => {
                let mut i = 0.0;
                while i < n && d.valid() {
                    loop {
                        self.base_offset(d, 1.0);
                        if !d.valid() || self.test(*d) {
                            break;
                        }
                    }
                    i += 1.0;
                }
            }
        }
    }
}

/// Whole units between two floored dates.
fn span(start: Date, end: Date, unit: f64) -> f64 {
    ((end.time() - start.time()) / unit).floor()
}

/// Ticks from start to stop inclusive, or `None` when there would be more than `limit`.
fn range(calendar: Calendar, start: f64, stop: f64, limit: f64) -> Option<Vec<f64>> {
    let mut d = Date::new(start - 1.0);
    calendar.floor(&mut d);
    calendar.offset(&mut d, 1.0);
    calendar.floor(&mut d);
    let mut out = Vec::new();
    let mut previous = f64::NEG_INFINITY;
    while d.time() <= stop && d.time() > previous {
        if out.len() as f64 >= limit {
            return None;
        }
        previous = d.time();
        out.push(previous);
        calendar.offset(&mut d, 1.0);
        calendar.floor(&mut d);
    }
    Some(out)
}

fn tick_step(start: f64, stop: f64, count: f64) -> f64 {
    let step = (stop - start) / count.max(0.0);
    let power = step.log10().floor();
    let error = step / 10f64.powf(power);
    let factor = if error >= 50f64.sqrt() {
        10.0
    } else if error >= 10f64.sqrt() {
        5.0
    } else if error >= std::f64::consts::SQRT_2 {
        2.0
    } else {
        1.0
    };
    10f64.powf(power) * factor
}

const AUTO: [(TickUnit, f64, f64); 18] = [
    (TickUnit::Second, 1.0, SECOND),
    (TickUnit::Second, 5.0, 5.0 * SECOND),
    (TickUnit::Second, 15.0, 15.0 * SECOND),
    (TickUnit::Second, 30.0, 30.0 * SECOND),
    (TickUnit::Minute, 1.0, MINUTE),
    (TickUnit::Minute, 5.0, 5.0 * MINUTE),
    (TickUnit::Minute, 15.0, 15.0 * MINUTE),
    (TickUnit::Minute, 30.0, 30.0 * MINUTE),
    (TickUnit::Hour, 1.0, HOUR),
    (TickUnit::Hour, 3.0, 3.0 * HOUR),
    (TickUnit::Hour, 6.0, 6.0 * HOUR),
    (TickUnit::Hour, 12.0, 12.0 * HOUR),
    (TickUnit::Day, 1.0, DAY),
    (TickUnit::Day, 2.0, 2.0 * DAY),
    (TickUnit::Week, 1.0, WEEK),
    (TickUnit::Month, 1.0, MONTH),
    (TickUnit::Month, 3.0, 3.0 * MONTH),
    (TickUnit::Year, 1.0, YEAR),
];

/// About `count` ticks at round times, chosen the way d3's time scale chooses them.
pub(crate) fn auto_ticks(start: f64, stop: f64, count: f64) -> Vec<f64> {
    let target = (stop - start) / count;
    let i = AUTO.iter().take_while(|a| a.2 <= target).count();
    let calendar = if i == AUTO.len() {
        Calendar::new(
            TickUnit::Year,
            tick_step(start / YEAR, stop / YEAR, count).max(1.0),
            0.0,
        )
    } else if i == 0 {
        Calendar::new(
            TickUnit::Millisecond,
            tick_step(start, stop, count).max(1.0),
            0.0,
        )
    } else {
        let k = if target / AUTO[i - 1].2 < AUTO[i].2 / target {
            i - 1
        } else {
            i
        };
        Calendar::new(AUTO[k].0, AUTO[k].1, 0.0)
    };
    range(calendar, start, stop, 4.0 * count + 8.0).unwrap_or_default()
}

const WEEK_STARTS: [&str; 7] = [
    "sunday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
];

/// Ticks for a `tickInterval` such as `1week`, or `None` when it is not one or asks for more
/// than `limit`.
pub(crate) fn interval_ticks(
    interval: &str,
    weekday: &str,
    start: f64,
    stop: f64,
    limit: f64,
) -> Option<Vec<f64>> {
    // `^([1-9]\d*)(millisecond|second|minute|hour|day|week|month)$`
    let digits = interval.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || interval.starts_with('0') {
        return None;
    }
    let unit = match &interval[digits..] {
        "millisecond" => TickUnit::Millisecond,
        "second" => TickUnit::Second,
        "minute" => TickUnit::Minute,
        "hour" => TickUnit::Hour,
        "day" => TickUnit::Day,
        "week" => TickUnit::Week,
        "month" => TickUnit::Month,
        _ => return None,
    };
    let step: f64 = interval[..digits].parse().unwrap_or(f64::INFINITY);
    let week_start = WEEK_STARTS.iter().position(|&d| d == weekday).unwrap_or(0) as f64;
    let calendar = Calendar::new(unit, step, week_start);
    if (stop - start) / (step.min(calendar.cycle()) * calendar.ms()) > limit {
        return None;
    }
    range(calendar, start, stop, limit)
}

fn pad(value: f64, fill: &str, width: usize) -> String {
    let sign = if value < 0.0 { "-" } else { "" };
    let digits = js_number(value.abs());
    let n = digits.len();
    if n < width {
        format!("{sign}{}{digits}", fill.repeat(width - n))
    } else {
        format!("{sign}{digits}")
    }
}

fn floored(date: Date, calendar: Calendar) -> Date {
    let mut d = date;
    calendar.floor(&mut d);
    d
}

fn unit(unit: TickUnit) -> Calendar {
    Calendar::new(unit, 1.0, 0.0)
}

fn week(start: f64) -> Calendar {
    Calendar::new(TickUnit::Week, 1.0, start)
}

/// Weeks since the last `week_start` day before the year began, which is d3's %U and %W.
fn week_of_year(date: Date, week_start: f64) -> f64 {
    let year_start = floored(date, unit(TickUnit::Year));
    span(
        floored(Date::new(year_start.time() - 1.0), week(week_start)),
        floored(date, week(week_start)),
        WEEK,
    )
}

/// The Thursday that decides which ISO week and ISO year a date belongs to.
fn iso_thursday(date: Date) -> Date {
    let thursday = week(4.0);
    let day = date.weekday();
    if day >= 4.0 || day == 0.0 {
        floored(date, thursday)
    } else {
        let mut d = floored(Date::new(date.time() - 1.0), thursday);
        thursday.offset(&mut d, 1.0);
        thursday.floor(&mut d);
        d
    }
}

/// Formats a date with a d3 time format specifier such as `%Y-%m-%d`.
pub(crate) fn time_format(specifier: &str, date: Date) -> String {
    let mut out = String::new();
    let mut chars = specifier.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut c = chars.next();
        let fill = match c {
            Some('-') => Some(""),
            Some('_') => Some(" "),
            Some('0') => Some("0"),
            _ => None,
        };
        let fill = match fill {
            Some(fill) => {
                c = chars.next();
                fill
            }
            None if c == Some('e') => " ",
            None => "0",
        };
        if let Some(c) = c {
            out += &directive(c, date, fill);
        }
    }
    out
}

fn directive(c: char, d: Date, p: &str) -> String {
    let weekday = d.weekday() as usize;
    let month = d.month() as usize;
    match c {
        'a' => WEEKDAYS[weekday][..3].to_string(),
        'A' => WEEKDAYS[weekday].to_string(),
        'b' => MONTHS[month][..3].to_string(),
        'B' => MONTHS[month].to_string(),
        'c' => time_format("%x, %X", d),
        'd' | 'e' => pad(d.date(), p, 2),
        'f' => pad(d.millis(), p, 3) + "000",
        'g' => pad(iso_thursday(d).year() % 100.0, p, 2),
        'G' => pad(iso_thursday(d).year() % 10000.0, p, 4),
        'H' => pad(d.hours(), p, 2),
        'I' => pad(
            if d.hours() % 12.0 == 0.0 {
                12.0
            } else {
                d.hours() % 12.0
            },
            p,
            2,
        ),
        'j' => pad(
            1.0 + span(
                floored(d, unit(TickUnit::Year)),
                floored(d, unit(TickUnit::Day)),
                DAY,
            ),
            p,
            3,
        ),
        'L' => pad(d.millis(), p, 3),
        'm' => pad(d.month() + 1.0, p, 2),
        'M' => pad(d.minutes(), p, 2),
        'p' => (if d.hours() >= 12.0 { "PM" } else { "AM" }).to_string(),
        'q' => js_number(1.0 + (d.month() / 3.0).floor()),
        'Q' => js_number(d.time()),
        's' => js_number((d.time() / 1000.0).floor()),
        'S' => pad(d.seconds(), p, 2),
        'u' => js_number(if weekday == 0 { 7.0 } else { weekday as f64 }),
        'U' => pad(week_of_year(d, 0.0), p, 2),
        'V' => {
            let thursday = iso_thursday(d);
            let year_start = floored(thursday, unit(TickUnit::Year));
            let first = floored(year_start, week(4.0));
            let extra = if year_start.weekday() == 4.0 {
                1.0
            } else {
                0.0
            };
            pad(span(first, thursday, WEEK) + extra, p, 2)
        }
        'w' => js_number(weekday as f64),
        'W' => pad(week_of_year(d, 1.0), p, 2),
        'x' => time_format("%-m/%-d/%Y", d),
        'X' => time_format("%-I:%M:%S %p", d),
        'y' => pad(d.year() % 100.0, p, 2),
        'Y' => pad(d.year() % 10000.0, p, 4),
        'Z' => "+0000".to_string(),
        c => c.to_string(),
    }
}
