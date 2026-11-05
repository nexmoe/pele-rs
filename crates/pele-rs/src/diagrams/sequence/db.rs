//! What a sequence diagram describes, built as Mermaid's `SequenceDB` builds it. Ported from
//! pele's `diagrams/sequence/db.ts`.

use std::collections::HashMap;

use super::colors::is_css_color;
use crate::error::{Error, ErrorCode};
use crate::preprocess::{Config, Value, parse_yaml};
use crate::util::{is_space, is_word, trim, trim_start};

/// Message kinds, numbered as in Mermaid.
pub(crate) mod line {
    pub const SOLID: i32 = 0;
    pub const DOTTED: i32 = 1;
    pub const NOTE: i32 = 2;
    pub const SOLID_CROSS: i32 = 3;
    pub const DOTTED_CROSS: i32 = 4;
    pub const SOLID_OPEN: i32 = 5;
    pub const DOTTED_OPEN: i32 = 6;
    pub const LOOP_START: i32 = 10;
    pub const LOOP_END: i32 = 11;
    pub const ALT_START: i32 = 12;
    pub const ALT_ELSE: i32 = 13;
    pub const ALT_END: i32 = 14;
    pub const OPT_START: i32 = 15;
    pub const OPT_END: i32 = 16;
    pub const ACTIVE_START: i32 = 17;
    pub const ACTIVE_END: i32 = 18;
    pub const PAR_START: i32 = 19;
    pub const PAR_AND: i32 = 20;
    pub const PAR_END: i32 = 21;
    pub const RECT_START: i32 = 22;
    pub const RECT_END: i32 = 23;
    pub const SOLID_POINT: i32 = 24;
    pub const DOTTED_POINT: i32 = 25;
    pub const AUTONUMBER: i32 = 26;
    pub const CRITICAL_START: i32 = 27;
    pub const CRITICAL_OPTION: i32 = 28;
    pub const CRITICAL_END: i32 = 29;
    pub const BREAK_START: i32 = 30;
    pub const BREAK_END: i32 = 31;
    pub const PAR_OVER_START: i32 = 32;
    pub const BIDIRECTIONAL_SOLID: i32 = 33;
    pub const BIDIRECTIONAL_DOTTED: i32 = 34;
    pub const SOLID_TOP: i32 = 41;
    pub const SOLID_BOTTOM: i32 = 42;
    pub const STICK_TOP: i32 = 43;
    pub const STICK_BOTTOM: i32 = 44;
    pub const SOLID_ARROW_TOP_REVERSE: i32 = 45;
    pub const SOLID_ARROW_BOTTOM_REVERSE: i32 = 46;
    pub const STICK_ARROW_TOP_REVERSE: i32 = 47;
    pub const STICK_ARROW_BOTTOM_REVERSE: i32 = 48;
    pub const SOLID_TOP_DOTTED: i32 = 51;
    pub const SOLID_BOTTOM_DOTTED: i32 = 52;
    pub const STICK_TOP_DOTTED: i32 = 53;
    pub const STICK_BOTTOM_DOTTED: i32 = 54;
    pub const SOLID_ARROW_TOP_REVERSE_DOTTED: i32 = 55;
    pub const SOLID_ARROW_BOTTOM_REVERSE_DOTTED: i32 = 56;
    pub const STICK_ARROW_TOP_REVERSE_DOTTED: i32 = 57;
    pub const STICK_ARROW_BOTTOM_REVERSE_DOTTED: i32 = 58;
    pub const CENTRAL_CONNECTION: i32 = 59;
    pub const CENTRAL_CONNECTION_REVERSE: i32 = 60;
    pub const CENTRAL_CONNECTION_DUAL: i32 = 61;
}

