//! Parts shared by several diagram types.

#[cfg(any(feature = "xychart", feature = "quadrantchart", feature = "block"))]
pub(crate) mod chars;
#[cfg(any(
    feature = "architecture",
    feature = "block",
    feature = "class",
    feature = "er",
    feature = "flowchart",
    feature = "gantt",
    feature = "git",
    feature = "ishikawa",
    feature = "journey",
    feature = "kanban",
    feature = "mindmap",
    feature = "packet",
    feature = "pie",
    feature = "quadrantchart",
    feature = "radar",
    feature = "requirement",
    feature = "sankey",
    feature = "sequence",
    feature = "state",
    feature = "swimlane",
    feature = "timeline",
    feature = "treemap",
    feature = "usecase",
    feature = "xychart",
    feature = "railroad",
    feature = "c4",
    feature = "wardley",
    feature = "agentflow"
))]
pub(crate) mod fit;
#[cfg(any(feature = "class", feature = "er"))]
pub(crate) mod generics;
// The helpers of the golden tests are only used by the types that have a compat module.
#[cfg(all(
    test,
    any(
        feature = "class",
        feature = "er",
        feature = "flowchart",
        feature = "gantt",
        feature = "git",
        feature = "journey",
        feature = "kanban",
        feature = "mindmap",
        feature = "pie",
        feature = "quadrantchart",
        feature = "radar",
        feature = "requirement",
        feature = "sankey",
        feature = "sequence",
        feature = "state",
        feature = "swimlane",
        feature = "timeline",
        feature = "treemap",
        feature = "xychart",
        feature = "block",
        feature = "architecture",
        feature = "ishikawa",
        feature = "railroad",
        feature = "c4",
        feature = "wardley",
        feature = "agentflow",
        feature = "usecase",
    )
))]
#[allow(dead_code, reason = "each type uses only part of the golden helpers")]
pub(crate) mod golden;
#[cfg(any(feature = "sequence", feature = "ishikawa"))]
pub(crate) mod literal;
#[cfg(any(
    feature = "state",
    feature = "class",
    feature = "er",
    feature = "requirement",
    feature = "sankey",
    feature = "quadrantchart",
    feature = "git",
    feature = "block",
    feature = "treemap",
    feature = "architecture",
    feature = "wardley"
))]
pub(crate) mod ordered;
#[cfg(any(feature = "kanban", feature = "mindmap"))]
pub(crate) mod outline;
#[cfg(any(feature = "timeline", feature = "journey", feature = "git"))]
pub(crate) mod plain;
#[cfg(any(
    feature = "timeline",
    feature = "journey",
    feature = "kanban",
    feature = "mindmap",
    feature = "gantt"
))]
#[cfg_attr(
    not(any(feature = "timeline", feature = "journey")),
    allow(dead_code, reason = "outlines and gantt charts use only part of it")
)]
pub(crate) mod scan;
#[cfg(any(
    feature = "pie",
    feature = "git",
    feature = "radar",
    feature = "treemap",
    feature = "architecture",
    feature = "railroad",
    feature = "wardley"
))]
#[cfg_attr(
    not(all(
        feature = "pie",
        feature = "git",
        feature = "radar",
        feature = "treemap",
        feature = "architecture"
    )),
    allow(dead_code, reason = "each type uses only part of the tokenizer")
)]
pub(crate) mod tokens;
