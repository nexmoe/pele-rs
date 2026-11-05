# Skipped info specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 info specs,
through `cargo xtask compat info --pele <DIR>`. `expected.rs` holds what Mermaid makes of each,
written by `node tools/oracle/golden.mjs <DIR> info`: the reference parses the text with
Mermaid's own Langium parser, the one of
`packages/parser/src/language/info/info.langium`.
`src/diagrams/info/compat.rs` checks that pele-rs's model of every case not listed here
matches: the title, the accessible text, or an error of the same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

None.

## Spec files not ported

None.
