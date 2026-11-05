//! The model of a user journey. Ported from pele's `diagrams/journey/db.ts`.

use crate::util::{collapse_indent, js_to_number, trim, trim_start};

#[derive(Debug, Clone)]
pub(crate) struct Task {
    pub task: String,
    /// As written: any number, or NaN when the score is not one.
    pub score: f64,
    pub people: Vec<String>,
    /// The name of the section in force when the task was declared, empty before the first.
    pub section: String,
}

#[derive(Debug, Default)]
pub(crate) struct JourneyDb {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub sections: Vec<String>,
    pub tasks: Vec<Task>,
    current_section: String,
}

/// Everyone named on a task, once each, in code unit order.
pub(crate) fn journey_actors(tasks: &[Task]) -> Vec<&str> {
    let mut names: Vec<&str> = tasks
        .iter()
        .flat_map(|t| t.people.iter().map(String::as_str))
        .collect();
    names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    names.dedup();
    names
}

impl JourneyDb {
    pub(crate) fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(trim_start(text).to_string());
    }

    pub(crate) fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(collapse_indent(text));
    }

    pub(crate) fn add_section(&mut self, text: String) {
        self.current_section.clone_from(&text);
        self.sections.push(text);
    }

    /// `data` is the rest of the line from its first colon: `:score` or `:score:actor, actor`.
    pub(crate) fn add_task(&mut self, task: String, data: &str) {
        let mut pieces = data[1..].split(':');
        let score = js_to_number(pieces.next().unwrap_or(""));
        let people = pieces
            .next()
            .map(|names| names.split(',').map(|n| trim(n).to_string()).collect())
            .unwrap_or_default();
        self.tasks.push(Task {
            task,
            score,
            people,
            section: self.current_section.clone(),
        });
    }
}
