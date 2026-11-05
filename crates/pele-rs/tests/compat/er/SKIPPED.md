# Skipped er specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 ER specs, through
`cargo xtask compat er --pele <DIR>`. `expected.rs` holds what pele makes of each, written by
`node tools/oracle/golden.mjs <DIR> er`. `src/diagrams/er/compat.rs` checks that pele-rs's
model of every case not listed here matches pele's: the same entities with their attributes,
relationships, classDefs, subgraphs, the nodes and edges that are drawn, and accessibility
text, or an error of the same kind. The specs of `erDb.spec.js` call the model directly rather
than parse a diagram, so their sources are not diagrams and both sides reject them. pele passes
these specs; its own `SKIPPED.md` lists only `erRenderer.spec.ts`, which tests ids of Mermaid's
legacy d3 renderer.

## Skipped tests

- `when parsing ER diagram it... > entity name > has non A-Za-z0-9_- chars > ${singleOccurrence} fails if not surrounded by quotes`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `when parsing ER diagram it... > entity name > has non A-Za-z0-9_- chars > "${singleOccurrence}" single occurrence`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `when parsing ER diagram it... > entity name > has non A-Za-z0-9_- chars > "${repeatedOccurrence}" repeated occurrence`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `when parsing ER diagram it... > entity name > has non A-Za-z0-9_- chars > "${singleOccurrence}" ends with`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `when parsing ER diagram it... > entity name > has non A-Za-z0-9_- chars > "${cannotStartWith}" cannot start with the character`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `when parsing ER diagram it... > accessible title and description > should allow for a accessibility title and description (accDescr)`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `when parsing ER diagram it... > accessible title and description > parses a multi line description (accDescr)`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
