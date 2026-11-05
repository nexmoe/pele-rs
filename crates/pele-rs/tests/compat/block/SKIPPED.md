# Skipped block specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 block specs,
through `cargo xtask compat block --pele <DIR>`. `expected.rs` holds what the parser Mermaid
generates from its own grammar makes of each, written by
`node tools/oracle/golden.mjs <DIR> block`.
`src/diagrams/block/compat.rs` checks that pele-rs's model of every case not listed here
matches: the blocks with their ids, their kinds and their labels, the links between them, the
classes, or an error of the same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

- `Block diagram > when parsing a block diagram graph it should handle >  > sanitizes labels with the current dompurify config`: Mermaid builds the label from a value in the test code, so the source is the word "b" and not a diagram.
- `Block diagram > prototype properties > should work with a __proto__ property`: The source is the word "__proto__" and not a diagram.
- `Block diagram > prototype properties > should work with a constructor property`: The source is the word "constructor" and not a diagram.
- `block colour slots > emits one composite rule per palette entry under a colour theme`: Tests the CSS stylesheet Mermaid generates for themes; the source is a CSS selector.
- `block colour slots > leaves the simple shapes to the flat theme colour`: Tests the CSS stylesheet Mermaid generates for themes; the source is a CSS rule.
- `block colour slots > emits nothing for a theme that carries no palette`: Tests the CSS stylesheet Mermaid generates for themes; the source is the word "default".
- `block colour slots > rejects a look that would break out of the selector`: Tests the CSS stylesheet Mermaid generates for themes; the source is a CSS selector.
- `block db edge styles > should correctly identify edge thickness`: The source is the link "-->" and not a diagram.
- `block db edge styles > should correctly identify edge patterns`: The source is the link "-->" and not a diagram.
- `block db edge styles > should correctly identify start and end arrows`: The source is the link "<-->" and not a diagram.
- `block db runtime config > should call getConfig at sanitization time, not at module load time`: Checks when Mermaid reads its global config for DOMPurify. The source is the word "getConfig".
- `Layout > should calculate position correctly`: The source is empty and not a diagram.
- `layout runtime config > should read block.padding from config at call time, not at import time`: The source is the word "root" and not a diagram.

## Spec files not ported

None.
