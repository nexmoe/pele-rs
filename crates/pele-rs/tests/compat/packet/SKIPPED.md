# Skipped packet specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 packet specs,
through `cargo xtask compat packet --pele <DIR>`. `expected.rs` holds what the reference makes
of each, written by `node tools/oracle/golden.mjs <DIR> packet`. The reference is Mermaid's own
Langium grammar with pele's port of Mermaid's packet database.
`src/diagrams/packet/compat.rs` checks that pele-rs's model of every case not listed here
matches: the rows of fields with the bits each field takes, the accessible text, or an error of
the same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

None.

## Spec files not ported

- `renderer.spec.ts`: Reads positions back from the DOM that Mermaid's renderer builds.
