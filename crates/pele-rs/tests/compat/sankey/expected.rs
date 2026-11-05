// What pele 1.9.0 makes of each case in cases.rs, as summarized by tools/oracle/golden.mjs.
// Generated; do not edit.

const EXPECTED: &[(&str, &str)] = &[
    ("Sankey diagram > when parsing an info graph it > parses csv with sankey-beta syntax", "error:syntax"),
    ("Sankey diagram > when parsing an info graph it > parses csv with sankey syntax", "error:syntax"),
    ("Sankey diagram > when parsing an info graph it > allows __proto__ as id with sankey-beta syntax", "node:\"__proto__\"|0\nnode:\"A\"|1\nlink:\"__proto__\"|\"A\"|\"0.597\"\nlink:\"A\"|\"__proto__\"|\"0.403\"\ntitle:-"),
    ("Sankey diagram > when parsing an info graph it > allows __proto__ as id with sankey syntax", "node:\"__proto__\"|0\nnode:\"A\"|1\nlink:\"__proto__\"|\"A\"|\"0.597\"\nlink:\"A\"|\"__proto__\"|\"0.403\"\ntitle:-"),
    ("Sankey diagram > when parsing an info graph it > parses the issue #7528 sample with ${syntax} syntax", "template"),
];
