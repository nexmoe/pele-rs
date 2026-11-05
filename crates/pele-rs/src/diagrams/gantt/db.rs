//! The model of a gantt chart, and the scheduling of its tasks. Ported from pele's
//! `diagrams/gantt/db.ts`.

use std::collections::HashMap;

use super::dates::{Date, Unit, WEEKDAYS, add, add_days, format, iso_date, parse_strict};
use super::legacy_date;
use crate::error::{Error, ErrorCode};
use crate::url::safe_url;
use crate::util::{collapse_indent, is_space, trim, trim_start};

/// Scheduling walks tasks day by day and parses dates with a format the diagram supplies.
/// This bounds the total, in rough character operations, so a hostile diagram fails instead
/// of hanging.
const WORK_LIMIT: f64 = 4e8;
const DAY_STEP: f64 = 400.0;
const DATED_STEP: f64 = 200.0;
const FORMAT_STEP: f64 = 400.0;

#[derive(Debug, Clone, Default)]
pub(crate) struct Task {
    pub id: String,
    /// The task's label, under the name Mermaid gives it.
    pub task: String,
    pub section: String,
    /// Unset until `schedule` has run, and still unset when what the task refers to never
    /// resolves.
    pub start_time: Option<Date>,
    pub end_time: Option<Date>,
    /// Where the bar ends when excluded days pushed the task's end out.
    pub render_end_time: Option<Date>,
    manual_end_time: bool,
    processed: bool,
    pub active: bool,
    pub done: bool,
    pub crit: bool,
    pub milestone: bool,
    pub vert: bool,
    /// Row index in source order; -1 for vertical markers, which take no row.
    pub order: i64,
    pub classes: Vec<String>,
    prev_task_id: Option<String>,
    raw_start: Option<String>,
    raw_end: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GanttDb {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub date_format: String,
    pub axis_format: String,
    pub tick_interval: Option<String>,
    pub today_marker: String,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub inclusive_end_dates: bool,
    pub top_axis: bool,
    pub display_mode: String,
    /// The day week-based ticks start on, when the chart names one.
    pub weekday: Option<&'static str>,
    pub weekend: &'static str,
    pub sections: Vec<String>,
    pub links: HashMap<String, String>,
    pub tasks: Vec<Task>,

    by_id: HashMap<String, usize>,
    current_section: String,
    task_count: usize,
    last_order: i64,
    last_task_id: Option<String>,
    now: f64,
    work: f64,
    excluded_days: [bool; 7],
    dated_tokens: bool,
}

/// `str.trim()` matched against `^(\d+(?:\.\d+)?)([Mdhmswy]|ms)$`.
pub(crate) fn parse_duration(text: &str) -> Option<(f64, Unit)> {
    let s = trim(text);
    let b = s.as_bytes();
    let mut i = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if i == 0 {
        return None;
    }
    if b.get(i) == Some(&b'.') {
        let frac = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
        if frac > 0 {
            i += 1 + frac;
        }
    }
    let unit = match &s[i..] {
        "M" => Unit::Month,
        "d" => Unit::D,
        "h" => Unit::H,
        "m" => Unit::M,
        "s" => Unit::S,
        "w" => Unit::W,
        "y" => Unit::Y,
        "ms" => Unit::Ms,
        _ => return None,
    };
    Some((s[..i].parse().unwrap_or(f64::NAN), unit))
}

fn merge_tokens(existing: &mut Vec<String>, text: &str) {
    for token in text.to_lowercase().split(|c: char| c == ',' || is_space(c)) {
        if !token.is_empty() && !existing.iter().any(|e| e == token) {
            existing.push(token.to_string());
        }
    }
}

/// `^\s*(active|done|crit|milestone|vert)\s*$`.
fn tag(field: &str) -> Option<&str> {
    let t = trim(field);
    ["active", "done", "crit", "milestone", "vert"]
        .into_iter()
        .find(|&tag| tag == t)
}

/// The ids after `after` or `until` and `\s+`, as `([\d\w- ]+)` takes them.
fn reference<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = text.strip_prefix(keyword)?;
    let spaced = trim_start(rest);
    if spaced.len() == rest.len() {
        return None;
    }
    // `\s+` backtracks: the space it takes last may also start the ids.
    let ids_at = |s: &'a str| {
        let n = s
            .bytes()
            .take_while(|&c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || c == b' ')
            .count();
        (n > 0).then(|| &s[..n])
    };
    ids_at(spaced).or_else(|| {
        let last_space = rest.len() - spaced.len();
        let before = &rest[..last_space];
        let c = before.chars().last()?;
        (c == ' ')
            .then(|| ids_at(&rest[last_space - 1..]))
            .flatten()
    })
}

