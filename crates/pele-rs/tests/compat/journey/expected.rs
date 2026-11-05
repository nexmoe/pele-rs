// What pele 1.9.0 makes of each case in cases.rs, as summarized by tools/oracle/golden.mjs.
// Generated; do not edit.

const EXPECTED: &[(&str, &str)] = &[
    ("when parsing a journey diagram it > should handle a title definition", "title:\"Adding journey diagram functionality to mermaid\"|acc:-|-"),
    ("when parsing a journey diagram it > should handle an accessibility description (accDescr)", "section:\"Order from website\"\ntitle:\"Adding journey diagram functionality to mermaid\"|acc:-|\"A user journey for family shopping\""),
    ("when parsing a journey diagram it > should handle an accessibility multiline description (accDescr)", "section:\"Order from website\"\ntitle:\"Adding journey diagram functionality to mermaid\"|acc:\"Adding acc journey diagram functionality to mermaid\"|\"A user journey for\\nfamily shopping\""),
    ("when parsing a journey diagram it > should handle an accessibility title (accDescr)", "section:\"Order from website\"\ntitle:-|acc:\"The title\"|-"),
    ("when parsing a journey diagram it > should handle a section definition", "section:\"Order from website\"\ntitle:\"Adding journey diagram functionality to mermaid\"|acc:-|-"),
    ("when parsing a journey diagram it > should handle multiline section titles with different line breaks", "section:\"Line1<br>Line2<br/>Line3</br />Line4<br\\t/>Line5\"\ntitle:\"Adding gantt diagram functionality to mermaid\"|acc:-|-"),
    ("when parsing a journey diagram it > should handle a task definition", "section:\"Documentation\"\nsection:\"Another section\"\ntask:\"A task\"|5|\"Alice\\u0001Bob\\u0001Charlie\"|\"Documentation\"\ntask:\"B task\"|3|\"Bob\\u0001Charlie\"|\"Documentation\"\ntask:\"C task\"|5|-|\"Documentation\"\ntask:\"D task\"|5|\"Charlie\\u0001Alice\"|\"Documentation\"\ntask:\"E task\"|5|\"\"|\"Documentation\"\ntask:\"P task\"|5|\"\"|\"Another section\"\ntask:\"Q task\"|5|\"\"|\"Another section\"\ntask:\"R task\"|5|\"\"|\"Another section\"\ntitle:\"Adding journey diagram functionality to mermaid\"|acc:-|-"),
    ("when using the journeyDb > tasks and actors should be added", "error:syntax"),
];
