//! The interface each diagram type implements, and the lookup from a detected type to the
//! implementation compiled in through its Cargo feature.

use crate::error::Error;
use crate::preprocess::Config;
use crate::text::TextMeasurer;
use crate::{DiagramKind, Link, RenderOptions};

/// What a diagram type is given to render.
#[cfg_attr(
    not(any(
        feature = "pie",
        feature = "flowchart",
        feature = "sequence",
        feature = "state",
        feature = "class",
        feature = "er",
        feature = "timeline",
        feature = "kanban",
        feature = "mindmap",
        feature = "gantt",
        feature = "sankey"
    )),
    allow(
        dead_code,
        reason = "read by the diagram types, none of which is compiled in"
    )
)]
pub(crate) struct Input<'a> {
    /// The text after preprocessing and entity encoding. It ends with a line break, as
    /// Mermaid's parsers expect.
    pub source: String,
    pub config: &'a Config,
    /// The title from front matter.
    pub title: Option<&'a str>,
    /// The number of front matter lines removed before `source`, for error positions.
    #[allow(
        dead_code,
        reason = "read by the types that keep comments, none of which is written yet"
    )]
    pub line_offset: usize,
    pub options: &'a RenderOptions,
    pub measurer: &'a dyn TextMeasurer,
}

pub(crate) struct Output {
    pub svg: String,
    pub width: f64,
    pub height: f64,
    pub links: Vec<Link>,
}

pub(crate) trait Diagram: Sync {
    /// Whether the parser is given the text with its `%%` comment lines kept, so that the
    /// positions it reports match the source.
    fn keep_comments(&self) -> bool {
        false
    }

    /// Whether `$$…$$` in the labels of the type is typeset, when the host has a typesetter.
    fn math(&self) -> bool {
        false
    }

    fn parse(&self, input: &Input<'_>) -> Result<Box<dyn Parsed>, Error>;
}

/// The implementation of a diagram type, when its feature is enabled.
pub(crate) fn lookup(kind: DiagramKind) -> Option<&'static dyn Diagram> {
    match kind {
        #[cfg(feature = "flowchart")]
        DiagramKind::Flowchart => Some(&crate::diagrams::flowchart::Flowchart),
        #[cfg(feature = "pie")]
        DiagramKind::Pie => Some(&crate::diagrams::pie::Pie),
        #[cfg(feature = "sequence")]
        DiagramKind::Sequence => Some(&crate::diagrams::sequence::Sequence),
        #[cfg(feature = "state")]
        DiagramKind::State => Some(&crate::diagrams::state::State),
        #[cfg(feature = "class")]
        DiagramKind::Class => Some(&crate::diagrams::class::Class),
        #[cfg(feature = "er")]
        DiagramKind::Er => Some(&crate::diagrams::er::Er),
        #[cfg(feature = "requirement")]
        DiagramKind::Requirement => Some(&crate::diagrams::requirement::Requirement),
        #[cfg(feature = "timeline")]
        DiagramKind::Timeline => Some(&crate::diagrams::timeline::Timeline),
        #[cfg(feature = "journey")]
        DiagramKind::Journey => Some(&crate::diagrams::journey::Journey),
        #[cfg(feature = "kanban")]
        DiagramKind::Kanban => Some(&crate::diagrams::kanban::Kanban),
        #[cfg(feature = "mindmap")]
        DiagramKind::Mindmap => Some(&crate::diagrams::mindmap::Mindmap),
        #[cfg(feature = "gantt")]
        DiagramKind::Gantt => Some(&crate::diagrams::gantt::Gantt),
        #[cfg(feature = "sankey")]
        DiagramKind::Sankey => Some(&crate::diagrams::sankey::Sankey),
        #[cfg(feature = "xychart")]
        DiagramKind::XyChart => Some(&crate::diagrams::xychart::XyChart),
        #[cfg(feature = "quadrantchart")]
        DiagramKind::QuadrantChart => Some(&crate::diagrams::quadrantchart::QuadrantChart),
        #[cfg(feature = "git")]
        DiagramKind::GitGraph => Some(&crate::diagrams::git::GitGraph),
        #[cfg(feature = "block")]
        DiagramKind::Block => Some(&crate::diagrams::block::Block),
        #[cfg(feature = "agentflow")]
        DiagramKind::Agentflow => Some(&crate::diagrams::agentflow::Agentflow),
        #[cfg(feature = "swimlane")]
        DiagramKind::Swimlane => Some(&crate::diagrams::swimlane::Swimlane),
        #[cfg(feature = "railroad")]
        DiagramKind::Railroad => Some(&crate::diagrams::railroad::Railroad),
        #[cfg(feature = "wardley")]
        DiagramKind::Wardley => Some(&crate::diagrams::wardley::Wardley),
        #[cfg(feature = "eventmodeling")]
        DiagramKind::EventModeling => Some(&crate::diagrams::eventmodeling::EventModeling),
        #[cfg(feature = "ishikawa")]
        DiagramKind::Ishikawa => Some(&crate::diagrams::ishikawa::Ishikawa),
        #[cfg(feature = "cynefin")]
        DiagramKind::Cynefin => Some(&crate::diagrams::cynefin::Cynefin),
        #[cfg(feature = "treeview")]
        DiagramKind::TreeView => Some(&crate::diagrams::treeview::TreeView),
        #[cfg(feature = "venn")]
        DiagramKind::Venn => Some(&crate::diagrams::venn::Venn),
        #[cfg(feature = "info")]
        DiagramKind::Info => Some(&crate::diagrams::info::Info),
        #[cfg(feature = "usecase")]
        DiagramKind::Usecase => Some(&crate::diagrams::usecase::Usecase),
        #[cfg(feature = "c4")]
        DiagramKind::C4 => Some(&crate::diagrams::c4::C4),
        #[cfg(feature = "treemap")]
        DiagramKind::Treemap => Some(&crate::diagrams::treemap::Treemap),
        #[cfg(feature = "radar")]
        DiagramKind::Radar => Some(&crate::diagrams::radar::Radar),
        #[cfg(feature = "packet")]
        DiagramKind::Packet => Some(&crate::diagrams::packet::Packet),
        #[cfg(feature = "architecture")]
        DiagramKind::Architecture => Some(&crate::diagrams::architecture::Architecture),
        // Every type is compiled in when all the features are on, so the arm is out of reach
        // then; it is what answers for a type whose feature is off.
        #[allow(
            unreachable_patterns,
            reason = "a type without its feature is not drawn"
        )]
        _ => None,
    }
}

/// A parsed model that owns its data and can be reused on background workers.
pub(crate) trait Parsed: Send + Sync {
    fn render(&self, input: &Input<'_>) -> Result<Output, Error>;
}

impl<F> Parsed for F
where
    F: for<'a> Fn(&Input<'a>) -> Result<Output, Error> + Send + Sync,
{
    fn render(&self, input: &Input<'_>) -> Result<Output, Error> {
        self(input)
    }
}
