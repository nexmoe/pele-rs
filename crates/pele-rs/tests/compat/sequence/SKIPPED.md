# Skipped sequence specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 sequence specs, through
`cargo xtask compat sequence --pele <DIR>`. `expected.rs` holds what pele makes of each, written
by `node tools/oracle/golden.mjs <DIR> sequence`. `src/diagrams/sequence/compat.rs` checks that
pele-rs's model of every case not listed here matches pele's: the same participants, boxes,
messages, created and destroyed participants, title and accessibility text, or an error of the
same kind. pele passes these specs, except for the ones its own `SKIPPED.md` lists, which test
Mermaid's renderer bounds and pixel positions, Jison internals, or a page element.

The cases are parsed as pele's spec adapter parses them: the text as given, with entities
encoded, and the default configuration.

The cases named `... [0]`, `[1]` and so on come from `it.each` tables. For the
`keywordActors.spec.ts` and the `%s` endpoint cases, the table holds only the participant name
the test builds its source from, so those cases check that pele-rs rejects the bare name as pele
does; the names themselves are covered by the cases around them.

## Skipped tests

None.
