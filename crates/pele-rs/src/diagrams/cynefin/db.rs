//! The model of a cynefin diagram: the domains with the items in them, and the transitions
//! between them. It follows Mermaid's cynefin database, which keeps the domains in a map and
//! the transitions in a list.

/// The five domains of the Cynefin framework.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Domain {
    Complex,
    Complicated,
    Clear,
    Chaotic,
    Confusion,
}

impl Domain {
    /// The name of the domain, as the grammar writes it.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Domain::Complex => "complex",
            Domain::Complicated => "complicated",
            Domain::Clear => "clear",
            Domain::Chaotic => "chaotic",
            Domain::Confusion => "confusion",
        }
    }

    /// The domain a name stands for.
    pub(crate) fn from_str(name: &str) -> Option<Domain> {
        match name {
            "complex" => Some(Domain::Complex),
            "complicated" => Some(Domain::Complicated),
            "clear" => Some(Domain::Clear),
            "chaotic" => Some(Domain::Chaotic),
            "confusion" => Some(Domain::Confusion),
            _ => None,
        }
    }

    /// The place the domain stands in, among the five. The drawing uses it to find the block
    /// of a domain, and the table of the names of the domains follows the same order.
    pub(crate) fn slot(self) -> usize {
        match self {
            Domain::Complex => 0,
            Domain::Complicated => 1,
            Domain::Chaotic => 2,
            Domain::Clear => 3,
            Domain::Confusion => 4,
        }
    }
}

/// A domain and the items in it.
pub(crate) struct CynefinDomain {
    pub items: Vec<String>,
}

/// A transition between two domains.
pub(crate) struct Transition {
    pub from: Domain,
    pub to: Domain,
    pub label: Option<String>,
}

/// The model of a cynefin diagram.
pub(crate) struct CynefinModel {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// The domains, in the order they were read. A domain that is named twice is one domain,
    /// and it keeps the place it first had.
    pub domains: Vec<(Domain, CynefinDomain)>,
    pub transitions: Vec<Transition>,
}

impl CynefinModel {
    /// The items of a domain, or none when the diagram has no such domain.
    pub(crate) fn items(&self, domain: Domain) -> &[String] {
        self.domains
            .iter()
            .find(|(name, _)| *name == domain)
            .map_or(&[], |(_, found)| found.items.as_slice())
    }
}

/// Fills the model from what the parser read.
pub(crate) fn build_cynefin(parsed: &super::parser::Parsed, title: Option<&str>) -> CynefinModel {
    let mut domains: Vec<(Domain, CynefinDomain)> = Vec::new();
    for block in &parsed.domains {
        let found = CynefinDomain {
            items: block.items.clone(),
        };
        match domains.iter_mut().find(|(name, _)| *name == block.domain) {
            Some(slot) => slot.1 = found,
            None => domains.push((block.domain, found)),
        }
    }
    // Mermaid drops a transition that starts and ends in the same domain, and takes no label
    // for a transition that has none.
    let transitions = parsed
        .transitions
        .iter()
        .filter(|t| t.from != t.to)
        .map(|t| Transition {
            from: t.from,
            to: t.to,
            label: t.label.clone().filter(|label| !label.is_empty()),
        })
        .collect();
    CynefinModel {
        // The title of the diagram is the one the source names, or the one the front matter
        // names when the source has none.
        title: parsed
            .title
            .clone()
            .filter(|title| !title.is_empty())
            .or_else(|| title.map(str::to_string)),
        acc_title: parsed.acc_title.clone().filter(|title| !title.is_empty()),
        acc_descr: parsed.acc_descr.clone().filter(|descr| !descr.is_empty()),
        domains,
        transitions,
    }
}
