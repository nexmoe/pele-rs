# Skipped radar specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 radar specs,
through `cargo xtask compat radar --pele <DIR>`. `expected.rs` holds what Mermaid makes of each,
written by `node tools/oracle/golden.mjs <DIR> radar`: the reference parses the text with
Mermaid's own Langium parser and fills the model with pele's port of Mermaid's radar database.
`src/diagrams/radar/compat.rs` checks that pele-rs's model of every case not listed here
matches: the axes with their labels, the curves with their values, the options, the accessible
text, or an error of the same kind.

Every case Mermaid's parser accepts is accepted here as well, and every case it refuses is
refused here as well.

## Skipped tests

- `radar diagrams > should limit the number of ticks to MAX_TICKS`: The spec builds the source from a value in the test code, so there is no model to compare.
- `radar diagrams > renderer > relativeRadius > should calculate relative radius`: The source is empty and not a diagram.
- `radar diagrams > renderer > relativeRadius > should handle min value`: The source is empty and not a diagram.
- `radar diagrams > renderer > relativeRadius > should handle max value`: The source is empty and not a diagram.
- `radar diagrams > renderer > relativeRadius > should clip values below min`: The source is empty and not a diagram.
- `radar diagrams > renderer > relativeRadius > should clip values above max`: The source is empty and not a diagram.
- `radar diagrams > renderer > relativeRadius > should handle negative min`: The source is empty and not a diagram.
- `radar diagrams > renderer > closedRoundCurve > should construct a polygon if tension is 0`: The source is a path of a curve and not a diagram.
- `radar diagrams > renderer > closedRoundCurve > should construct a simple round curve`: The source is a path of a curve and not a diagram.
- `radar diagrams > renderer > closedRoundCurve > should construct a closed round curve`: The source is a path of a curve and not a diagram.
- `radar > should handle axis > should handle one axis [0]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle axis > should handle one axis [1]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle axis > should handle multiple axes [0]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle axis > should handle multiple axes [1]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle axis > should handle multiple axes [2]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle axis > should handle multiple axes [3]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle axis > should handle axis labels [0]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle axis > should handle axis labels [1]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle curves > should handle one curve with one entry [0]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle curves > should handle one curve with one entry [1]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle curves > should handle one curve with one entry [2]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle curves > should handle one curve with one detailed entry [0]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle curves > should handle one curve with one detailed entry [1]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle curves > should handle one curve with one detailed entry [2]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle curves > should handle one curve with multiple detailed entries [0]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle curves > should handle one curve with multiple detailed entries [1]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle curves > should handle one curve with multiple detailed entries [2]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle curves > should handle multiple curves [0]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle curves > should handle multiple curves [1]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle curves > should handle multiple curves [2]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle options > should handle number option %s [0]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle options > should handle number option %s [1]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle options > should handle number option %s [2]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle options > should handle string option %s [0]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle options > should handle string option %s [1]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle options > should handle boolean option %s [0]`: The source is a fragment of a diagram and not a diagram.
- `radar > should handle options > should handle boolean option %s [1]`: The source is a fragment of a diagram and not a diagram.
- `radar > error messages with line and column numbers > should include line and column numbers in parser errors for radar diagrams`: Checks the wording of Langium's MermaidParseError.
- `radar > error messages with line and column numbers > should include line and column numbers for missing curve entries`: Checks the wording of Langium's MermaidParseError.
- `radar > error messages with line and column numbers > should include line and column numbers for invalid axis syntax`: Checks the wording of Langium's MermaidParseError.
- `radar > error messages with line and column numbers > should handle lexer errors with line and column numbers`: Checks the wording of Langium's MermaidParseError.
- `radar > error messages with line and column numbers > should format error message with "Parse error on line X, column Y" prefix`: Checks the wording of Langium's MermaidParseError.
- `radar > error messages with line and column numbers > should report an unknown location for errors at the end of input`: Checks the wording of Langium's MermaidParseError.
## Spec files not ported
