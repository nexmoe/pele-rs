# Skipped state specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 state specs, through
`cargo xtask compat state --pele <DIR>`. `expected.rs` holds what pele makes of each, written by
`node tools/oracle/golden.mjs <DIR> state`. `src/diagrams/state/compat.rs` checks that
pele-rs's model of every case not listed here matches pele's: the same states, relations,
classes, links, the nodes and edges that are drawn, and accessibility text, or an error of the
same kind. pele passes these specs, except for the ones its own `SKIPPED.md` lists, which
concern DOMPurify's serialization and Jison internals.

## Skipped tests

None.
