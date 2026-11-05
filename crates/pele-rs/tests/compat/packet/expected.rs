// What pele 1.9.0 makes of each case in cases.rs, as summarized by tools/oracle/golden.mjs.
// Generated; do not edit.

const EXPECTED: &[(&str, &str)] = &[
    ("packet diagrams > should handle a packet-beta definition", "title:-|acc:-|-\nbits:32"),
    ("packet diagrams > should handle a packet definition", "title:-|acc:-|-\nbits:32"),
    ("packet diagrams > should handle diagram with data and title", "title:\"Packet diagram\"|acc:\"Packet accTitle\"|\"Packet accDescription\"\nbits:32\nblock:0|0|0|10|11|\"test\""),
    ("packet diagrams > should handle single bits", "title:-|acc:-|-\nbits:32\nblock:0|0|0|10|11|\"test\"\nblock:0|1|11|11|1|\"single\""),
    ("packet diagrams > should handle bit counts", "title:-|acc:-|-\nbits:32\nblock:0|0|0|7|8|\"byte\"\nblock:0|1|8|23|16|\"word\""),
    ("packet diagrams > should handle bit counts with bit or bits", "title:-|acc:-|-\nbits:32\nblock:0|0|0|7|8|\"byte\"\nblock:0|1|8|23|16|\"word\""),
    ("packet diagrams > should split into multiple rows", "title:-|acc:-|-\nbits:32\nblock:0|0|0|10|11|\"test\"\nblock:0|1|11|31|20|\"multiple\"\nblock:1|0|32|63|31|\"multiple\"\nblock:2|0|64|90|26|\"multiple\""),
    ("packet diagrams > should split into multiple rows when cut at exact length", "title:-|acc:-|-\nbits:32\nblock:0|0|0|16|17|\"test\"\nblock:0|1|17|31|14|\"multiple\"\nblock:1|0|32|63|31|\"multiple\""),
    ("packet diagrams > should throw error if numbers are not continuous", "error:semantic"),
    ("packet diagrams > should throw error if numbers are not continuous with bit counts", "error:semantic"),
    ("packet diagrams > should throw error if numbers are not continuous for single packets", "error:semantic"),
    ("packet diagrams > should throw error if numbers are not continuous for single packets with bit counts", "error:semantic"),
    ("packet diagrams > should throw error if numbers are not continuous for single packets - 2", "error:semantic"),
    ("packet diagrams > should throw error if end is less than start", "error:semantic"),
    ("packet diagrams > should throw error if bit count is 0", "error:semantic"),
    ("packet > should handle regular packet [0]", "title:-|acc:-|-\nbits:32"),
    ("packet > should handle regular packet [1]", "title:-|acc:-|-\nbits:32"),
    ("packet > should handle regular packet [2]", "title:-|acc:-|-\nbits:32"),
    ("packet > should handle regular packet [3]", "title:-|acc:-|-\nbits:32"),
    ("packet > should handle regular packet [4]", "title:-|acc:-|-\nbits:32"),
    ("packet > should handle regular packet [5]", "title:-|acc:-|-\nbits:32"),
    ("packet > should handle regular packet [6]", "title:-|acc:-|-\nbits:32"),
    ("packet > should handle regular packet [7]", "title:-|acc:-|-\nbits:32"),
];
