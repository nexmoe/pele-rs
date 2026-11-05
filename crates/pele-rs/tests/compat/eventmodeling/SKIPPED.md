# Skipped event modeling specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 event modeling specs,
through `cargo xtask compat eventmodeling --pele <DIR>`. `expected.rs` holds what the reference
makes of each, written by `node tools/oracle/golden.mjs <DIR> eventmodeling`: the reference
parses the source with the grammar Mermaid generates from its own and fills the model with
pele's port of Mermaid's event modeling database.
`src/diagrams/eventmodeling/compat.rs` checks that pele-rs's model of every case not listed
here matches the reference's: the frames with their numbers, their kinds, their entities and
the data they carry, the lanes they are drawn in, the relations between them, the notes and
the specifications, or an error of the same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

- `Parse Event Model > Validation > registerValidationChecks > registers checks against the validation registry`: The source is empty and not a diagram.
- `Parse Event Model > Validation > should allow evt sourced from cmd`: The source is the word "evt" and not a diagram.
- `Parse Event Model > Validation > should allow event sourced from command`: The source is the word "event" and not a diagram.
- `Parse Event Model > Validation > should reject evt sourced from rmo`: The source is the word "evt" and not a diagram.
- `Parse Event Model > Validation > should reject evt sourced from pcr`: The source is the word "evt" and not a diagram.
- `Parse Event Model > Validation > should allow cmd sourced from ui`: The source is the word "cmd" and not a diagram.
- `Parse Event Model > Validation > should allow command sourced from ui`: The source is the word "command" and not a diagram.
- `Parse Event Model > Validation > should allow cmd sourced from pcr`: The source is the word "cmd" and not a diagram.
- `Parse Event Model > Validation > should allow command sourced from processor`: The source is the word "command" and not a diagram.
- `Parse Event Model > Validation > should reject cmd sourced from cmd`: The source is the word "cmd" and not a diagram.
- `Parse Event Model > Validation > should reject cmd sourced from evt`: The source is the word "cmd" and not a diagram.
- `Parse Event Model > Validation > should reject ui sourced from evt`: The source is the word "ui" and not a diagram.
- `Parse Event Model > Validation > should allow ui sourced from rmo`: The source is the word "ui" and not a diagram.
- `Parse Event Model > Validation > should allow ui sourced from readmodel`: The source is the word "ui" and not a diagram.
- `Parse Event Model > Validation > should reject ui sourced from cmd`: The source is the word "ui" and not a diagram.
- `Parse Event Model > Validation > should reject cmd sourced from rmo`: The source is the word "cmd" and not a diagram.
- `Parse Event Model > Validation > should allow rmo sourced from evt`: The source is the word "rmo" and not a diagram.
- `Parse Event Model > Validation > should allow readmodel sourced from event`: The source is the word "readmodel" and not a diagram.
- `Parse Event Model > Validation > should allow pcr sourced from rmo`: The source is the word "pcr" and not a diagram.
- `Parse Event Model > Validation > should allow processor sourced from readmodel`: The source is the word "processor" and not a diagram.
- `Parse Event Model > Validation > should reject rmo sourced directly from pcr`: The source is the word "rmo" and not a diagram.
- `Parse Event Model > Validation > should reject pcr sourced directly from evt`: The source is the word "pcr" and not a diagram.
- `Parse Event Model > Validation > should reject pcr sourced from cmd`: The source is the word "pcr" and not a diagram.
- `Parse Event Model > Validation > should reject rmo sourced from ui`: The source is the word "rmo" and not a diagram.
- `Parse Event Model > Validation > should report an error for each invalid source`: The source is the word "pcr" and not a diagram.
- `Parse Event Model > Validation > should not validate frames with no sources`: The source is the word "rmo" and not a diagram.
## Spec files not ported

- `eventmodeling.spec.ts`: Its tests are written inside the source file, so there is no spec
  file to copy; the cases of `cases.rs` come from pele's port of them.
