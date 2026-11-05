# Skipped Ishikawa specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 Ishikawa specs,
through `cargo xtask compat ishikawa --pele <DIR>`. `expected.rs` holds what the parser
Mermaid generates from its own grammar makes of each, written by
`node tools/oracle/golden.mjs <DIR> ishikawa`.
`src/diagrams/ishikawa/compat.rs` checks that pele-rs's model of every case not listed here
matches: the effect and the causes under it, or an error of the same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

None.

## Spec files not ported

None.
