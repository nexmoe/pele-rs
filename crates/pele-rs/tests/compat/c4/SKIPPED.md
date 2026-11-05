# Skipped C4 specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 C4 specs,
through `cargo xtask compat c4 --pele <DIR>`. `expected.rs` holds what the parser Mermaid
generates from its own grammar makes of each, written by
`node tools/oracle/golden.mjs <DIR> c4`.
`src/diagrams/c4/compat.rs` checks that pele-rs's model of every case not listed here
matches: the shapes with their kinds and their labels, the boundaries that hold them, the
relations between them, or an error of the same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

- `should parse a C4 diagram with one Boundary correctly`: Mermaid builds the source from a value in the test code, so it holds a placeholder rather than a diagram.
- `should parse the alias`: Mermaid builds the source from a value in the test code, so it holds a placeholder rather than a diagram.
- `should parse the label`: Mermaid builds the source from a value in the test code, so it holds a placeholder rather than a diagram.
- `should parse the type`: Mermaid builds the source from a value in the test code, so it holds a placeholder rather than a diagram.
- `should parse a link`: Mermaid builds the source from a value in the test code, so it holds a placeholder rather than a diagram.
- `should parse tags`: Mermaid builds the source from a value in the test code, so it holds a placeholder rather than a diagram.
- `should resolve a boundary alias as a relation endpoint`: Mermaid builds the source from a value in the test code, so it holds a placeholder rather than a diagram.
- `should parse a C4 diagram with one Component correctly`: Mermaid builds the source from a value in the test code, so it holds a placeholder rather than a diagram.
- `should parse a C4 diagram with one Container correctly`: Mermaid builds the source from a value in the test code, so it holds a placeholder rather than a diagram.
- `should parse the technology`: Mermaid builds the source from a value in the test code, so it holds a placeholder rather than a diagram.
- `should parse the description`: Mermaid builds the source from a value in the test code, so it holds a placeholder rather than a diagram.
- `should parse a sprite`: Mermaid builds the source from a value in the test code, so it holds a placeholder rather than a diagram.
- `should parse a C4 diagram with one System correctly`: Mermaid builds the source from a value in the test code, so it holds a placeholder rather than a diagram.

## Spec files not ported

None.
