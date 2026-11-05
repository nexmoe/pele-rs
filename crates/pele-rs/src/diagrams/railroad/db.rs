//! The model of a railroad diagram: the rules and the expressions they are made of.
//!
//! Mermaid has four grammars for this diagram, but they all end in the same model: a rule is
//! a name and a tree of nodes, which the drawing reads. The tree is the same in all four, so
//! that one drawing serves them all.

/// Which of Mermaid's four grammars the text was written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Notation {
    /// `railroad-beta`, Mermaid's own notation.
    Railroad,
    /// `railroad-ebnf-beta`, EBNF in the W3C and ISO 14977 styles.
    Ebnf,
    /// `railroad-abnf-beta`, ABNF as in RFC 5234.
    Abnf,
    /// `railroad-peg-beta`, parsing expression grammars.
    Peg,
}

/// A node of a rule's definition.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Node {
    /// A piece of text the rule matches.
    Terminal(String),
    /// The name of another rule.
    NonTerminal(String),
    /// Anything else, such as a special sequence.
    Special(String),
    /// Elements that follow one another.
    Sequence(Vec<Node>),
    /// Alternatives, one of which is taken.
    Choice(Vec<Node>),
    /// An element that may be left out.
    Optional(Box<Node>),
    /// An element that is taken between `min` and `max` times. The upper bound is `None` when
    /// the element may be taken as often as wanted.
    Repetition {
        element: Box<Node>,
        min: u64,
        max: Option<u64>,
    },
}

/// One rule: a name and what it matches.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Rule {
    pub name: String,
    pub definition: Node,
}

/// The whole diagram.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RailroadModel {
    pub notation: Notation,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// Every rule in the order it was written. A name written twice is drawn twice.
    pub rules: Vec<Rule>,
}

impl Node {
    /// The nodes below this one, in the order they are drawn.
    pub(crate) fn parts(&self) -> Vec<&Node> {
        match self {
            Node::Sequence(elements) => elements.iter().collect(),
            Node::Choice(alternatives) => alternatives.iter().collect(),
            Node::Optional(element) | Node::Repetition { element, .. } => vec![element.as_ref()],
            Node::Terminal(_) | Node::NonTerminal(_) | Node::Special(_) => Vec::new(),
        }
    }

    /// A sequence of one element is that element, as in every notation of Mermaid.
    pub(crate) fn sequence(elements: Vec<Node>) -> Node {
        if elements.len() == 1 {
            elements.into_iter().next().expect("one element")
        } else {
            Node::Sequence(elements)
        }
    }

    /// A choice of one alternative is that alternative, as in every notation of Mermaid.
    pub(crate) fn choice(alternatives: Vec<Node>) -> Node {
        if alternatives.len() == 1 {
            alternatives.into_iter().next().expect("one alternative")
        } else {
            Node::Choice(alternatives)
        }
    }

    /// An element that is taken between `min` and `max` times.
    pub(crate) fn repetition(element: Node, min: u64, max: Option<u64>) -> Node {
        Node::Repetition {
            element: Box::new(element),
            min,
            max,
        }
    }
}
