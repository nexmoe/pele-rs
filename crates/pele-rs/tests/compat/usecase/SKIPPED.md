# Skipped use case specs

Mermaid's use case diagrams have no grammar file: its parser is written with Chevrotain, in a
lexer, a parser and a visitor. The cases in `cases.rs` come from pele's port of Mermaid's own
specs, through `cargo xtask compat usecase --pele <DIR>`. `expected.rs` holds what Mermaid
makes of each, written by `node tools/oracle/golden.mjs <DIR> usecase`: the reference parses the
text with pele's port of Mermaid's parser and fills the model with pele's port of Mermaid's use
case database. `src/diagrams/usecase/compat.rs` checks that pele-rs's model of every case not
listed here matches: the actors, the use cases, the boundaries, the notes, the JSON tables, the
relationships with their arrows and labels, the classes and the styles, or an error of the same
kind.

Every case is checked, so nothing is skipped.

## Skipped tests

## Spec files not ported

- `tests/compat/usecase-parser` in the pele checkout holds the specs of Mermaid's lexer, its
  parser and its JSON reader on their own. They are the same cases as the ones in
  `tests/compat/usecase`, so they are not ported twice.
