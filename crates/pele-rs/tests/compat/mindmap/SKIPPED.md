# Skipped mindmap specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 mindmap specs,
through `cargo xtask compat mindmap --pele <DIR>`. `expected.rs` holds what pele makes of
each, written by `node tools/oracle/golden.mjs <DIR> mindmap`.
`src/diagrams/mindmap/compat.rs` checks that pele-rs's model of every case not listed here
matches pele's: the same nodes in source order, with their ids, levels, text, types, parents,
branches, classes and icons, and the title, or an error of the same kind.

## Skipped tests

- `MindmapDb getData function > getData > should assign correct section classes based on sibling position`: builds the model by calling `addNode`, not from diagram text, and checks Mermaid's `getData` layout data, which pele-rs does not have.
- `MindmapDb getData function > getData > should assign correct section classes to edges`: builds the model by calling `addNode`, not from diagram text, and checks Mermaid's `getData` layout data, which pele-rs does not have.
- `MindmapDb getData function > getData > should generate unique edge IDs`: builds the model by calling `addNode`, not from diagram text, and checks Mermaid's `getData` layout data, which pele-rs does not have.
- `MindmapDb getData function > getData > should handle nodes with missing optional properties`: builds the model by calling `addNode`, not from diagram text, and checks Mermaid's `getData` layout data, which pele-rs does not have.
- `MindmapDb getData function > getData > should not create any fake root nodes`: builds the model by calling `addNode`, not from diagram text, and checks Mermaid's `getData` layout data, which pele-rs does not have.
- `MindmapDb getData function > getData > should preserve custom classes while adding section classes`: builds the model by calling `addNode`, not from diagram text, and checks Mermaid's `getData` layout data, which pele-rs does not have.
- `MindmapDb getData function > getData > should preserve node properties in processed data`: builds the model by calling `addNode`, not from diagram text, and checks Mermaid's `getData` layout data, which pele-rs does not have.
- `MindmapDb getData function > getData > should return empty data when no mindmap is set`: builds the model by calling `addNode`, not from diagram text, and checks Mermaid's `getData` layout data, which pele-rs does not have.
- `MindmapDb getData function > getData > should return structured data for hierarchical mindmap`: builds the model by calling `addNode`, not from diagram text, and checks Mermaid's `getData` layout data, which pele-rs does not have.
- `MindmapDb getData function > getData > should return structured data for simple mindmap`: builds the model by calling `addNode`, not from diagram text, and checks Mermaid's `getData` layout data, which pele-rs does not have.
- `MindmapDb getData function > getData > should wrap section numbers when there are more than 11 level 2 nodes`: builds the model by calling `addNode`, not from diagram text, and checks Mermaid's `getData` layout data, which pele-rs does not have.

## Spec files not ported

- `mindmapLayout.spec.ts`: Tests which of Mermaid's layout engines (cose-bilkent, ELK, dagre) the config selects. pele-rs has one mindmap layout.
