//! The model of a timeline. Ported from pele's `diagrams/timeline/db.ts`.

use crate::util::{collapse_indent, trim_start};

#[derive(Debug, Clone)]
pub(crate) struct Period {
    pub text: String,
    pub events: Vec<String>,
    /// The index into `sections` of the section in force when the period was declared, `None`
    /// before the first one.
    pub section_index: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct TimelineDb {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub direction: &'static str,
    pub sections: Vec<String>,
    pub periods: Vec<Period>,
}

impl TimelineDb {
    pub(crate) fn new() -> TimelineDb {
        TimelineDb {
            title: None,
            acc_title: None,
            acc_descr: None,
            direction: "LR",
            sections: Vec::new(),
            periods: Vec::new(),
        }
    }

    pub(crate) fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(trim_start(text).to_string());
    }

    pub(crate) fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(collapse_indent(text));
    }

    pub(crate) fn add_section(&mut self, text: String) {
        self.sections.push(text);
    }

    pub(crate) fn add_period(&mut self, text: String) {
        self.periods.push(Period {
            text,
            events: Vec::new(),
            section_index: self.sections.len().checked_sub(1),
        });
    }

    /// Adds an event to the last period. Returns false when there is none; Mermaid fails then
    /// too, with a TypeError.
    pub(crate) fn add_event(&mut self, event: String) -> bool {
        match self.periods.last_mut() {
            Some(period) => {
                period.events.push(event);
                true
            }
            None => false,
        }
    }
}
