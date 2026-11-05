# Skipped xychart specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 xychart specs,
through `cargo xtask compat xychart --pele <DIR>`. `expected.rs` holds what pele makes of
each, written by `node tools/oracle/golden.mjs <DIR> xychart`.
`src/diagrams/xychart/compat.rs` checks that pele-rs's model of every case not listed here
matches pele's: the axes with their titles and their range, the series with their values and
their point labels, the orientation and the accessible text, or an error of the same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests


## Spec files not ported

- `chartBuilder/textDimensionCalculator.spec.ts`: Tests Mermaid's DOM text measurement.
- `chartBuilder/components/chartTitle.spec.ts`: Tests the title component of Mermaid's chart layout.
- `chartBuilder/components/legend.spec.ts`: Tests the legend component of Mermaid's chart layout.
