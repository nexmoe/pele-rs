# Skipped tree view specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 tree view specs,
through `cargo xtask compat treeview --pele <DIR>`. `expected.rs` holds what Mermaid's own
parser makes of each, written by `node tools/oracle/golden.mjs <DIR> treeview`: the reference
reads the text with the Langium grammar Mermaid has, turns the box-drawing characters into an
indentation the way Mermaid does, and fills the model with pele's port of Mermaid's tree view
database.
`src/diagrams/treeview/compat.rs` checks that pele-rs's model of every case not listed here
matches: the nodes with their depth, their names, their kinds, their classes, their icons and
their descriptions, the accessible text, or an error of the same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

- `icons > treeViewIcons pack > every icon has a non-empty body that inherits color via currentColor`: The spec builds the source from a value in the test code, so there is no model to compare.

## Spec files not ported

- None: Mermaid 12.1.0 has no spec file of its own for the drawing of a tree view.
