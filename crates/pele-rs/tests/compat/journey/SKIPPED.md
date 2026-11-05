# Skipped journey specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 journey specs,
through `cargo xtask compat journey --pele <DIR>`. `expected.rs` holds what pele makes of
each, written by `node tools/oracle/golden.mjs <DIR> journey`.
`src/diagrams/journey/compat.rs` checks that pele-rs's model of every case not listed here
matches pele's: the same sections, tasks with their scores, people and sections, title and
accessibility text, or an error of the same kind. pele passes these specs and skips none.

## Skipped tests

