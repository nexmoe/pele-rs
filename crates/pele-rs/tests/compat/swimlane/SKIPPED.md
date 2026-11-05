# Skipped swimlane specs

The cases in `cases.rs` are Mermaid's own swimlane source and the examples of Mermaid's
documentation, which the corpus of `tests/compat/swimlane` collects. Mermaid's own swimlane specs
test the layout engine and the theme, not the syntax: the syntax is the flowchart's, and Mermaid's
flowchart specs cover it, so there is nothing of the swimlane's own to skip.

`expected.rs` holds what Mermaid's own flowchart grammar makes of each case, written by
`node tools/oracle/golden.mjs <DIR> swimlane`. `src/diagrams/swimlane/compat.rs` checks that
pele-rs's model of every case matches it: the same nodes, edges, subgraphs, classes, tooltips and
accessibility text, or an error of the same kind.

The cases are run with the default limits, so the specs that set `maxEdges` or `inheritDir` are
checked with Mermaid's defaults; pele-rs's own tests cover those settings.

## Skipped tests

## Spec files

- `swimlanes/swimlanesDiagram.spec.ts`: Mermaid's own swimlane specs test which layout engine is
  picked and what the theme's CSS says, not the syntax. The source they draw is a case here.
- `swimlanes/lanePalette.spec.ts`: Mermaid's own lane palette specs test the theme's CSS, which
  pele-rs does not generate.
