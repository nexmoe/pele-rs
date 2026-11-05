# Skipped venn specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 venn specs,
through `cargo xtask compat venn --pele <DIR>`. `expected.rs` holds what the parser Mermaid
generates from its own grammar makes of each, written by
`node tools/oracle/golden.mjs <DIR> venn`: the reference parses the text with the grammar
Mermaid has and fills the model with pele's port of Mermaid's venn database.
`src/diagrams/venn/compat.rs` checks that pele-rs's model of every case not listed here
matches: the sets with their sizes and their labels, the texts, the styles, or an error of the
same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

- `vennRenderer > renders a title when provided`: Draws with Mermaid's renderer into a jsdom document and reads the elements back. The source is an SVG element and not a diagram.
- `vennRenderer > renders text nodes with custom color via style data`: Draws with Mermaid's renderer into a jsdom document and reads the elements back. The source is an SVG element and not a diagram.
- `vennRenderer > applies theme colors to circles`: Draws with Mermaid's renderer into a jsdom document and reads the elements back. The source is an SVG element and not a diagram.
- `vennRenderer > user override colors take priority over theme via style data`: Draws with Mermaid's renderer into a jsdom document and reads the elements back. The source is an SVG element and not a diagram.
- `vennRenderer > computes contrasting text color for dark backgrounds`: Draws with Mermaid's renderer into a jsdom document and reads the elements back. The source is an SVG element and not a diagram.
- `vennRenderer > renders debug layout helpers when enabled`: Draws with Mermaid's renderer into a jsdom document and reads the elements back. The source is an SVG element and not a diagram.
- `vennRenderer > ensurePairwiseSubsets > returns the same reference for empty array`: Tests the helper that adds the pairwise overlaps of a union of three or more sets. The source is empty and not a diagram.
- `vennRenderer > ensurePairwiseSubsets > returns the same reference when no 3+-set unions exist`: Tests the helper that adds the pairwise overlaps of a union of three or more sets. The source is a fragment of a diagram and not a diagram.
- `vennRenderer > ensurePairwiseSubsets > adds pairwise subsets for a 3-set union`: Tests the helper that adds the pairwise overlaps of a union of three or more sets. The source is a fragment of a diagram and not a diagram.
- `vennRenderer > ensurePairwiseSubsets > handles partial pairwise coverage: adds only missing pairs`: Tests the helper that adds the pairwise overlaps of a union of three or more sets. The source is a fragment of a diagram and not a diagram.
- `vennRenderer > ensurePairwiseSubsets > handles sets out of alphabetical order`: Tests the helper that adds the pairwise overlaps of a union of three or more sets. The source is a fragment of a diagram and not a diagram.
- `vennRenderer > ensurePairwiseSubsets > falls back to default size when individual set sizes are unknown`: Tests the helper that adds the pairwise overlaps of a union of three or more sets. The source is a fragment of a diagram and not a diagram.

## Spec files not ported

- `vennPalette.spec.ts`: Tests which colors each Mermaid theme gives the circles. Fermaid takes its circle colors from the `--pele-rs-series` tokens.
