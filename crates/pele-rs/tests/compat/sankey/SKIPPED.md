# Skipped sankey specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 sankey specs,
through `cargo xtask compat sankey --pele <DIR>`. `expected.rs` holds what pele makes of
each, written by `node tools/oracle/golden.mjs <DIR> sankey`.
`src/diagrams/sankey/compat.rs` checks that pele-rs's model of every case not listed here
matches pele's: the nodes in the order they were named, the bands between them with their
values, and the title, or an error of the same kind.

Mermaid's sankey specs are few, and three of the five read a CSV file or a value from the
test code, so the differential run against Mermaid's own grammar carries most of the weight
here.

## Skipped tests

- `Sankey diagram > when parsing an info graph it > parses csv with sankey-beta syntax`: reads the CSV file `upstream/energy.csv`, so its extracted text is a path, not a diagram.
- `Sankey diagram > when parsing an info graph it > parses csv with sankey syntax`: reads the CSV file `upstream/energy.csv`, so its extracted text is a path, not a diagram.
- `Sankey diagram > when parsing an info graph it > parses the issue #7528 sample with ${syntax} syntax`: builds the diagram from a value in the test code, and runs it once per syntax.

## Spec files not ported

None.
