# Skipped quadrantChart specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 quadrantChart specs,
through `cargo xtask compat quadrantchart --pele <DIR>`. `expected.rs` holds what pele makes
of each, written by `node tools/oracle/golden.mjs <DIR> quadrantchart`.
`src/diagrams/quadrantchart/compat.rs` checks that pele-rs's model of every case not listed
here matches pele's: the four boxes, the four axis labels, the points with their place and
their style, the classes, and the accessible text, or an error of the same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

- `quadrant db runtime config > should call getConfig at sanitization time, not at module load time`: checks when Mermaid reads its global config to sanitize text with DOMPurify. Pele has no global config and escapes text when writing SVG.

## Spec files not ported

None.
