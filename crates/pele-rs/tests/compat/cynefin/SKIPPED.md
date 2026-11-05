# Skipped cynefin specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 cynefin specs,
through `cargo xtask compat cynefin --pele <DIR>`. `expected.rs` holds what Mermaid makes of
each, written by `node tools/oracle/golden.mjs <DIR> cynefin`: the reference parses the text
with Mermaid's own Langium parser and fills the model with pele's port of Mermaid's cynefin
database. `src/diagrams/cynefin/compat.rs` checks that pele-rs's model of every case not
listed here matches: the domains with their items, the transitions with their labels, the
accessible text, or an error of the same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

- `Cynefin Database > should start empty`: The source is empty and not a diagram.
- `Cynefin Database > should set domains from AST blocks`: The source is the name of a domain and not a diagram.
- `Cynefin Database > should set transitions`: The source is the name of a domain and not a diagram.
- `Cynefin Database > should set transitions without labels`: The source is the name of a domain and not a diagram.
- `Cynefin Database > should filter out self-loop transitions`: The source is the name of a domain and not a diagram.
- `Cynefin Database > should handle a list of only self-loops gracefully`: The source is the name of a domain and not a diagram.
- `Cynefin Database > should handle null/undefined blocks`: The source is empty and not a diagram.
- `Cynefin Database > should clear all data`: The source is the name of a domain and not a diagram.
- `Cynefin Database > should handle multiple domains`: The source is the name of a domain and not a diagram.
- `Cynefin Database > should handle domain with multiple items`: The source is the name of a domain and not a diagram.
- `Cynefin Database > should return config`: The source is the word "object" and not a diagram.
- `Cynefin Boundaries > seededRandom should return deterministic values`: The source is empty and not a diagram.
- `Cynefin Boundaries > seededRandom should return values between 0 and 1`: The source is empty and not a diagram.
- `Cynefin Boundaries > hashString should return consistent hashes`: The source is the word "cynefin" and not a diagram.
- `Cynefin Boundaries > hashString should return different hashes for different strings`: The source is the name of a domain and not a diagram.
- `Cynefin Boundaries > generateFoldPath should return valid SVG path`: The source is a letter and not a diagram.
- `Cynefin Boundaries > generateFoldPath should be deterministic`: The source is empty and not a diagram.
- `Cynefin Boundaries > generateHorizontalBoundary should return valid SVG path`: The source is a letter and not a diagram.
- `Cynefin Boundaries > generateCliffPath should return valid SVG path`: The source is a letter and not a diagram.
- `Cynefin Boundaries > generateConfusionPath should return valid ellipse path`: The source is a letter and not a diagram.
- `Cynefin Boundaries > generateConfusionPath should use provided center and radii`: The source is a number and not a diagram.
- `resolveSeed > returns the configured seed when it is a non-zero number`: The source is the name of an element and not a diagram.
- `resolveSeed > falls back to hashString(id) when the configured seed is 0`: The source is the name of an element and not a diagram.
- `resolveSeed > falls back to hashString(id) when the configured seed is undefined`: The source is the name of an element and not a diagram.
- `resolveSeed > ignores the id when an explicit seed is configured`: The source is a letter and not a diagram.
- `resolveSeed > produces different fallback seeds for different ids`: The source is the name of an element and not a diagram.
- `resolveSeed > treats NaN / Infinity as unconfigured and falls back to hashString(id)`: The source is the name of an element and not a diagram.

## Spec files not ported

None.
