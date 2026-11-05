//! The parser of a cynefin diagram. It follows the grammar Mermaid gives the type with Langium:
//!
//! ```text
//! entry Cynefin:
//!     NEWLINE*
//!     ('cynefin-beta' | 'cynefin-beta:')
//!     NEWLINE*
//!     ( TitleAndAccessibilities | domains+=DomainBlock | transitions+=Transition | NEWLINE )*;
//!
//! DomainBlock: domain=DOMAIN_NAME NEWLINE* (items+=DomainItem NEWLINE*)*;
//! DomainItem: label=STRING;
//! Transition: from=DOMAIN_NAME '-->' to=DOMAIN_NAME (':' label=STRING)? EOL;
//! ```
//!
//! Mermaid's parser is hand-written, and pele's port of it reads the same sources the same way;
//! where the two differ, this follows Mermaid.

use super::db::Domain;
use super::lexer::{self, Kind, Reader};
use crate::error::Error;

/// A block of items in a domain.
pub(crate) struct DomainBlock {
    pub domain: Domain,
    pub items: Vec<String>,
}

/// A transition between two domains.
pub(crate) struct Transition {
    pub from: Domain,
    pub to: Domain,
    pub label: Option<String>,
}

/// What the parser reads out of a source.
pub(crate) struct Parsed {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub domains: Vec<DomainBlock>,
    pub transitions: Vec<Transition>,
}

/// Reads a cynefin diagram.
pub(crate) fn parse_cynefin(src: &str) -> Result<Parsed, Error> {
    let tokens = lexer::tokenize(src)?;
    let mut r = Reader::new(tokens);
    while r.accept(Kind::Newline) {}
    if !r.accept(Kind::CynefinBeta) && !r.accept(Kind::CynefinBetaColon) {
        return Err(r.fail("the keyword 'cynefin-beta'"));
    }
    while r.accept(Kind::Newline) {}
    let mut parsed = Parsed {
        title: None,
        acc_title: None,
        acc_descr: None,
        domains: Vec::new(),
        transitions: Vec::new(),
    };
    loop {
        match r.kind() {
            Kind::End => break,
            Kind::Newline => {
                r.take();
            }
            Kind::Title | Kind::AccTitle | Kind::AccDescr => {
                // TitleAndAccessibilities: ((accDescr | accTitle | title) EOL)+
                loop {
                    match r.kind() {
                        Kind::Title => {
                            let text = r.take();
                            parsed.title = Some(lexer::title_value(text));
                        }
                        Kind::AccTitle => {
                            let text = r.take();
                            parsed.acc_title = Some(lexer::acc_title_value(text));
                        }
                        Kind::AccDescr => {
                            let text = r.take();
                            parsed.acc_descr = Some(lexer::acc_descr_value(text));
                        }
                        _ => break,
                    }
                    // EOL: NEWLINE+ | EOF
                    if r.kind() != Kind::End && !r.accept(Kind::Newline) {
                        return Err(r.fail("the end of the line"));
                    }
                }
            }
            Kind::DomainName => {
                // The two rules that begin with a domain name are told apart by what follows
                // it: an arrow makes a transition, anything else a block of items.
                if r.peek(1) == Kind::Arrow {
                    parsed.transitions.push(transition(&mut r)?);
                } else {
                    parsed.domains.push(domain_block(&mut r)?);
                }
            }
            _ => {
                return Err(r.fail("a domain, a transition or the end of the input"));
            }
        }
    }
    Ok(parsed)
}

/// `domain=DOMAIN_NAME NEWLINE* (items+=DomainItem NEWLINE*)*`
fn domain_block(r: &mut Reader<'_>) -> Result<DomainBlock, Error> {
    let domain = domain_of(r.take());
    while r.accept(Kind::Newline) {}
    let mut items = Vec::new();
    while r.kind() == Kind::Str {
        let text = r.take();
        items.push(lexer::string_value(text));
        while r.accept(Kind::Newline) {}
    }
    Ok(DomainBlock { domain, items })
}

/// `from=DOMAIN_NAME '-->' to=DOMAIN_NAME (':' label=STRING)? EOL`
fn transition(r: &mut Reader<'_>) -> Result<Transition, Error> {
    let from = domain_of(r.take());
    r.expect(Kind::Arrow)?;
    let to = domain_of(r.expect(Kind::DomainName)?);
    let mut label = None;
    if r.accept(Kind::Colon) {
        let text = r.expect(Kind::Str)?;
        label = Some(lexer::string_value(text));
    }
    // EOL: NEWLINE+ | EOF
    if r.kind() != Kind::End && !r.accept(Kind::Newline) {
        return Err(r.fail("the end of the line"));
    }
    Ok(Transition { from, to, label })
}

/// The domain a name stands for. The lexer reads no other name.
fn domain_of(name: &str) -> Domain {
    Domain::from_str(name).expect("the lexer reads only the names of the domains")
}
