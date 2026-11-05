// What pele 1.9.0 makes of each case in cases.rs, as summarized by tools/oracle/golden.mjs.
// Generated; do not edit.

const EXPECTED: &[(&str, &str)] = &[
    ("when parsing an Ishikawa diagram > should parse a basic Ishikawa hierarchy", "node:0|\"Blurry Photo\"\nnode:0.0|\"Process\"\nnode:0.0.0|\"Out of focus\"\nnode:0.1|\"User\"\nnode:0.1.0|\"Shaky hands\"\ntitle:-"),
    ("when parsing an Ishikawa diagram > should support an unindented root with nested causes", "node:0|\"Problem\"\nnode:0.0|\"Cause A\"\nnode:0.0.0|\"Subcause A1\"\nnode:0.1|\"Cause B\"\ntitle:-"),
    ("when parsing an Ishikawa diagram > should handle effect indented more than causes", "node:0|\"Problem\"\nnode:0.0|\"Cause A\"\nnode:0.0.0|\"Subcause A1\"\nnode:0.1|\"Cause B\"\ntitle:-"),
];
