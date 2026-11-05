# Skipped flowchart specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 flowchart specs, through
`cargo xtask compat flowchart --pele <DIR>`. `expected.rs` holds what pele makes of each, written
by `node tools/oracle/golden.mjs <DIR> flowchart`. `src/diagrams/flowchart/compat.rs` checks
that pele-rs's model of every case not listed here matches pele's: the same nodes, edges,
subgraphs, classes, tooltips and accessibility text, or an error of the same kind. pele passes
these specs, except for the two its own `SKIPPED.md` lists, which concern how Mermaid stores
`<` and Jison internals.

The cases are run with the default limits, so the specs that set `maxEdges` or `inheritDir`
are checked with Mermaid's defaults; pele-rs's own tests cover those settings.

## Skipped tests

- `[Edges] when parsing > edge length > should handle dotted edges with arrows with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle dotted edges with double arrows with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle dotted edges with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle dotted labelled edges with arrows with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle dotted labelled edges with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle normal edges with arrows with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle normal edges with double arrows with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle normal edges with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle normal labelled edges with arrows with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle normal labelled edges with double arrows with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle normal labelled edges with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle thick edges with arrows with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle thick edges with double arrows with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle thick edges with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle thick labelled edges with arrows with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle thick labelled edges with double arrows with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edge length > should handle thick labelled edges with length ${length}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edges > should handle ${edgeType.stroke} ${edgeType.type} with no text`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edges > should handle ${edgeType.stroke} ${edgeType.type} with text`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edges with ids > double ended edges with ids and labels > should handle ${edgeType.stroke} ${edgeType.type} with  text`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edges with ids > open ended edges with ids and labels > should handle ${edgeType.stroke} ${edgeType.type} with no text`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `[Edges] when parsing > edges with ids > open ended edges with ids and labels > should handle ${edgeType.stroke} ${edgeType.type} with text`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `parsing a flow chart > should work with class id ${unsafeProp}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `parsing a flow chart > should work with node id ${unsafeProp}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `parsing a flow chart > should work with subgraph id ${unsafeProp}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.
- `parsing a flow chart > should work with tooltip id ${unsafeProp}`: The source is built from values in the test code, which the extractor does not evaluate. The same syntax is covered by the cases around it.

## Spec files not ported

- `flowChartShapes.spec.js`: Tests Mermaid's legacy dagre-d3 shape renderer.
- `flowDiagram.spec.ts`: Tests Mermaid's diagram registration and config plumbing.
- `flowRenderer-elk-default.spec.ts`: Tests Mermaid's choice between its dagre and ELK layout engines.
- `flowRenderer-v3-unified.spec.ts`: Tests Mermaid's DOM renderer wiring.
- `flowDb-elk-duplicate-subgraph.spec.ts`: Runs ELK layout.
