# Skipped treemap specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 treemap specs,
through `cargo xtask compat treemap --pele <DIR>`. `expected.rs` holds what Mermaid makes of
each, written by `node tools/oracle/golden.mjs <DIR> treemap`.
`src/diagrams/treemap/compat.rs` checks that pele-rs's model of every case not listed here
matches Mermaid's: the items with their names, their kinds, their values, the classes they name
and the styles of those classes, the boxes they are drawn in, the accessible text, or an error of
the same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

- `Treemap Parser > ClassDef and Class Statements > should parse a class assignment statement`: Mermaid's grammar has no rule for `class`, so Mermaid's own parser refuses this source, and so does pele-rs.
- `Treemap Parser > ClassDef and Class Statements > should parse a class assignment statement with semicolon`: Mermaid's grammar has no rule for `class`, so Mermaid's own parser refuses this source, and so does pele-rs.
- `treemap utilities > buildHierarchy > should convert a flat array into a hierarchical structure`: The source is a name, not a diagram: the spec builds the tree from values in the test code, so there is no model to compare.
- `treemap utilities > buildHierarchy > should handle empty input`: The source is empty, not a diagram: the spec builds the tree from values in the test code, so there is no model to compare.
- `treemap utilities > buildHierarchy > should handle only root nodes`: The source is a name, not a diagram: the spec builds the tree from values in the test code, so there is no model to compare.
- `treemap utilities > buildHierarchy > should handle complex nesting levels`: The source is a name, not a diagram: the spec builds the tree from values in the test code, so there is no model to compare.

## Spec files not ported

None.
