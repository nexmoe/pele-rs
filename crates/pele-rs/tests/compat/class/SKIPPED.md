# Skipped class specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 class specs, through
`cargo xtask compat class --pele <DIR>`. `expected.rs` holds what pele makes of each, written by
`node tools/oracle/golden.mjs <DIR> class`. `src/diagrams/class/compat.rs` checks that
pele-rs's model of every case not listed here matches pele's: the same classes, members,
relations, notes, namespaces, interfaces, classDefs, the nodes and edges that are drawn (with
and without hierarchical namespaces), and accessibility text, or an error of the same kind.
Many class specs test members on their own, so each source is also read as a method and as an
attribute. pele passes these specs, except for the ones its own `SKIPPED.md` lists, which pin
Mermaid's DOM ids and Jison internals.

## Skipped tests

None.
