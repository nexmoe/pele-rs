# Skipped git specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 git graph specs,
through `cargo xtask compat git --pele <DIR>`. `expected.rs` holds what pele makes of each,
written by `node tools/oracle/golden.mjs <DIR> git`.
`src/diagrams/git/compat.rs` checks that pele-rs's model of every case not listed here matches
pele's: the commits with their ids, their messages, their kinds and their tags, the branches
and the lanes they are drawn in, the accessible text, or an error of the same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

- `when parsing a gitGraph > when parsing more advanced gitGraphs > unsafe properties > should work with custom commit id or branch name ${prop}`: The spec builds the source from a value in the test code, so there is no model to compare.
- `Parsing Commit Statements > should ignore malformed properties and not break parsing`: Reads the partial tree that the error recovery of the parser leaves behind. Mermaid's parser refuses this input, and so does pele.
- `Parsing Merge Statements > should ignore malformed properties in merge statements`: Reads the partial tree that the error recovery of the parser leaves behind. Mermaid's parser refuses this input, and so does pele.
- `Parsing CherryPicking Statements > should parse cherry-picking with a commit id`: Reads the partial tree that the error recovery of the parser leaves behind. Mermaid's parser refuses this input, and so does pele.

## Spec files not ported

- `gitGraphParser.ts`: Its tests are written inside the source file, so there is no spec file to copy.
- `gitGraphRenderer.ts`: Its tests are written inside the source file and check the pixel positions and class names of Mermaid's own renderer.
