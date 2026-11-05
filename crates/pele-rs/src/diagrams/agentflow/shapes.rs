//! The shapes of the agentflow syntax: the v0.8.1 aliases, the shapes that v0.8.1 removed,
//! and the shapes it allows. Ported from Mermaid's `diagrams/agentflow/shapes.ts`.

/// The v0.8.1 names of the domain shapes, and the Mermaid shape each stands for.
const SHAPE_ALIASES: &[(&str, &str)] = &[
    ("task", "roundedRect"),
    ("tool", "subroutine"),
    ("input", "lean-right"),
    ("decision", "diamond"),
    ("refdoc", "lin-doc"),
    ("action", "hexagon"),
    ("round", "rect"),
];

/// The shapes v0.8.1 removed. Authoring one is a hard error.
const REMOVED_SHAPES: &[&str] = &[
    "doc",
    "stadium",
    "terminal",
    "circle",
    "trapezoid",
    "inv_trapezoid",
    "inv-trapezoid",
    "doublecircle",
    "double-circle",
    "typeDeclaration",
    "procs",
    "lean_left",
    "lean-left",
    "in-out",
    "cylinder",
    "ellipse",
    "odd",
    // Instance shapes: the instancing mechanism was removed entirely.
    "tag-rect",
    "tagged-rectangle",
    "delay",
    "half-rounded-rectangle",
    "lin-rect",
    "lined-rectangle",
    "win-pane",
    "window-pane",
    "curv-trap",
    "curved-trapezoid",
];

/// The shapes v0.8.1 allows, as canonical Mermaid shape ids after alias resolution.
/// `rect` and `squareRect` are normalised to the default before this set is consulted.
const ALLOWED_SHAPES: &[&str] = &[
    "roundedRect",
    "subroutine",
    "subprocess",
    "subproc",
    "framed-rectangle",
    "lean-right",
    "diamond",
    "lin-doc",
    "lined-document",
    "hexagon",
    "hex",
    "connector",
    // The shape of a collapsed flow container.
    "collapsedGroup",
];

/// The shape a node has when the one it asked for is not one agentflow has.
pub(crate) const DEFAULT_SHAPE: &str = "roundedRect";

/// The Mermaid shape a name stands for. A name that is no alias stands for itself.
pub(crate) fn resolve_shape_alias(shape: &str) -> String {
    SHAPE_ALIASES
        .iter()
        .find(|(from, _)| *from == shape)
        .map(|(_, to)| (*to).to_string())
        .unwrap_or_else(|| shape.to_string())
}

/// Whether a name is a shape Mermaid knows.
pub(crate) fn is_valid_shape(name: &str) -> bool {
    // Mermaid checks the name against its whole shape map; these are the names it holds.
    const KNOWN: &[&str] = &[
        "rect",
        "proc",
        "process",
        "rectangle",
        "squareRect",
        "rounded",
        "event",
        "roundedRect",
        "stadium",
        "terminal",
        "pill",
        "fr-rect",
        "subprocess",
        "subproc",
        "framed-rectangle",
        "subroutine",
        "cyl",
        "db",
        "database",
        "cylinder",
        "datastore",
        "data-store",
        "folder",
        "directory",
        "bucket",
        "console",
        "browser",
        "person",
        "circle",
        "circ",
        "bang",
        "cloud",
        "diam",
        "decision",
        "diamond",
        "question",
        "hex",
        "hexagon",
        "prepare",
        "lean-r",
        "lean-right",
        "in-out",
        "lean_right",
        "lean-l",
        "lean-left",
        "out-in",
        "lean_left",
        "trap-b",
        "priority",
        "trapezoid-bottom",
        "trapezoid",
        "trap-t",
        "manual",
        "trapezoid-top",
        "inv-trapezoid",
        "inv_trapezoid",
        "dbl-circ",
        "double-circle",
        "doublecircle",
        "text",
        "notch-rect",
        "card",
        "notched-rectangle",
        "lin-rect",
        "lined-rectangle",
        "lined-process",
        "lin-proc",
        "shaded-process",
        "sm-circ",
        "start",
        "small-circle",
        "stateStart",
        "fr-circ",
        "stop",
        "framed-circle",
        "stateEnd",
        "fork",
        "join",
        "forkJoin",
        "hourglass",
        "collate",
        "brace",
        "comment",
        "brace-l",
        "brace-r",
        "braces",
        "bolt",
        "com-link",
        "lightning-bolt",
        "doc",
        "document",
        "delay",
        "half-rounded-rectangle",
        "h-cyl",
        "das",
        "horizontal-cylinder",
        "lin-cyl",
        "disk",
        "lined-cylinder",
        "curv-trap",
        "curved-trapezoid",
        "display",
        "div-rect",
        "div-proc",
        "divided-rectangle",
        "divided-process",
        "tri",
        "extract",
        "triangle",
        "win-pane",
        "internal-storage",
        "window-pane",
        "f-circ",
        "junction",
        "filled-circle",
        "notch-pent",
        "loop-limit",
        "notched-pentagon",
        "flip-tri",
        "manual-file",
        "flipped-triangle",
        "sl-rect",
        "manual-input",
        "sloped-rectangle",
        "docs",
        "documents",
        "st-doc",
        "stacked-document",
        "st-rect",
        "procs",
        "processes",
        "stacked-rectangle",
        "bow-rect",
        "stored-data",
        "bow-tie-rectangle",
        "cross-circ",
        "summary",
        "crossed-circle",
        "tag-doc",
        "tagged-document",
        "tag-rect",
        "tagged-rectangle",
        "tag-proc",
        "tagged-process",
        "flag",
        "paper-tape",
        "odd",
        "rect_left_inv_arrow",
        "lin-doc",
        "lined-document",
        // Shapes Mermaid accepts by name without documenting them.
        "state",
        "choice",
        "note",
        "composite",
        "rectWithTitle",
        "labelRect",
        "block_arrow",
        "collapsedGroup",
        "iconSquare",
        "iconCircle",
        "icon",
        "iconRounded",
        "imageSquare",
        "anchor",
        "kanbanItem",
        "mindmapCircle",
        "defaultMindmapNode",
        "classBox",
        "erBox",
        "requirementBox",
        "usecaseActor",
        "usecaseActorHollow",
        "usecaseActorAwesome",
        "usecaseActorIcon",
        "usecaseBusiness",
        "usecaseEllipse",
        "usecaseJsonTable",
    ];
    KNOWN.contains(&name)
}

/// Whether a shape v0.8.1 removed.
pub(crate) fn is_removed_shape(shape: &str) -> bool {
    REMOVED_SHAPES.contains(&shape)
}

/// Whether a shape v0.8.1 allows.
pub(crate) fn is_allowed_shape(shape: &str) -> bool {
    ALLOWED_SHAPES.contains(&shape)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_the_agentflow_aliases() {
        assert_eq!(resolve_shape_alias("task"), "roundedRect");
        assert_eq!(resolve_shape_alias("tool"), "subroutine");
        assert_eq!(resolve_shape_alias("input"), "lean-right");
        assert_eq!(resolve_shape_alias("decision"), "diamond");
        assert_eq!(resolve_shape_alias("refdoc"), "lin-doc");
        assert_eq!(resolve_shape_alias("action"), "hexagon");
        assert_eq!(resolve_shape_alias("round"), "rect");
        assert_eq!(resolve_shape_alias("square"), "square");
    }

    #[test]
    fn knows_the_shapes_mermaid_knows() {
        assert!(is_valid_shape("roundedRect"));
        assert!(is_valid_shape("bang"));
        assert!(!is_valid_shape("no-such-shape"));
    }
}
