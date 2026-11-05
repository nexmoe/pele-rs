use crate::util::{is_space, is_word, trim_start};

/// A diagram type, named as Mermaid's documentation names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DiagramKind {
    Flowchart,
    Sequence,
    Class,
    State,
    Er,
    Pie,
    Gantt,
    GitGraph,
    Mindmap,
    Timeline,
    Journey,
    XyChart,
    QuadrantChart,
    Requirement,
    C4,
    Sankey,
    Block,
    Packet,
    Kanban,
    Architecture,
    Radar,
    Treemap,
    Info,
    Agentflow,
    Usecase,
    Railroad,
    TreeView,
    Venn,
    Cynefin,
    Wardley,
    Swimlane,
    Ishikawa,
    EventModeling,
}

impl DiagramKind {
    /// The type's name as used in class names and Mermaid config sections, such as `gitGraph`.
    pub fn as_str(self) -> &'static str {
        use DiagramKind::*;
        match self {
            Flowchart => "flowchart",
            Sequence => "sequence",
            Class => "class",
            State => "state",
            Er => "er",
            Pie => "pie",
            Gantt => "gantt",
            GitGraph => "gitGraph",
            Mindmap => "mindmap",
            Timeline => "timeline",
            Journey => "journey",
            XyChart => "xychart",
            QuadrantChart => "quadrantChart",
            Requirement => "requirement",
            C4 => "c4",
            Sankey => "sankey",
            Block => "block",
            Packet => "packet",
            Kanban => "kanban",
            Architecture => "architecture",
            Radar => "radar",
            Treemap => "treemap",
            Info => "info",
            Agentflow => "agentflow",
            Usecase => "usecase",
            Railroad => "railroad",
            TreeView => "treeView",
            Venn => "venn",
            Cynefin => "cynefin",
            Wardley => "wardley",
            Swimlane => "swimlane",
            Ishikawa => "ishikawa",
            EventModeling => "eventmodeling",
        }
    }
}