fn semantic(message: String) -> Error {
    Error::new(ErrorCode::Semantic, message)
}

impl GanttDb {
    pub(crate) fn new() -> GanttDb {
        GanttDb {
            weekend: "saturday",
            ..GanttDb::default()
        }
    }

    pub(crate) fn set_includes(&mut self, text: &str) {
        merge_tokens(&mut self.includes, text);
    }

    pub(crate) fn set_excludes(&mut self, text: &str) {
        merge_tokens(&mut self.excludes, text);
    }

    pub(crate) fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(trim_start(text).to_string());
    }

    pub(crate) fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(collapse_indent(text));
    }

    pub(crate) fn add_section(&mut self, text: String) {
        self.current_section = text.clone();
        self.sections.push(text);
    }

    pub(crate) fn add_task(&mut self, descr: String, data: &str) -> Result<(), Error> {
        let mut fields: Vec<&str> = data.strip_prefix(':').unwrap_or(data).split(',').collect();
        let mut task = Task {
            section: self.current_section.clone(),
            order: -1,
            prev_task_id: self.last_task_id.clone(),
            ..Task::default()
        };
        while let Some(found) = fields.first().and_then(|f| tag(f)) {
            match found {
                "active" => task.active = true,
                "done" => task.done = true,
                "crit" => task.crit = true,
                "milestone" => task.milestone = true,
                _ => task.vert = true,
            }
            fields.remove(0);
        }
        // Mermaid fails with a TypeError on both of these.
        if fields.is_empty() {
            return Err(semantic(format!(
                "Task \"{}\" needs a duration or an end date.",
                trim(&descr)
            )));
        }
        if fields.len() > 3 {
            return Err(semantic(format!(
                "Task \"{}\" has too many fields. A task takes an id, a start, and an end or duration.",
                trim(&descr)
            )));
        }
        let fields: Vec<String> = fields.iter().map(|f| trim(f).to_string()).collect();
        if fields.len() == 3 {
            task.id = fields[0].clone();
        } else {
            self.task_count += 1;
            task.id = format!("task{}", self.task_count);
        }
        if fields.len() > 1 {
            task.raw_start = Some(fields[fields.len() - 2].clone());
        }
        task.raw_end = fields[fields.len() - 1].clone();
        task.task = descr;
        if !task.vert {
            task.order = self.last_order;
            self.last_order += 1;
        }
        self.last_task_id = Some(task.id.clone());
        self.by_id.insert(task.id.clone(), self.tasks.len());
        self.tasks.push(task);
        Ok(())
    }

    pub(crate) fn set_link(&mut self, ids: &str, link: &str) {
        let href = safe_url(link);
        for id in ids.split(',') {
            if self.by_id.contains_key(id) {
                self.links.insert(id.to_string(), href.clone());
            }
        }
        self.set_class(ids, "clickable");
    }

    pub(crate) fn set_class(&mut self, ids: &str, class: &str) {
        for id in ids.split(',') {
            if let Some(&k) = self.by_id.get(id) {
                self.tasks[k].classes.push(class.to_string());
            }
        }
    }

    /// Resolves every task's start and end. A task may refer to tasks defined after it, so,
    /// like Mermaid, this makes up to eleven passes, each of which schedules every task again.
    pub(crate) fn schedule(&mut self, now: f64) -> Result<(), Error> {
        self.now = now;
        self.work = 0.0;
        let weekend_start = if self.weekend == "friday" { 5 } else { 6 };
        let weekends = self.excludes.iter().any(|e| e == "weekends");
        for (day, name) in WEEKDAYS.iter().enumerate() {
            let iso = if day == 0 { 7 } else { day };
            self.excluded_days[day] = (weekends
                && (iso == weekend_start || iso == weekend_start + 1))
                || self.excludes.contains(&name.to_lowercase());
        }
        // A date written with only these tokens and separators can never read as a weekday
        // name, so when nothing else is excluded or included there is no need to format each
        // day.
        let numeric = trim(&self.date_format)
            .chars()
            .all(|c| "YMDHhmsSZXx-/.:, T".contains(c));
        self.dated_tokens = !self.includes.is_empty() || !numeric;
        if self.excludes.iter().any(|token| {
            token != "weekends" && !WEEKDAYS.iter().any(|d| d.to_lowercase() == *token)
        }) {
            self.dated_tokens = true;
        }
        for task in &mut self.tasks {
            task.start_time = None;
            task.end_time = None;
            task.render_end_time = None;
            task.processed = false;
            task.manual_end_time = false;
        }
        let mut all = self.compile_tasks()?;
        let mut i = 0;
        while !all && i < 10 {
            all = self.compile_tasks()?;
            i += 1;
        }
        Ok(())
    }

    /// Whether a day is excluded. A day matches a token by its weekday name, by its date as
    /// YYYY-MM-DD, or by its date written in the chart's own format.
    pub(crate) fn is_invalid_date(&mut self, date: Date) -> Result<bool, Error> {
        let weekday = self.excluded_days[date.weekday() as usize];
        if !self.dated_tokens {
            return Ok(weekday);
        }
        let template = trim(&self.date_format).to_string();
        let date_only = iso_date(date);
        let mut formatted = Some(date_only.clone());
        self.spend(DATED_STEP)?;
        if template != "YYYY-MM-DD" {
            self.spend(FORMAT_STEP + template.encode_utf16().count() as f64 * 8.0)?;
            formatted = format(date, &template);
        }
        let listed = |set: &[String]| {
            set.contains(&date_only) || formatted.as_ref().is_some_and(|f| set.contains(f))
        };
        if listed(&self.includes) {
            return Ok(false);
        }
        Ok(weekday || listed(&self.excludes))
    }

    fn spend(&mut self, units: f64) -> Result<(), Error> {
        self.work += units;
        if self.work > WORK_LIMIT {
            return Err(Error::new(
                ErrorCode::Limit,
                "The gantt chart takes too much work to schedule.",
            ));
        }
        Ok(())
    }

    fn parse(&mut self, text: &str, template: &str) -> Result<Option<Date>, Error> {
        self.spend(
            (template.encode_utf16().count() as f64 + 1.0)
                * (text.encode_utf16().count() as f64 + 1.0),
        )?;
        Ok(parse_strict(text, template, Date(self.now)))
    }

    fn today(&self) -> Date {
        let mut today = Date(self.now);
        today.set_hours(0.0, 0.0, 0.0, 0.0);
        today
    }

    fn compile_tasks(&mut self) -> Result<bool, Error> {
        let mut all = true;
        for k in 0..self.tasks.len() {
            self.compile_task(k)?;
            all = all && self.tasks[k].processed;
        }
        Ok(all)
    }

    fn compile_task(&mut self, k: usize) -> Result<(), Error> {
        match self.tasks[k].raw_start.clone() {
            None => {
                // Mermaid fails with a TypeError when the first task has no start. It starts
                // today here.
                let previous = self.tasks[k]
                    .prev_task_id
                    .as_ref()
                    .and_then(|id| self.by_id.get(id));
                self.tasks[k].start_time = match previous {
                    Some(&p) => self.tasks[p].end_time,
                    None => Some(self.today()),
                };
            }
            Some(start) => {
                if let Some(time) = self.start_date(&start)? {
                    self.tasks[k].start_time = Some(time);
                }
            }
        }
        let Some(start) = self.tasks[k].start_time else {
            return Ok(());
        };
        let raw_end = self.tasks[k].raw_end.clone();
        let Some(end) = self.end_date(start, &raw_end)? else {
            return Ok(());
        };
        self.tasks[k].end_time = Some(end);
        self.tasks[k].processed = true;
        self.tasks[k].manual_end_time = self.parse(&raw_end, "YYYY-MM-DD")?.is_some();
        self.check_task_dates(k)
    }

    fn references(&self, list: &str) -> Vec<usize> {
        list.split(' ')
            .filter(|id| !id.is_empty())
            .filter_map(|id| self.by_id.get(id).copied())
            .collect()
    }

    fn start_date(&mut self, raw: &str) -> Result<Option<Date>, Error> {
        let text = trim(raw);
        let template = trim(&self.date_format).to_string();
        if (template == "x" || template == "X")
            && !text.is_empty()
            && text.bytes().all(|c| c.is_ascii_digit())
        {
            let date = Date::new(text.parse().unwrap_or(f64::NAN));
            if !date.valid() {
                return Err(semantic(format!("Invalid date:{text}")));
            }
            return Ok(Some(date));
        }
        if let Some(ids) = reference(text, "after") {
            let mut latest: Option<usize> = None;
            for t in self.references(ids) {
                let later = match latest {
                    None => true,
                    Some(l) => time(self.tasks[t].end_time) > time(self.tasks[l].end_time),
                };
                if later {
                    latest = Some(t);
                }
            }
            return Ok(match latest {
                Some(l) => self.tasks[l].end_time,
                None => Some(self.today()),
            });
        }
        if let Some(parsed) = self.parse(text, &template)? {
            return Ok(Some(parsed));
        }
        // What does not fit the format gets a second chance with the JavaScript date parser,
        // as in Mermaid.
        let date = legacy_date::parse(text);
        if !date.valid() || date.year() < -10000.0 || date.year() > 10000.0 {
            return Err(semantic(format!("Invalid date:{text}")));
        }
        Ok(Some(date))
    }

    fn end_date(&mut self, start: Date, raw: &str) -> Result<Option<Date>, Error> {
        let text = trim(raw);
        if let Some(ids) = reference(text, "until") {
            let mut earliest: Option<usize> = None;
            for t in self.references(ids) {
                let sooner = match earliest {
                    None => true,
                    Some(e) => time(self.tasks[t].start_time) < time(self.tasks[e].start_time),
                };
                if sooner {
                    earliest = Some(t);
                }
            }
            return Ok(match earliest {
                Some(e) => self.tasks[e].start_time,
                None => Some(self.today()),
            });
        }
        let template = trim(&self.date_format).to_string();
        if let Some(parsed) = self.parse(text, &template)? {
            return Ok(Some(if self.inclusive_end_dates {
                add_days(parsed, 1.0)
            } else {
                parsed
            }));
        }
        let Some((value, unit)) = parse_duration(text) else {
            return Ok(Some(start));
        };
        let end = add(start, value, unit);
        Ok(Some(if end.valid() { end } else { start }))
    }

    /// Pushes the end of a task out by one day for every excluded day it covers.
    fn check_task_dates(&mut self, k: usize) -> Result<(), Error> {
        let task = &self.tasks[k];
        if self.excludes.is_empty() || task.manual_end_time {
            return Ok(());
        }
        let (Some(start), Some(mut end)) = (task.start_time, task.end_time) else {
            return Ok(());
        };
        let mut cursor = add_days(start, 1.0);
        let max_end = add_days(end, 10000.0).time();
        let mut invalid = false;
        let mut render_end = None;
        while cursor.time() <= end.time() {
            self.spend(DAY_STEP)?;
            if !invalid {
                render_end = Some(end);
            }
            invalid = self.is_invalid_date(cursor)?;
            if invalid {
                end.set_date(end.date() + 1.0);
                if end.time() > max_end {
                    return Err(semantic(
                        "Failed to find a valid date that was not excluded by `excludes` after 10,000 iterations.".to_string(),
                    ));
                }
            }
            cursor.set_date(cursor.date() + 1.0);
        }
        self.tasks[k].end_time = Some(end);
        self.tasks[k].render_end_time = render_end;
        Ok(())
    }
}

/// A time as JavaScript compares an optional date: NaN when there is none.
fn time(date: Option<Date>) -> f64 {
    date.map_or(f64::NAN, Date::time)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_references_and_durations() {
        assert_eq!(reference("after a b", "after"), Some("a b"));
        assert_eq!(reference("after  a", "after"), Some("a"));
        assert_eq!(reference("after  #", "after"), Some(" "));
        assert_eq!(reference("aftera", "after"), None);
        assert_eq!(parse_duration(" 2.5d "), Some((2.5, Unit::D)));
        assert_eq!(parse_duration("3ms"), Some((3.0, Unit::Ms)));
        assert_eq!(parse_duration("3.d"), None);
    }
}
