# Skipped kanban specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 kanban specs,
through `cargo xtask compat kanban --pele <DIR>`. `expected.rs` holds what pele makes of
each, written by `node tools/oracle/golden.mjs <DIR> kanban`.
`src/diagrams/kanban/compat.rs` checks that pele-rs's model of every case not listed here
matches pele's: the same columns and cards, with their levels, labels, icons, assignees,
tickets, priorities and classes, and the title, or an error of the same kind. pele passes
these specs and skips none.

## Skipped tests

## Spec files not ported
