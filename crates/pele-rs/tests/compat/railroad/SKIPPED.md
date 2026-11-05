# Skipped railroad specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 railroad specs,
through `cargo xtask compat railroad --pele <DIR>`. `expected.rs` holds what Mermaid's own
parser makes of each, written by `node tools/oracle/golden.mjs <DIR> railroad`.
`src/diagrams/railroad/compat.rs` checks that pele-rs's model of every case not listed here
matches Mermaid's: the rules with their names and their definitions, the accessible text, or
an error of the same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

There are none: every case the specs build has a model to compare, or is refused by both
sides.

## Spec files not ported

- `railroadRenderer.spec.ts`: Tests Mermaid's DOM renderer: element transforms and path
  coordinates from a stubbed getBBox. pele-rs has its own layout, which is pele's.
- `styles.spec.ts`: Tests the stylesheet text Mermaid generates from theme variables.
  pele-rs writes no stylesheet; the colours come from CSS custom properties.
