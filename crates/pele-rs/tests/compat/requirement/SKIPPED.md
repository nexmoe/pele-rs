# Skipped requirement specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 requirement specs,
through `cargo xtask compat requirement --pele <DIR>`. `expected.rs` holds what pele makes of
each, written by `node tools/oracle/golden.mjs <DIR> requirement`.
`src/diagrams/requirement/compat.rs` checks that pele-rs's model of every case not listed here
matches pele's: the same requirements, elements, relationships and classes, the nodes and edges
that are drawn, and accessibility text, or an error of the same kind. The specs of
`requirementDb.spec.ts` call the model directly, and those of `requirementDiagram.spec.js`
build their diagrams from arrays of lines, so the extractor's first string is rarely a whole
diagram and both sides reject most cases. The renderer is compared with pele's on whole
diagrams instead, and the parser with Mermaid's jison grammar by `tools/oracle/diff.mjs`.

## Skipped tests

- `when parsing requirement diagram it... > will accept ${property} as requirement id`: The source is built from values in the test code, which the extractor does not evaluate.
- `when parsing requirement diagram it... > will accept ${property} as element id`: The source is built from values in the test code, which the extractor does not evaluate.