pub(crate) const LEFT_OF: i32 = 0;
pub(crate) const RIGHT_OF: i32 = 1;
pub(crate) const OVER: i32 = 2;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SeqText {
    pub text: String,
    pub wrap: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SeqBoxData {
    pub text: Option<String>,
    pub color: String,
    pub wrap: Option<bool>,
}

#[derive(Debug, Clone)]
pub(crate) struct SeqBox {
    pub name: Option<String>,
    pub wrap: bool,
    pub fill: String,
    pub actor_keys: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SeqActor {
    /// Index into `boxes`.
    pub boxed: Option<usize>,
    pub name: String,
    pub description: String,
    pub wrap: bool,
    pub links: Vec<(String, Value)>,
    pub properties: Vec<(String, Value)>,
    pub details: Option<String>,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MessageText {
    Text(String),
    Numbering {
        start: Option<f64>,
        step: Option<f64>,
        visible: bool,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct SeqMessage {
    pub id: usize,
    pub from: Option<String>,
    pub to: Option<String>,
    pub message: MessageText,
    pub wrap: bool,
    pub kind: Option<i32>,
    pub placement: Option<i32>,
    pub central: i32,
}

/// A statement of the diagram, in the order the model is built from them.
#[derive(Debug, Clone)]
pub(crate) enum Stmt {
    Add {
        actor: String,
        draw: String,
        description: Option<SeqText>,
        config: Option<String>,
    },
    Create {
        actor: String,
        draw: String,
        description: Option<SeqText>,
        config: Option<String>,
    },
    Destroy {
        actor: String,
    },
    Mention {
        actor: String,
    },
    Message {
        from: String,
        to: String,
        signal: i32,
        msg: SeqText,
        activate: bool,
        central: i32,
    },
    /// An activation, deactivation or connection circle on one participant.
    On {
        actor: String,
        signal: i32,
    },
    Note {
        actors: (String, String),
        placement: i32,
        text: SeqText,
    },
    Links {
        actor: String,
        text: SeqText,
    },
    ALink {
        actor: String,
        text: SeqText,
    },
    Properties {
        actor: String,
        text: SeqText,
    },
    Details {
        actor: String,
        text: SeqText,
    },
    Index {
        start: Option<f64>,
        step: Option<f64>,
        visible: bool,
    },
    BoxStart(SeqBoxData),
    BoxEnd,
    /// The start, a divider or the end of a block.
    Block {
        signal: i32,
        text: Option<SeqText>,
    },
}

fn semantic(message: String) -> Error {
    Error::new(ErrorCode::Semantic, message)
}

/// `^:?wrap:` / `^:?nowrap:` prefixes.
pub(crate) fn extract_wrap(raw: &str) -> SeqText {
    let text = trim(raw);
    let bare = text.strip_prefix(':').unwrap_or(text);
    let (wrap, rest) = if let Some(rest) = bare.strip_prefix("wrap:") {
        (Some(true), rest)
    } else if let Some(rest) = bare.strip_prefix("nowrap:") {
        (Some(false), rest)
    } else {
        (None, text)
    };
    SeqText {
        text: trim(rest).to_string(),
        wrap,
    }
}

fn is_line_terminator(c: char) -> bool {
    matches!(c, '\r' | '\u{2028}' | '\u{2029}')
}

/// Splits a box line into a leading color and the rest, as Mermaid's
/// `/^((?:rgba?|hsla?)\s*\(.*\)|\w*)(.*)$/` does.
fn split_box(s: &str) -> Option<(String, String)> {
    for name in ["rgba", "rgb", "hsla", "hsl"] {
        let Some(rest) = s.strip_prefix(name) else {
            continue;
        };
        let after_ws = rest.trim_start_matches(is_space);
        if !after_ws.starts_with('(') {
            continue;
        }
        let fn_len = s.len() - after_ws.len() + 1;
        if let Some(close) = s.rfind(')').filter(|&c| c >= fn_len) {
            if s[fn_len..].contains(is_line_terminator) {
                return None;
            }
            return Some((s[..=close].to_string(), s[close + 1..].to_string()));
        }
        break;
    }
    if s.contains(is_line_terminator) {
        return None;
    }
    let word = s.find(|c: char| !is_word(c)).unwrap_or(s.len());
    Some((s[..word].to_string(), s[word..].to_string()))
}

/// The entries of a parsed `links` or `properties` value, as JavaScript's `Object.entries`.
fn entries(value: Value) -> Vec<(String, Value)> {
    match value {
        Value::String(s) => s
            .encode_utf16()
            .enumerate()
            .map(|(i, u)| (i.to_string(), Value::String(String::from_utf16_lossy(&[u]))))
            .collect(),
        Value::Seq(items) => items
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
        Value::Map(map) => map.into_iter().collect(),
        _ => Vec::new(),
    }
}

fn set(list: &mut Vec<(String, Value)>, key: String, value: Value) {
    match list.iter_mut().find(|(k, _)| *k == key) {
        Some(entry) => entry.1 = value,
        None => list.push((key, value)),
    }
}

pub(crate) struct SeqDb {
    /// Participants in the order first named.
    pub actors: Vec<(String, SeqActor)>,
    actor_index: HashMap<String, usize>,
    pub created: HashMap<String, usize>,
    pub destroyed: HashMap<String, usize>,
    pub boxes: Vec<SeqBox>,
    pub messages: Vec<SeqMessage>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    current_box: Option<usize>,
    last_created: Option<String>,
    last_destroyed: Option<String>,
    wrap_enabled: Option<bool>,
    sequence_wrap: bool,
    active: HashMap<String, i64>,
}

impl SeqDb {
    pub(crate) fn new(wrap: Option<bool>, sequence_wrap: bool) -> Self {
        Self {
            actors: Vec::new(),
            actor_index: HashMap::new(),
            created: HashMap::new(),
            destroyed: HashMap::new(),
            boxes: Vec::new(),
            messages: Vec::new(),
            title: None,
            acc_title: None,
            acc_descr: None,
            current_box: None,
            last_created: None,
            last_destroyed: None,
            wrap_enabled: wrap,
            sequence_wrap,
            active: HashMap::new(),
        }
    }

    fn auto_wrap(&self) -> bool {
        self.wrap_enabled.unwrap_or(self.sequence_wrap)
    }

    fn actor_mut(&mut self, id: &str) -> Option<&mut SeqActor> {
        self.actor_index.get(id).map(|&i| &mut self.actors[i].1)
    }

    pub(crate) fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(trim_start(text).to_string());
    }

    pub(crate) fn set_acc_description(&mut self, text: &str) {
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            out.push(c);
            if c == '\n' {
                while chars.next_if(|&c| is_space(c)).is_some() {}
            }
        }
        self.acc_descr = Some(out);
    }

    pub(crate) fn parse_box_data(s: &str) -> SeqBoxData {
        let found = split_box(s);
        let mut color = found
            .as_ref()
            .map(|(c, _)| trim(c).to_string())
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| "transparent".to_string());
        let mut title = found
            .as_ref()
            .map(|(_, t)| t)
            .filter(|t| !t.is_empty())
            .map(|t| trim(t).to_string());
        if !is_css_color(&color) {
            color = "transparent".to_string();
            title = Some(trim(s).to_string());
        }
        match title {
            None => SeqBoxData {
                text: None,
                color,
                wrap: None,
            },
            Some(title) => {
                let SeqText { text, wrap } = extract_wrap(&title);
                SeqBoxData {
                    text: (!text.is_empty()).then_some(text),
                    color,
                    wrap,
                }
            }
        }
    }

    fn add_box(&mut self, data: &SeqBoxData) {
        self.boxes.push(SeqBox {
            name: data.text.clone(),
            wrap: data.wrap.unwrap_or(self.auto_wrap()),
            fill: data.color.clone(),
            actor_keys: Vec::new(),
        });
        self.current_box = Some(self.boxes.len() - 1);
    }

    fn add_actor(
        &mut self,
        id: &str,
        description: Option<&SeqText>,
        mut kind: Option<String>,
        metadata: Option<&str>,
    ) -> Result<(), Error> {
        let name = id;
        let mut assigned = self.current_box;
        let mut doc: Option<Config> = None;
        if let Some(metadata) = metadata {
            let yaml = if metadata.contains('\n') {
                format!("{metadata}\n")
            } else {
                format!("{{\n{metadata}\n}}")
            };
            match parse_yaml(&yaml) {
                Ok(Value::Map(map)) => doc = Some(map),
                Ok(_) => {}
                Err(message) => {
                    return Err(Error::new(
                        ErrorCode::Syntax,
                        format!("Config of participant \"{id}\" is not valid YAML. {message}"),
                    ));
                }
            }
        }
        let mut description = description.cloned();
        if let Some(doc) = &doc {
            if let Some(t) = doc.get("type").filter(|v| !matches!(v, Value::Null)) {
                kind = Some(t.js_string());
            }
            let alias = doc.get("alias").filter(|v| match v {
                Value::Null => false,
                Value::Bool(b) => *b,
                Value::Number(n) => *n != 0.0 && !n.is_nan(),
                Value::String(s) => !s.is_empty(),
                _ => true,
            });
            if let Some(alias) = alias {
                if description.as_ref().is_none_or(|d| d.text == name) {
                    description = Some(SeqText {
                        text: alias.js_string(),
                        wrap: description.and_then(|d| d.wrap),
                    });
                }
            }
        }

        if let Some(&i) = self.actor_index.get(id) {
            let old_box = self.actors[i].1.boxed;
            if let (Some(current), Some(old)) = (self.current_box, old_box) {
                if current != old {
                    let old_name = &self.actors[i].1.name;
                    return Err(semantic(format!(
                        "A same participant should only be defined in one Box: {old_name} can't be in '{}' and in '{}' at the same time.",
                        self.boxes[old].name.as_deref().unwrap_or("undefined"),
                        self.boxes[current].name.as_deref().unwrap_or("undefined"),
                    )));
                }
            }
            assigned = old_box.or(self.current_box);
            self.actors[i].1.boxed = assigned;
            if name == self.actors[i].1.name && description.is_none() {
                return Ok(());
            }
        }
        // A participant that is only mentioned in a message has no kind and takes its name as
        // its label.
        let wrap = description.as_ref().and_then(|d| d.wrap);
        let description = match (&description, &kind) {
            (Some(d), Some(_)) => d.text.clone(),
            _ => name.to_string(),
        };
        let actor = SeqActor {
            boxed: assigned,
            name: name.to_string(),
            description,
            wrap: if kind.is_some() {
                wrap.unwrap_or(self.auto_wrap())
            } else {
                self.auto_wrap()
            },
            links: Vec::new(),
            properties: Vec::new(),
            details: None,
            kind: kind.unwrap_or_else(|| "participant".to_string()),
        };
        match self.actor_index.get(id) {
            Some(&i) => self.actors[i].1 = actor,
            None => {
                self.actor_index.insert(id.to_string(), self.actors.len());
                self.actors.push((id.to_string(), actor));
            }
        }
        if let Some(b) = self.current_box {
            self.boxes[b].actor_keys.push(id.to_string());
        }
        Ok(())
    }

    fn add_signal(
        &mut self,
        from: Option<&str>,
        to: Option<&str>,
        message: Option<&SeqText>,
        kind: i32,
        central: i32,
    ) -> Result<(), Error> {
        // Mermaid recounts every earlier message here; a running count per participant says
        // the same.
        if kind == line::ACTIVE_END {
            let count = from.map_or(0, |f| self.active.get(f).copied().unwrap_or(0));
            if count < 1 {
                return Err(semantic(format!(
                    "Trying to inactivate an inactive participant ({})",
                    from.unwrap_or("undefined")
                )));
            }
            if let Some(f) = from {
                self.active.insert(f.to_string(), count - 1);
            }
        } else if kind == line::ACTIVE_START {
            if let Some(f) = from {
                *self.active.entry(f.to_string()).or_insert(0) += 1;
            }
        }
        self.messages.push(SeqMessage {
            id: self.messages.len(),
            from: from.map(str::to_string),
            to: to.map(str::to_string),
            message: MessageText::Text(message.map_or(String::new(), |m| m.text.clone())),
            wrap: message.and_then(|m| m.wrap).unwrap_or(self.auto_wrap()),
            kind: Some(kind),
            placement: None,
            central,
        });
        Ok(())
    }

    fn add_note(&mut self, actors: &(String, String), placement: i32, message: &SeqText) {
        let wrap = message.wrap.unwrap_or(self.auto_wrap());
        self.messages.push(SeqMessage {
            id: self.messages.len(),
            from: Some(actors.0.clone()),
            to: Some(actors.1.clone()),
            message: MessageText::Text(message.text.clone()),
            wrap,
            kind: Some(line::NOTE),
            placement: Some(placement),
            central: 0,
        });
    }

    /// Builds the model from the statements, in order.
    pub(crate) fn apply(&mut self, statements: &[Stmt]) -> Result<(), Error> {
        for stmt in statements {
            self.apply_one(stmt)?;
        }
        Ok(())
    }

    fn apply_one(&mut self, stmt: &Stmt) -> Result<(), Error> {
        match stmt {
            Stmt::Index {
                start,
                step,
                visible,
            } => self.messages.push(SeqMessage {
                id: self.messages.len(),
                from: None,
                to: None,
                message: MessageText::Numbering {
                    start: *start,
                    step: *step,
                    visible: *visible,
                },
                wrap: false,
                kind: Some(line::AUTONUMBER),
                placement: None,
                central: 0,
            }),
            Stmt::Add {
                actor,
                draw,
                description,
                config,
            } => {
                self.add_actor(
                    actor,
                    description.as_ref(),
                    Some(draw.clone()),
                    config.as_deref(),
                )?;
            }
            Stmt::Mention { actor } => self.add_actor(actor, None, None, None)?,
            Stmt::Create {
                actor,
                draw,
                description,
                config,
            } => {
                if self.actor_index.contains_key(actor) {
                    return Err(semantic(
                        "It is not possible to have actors with the same id, even if one is destroyed before the next is created. Use 'AS' aliases to simulate the behavior".to_string(),
                    ));
                }
                self.last_created = Some(actor.clone());
                self.add_actor(
                    actor,
                    description.as_ref(),
                    Some(draw.clone()),
                    config.as_deref(),
                )?;
                self.created.insert(actor.clone(), self.messages.len());
            }
            Stmt::Destroy { actor } => {
                self.last_destroyed = Some(actor.clone());
                self.destroyed.insert(actor.clone(), self.messages.len());
            }
            Stmt::On { actor, signal } => self.add_signal(Some(actor), None, None, *signal, 0)?,
            Stmt::Note {
                actors,
                placement,
                text,
            } => self.add_note(actors, *placement, text),
            Stmt::Links { actor, text } | Stmt::Properties { actor, text } => {
                let links = matches!(stmt, Stmt::Links { .. });
                let Some(parsed) = crate::preprocess::parse_json(&text.text) else {
                    return Ok(());
                };
                if let Some(a) = self.actor_mut(actor) {
                    for (k, v) in entries(parsed) {
                        set(
                            if links {
                                &mut a.links
                            } else {
                                &mut a.properties
                            },
                            k,
                            v,
                        );
                    }
                }
            }
            Stmt::ALink { actor, text } => {
                if let Some(a) = self.actor_mut(actor) {
                    // `text.slice(0, sep - 1)` with `sep` = -1 when there is no `@`.
                    let units: Vec<u16> = text.text.encode_utf16().collect();
                    let sep = units.iter().position(|&u| u == 64).map_or(-1, |s| s as i64);
                    let len = units.len() as i64;
                    let end = if sep - 1 < 0 { len + sep - 1 } else { sep - 1 };
                    let key = String::from_utf16_lossy(&units[..end.clamp(0, len) as usize]);
                    let value = String::from_utf16_lossy(&units[(sep + 1) as usize..]);
                    set(
                        &mut a.links,
                        trim(&key).to_string(),
                        Value::String(trim(&value).to_string()),
                    );
                }
            }
            Stmt::Details { actor, text } => {
                if let Some(a) = self.actor_mut(actor) {
                    a.details = Some(text.text.clone());
                }
            }
            Stmt::Message {
                from,
                to,
                signal,
                msg,
                central,
                ..
            } => {
                if let Some(created) = self.last_created.take() {
                    if *to != created {
                        return Err(semantic(format!(
                            "The created participant {created} does not have an associated creating message after its declaration. Please check the sequence diagram."
                        )));
                    }
                } else if let Some(destroyed) = self.last_destroyed.take() {
                    if *to != destroyed && *from != destroyed {
                        return Err(semantic(format!(
                            "The destroyed participant {destroyed} does not have an associated destroying message after its declaration. Please check the sequence diagram."
                        )));
                    }
                }
                self.add_signal(Some(from), Some(to), Some(msg), *signal, *central)?;
            }
            Stmt::BoxStart(data) => self.add_box(data),
            Stmt::BoxEnd => self.current_box = None,
            Stmt::Block { signal, text } => {
                self.add_signal(None, None, text.as_ref(), *signal, 0)?
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_wrap_prefixes_and_boxes() {
        assert_eq!(
            extract_wrap(" :wrap: hi "),
            SeqText {
                text: "hi".to_string(),
                wrap: Some(true)
            }
        );
        assert_eq!(
            extract_wrap("nowrap:x"),
            SeqText {
                text: "x".to_string(),
                wrap: Some(false)
            }
        );
        assert_eq!(
            extract_wrap("plain"),
            SeqText {
                text: "plain".to_string(),
                wrap: None
            }
        );
        let b = SeqDb::parse_box_data("Aqua Group A");
        assert_eq!(
            (b.color.as_str(), b.text.as_deref()),
            ("Aqua", Some("Group A"))
        );
        let b = SeqDb::parse_box_data("rgb(1, 2, 3) Team");
        assert_eq!(
            (b.color.as_str(), b.text.as_deref()),
            ("rgb(1, 2, 3)", Some("Team"))
        );
        let b = SeqDb::parse_box_data("Team One");
        assert_eq!(
            (b.color.as_str(), b.text.as_deref()),
            ("transparent", Some("Team One"))
        );
    }
}
