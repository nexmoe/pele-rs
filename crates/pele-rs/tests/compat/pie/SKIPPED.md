# Skipped pie specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 pie specs, through
`cargo xtask compat pie --pele <DIR>`. `src/diagrams/pie/compat.rs` checks each case that is not
listed here.

## Skipped tests

- `pie > config > setConfig`: A todo in Mermaid. It tests Mermaid's pie database.
- `pie > config > getConfig`: Tests the defaults of Mermaid's pie database, which pele-rs does not have. pele-rs's defaults (text position 0.75, legend on the right, no donut hole) are covered by the render tests.
- `pie > config > resetConfig`: A todo in Mermaid. It tests Mermaid's pie database.

## Spec files not ported

- `pieRenderer.spec.ts`: Tests Mermaid's d3 renderer. pele does not port it either.
