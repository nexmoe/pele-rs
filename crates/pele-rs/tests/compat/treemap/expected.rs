// What pele 1.9.0 makes of each case in cases.rs, as summarized by tools/oracle/golden.mjs.
// Generated; do not edit.

const EXPECTED: &[(&str, &str)] = &[
    ("Treemap Parser > Basic Parsing > should parse empty treemap", "title:-|accTitle:-|accDescr:-"),
    ("Treemap Parser > Basic Parsing > should parse a section node", "title:-|accTitle:-|accDescr:-\nnode:0|\"Root\"|Section|-|0|-|-"),
    ("Treemap Parser > Basic Parsing > should parse a section with leaf nodes", "title:-|accTitle:-|accDescr:-\nnode:0|\"Root\"|Section|-|300|-|-\nnode:1|\"Child1\"|Leaf|100|100|-|-\nnode:1|\"Child2\"|Leaf|200|200|-|-\nbox:0|0|0|640|400\nbox:0|6|28|417|366\nbox:0|427|28|207|366"),
    ("Treemap Parser > Data Types > should correctly parse string values", "title:-|accTitle:-|accDescr:-\nnode:0|\"My Section\"|Section|-|0|-|-"),
    ("Treemap Parser > Data Types > should correctly parse number values", "title:-|accTitle:-|accDescr:-\nnode:0|\"Item\"|Leaf|123.45|123.45|-|-\nbox:0|0|0|640|400"),
    ("Treemap Parser > Validation > should parse multiple root nodes", "title:-|accTitle:-|accDescr:-\nnode:0|\"Root1\"|Section|-|0|-|-\nnode:0|\"Root2\"|Section|-|0|-|-"),
    ("Treemap Parser > Title and Accessibilities > should parse a treemap with title", "title:\"My Treemap Diagram\"|accTitle:-|accDescr:-\nnode:0|\"Root\"|Section|-|100|-|-\nnode:1|\"Child\"|Leaf|100|100|-|-\nbox:0|0|0|640|400\nbox:0|6|28|628|366"),
    ("Treemap Parser > Title and Accessibilities > should parse a treemap with accTitle", "title:-|accTitle:\"Accessible Title\"|accDescr:-\nnode:0|\"Root\"|Section|-|100|-|-\nnode:1|\"Child\"|Leaf|100|100|-|-\nbox:0|0|0|640|400\nbox:0|6|28|628|366"),
    ("Treemap Parser > Title and Accessibilities > should parse a treemap with accDescr", "title:-|accTitle:-|accDescr:\"This is an accessible description\"\nnode:0|\"Root\"|Section|-|100|-|-\nnode:1|\"Child\"|Leaf|100|100|-|-\nbox:0|0|0|640|400\nbox:0|6|28|628|366"),
    ("Treemap Parser > Title and Accessibilities > should parse a treemap with multiple accessibility attributes", "title:\"My Treemap Diagram\"|accTitle:\"Accessible Title\"|accDescr:\"This is an accessible description\"\nnode:0|\"Root\"|Section|-|100|-|-\nnode:1|\"Child\"|Leaf|100|100|-|-\nbox:0|0|0|640|400\nbox:0|6|28|628|366"),
    ("Treemap Parser > ClassDef and Class Statements > should parse a classDef statement", "title:-|accTitle:-|accDescr:-\nclass:\"myClass\"|\"fill:red\""),
    ("Treemap Parser > ClassDef and Class Statements > should parse a classDef statement without semicolon", "title:-|accTitle:-|accDescr:-\nclass:\"myClass\"|\"fill:red\""),
    ("Treemap Parser > ClassDef and Class Statements > should parse a classDef statement with multiple style properties", "title:-|accTitle:-|accDescr:-\nclass:\"complexClass\"|\"fill:blue stroke:#ff0000 stroke-width:2px\""),
    ("Treemap Parser > ClassDef and Class Statements > should parse a class assignment statement", "error:syntax"),
    ("Treemap Parser > ClassDef and Class Statements > should parse a class assignment statement with semicolon", "error:syntax"),
    ("Treemap Parser > ClassDef and Class Statements > should parse a section with inline class style using :::", "title:-|accTitle:-|accDescr:-\nnode:0|\"My Section\"|Section|-|0|\"sectionClass\"|-"),
    ("Treemap Parser > ClassDef and Class Statements > should parse a leaf with inline class style using :::", "title:-|accTitle:-|accDescr:-\nnode:0|\"My Leaf\"|Leaf|100|100|\"leafClass\"|-\nbox:0|0|0|640|400"),
    ("treemap utilities > buildHierarchy > should convert a flat array into a hierarchical structure", "error:syntax"),
    ("treemap utilities > buildHierarchy > should handle empty input", "error:syntax"),
    ("treemap utilities > buildHierarchy > should handle only root nodes", "error:syntax"),
    ("treemap utilities > buildHierarchy > should handle complex nesting levels", "error:syntax"),
];
