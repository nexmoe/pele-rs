// What pele 1.9.0 makes of each case in cases.rs, as summarized by tools/oracle/golden.mjs.
// Generated; do not edit.

const EXPECTED: &[(&str, &str)] = &[
    ("when parsing a timeline  > Timeline > should default to LR direction when none is provided", "dir:\"LR\"\nsection:\"abc-123\"\ntitle:-|acc:-|-"),
    ("when parsing a timeline  > Timeline > should parse TD direction", "dir:\"TD\"\nsection:\"abc-123\"\ntitle:-|acc:-|-"),
    ("when parsing a timeline  > Timeline > should parse LR direction", "dir:\"LR\"\nsection:\"abc-123\"\ntitle:-|acc:-|-"),
    ("when parsing a timeline  > Timeline > should handle a simple section definition abc-123", "dir:\"LR\"\nsection:\"abc-123\"\ntitle:-|acc:-|-"),
    ("when parsing a timeline  > Timeline > should handle a simple section and only two tasks", "dir:\"LR\"\nsection:\"abc-123\"\nperiod:\"task1\"|0|-\nperiod:\"task2\"|0|-\ntitle:-|acc:-|-"),
    ("when parsing a timeline  > Timeline > should handle a two section and two corresponding tasks", "dir:\"LR\"\nsection:\"abc-123\"\nsection:\"abc-456\"\nperiod:\"task1\"|0|-\nperiod:\"task2\"|0|-\nperiod:\"task3\"|1|-\nperiod:\"task4\"|1|-\ntitle:-|acc:-|-"),
    ("when parsing a timeline  > Timeline > should handle a section, and task and its events", "dir:\"LR\"\nsection:\"abc-123\"\nperiod:\"task1\"|0|\"event1\"\nperiod:\"task2\"|0|\"event2\\u0001event3\"\ntitle:-|acc:-|-"),
    ("when parsing a timeline  > Timeline > should handle a section, and task and its events including markdown link", "dir:\"LR\"\nsection:\"abc-123\"\nperiod:\"task1\"|0|\"[event1](http://example.com)\"\nperiod:\"task2\"|0|\"event2\\u0001event3\"\ntitle:-|acc:-|-"),
    ("when parsing a timeline  > Timeline > should handle a section, and task and its multi line events", "dir:\"LR\"\nsection:\"abc-123\"\nperiod:\"task1\"|0|\"event1\"\nperiod:\"task2\"|0|\"event2\\u0001event3\\u0001event4\\u0001event5\"\ntitle:-|acc:-|-"),
    ("when parsing a timeline  > Timeline > should handle a title, section, task, and events with semicolons", "dir:\"LR\"\nsection:\";a;bc-123;\"\nperiod:\";ta;sk1;\"|0|\";ev;ent1; \\u0001;ev;ent2; \\u0001;ev;ent3;\"\ntitle:\";my;title;\"|acc:-|-"),
    ("when parsing a timeline  > Timeline > should handle a title, section, task, and events with hashtags", "dir:\"LR\"\nsection:\"#a#bc-123#\"\nperiod:\"task1\"|0|\"#ev#ent1# \\u0001#ev#ent2# \\u0001#ev#ent3#\"\ntitle:\"#my#title#\"|acc:-|-"),
];
