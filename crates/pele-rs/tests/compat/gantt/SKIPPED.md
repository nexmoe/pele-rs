# Skipped gantt specs

The cases in `cases.rs` come from pele 1.9.0's port of the Mermaid 12.1.0 gantt specs,
through `cargo xtask compat gantt --pele <DIR>`. `expected.rs` holds what pele makes of
each, written by `node tools/oracle/golden.mjs <DIR> gantt` in UTC. `src/diagrams/gantt/compat.rs`
checks that pele-rs's model of every case not listed here matches pele's: the settings, the
sections, the tasks with their start and end as scheduled at time 0, their states, rows and
classes, and the links, or an error of the same kind.

## Skipped tests

- `when using the ganttDb > should handle milliseconds`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should handle relative start date based on id regardless of sections`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should handle relative end date based on id regardless of sections`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should handle relative start date based on multiple id`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should handle relative end date based on multiple id`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should ignore weekends`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should ignore weekends starting on friday`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should not infinite loop when excluding everything`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should merge tokens across multiple setExcludes calls (issue #6270)`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should dedupe tokens across multiple setExcludes calls (issue #6270)`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should merge tokens across multiple setIncludes calls (issue #6270)`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should maintain the order in which tasks are created`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should work when end date is the 31st`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should add 1 day even on days with 25 hours`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > when setting inclusive end dates > should automatically add one day to all end dates`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should reject dates with ridiculous years`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should handle seconds-only format with valid numeric values (issue #5496)`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > should handle dates with year typo like 202 instead of 2024 (issue #5496)`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > when a task references a task id that does not exist > should warn about unknown ids in an "after" statement (issue #4121)`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > when a task references a task id that does not exist > should warn about unknown ids in an "until" statement`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > when a task references a task id that does not exist > should not warn when every referenced id exists`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.
- `when using the ganttDb > when a task references a task id that does not exist > should warn when a milestone is declared without a duration (issue #4121)`: builds the model by calling `GanttDb` methods, not from diagram text, so its extracted text is not a diagram.

## Spec files not ported

None.
