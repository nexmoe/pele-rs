// What pele 1.9.0 makes of each case in cases.rs, as summarized by tools/oracle/golden.mjs.
// Generated; do not edit.

const EXPECTED: &[(&str, &str)] = &[
    ("info > should handle an info definition", "title:-|acc:-|-"),
    ("info > should handle an info definition with showInfo", "title:-|acc:-|-"),
    ("info > should throw because of unsupported info grammar", "error:syntax"),
    ("info > should throw because of unsupported info grammar", "error:syntax"),
    ("info > should handle empty info [0]", "title:-|acc:-|-"),
    ("info > should handle empty info [1]", "title:-|acc:-|-"),
    ("info > should handle empty info [2]", "title:-|acc:-|-"),
    ("info > should handle empty info [3]", "title:-|acc:-|-"),
    ("info > should handle showInfo [0]", "title:-|acc:-|-"),
    ("info > should handle showInfo [1]", "title:-|acc:-|-"),
    ("info > should handle showInfo [2]", "title:-|acc:-|-"),
    ("info > should handle showInfo [3]", "title:-|acc:-|-"),
    ("info > should handle showInfo [4]", "title:-|acc:-|-"),
    ("info > should handle showInfo [5]", "title:-|acc:-|-"),
    ("info > should handle showInfo [6]", "title:-|acc:-|-"),
    ("info > should handle showInfo [7]", "title:-|acc:-|-"),
];
