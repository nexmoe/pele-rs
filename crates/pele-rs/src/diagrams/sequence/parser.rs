//! A hand-written parser for Mermaid's sequence grammar. Ported from pele's
//! `diagrams/sequence/parser.ts`. Blocks nest without recursion: open ones are kept on a stack.

use super::db::{LEFT_OF, OVER, RIGHT_OF, SeqDb, SeqText, Stmt, extract_wrap, line};
use super::lexer::{T, tokenize};
use crate::error::{Error, ErrorCode};
use crate::util::trim;

fn signal_of(t: T) -> Option<i32> {
    use T::*;
    Some(match t {
        SolidOpenArrow => line::SOLID_OPEN,
        DottedOpenArrow => line::DOTTED_OPEN,
        SolidArrow => line::SOLID,
        DottedArrow => line::DOTTED,
        BidirectionalSolidArrow => line::BIDIRECTIONAL_SOLID,
        BidirectionalDottedArrow => line::BIDIRECTIONAL_DOTTED,
        SolidCross => line::SOLID_CROSS,
        DottedCross => line::DOTTED_CROSS,
        SolidPoint => line::SOLID_POINT,
        DottedPoint => line::DOTTED_POINT,
        SolidArrowTop => line::SOLID_TOP,
        SolidArrowBottom => line::SOLID_BOTTOM,
        StickArrowTop => line::STICK_TOP,
        StickArrowBottom => line::STICK_BOTTOM,
        SolidArrowTopDotted => line::SOLID_TOP_DOTTED,
        SolidArrowBottomDotted => line::SOLID_BOTTOM_DOTTED,
        StickArrowTopDotted => line::STICK_TOP_DOTTED,
        StickArrowBottomDotted => line::STICK_BOTTOM_DOTTED,
        SolidArrowTopReverse => line::SOLID_ARROW_TOP_REVERSE,
        SolidArrowBottomReverse => line::SOLID_ARROW_BOTTOM_REVERSE,
        StickArrowTopReverse => line::STICK_ARROW_TOP_REVERSE,
        StickArrowBottomReverse => line::STICK_ARROW_BOTTOM_REVERSE,
        SolidArrowTopReverseDotted => line::SOLID_ARROW_TOP_REVERSE_DOTTED,
        SolidArrowBottomReverseDotted => line::SOLID_ARROW_BOTTOM_REVERSE_DOTTED,
        StickArrowTopReverseDotted => line::STICK_ARROW_TOP_REVERSE_DOTTED,
        StickArrowBottomReverseDotted => line::STICK_ARROW_BOTTOM_REVERSE_DOTTED,
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum K {
    Top,
    Box,
    Loop,
    Rect,
    Opt,
    Break,
    Alt,
    Par,
    Critical,
}

struct Frame {
    kind: K,
}

const SIGNAL_TOKENS: &str = "'SOLID_OPEN_ARROW', 'DOTTED_OPEN_ARROW', 'SOLID_ARROW', 'DOTTED_ARROW', \
     'SOLID_CROSS', 'DOTTED_CROSS', 'SOLID_POINT', 'DOTTED_POINT'";

struct Parser<'a> {
    types: Vec<T>,
    texts: Vec<String>,
    starts: Vec<usize>,
    i: usize,
    src: &'a str,
    out: Vec<Stmt>,
}

type Res<T> = Result<T, Error>;

impl Parser<'_> {
    fn t(&self) -> T {
        self.types[self.i]
    }

    fn take(&mut self) -> String {
        let text = std::mem::take(&mut self.texts[self.i]);
        self.i += 1;
        text
    }

    fn fail<X>(&self, expected: &str) -> Res<X> {
        let units = self.starts[self.i];
        let mut count = 0;
        let mut at = self.src.len();
        for (i, c) in self.src.char_indices() {
            if count >= units {
                at = i;
                break;
            }
            count += c.len_utf16();
        }
        if self.t() == T::Error {
            return Err(Error::new(ErrorCode::Syntax, "Unrecognized text.").at(self.src, at));
        }
        Err(Error::new(
            ErrorCode::Syntax,
            format!("Expecting {expected}, got '{}'", self.t().name()),
        )
        .at(self.src, at))
    }

    fn expect(&mut self, t: T) -> Res<String> {
        if self.t() != t {
            return self.fail(&format!("'{}'", t.name()));
        }
        Ok(self.take())
    }

    fn actor(&mut self) -> Res<String> {
        if !matches!(self.t(), T::Actor | T::Link) {
            return self.fail("'ACTOR'");
        }
        Ok(self.take())
    }

    /// The text of a message: what follows its colon.
    fn text2(&mut self) -> Res<SeqText> {
        let raw = self.expect(T::Txt)?;
        let trimmed = trim(&raw);
        Ok(extract_wrap(trimmed.get(1..).unwrap_or("")))
    }

    /// A participant statement, as `(actor, draw, description, config)`, or `None` for `destroy`.
    fn participant(&mut self) -> Res<Stmt> {
        let t = self.t();
        self.i += 1;
        if t == T::Destroy {
            let actor = self.actor()?;
            self.expect(T::Newline)?;
            return Ok(Stmt::Destroy { actor });
        }
        let was_actor_token = self.t() == T::Actor;
        let actor = self.actor()?;
        let mut config = None;
        if self.t() == T::ConfigStart && was_actor_token {
            self.i += 1;
            let content = self.expect(T::ConfigContent)?;
            self.expect(T::ConfigEnd)?;
            config = Some(trim(&content).to_string());
        }
        let draw = if t == T::Participant {
            "participant"
        } else {
            "actor"
        }
        .to_string();
        let mut description = None;
        if self.t() == T::As {
            self.i += 1;
            let alias = self.expect(T::RestOfLine)?;
            self.expect(T::Newline)?;
            description = Some(extract_wrap(&alias));
        } else if self.t() == T::Newline {
            self.i += 1;
        } else {
            return self.fail("'AS', 'NEWLINE'");
        }
        Ok(Stmt::Add {
            actor,
            draw,
            description,
            config,
        })
    }

    fn signal(&mut self, from: String) -> Res<()> {
        let mut central_from = false;
        if self.t() == T::Central {
            central_from = true;
            self.i += 1;
        }
        let Some(signal) = signal_of(self.t()) else {
            return self.fail(SIGNAL_TOKENS);
        };
        self.i += 1;
        let mark = self.t();
        if mark == T::Central || (!central_from && (mark == T::Plus || mark == T::Minus)) {
            self.i += 1;
        }
        let to = self.actor()?;
        let msg = self.text2()?;
        self.out.push(Stmt::Mention {
            actor: from.clone(),
        });
        self.out.push(Stmt::Mention { actor: to.clone() });
        let mut message = Stmt::Message {
            from: from.clone(),
            to: to.clone(),
            signal,
            msg,
            activate: false,
            central: 0,
        };
        let mut after = Vec::new();
        let set = |message: &mut Stmt, act: bool, cc: i32| {
            if let Stmt::Message {
                activate, central, ..
            } = message
            {
                *activate = act;
                *central = cc;
            }
        };
        if central_from {
            let dual = mark == T::Central;
            set(
                &mut message,
                dual,
                if dual {
                    line::CENTRAL_CONNECTION_DUAL
                } else {
                    line::CENTRAL_CONNECTION_REVERSE
                },
            );
            if dual {
                after.push(Stmt::On {
                    actor: to.clone(),
                    signal: line::CENTRAL_CONNECTION,
                });
            }
            after.push(Stmt::On {
                actor: from,
                signal: line::CENTRAL_CONNECTION_REVERSE,
            });
        } else if mark == T::Plus {
            set(&mut message, true, 0);
            after.push(Stmt::On {
                actor: to,
                signal: line::ACTIVE_START,
            });
        } else if mark == T::Minus {
            after.push(Stmt::On {
                actor: from,
                signal: line::ACTIVE_END,
            });
        } else if mark == T::Central {
            set(&mut message, true, line::CENTRAL_CONNECTION);
            after.push(Stmt::On {
                actor: to,
                signal: line::CENTRAL_CONNECTION,
            });
        }
        self.out.push(message);
        self.out.extend(after);
        Ok(())
    }

    fn menu(&mut self, kind: fn(String, SeqText) -> Stmt) -> Res<()> {
        self.i += 1;
        let who = self.actor()?;
        let text = self.text2()?;
        self.out.push(Stmt::Mention { actor: who.clone() });
        self.out.push(kind(who, text));
        Ok(())
    }

    fn note(&mut self) -> Res<()> {
        self.i += 1;
        let t = self.t();
        if t == T::Over {
            self.i += 1;
            let first = self.actor()?;
            let mut ids = (first.clone(), first.clone());
            self.out.push(Stmt::Mention {
                actor: first.clone(),
            });
            if self.t() == T::Comma {
                self.i += 1;
                let second = self.actor()?;
                self.out.push(Stmt::Mention {
                    actor: second.clone(),
                });
                ids = (first, second);
            }
            let text = self.text2()?;
            self.out.push(Stmt::Note {
                actors: ids,
                placement: OVER,
                text,
            });
            return Ok(());
        }
        if t != T::LeftOf && t != T::RightOf {
            return self.fail("'over', 'left_of', 'right_of'");
        }
        self.i += 1;
        let who = self.actor()?;
        let placement = if t == T::LeftOf { LEFT_OF } else { RIGHT_OF };
        let text = self.text2()?;
        self.out.push(Stmt::Mention { actor: who.clone() });
        self.out.push(Stmt::Note {
            actors: (who.clone(), who),
            placement,
            text,
        });
        Ok(())
    }

    fn autonumber(&mut self) -> Res<()> {
        self.i += 1;
        let mut start = None;
        let mut step = None;
        let mut visible = true;
        if self.t() == T::Num {
            start = Some(self.take().parse().unwrap_or(f64::NAN));
            step = Some(if self.t() == T::Num {
                self.take().parse().unwrap_or(f64::NAN)
            } else {
                1.0
            });
        } else if self.t() == T::Off {
            self.i += 1;
            visible = false;
        }
        if self.t() != T::Newline {
            return self.fail("'NEWLINE', 'NUM', 'off'");
        }
        self.i += 1;
        self.out.push(Stmt::Index {
            start,
            step,
            visible,
        });
        Ok(())
    }

    fn close(&mut self, frame: Frame) {
        // The end of a block carries no text, whatever its start had.
        let signal = match frame.kind {
            K::Box => {
                self.out.push(Stmt::BoxEnd);
                return;
            }
            K::Loop => line::LOOP_END,
            K::Rect => line::RECT_END,
            K::Opt => line::OPT_END,
            K::Break => line::BREAK_END,
            K::Alt => line::ALT_END,
            K::Par => line::PAR_END,
            K::Critical => line::CRITICAL_END,
            K::Top => return,
        };
        self.out.push(Stmt::Block { signal, text: None });
    }
}

pub(crate) fn parse_sequence(src: &str, db: &mut SeqDb, max_depth: usize) -> Result<(), Error> {
    let units: Vec<u16> = src.encode_utf16().collect();
    let tokens = tokenize(&units);
    let mut p = Parser {
        types: tokens.types,
        texts: tokens.texts,
        starts: tokens.starts,
        i: 0,
        src,
        out: Vec::new(),
    };
    while p.t() == T::Newline {
        p.i += 1;
    }
    p.expect(T::Sd)?;
    let mut stack: Vec<Frame> = Vec::new();
    let mut kind = K::Top;
    loop {
        let t = p.t();
        if kind == K::Box {
            match t {
                T::Participant | T::ParticipantActor | T::Destroy => {
                    let stmt = p.participant()?;
                    p.out.push(stmt);
                }
                T::Newline => p.i += 1,
                T::EndKw => {
                    p.i += 1;
                    if let Some(frame) = stack.pop() {
                        p.close(frame);
                    }
                    kind = stack.last().map_or(K::Top, |f| f.kind);
                }
                _ => {
                    return p
                        .fail("'end', 'NEWLINE', 'participant', 'participant_actor', 'destroy'");
                }
            }
            continue;
        }
        match t {
            T::Newline | T::Invalid => p.i += 1,
            T::Participant | T::ParticipantActor | T::Destroy => {
                let stmt = p.participant()?;
                p.out.push(stmt);
            }
            T::Create => {
                p.i += 1;
                if !matches!(p.t(), T::Participant | T::ParticipantActor | T::Destroy) {
                    return p.fail("'participant', 'participant_actor', 'destroy'");
                }
                let stmt = match p.participant()? {
                    Stmt::Add {
                        actor,
                        draw,
                        description,
                        config,
                    } => Stmt::Create {
                        actor,
                        draw,
                        description,
                        config,
                    },
                    // `create destroy x` creates nothing in Mermaid either: it is a destroy.
                    Stmt::Destroy { actor } => Stmt::Create {
                        actor,
                        draw: "participant".to_string(),
                        description: None,
                        config: None,
                    },
                    other => other,
                };
                p.out.push(stmt);
            }
            T::Actor => {
                let from = p.actor()?;
                p.signal(from)?;
                p.expect(T::Newline)?;
            }
            T::Link => {
                let next = p.types[p.i + 1];
                if next == T::Actor || next == T::Link {
                    p.menu(|actor, text| Stmt::ALink { actor, text })?;
                } else {
                    let from = p.actor()?;
                    p.signal(from)?;
                }
                p.expect(T::Newline)?;
            }
            T::Links => {
                p.menu(|actor, text| Stmt::Links { actor, text })?;
                p.expect(T::Newline)?;
            }
            T::Properties => {
                p.menu(|actor, text| Stmt::Properties { actor, text })?;
                p.expect(T::Newline)?;
            }
            T::Details => {
                p.menu(|actor, text| Stmt::Details { actor, text })?;
                p.expect(T::Newline)?;
            }
            T::Note => {
                p.note()?;
                p.expect(T::Newline)?;
            }
            T::Autonumber => p.autonumber()?,
            T::Activate | T::Deactivate => {
                p.i += 1;
                let who = p.actor()?;
                p.expect(T::Newline)?;
                let signal = if t == T::Activate {
                    line::ACTIVE_START
                } else {
                    line::ACTIVE_END
                };
                p.out.push(Stmt::On { actor: who, signal });
            }
            T::Title | T::LegacyTitle => {
                let text = p.take();
                let skip = if t == T::Title { 6 } else { 7 };
                let units: Vec<u16> = text.encode_utf16().collect();
                db.title = Some(String::from_utf16_lossy(&units[skip.min(units.len())..]));
            }
            T::AccTitle => {
                p.i += 1;
                let value = p.expect(T::AccTitleValue)?;
                db.set_acc_title(trim(&value));
            }
            T::AccDescr => {
                p.i += 1;
                let value = p.expect(T::AccDescrValue)?;
                db.set_acc_description(trim(&value));
            }
            T::AccDescrMultilineValue => {
                let value = p.take();
                db.set_acc_description(trim(&value));
            }
            T::Loop
            | T::Rect
            | T::Opt
            | T::Break
            | T::Alt
            | T::Par
            | T::ParOver
            | T::Critical
            | T::Box => {
                if stack.len() >= max_depth {
                    return Err(Error::new(
                        ErrorCode::Limit,
                        format!("Blocks are nested deeper than the limit of {max_depth}."),
                    ));
                }
                p.i += 1;
                let text = p.expect(T::RestOfLine)?;
                let signal = match t {
                    T::Loop => Some((K::Loop, line::LOOP_START)),
                    T::Rect => Some((K::Rect, line::RECT_START)),
                    T::Opt => Some((K::Opt, line::OPT_START)),
                    T::Break => Some((K::Break, line::BREAK_START)),
                    T::Alt => Some((K::Alt, line::ALT_START)),
                    T::Par => Some((K::Par, line::PAR_START)),
                    T::ParOver => Some((K::Par, line::PAR_OVER_START)),
                    T::Critical => Some((K::Critical, line::CRITICAL_START)),
                    _ => None,
                };
                let (k, start) = match signal {
                    Some((k, signal)) => (
                        k,
                        Stmt::Block {
                            signal,
                            text: Some(extract_wrap(&text)),
                        },
                    ),
                    None => (K::Box, Stmt::BoxStart(SeqDb::parse_box_data(&text))),
                };
                p.out.push(start);
                stack.push(Frame { kind: k });
                kind = k;
            }
            T::Else | T::And | T::Option => {
                let wanted = match t {
                    T::Else => K::Alt,
                    T::And => K::Par,
                    _ => K::Critical,
                };
                if kind != wanted {
                    return p.fail("'end'");
                }
                p.i += 1;
                let text = p.expect(T::RestOfLine)?;
                let signal = match t {
                    T::Else => line::ALT_ELSE,
                    T::And => line::PAR_AND,
                    _ => line::CRITICAL_OPTION,
                };
                p.out.push(Stmt::Block {
                    signal,
                    text: Some(extract_wrap(&text)),
                });
            }
            T::EndKw => {
                if kind == K::Top {
                    return p.fail("'NEWLINE', 'EOF'");
                }
                p.i += 1;
                if let Some(frame) = stack.pop() {
                    p.close(frame);
                }
                kind = stack.last().map_or(K::Top, |f| f.kind);
            }
            T::End => {
                if kind != K::Top {
                    return p.fail("'end'");
                }
                let statements = std::mem::take(&mut p.out);
                return db.apply(&statements);
            }
            _ => return p.fail("'NEWLINE', 'ACTOR', 'participant', 'note', 'end'"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagrams::sequence::db::MessageText;

    fn parse(src: &str) -> SeqDb {
        let mut db = SeqDb::new(None, false);
        parse_sequence(src, &mut db, 64).unwrap_or_else(|e| panic!("{src:?}: {}", e.message));
        db
    }

    #[test]
    fn parses_messages_and_participants() {
        let db = parse(
            "sequenceDiagram\nparticipant A as Alice\nactor B\nA->>+B: Hello\nB-->>-A: Hi\nNote over A,B: both\n",
        );
        let ids: Vec<&str> = db.actors.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["A", "B"]);
        assert_eq!(db.actors[0].1.description, "Alice");
        assert_eq!(db.actors[1].1.kind, "actor");
        let texts: Vec<_> = db
            .messages
            .iter()
            .map(|m| (m.kind, m.message.clone()))
            .collect();
        assert_eq!(
            texts[0],
            (Some(line::SOLID), MessageText::Text("Hello".to_string()))
        );
        assert_eq!(texts[1].0, Some(line::ACTIVE_START));
        assert_eq!(texts[3].0, Some(line::ACTIVE_END));
        assert_eq!(
            db.messages
                .iter()
                .filter(|m| m.kind == Some(line::NOTE))
                .count(),
            1
        );
    }

    #[test]
    fn parses_blocks() {
        let db = parse(
            "sequenceDiagram\nalt ok\nA->>B: x\nelse no\nA->>B: y\nend\nloop forever\nB->>A: z\nend\n",
        );
        let kinds: Vec<i32> = db.messages.iter().filter_map(|m| m.kind).collect();
        assert_eq!(
            kinds,
            [
                line::ALT_START,
                line::SOLID,
                line::ALT_ELSE,
                line::SOLID,
                line::ALT_END,
                line::LOOP_START,
                line::SOLID,
                line::LOOP_END
            ]
        );
    }

    #[test]
    fn reports_errors() {
        let mut db = SeqDb::new(None, false);
        let err = parse_sequence("sequenceDiagram\nA->>B\n", &mut db, 64).unwrap_err();
        assert_eq!(err.code, ErrorCode::Syntax);
        let mut db = SeqDb::new(None, false);
        let err =
            parse_sequence("sequenceDiagram\nA->>B: x\ndeactivate B\n", &mut db, 64).unwrap_err();
        assert_eq!(err.code, ErrorCode::Semantic);
        let deep = format!(
            "sequenceDiagram\n{}A->>B: x\n{}",
            "loop x\n".repeat(70),
            "end\n".repeat(70)
        );
        let mut db = SeqDb::new(None, false);
        assert_eq!(
            parse_sequence(&deep, &mut db, 64).unwrap_err().code,
            ErrorCode::Limit
        );
    }
}
