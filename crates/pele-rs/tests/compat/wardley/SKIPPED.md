# Skipped wardley specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 wardley specs,
through `cargo xtask compat wardley --pele <DIR>`. `expected.rs` holds what the reference makes
of each, written by `node tools/oracle/golden.mjs <DIR> wardley`. The reference is Mermaid's own
Langium grammar with pele's port of Mermaid's wardley database.
`src/diagrams/wardley/compat.rs` checks that pele-rs's model of every case not listed here
matches: the nodes with their places and their shapes, the links, the trends, the pipelines, the
notes, the annotations and the forces, the axes, the accessible text, or an error of the same
kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

- `WardleyBuilder.resolveNodeId > returns the id when a node exists with that exact id`: the source of the case is the word "API", which is not a diagram. Both sides reject it.
- `WardleyBuilder.resolveNodeId > falls back to label match for pipeline-style synthetic ids`: the source of the case is the name of a node, not a diagram. Both sides reject it.
- `WardleyBuilder.resolveNodeId > returns the input unchanged when no id or label matches`: the source of the case is the word "API", which is not a diagram. Both sides reject it.
- `WardleyBuilder.resolveNodeId > prefers exact id over label match when both exist`: the source of the case is the name of a node, not a diagram. Both sides reject it.

## Spec files not ported

None.