/// How a detector matches the text after its leading whitespace.
enum Rule {
    /// Starts with one of the keywords.
    Prefix(&'static [&'static str]),
    /// Starts with the keyword, ignoring ASCII case.
    PrefixNoCase(&'static str),
    /// Starts with the keyword, which is not followed by a word character.
    Word(&'static str),
    /// Like `Word`, ignoring ASCII case.
    WordNoCase(&'static str),
    /// Starts with the keyword, followed by whitespace, one of the given characters, or the end.
    Then(&'static str, &'static [char]),
}

use DiagramKind as K;

// Mermaid's detectors, in the order Mermaid registers them. The first match wins.
const DETECTORS: &[(DiagramKind, Rule)] = &[
    (K::Flowchart, Rule::Prefix(&["flowchart-elk"])),
    (K::Mindmap, Rule::Prefix(&["mindmap"])),
    (K::Architecture, Rule::Prefix(&["architecture"])),
    (K::Agentflow, Rule::Word("agentflow-beta")),
    (
        K::C4,
        Rule::Prefix(&[
            "C4Context",
            "C4Container",
            "C4Component",
            "C4Dynamic",
            "C4Deployment",
        ]),
    ),
    (K::Kanban, Rule::Prefix(&["kanban"])),
    (K::Class, Rule::Prefix(&["classDiagram"])),
    (K::Er, Rule::Prefix(&["erDiagram"])),
    (K::Gantt, Rule::Prefix(&["gantt"])),
    (K::Info, Rule::Prefix(&["info"])),
    (K::Pie, Rule::Prefix(&["pie"])),
    (K::Requirement, Rule::Prefix(&["requirement"])),
    (K::Sequence, Rule::Prefix(&["sequenceDiagram"])),
    (K::Swimlane, Rule::Word("swimlane-beta")),
    (K::Flowchart, Rule::Prefix(&["graph", "flowchart"])),
    (K::Timeline, Rule::Prefix(&["timeline"])),
    (K::GitGraph, Rule::Prefix(&["gitGraph"])),
    (K::State, Rule::Prefix(&["stateDiagram"])),
    (K::Journey, Rule::Prefix(&["journey"])),
    (K::QuadrantChart, Rule::Prefix(&["quadrantChart"])),
    (K::Sankey, Rule::Prefix(&["sankey"])),
    (K::Packet, Rule::Prefix(&["packet"])),
    (K::XyChart, Rule::Prefix(&["xychart"])),
    (K::Block, Rule::Prefix(&["block"])),
    (K::EventModeling, Rule::Prefix(&["eventmodeling"])),
    (K::TreeView, Rule::Prefix(&["treeView-beta"])),
    (K::Radar, Rule::Prefix(&["radar-beta"])),
    (K::Ishikawa, Rule::WordNoCase("ishikawa")),
    (K::Treemap, Rule::Prefix(&["treemap"])),
    (K::Railroad, Rule::PrefixNoCase("railroad-beta")),
    (K::Railroad, Rule::PrefixNoCase("railroad-ebnf-beta")),
    (K::Railroad, Rule::PrefixNoCase("railroad-abnf-beta")),
    (K::Railroad, Rule::PrefixNoCase("railroad-peg-beta")),
    (K::Venn, Rule::Prefix(&["venn-beta"])),
    (K::Wardley, Rule::PrefixNoCase("wardley-beta")),
    (K::Cynefin, Rule::Then("cynefin-beta", &[':'])),
    (K::Usecase, Rule::Then("usecase-beta", &[])),
];

fn strip_prefix_no_case<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let head = text.get(..keyword.len())?;
    head.eq_ignore_ascii_case(keyword)
        .then(|| &text[keyword.len()..])
}

fn ends_word(rest: &str) -> bool {
    !rest.starts_with(is_word)
}

impl Rule {
    fn matches(&self, text: &str) -> bool {
        match *self {
            Rule::Prefix(keywords) => keywords.iter().any(|k| text.starts_with(k)),
            Rule::PrefixNoCase(keyword) => strip_prefix_no_case(text, keyword).is_some(),
            Rule::Word(keyword) => text.strip_prefix(keyword).is_some_and(ends_word),
            Rule::WordNoCase(keyword) => strip_prefix_no_case(text, keyword).is_some_and(ends_word),
            Rule::Then(keyword, extra) => text.strip_prefix(keyword).is_some_and(|rest| {
                rest.chars()
                    .next()
                    .is_none_or(|c| is_space(c) || extra.contains(&c))
            }),
        }
    }
}

/// Detects the diagram type of text that has been through preprocessing.
pub(crate) fn detect(cleaned: &str) -> Option<DiagramKind> {
    let text = trim_start(cleaned);
    DETECTORS
        .iter()
        .find(|(_, rule)| rule.matches(text))
        .map(|&(kind, _)| kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_in_mermaid_order() {
        assert_eq!(detect("  graph TD"), Some(K::Flowchart));
        assert_eq!(detect("flowchart-elk LR"), Some(K::Flowchart));
        assert_eq!(detect("\n\npie title x"), Some(K::Pie));
        assert_eq!(detect("infoX"), Some(K::Info));
        assert_eq!(detect("requirementDiagram"), Some(K::Requirement));
        assert_eq!(detect("sankey-beta"), Some(K::Sankey));
        assert_eq!(detect("hello"), None);
        assert_eq!(detect(""), None);
    }

    #[test]
    fn checks_word_boundaries_and_case() {
        assert_eq!(detect("agentflow-beta"), Some(K::Agentflow));
        assert_eq!(detect("agentflow-betax"), None);
        assert_eq!(detect("ISHIKAWA-beta"), Some(K::Ishikawa));
        assert_eq!(detect("ishikawax"), None);
        assert_eq!(detect("Railroad-PEG-beta"), Some(K::Railroad));
        assert_eq!(detect("cynefin-beta: x"), Some(K::Cynefin));
        assert_eq!(detect("cynefin-beta"), Some(K::Cynefin));
        assert_eq!(detect("cynefin-betax"), None);
        assert_eq!(detect("usecase-beta:"), None);
    }
}
