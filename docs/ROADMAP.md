# Historical fermaid roadmap

This is the unmodified upstream roadmap at fermaid 0.5.1. Package names, release claims and completed milestones below refer to fermaid. Current pele-rs behavior and integration are documented in [NATIVE.md](NATIVE.md).

# fermaid Roadmap

fermaid renders Mermaid diagram text to SVG in pure Rust. Behavior, layout
algorithms, text metrics and limits are ported from
[obsidianmd/pele](https://github.com/obsidianmd/pele), pinned at
**v1.9.0 (`e7f092a4de147cc32210913c20e36eaa5fcd0ef1`)**. Mermaid syntax target: 12.2.0 (see M7a).

Milestones M0–M6 are done. M7 is planned to task level.
Every diagram type Mermaid 12.2.0 builds is supported.

## Decisions

| Topic | Decision |
|---|---|
| Deliverables | Library crate + thin CLI. Core does no I/O, so a WASM build stays possible later. |
| Relationship to pele | Port algorithms and data faithfully (layered layout, metrics, wrapping, limits), written as idiomatic Rust. Not a line-by-line translation. Output should be close to pele's, not byte-identical. Credit pele in `NOTICE` and README. |
| Runtime dependencies | None. Dev-dependencies (insta, proptest, cargo-fuzz) are allowed. |
| Errors | Strict: `Err(Error)`, no partial SVG. No lenient mode for now. |
| Platform | `std`, `rust-version = "1.85"`, edition 2024. |
| Diagram selection | One Cargo feature per diagram type, `default = ["all"]`. The `Diagram` trait stays internal; there is no runtime `register()`. |
| Links | Click targets are sanitized as `@braintree/sanitize-url` does (as Mermaid does), then kept only when relative or of a scheme in `RenderOptions::link_schemes` (default `http`, `https`, `mailto`, `tel`). A refused link is left out, not written as `about:blank`. |
| Theming | CSS variables `--fermaid-bg`, `--fermaid-fg`, `--fermaid-line`, `--fermaid-accent`, `--fermaid-series-1..8`, … (same suffixes as pele). |
| Limits | Same defaults as pele: source 50,000 chars, output 4,000,000 chars, 5,000 edges, plus an explicit nesting depth limit (default 64), because a Rust stack overflow aborts and cannot be caught. Diagram config can never raise limits. |
| Determinism | Same input + same options ⇒ byte-identical output. Coordinates use at most 2 decimals with trailing zeros trimmed. Ids are `{id_prefix}-{n}`. Output never depends on hash-map iteration order. |
| Text measurement | `TextMeasurer` trait; the default `MetricsMeasurer` ports pele's Arial ASCII tables (regular/bold) and its non-ASCII rules (CJK/fullwidth 1 em, other non-ASCII 580, mono 600, combining/zero-width 0). Wrapping rules are ported too. The tables are embedded in pele's packed form and decoded at compile time (`const fn`), so no generator is needed. |
| Dates | Gantt dates are read and drawn in UTC: there is no time zone database, and output must be deterministic. "Today" is `RenderOptions::now` (milliseconds since the epoch, CLI `--now`); `None` is 1970-01-01, not the clock. A date no format matches falls back to `new Date(text)`, which V8 reads with a parser of its own, ported in `gantt/legacy_date.rs`; the dates it reads are in `gantt/date_cases.rs`, and `tools/oracle/dates.mjs` writes them. |
| Preprocessing | `%%` comments, a front-matter YAML subset (`title`, nested `config` maps and scalars), and `%%{init}%%` parsed as strict JSON, like `JSON.parse` in pele (not JSON5). Unknown keys are ignored. |
| Testing | insta snapshots; Mermaid spec cases ported from pele's `tests/compat/<type>`, each with a `SKIPPED.md`; per-type security tests; cargo-fuzz (no panics, limits hold); differential fuzzing against Mermaid's parsers from M3 (dev-only Node harness, outside regular CI). |
| CI | GitHub Actions: `fmt --check`, `clippy -D warnings`, `test` on stable and MSRV. A commit that touches only the documentation skips the run, because prose cannot break the build. The fuzz targets run nightly and on demand rather than on every push: running all of them takes half an hour, which is too long to wait for. |
| Releases | `0.2.0` is the first published release, after M5; it reserves the name on crates.io. Breaking changes are allowed before 1.0 and recorded in `CHANGELOG.md`. |

## Public API (initial shape)

```rust
pub fn render(src: &str, opts: &RenderOptions) -> Result<Rendered, Error>;
pub fn detect_type(src: &str) -> Option<DiagramKind>;

pub struct Rendered {
    pub svg: String,
    pub width: f64,
    pub height: f64,
    pub links: Vec<Link>,
    pub kind: DiagramKind,
}

pub struct Link {
    pub id: String,     // element id as written in the diagram
    pub href: String,
    pub internal: bool, // points into the page
}

pub struct Error {
    pub code: ErrorCode, // Unsupported | Syntax | Semantic | Limit
    pub kind: Option<DiagramKind>, // None when the type was not detected
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub snippet: String, // source line with a caret under the error position
}

// RenderOptions: Default; fields font_family, font_family_mono,
// font_size: Option<f64>, id_prefix, max_width: Option<f64> (M1), padding: Option<f64>, responsive,
// limits { source, output, max_edges, max_depth },
// measurer: Option<Box<dyn TextMeasurer>>.
// link_schemes is added in M2 together with URL sanitizing.
```

## Repository layout

```
crates/fermaid       library (published as `fermaid`)
crates/fermaid-cli   binary `fermaid`, hand-rolled arg parsing
xtask/               (M1) compat case conversion
fuzz/                cargo-fuzz targets
tools/oracle/        (M3) dev-only Node harness for differential fuzzing
```

## M0 — Foundation (done)

1. [x] Convert to a workspace with the layout above; set `rust-version = "1.85"`.
2. [x] Core skeleton: `Error`, `ErrorCode`, `RenderOptions`, `Limits`, `Rendered`,
   `DiagramKind`, `detect_type`, an internal `Diagram` trait, and feature-gated dispatch.
3. [x] `svg` module: zero-dependency builder with escaping, deterministic number
   formatting and sequential ids.
4. [x] `text` module: `TextMeasurer` trait and `MetricsMeasurer` with pele's tables
   embedded and decoded at compile time; port the wrapping rules (label layout).
5. [x] `preprocess` module: comments, front-matter YAML subset, `%%{init}%%`, entity codes.
6. [x] CI workflow; a `render` fuzz target.
7. [x] `NOTICE` crediting pele; a project introduction in `README.md` and its three translations.

**Done when**
- CI is green.
- `fermaid` returns an `Unsupported` error with line and column for an unknown diagram type.
- Measurer unit tests match pele's `metricsMeasurer` on a fixed set of cases.

## M1 — Pie (end-to-end pipeline) (done)

0. [x] `xtask`: convert pele's `tests/compat/<type>` cases into Rust test data
   (`cargo xtask compat <type> --pele <DIR>` writes `crates/fermaid/tests/compat/<type>/cases.rs`).
1. [x] Pie lexer, parser and model (`showData`, `title`, `accTitle`, `accDescr`), on a shared
   tokenizer for Langium-defined types (`diagrams/common/tokens.rs`).
2. [x] Layout and emitter using `--fermaid-series-*`, with `RenderOptions::max_width`
   (pele's `fitWidth`).
3. [x] Port the Mermaid spec cases from pele's `tests/compat/pie` and add `SKIPPED.md`.
4. [x] insta snapshots, security tests (escaping, limits; pie has no nesting), a fuzz target.
5. [x] First version of the CLI: `fermaid [INPUT|-] [-o OUT] [--font-size N] [--max-width N] [--id-prefix P]`.
6. Publishing is deferred to one release after M5: `0.2.0` (see "Release" below).

Notes:
- Output was compared byte for byte with pele 1.9.0 (after renaming `pele` to `fermaid`) on
  pele's pie corpus, its hostile payloads and 3,000 mutated inputs: identical, errors and their
  positions included.
- 10 minutes of fuzzing the `pie` target (1.79 million runs, seeded with the pie corpus) found
  no panic.
- As in pele, pie errors are positioned in the text after preprocessing (comment lines and
  front matter removed), so a line number can be off when comments precede the error.

**Done when**
- Compat tests pass; every skipped case is listed in `SKIPPED.md`.
- 10 minutes of fuzzing produces no panic.
- The SVG renders in a browser and its theme can be changed through CSS.

## M2 — Flowchart (core layout) (done)

1. [x] `layout::layered`: port pele's Sugiyama layout (ranking, crossing
   reduction, dummy/label nodes, edge tracks, straighten/settle/snap).
2. [x] `layout::compound` for subgraphs. (pele's `components` is used only by other diagram
   types; it moves to the milestone of the first type that needs it.)
3. [x] Flowchart parser covering all node shapes, edge kinds, `classDef`, `style`,
   `click` and `linkStyle`. The lexer works on UTF-16 code units, as Mermaid's jison lexer does.
4. [x] Port svg shapes, edges and clusters.
5. [x] Security: URL sanitizing and a link scheme allowlist (`RenderOptions::link_schemes`,
   `image_schemes`, `link_rel`); `style` limited to allowed properties.
6. [x] Compat, snapshot and security tests; a fuzz target.
7. Publishing is deferred to one release after M5: `0.2.0` (see "Release" below).

Notes:
- Compat: 301 cases from pele's port of the Mermaid flowchart specs. `tools/oracle/golden.mjs`
  records what pele makes of each, and fermaid's model matches it on all 274 that are not
  skipped (27 are built from test-code values; see `SKIPPED.md`).
- All 114 flowchart examples from the Mermaid docs render. On pele's flowchart corpus (379
  inputs), the inputs in pele's own flowchart tests (102) and 2,000 mutated inputs, the output
  is byte for byte pele's, errors included, except where a node has an icon: with no icon
  resolver, fermaid draws a dashed placeholder where pele leaves the space empty. This also
  settles the side-by-side review.
- A random graph of 5,000 edges renders in about 0.1 s in a release build (pele: 0.4 s).
- 10 minutes of fuzzing the `flowchart` target (625,000 runs, seeded with pele's flowchart
  corpus) found no panic.

**Done when**
- Compat tests pass.
- Every flowchart example from the Mermaid docs (pele's corpus) renders.
- A side-by-side manual review against pele on 20 fixed examples shows comparable layouts.
- A 5,000-edge stress test finishes in under 1 s in a release build
  (provisional target; adjust once a baseline exists).

## M3 — Sequence + differential fuzzing (done)

1. [x] Sequence diagrams: participants, actors, messages, activations,
   `loop`/`alt`/`opt`/`par`, notes, `autonumber`.
2. [x] `tools/oracle/`: dev-only Node harness that loads Mermaid's jison and Langium
   parsers and runs differential fuzzing (about 3,000 mutated inputs, fixed seed)
   for pie, flowchart and sequence.
3. [x] Refine the M4 plan.

Notes:
- Sequence diagrams cover everything pele draws: every participant kind (including
  `@{ "type": … }`), every arrow kind with half heads and central connections, activations,
  notes, all block kinds, boxes, `autonumber`, created and destroyed participants, and the
  `sequence` config keys `mirrorActors`, `rightAngles`, `actorMargin`, `width`, `height`,
  `messageMargin`, `noteMargin`, `boxMargin`, `messageAlign`, `showSequenceNumbers`,
  `hideUnusedParticipants` and `wrap`. As in pele, participant menus (`link`, `links`) are
  parsed but never drawn as links.
- Compat: 165 cases from pele's port of the Mermaid sequence specs; fermaid's model matches
  pele's on all of them.
- On pele's sequence corpus (200 inputs), with and without `max_width` and a non-default
  `sequence` config, and on 3,000 mutated inputs, the output is byte for byte pele's, errors
  included.
- 7 minutes of fuzzing the `sequence` target (680,000 runs, seeded with pele's sequence corpus)
  found no panic.
- `tools/oracle/diff.mjs` parses each type's corpus and 3,000 mutated inputs with Mermaid's own
  parsers (`@mermaid-js/parser` for pie; the parsers jison generates from Mermaid's flowchart
  and sequence grammars, driving pele's port of Mermaid's model) and with fermaid's, and
  compares the results. With seeds 1, 7, 42 and 1234 (5,000 inputs each), the three types
  agree on every input. A fourth check, `url`, compares the link sanitizer with
  `@braintree/sanitize-url`; it found that fermaid's imitation of `new URL()` let through
  hosts that `new URL()` refuses, and missed percent-encoding and dot segments, which are now
  fixed. The differences that remain are in `tools/oracle/KNOWN.md`: one comes from Node's
  URL parser departing from the WHATWG standard, the other from IDNA tables fermaid does not
  carry. The workflow `.github/workflows/oracle.yml` runs it nightly and on demand.

**Done when**
- Differential fuzzing reports no unexplained differences for the three types;
  intentional differences are recorded in `SKIPPED.md`.
- Regular CI does not need Node; the oracle runs only in a nightly or
  manually triggered workflow.

## M4 — State, class, ER, requirement ✅

Each type is ported from pele as flowchart and sequence were: parser, model, render, and a
check against pele's output on its corpus and on mutated inputs.

1. State diagrams: states, composite states with their own direction, start and end, choice,
   fork and join, notes, concurrency regions, `classDef` and `class`, `direction`. Reuses
   the compound layout.
2. Class diagrams: classes with members and generics, relations with cardinality, namespaces,
   annotations, notes, `classDef`, `style`, `cssClass`, links and tooltips (sanitized as in
   flowcharts). Reuses the layered layout.
3. ER diagrams: entities with attributes and keys, relationships with cardinality and
   identification, aliases, `classDef` and `style`.
4. Requirement diagrams: requirements, elements and relationships, `classDef` and `style`.
5. For each: compat cases from pele's `tests/compat/<type>` with `SKIPPED.md` (golden where the
   specs test a model), snapshot, security and speed tests, a fuzz target, and a place in
   `tools/oracle/diff.mjs` (state, class and ER have jison grammars in pele; requirement too).
6. Port pele's `components` layout only if one of these needs it.

**Done when**
- Compat tests pass, and each type's output matches pele's on its corpus and on mutated
  inputs, apart from recorded differences.
- The differential run reports no unexplained differences for the new types.
- 10 minutes of fuzzing each new target produces no panic.

Notes:
- All four types are ported. Class diagrams needed pele's `components` layout, which wraps
  disconnected parts into rows to fit a narrow host; it is in `layout/components.rs`.
- On each type's corpus from pele, on 3,000 mutated inputs, and with `max_width`, the output
  is byte for byte pele's, errors included.
- Compat cases from pele's ports of the Mermaid specs pass for all four types. The ER and
  requirement specs partly call the model directly or build diagrams from arrays of lines, so
  many of their extracted sources are not diagrams and both sides reject them; `SKIPPED.md`
  says so, and the differential run covers the parsers instead.
- The differential run against Mermaid's jison grammars (state, class, ER, requirement) agrees
  on every input.
- Fuzzing found no panic (class 831,000 runs, ER 895,000 runs in 10 minutes).

## M5 — Gantt, timeline, journey, kanban, mindmap ✅

Each type is ported from pele as in M4, with compat cases, snapshot, security and speed tests,
and a fuzz target.

Notes:
- Timelines and journeys share a plain line parser (`common/plain.rs`); kanban boards and mind
  maps share an indented outline parser (`common/outline.rs`); all four use the tokenizer in
  `common/scan.rs`.
- On each type's corpus from pele, on hand-written cases, on 3,000 mutated inputs, and with
  `max_width`, the output is byte for byte pele's, errors included. For gantt charts pele runs
  with `TZ=UTC` and `now: 0` (and other fixed times). The dates V8's own date parser reads are
  the ones Mermaid reads, so nothing is left out.
- Compat cases pass. The gantt and mind map specs partly call the model directly; those cases
  are listed in `SKIPPED.md`.
- The differential run against Mermaid's grammars covers all five types, and every one of
  them agrees on every input.
- Fuzzing found no panic.
- Re-verified against pele 1.9.0 (`e7f092a`) and Node 22: the golden files of the five types
  regenerate byte for byte, the differential run over 3,000 mutated inputs per type reports no
  difference for any of the thirteen types, and ten minutes of fuzzing per new target found no
  panic (gantt 757,835 runs, timeline 233,850, journey 293,981, kanban 279,941, mind map
  311,144).

## Release — 0.2.0

The first published version.

1. [x] Publish `fermaid` 0.2.0 to crates.io, then `fermaid-cli` 0.2.0, which depends on it.
2. [x] Tag `v0.2.0`, push the tag and publish the GitHub release.

Notes:
- The crates.io names `fermaid` and `fermaid-cli` were free, and `0.0.1` and `0.1.0` were
  never published, so `0.2.0` is the only version crates.io shows.
- The library is `fermaid` and the command line tool is `fermaid-cli`; the tool depends on
  the library, so the library goes first.

## Release — 0.3.0

The version that follows M6, when every diagram type Mermaid 12.1.0 builds is done.

1. [x] Add `LICENSE.md`, so that the licence texts travel with the published crates: the
   licence files sit at the root of the repository, and cargo packages only what is inside the
   crate's own directory. Every crate therefore carries a copy of them, and a test keeps the
   copies and the ones at the root the same.
2. [x] Publish the library, then the command line tool, which depends on it.
3. [x] Tag the version, push the tag and publish the GitHub release.

Notes:
- The information diagram draws the version of the library under its title, so its snapshots
  carry the version of the crate. A version bump therefore regenerates them:
  `INSTA_UPDATE=always cargo test -p fermaid --all-features --test info`.
- `fermaid` 0.3.0 and `fermaid-cli` 0.3.0 are on crates.io, under the owner `tenyi`. The library
  goes first, because the command line tool depends on it.

## M6 — The remaining types ✅

Mermaid 12.1.0 builds 33 diagram types. M0–M5 cover twelve of them; M6 covers the rest, in
three batches, ordered by how much the types are used. The niche types come last, as the
roadmap says they should.

Each type is a self-contained unit, as in M2–M5: a lexer, a parser, a model, a layout and an
emitter behind its own Cargo feature, with the Mermaid spec cases from pele's
`tests/compat/<type>`, a golden file that says what pele makes of them, a snapshot, security
and speed tests, a fuzz target, and a place in `tools/oracle/diff.mjs`. A type that pele
draws differently from Mermaid follows Mermaid, and the difference is recorded in
`tools/oracle/KNOWN.md`.

### M6a — The charts people reach for

- [x] Sankey charts, behind the `sankey` feature.
- [x] XY charts, behind the `xychart` feature.
- [x] Quadrant charts, behind the `quadrantchart` feature.
- [x] Git graphs, behind the `git` feature.

Notes:
- A sankey chart is written as CSV, as Mermaid's grammar reads it: a `source,target,value`
  record per line, with the fields quoted as RFC 4180 says. The layout, the settling of the
  columns and the labels are ported from pele.
- Mermaid's grammar lists only the printable ASCII characters as the text of a field, so a name
  with a letter outside that range is a parse error in Mermaid. pele reads it; fermaid follows
  Mermaid, and a test pins that.
- The gradient of the bands is a `url(#…)` reference to an element of the same document, which
  the inertness check allows; a `url()` that points outside the document is still refused.
- The differential run against Mermaid's own grammar reports no difference over 2,775 inputs,
  the golden file of the Mermaid specs regenerates byte for byte from pele, and ten minutes of
  fuzzing (1,072,313 runs) found no panic.
- An XY chart is read with Mermaid's own `xychart.jison` grammar, through a hand-written lexer
  that walks UTF-16 code units, as the jison lexer does. The layout is pele's own, as it is
  for every type: Mermaid draws the chart with d3, and the two are not the same picture.
- The configuration lives under `xyChart`, as in Mermaid's schema, and not under `xychart`.
- The value of a bar is always written past its end, which is what `showDataLabelOutsideBar`
  asks for in Mermaid. Inside the bar, whether the value fits changes from bar to bar and
  with the width of the chart, so fermaid writes it outside as well.
- A tick step below one is the negative of its inverse, so that the tick values are found by
  division and print without rounding noise: 0.3, not 0.30000000000000004.
- The differential run against Mermaid's own grammar reports no difference over 3,070 inputs,
  the 64 Mermaid spec cases pass, and ten minutes of fuzzing (1,774,589 runs) found no panic.
- The picture is compared with pele's, chart by chart, by `tools/oracle/render-compare.mjs`:
  eight charts, with lines, bars, a horizontal one, a point-label one and a long one, come out
  the same to the byte once the names of the classes and of the CSS variables are normalized.
- A quadrant chart is read with Mermaid's own `quadrant.jison` grammar, through a hand-written
  lexer that walks UTF-16 code units. The layout is pele's own, as it is for every type.
- The configuration lives under `quadrantChart`, as in Mermaid's schema.
- The label of a point looks for a place that is free: below the point, above it, right or left
  of it, or below the title of the quadrant it is in. A chart of more than three hundred points
  skips the search and puts every label below its point.
- A stroke color is only drawn when there is a stroke width to draw it with, as in Mermaid.
- The differential run against Mermaid's own grammar reports no difference over 3,029 inputs,
  the 29 Mermaid spec cases pass, ten minutes of fuzzing (1,404,030 runs) found no panic, and
  the picture comes out the same as pele's, chart by chart.
- A git graph is read with Mermaid's own Langium grammar, through a hand-written lexer that
  answers as Chevrotain does: the keywords are tried first, and a name that is longer than the
  keyword it starts with wins, so `commits` is a branch and not the word `commit`. The layout
  is pele's own, as it is for every type.
- Mermaid ends the id of a commit it made up with seven random digits; fermaid derives them from
  the number of the commit, so the same text always gives the same ids.
- The differential run against Mermaid's own grammar reports no difference over 3,094 inputs,
  the 100 Mermaid spec cases pass, ten minutes of fuzzing (1,236,337 runs) found no panic, and
  the picture comes out the same as pele's, chart by chart.

### M6b — The structural diagrams

- [x] Block diagrams, behind the `block` feature.
- [x] Architecture diagrams, behind the `architecture` feature.
- [x] Packet diagrams, behind the `packet` feature.
- [x] Radar charts, behind the `radar` feature.
- [x] Treemaps, behind the `treemap` feature.

Notes:
- A block diagram is read with Mermaid's own `block.jison` grammar, through a hand-written lexer
  that walks UTF-16 code units, as the jison lexer does. The layout is pele's own, as it is for
  every type.
- Mermaid's grammar reads a chain of links from the left and keeps only the last one, so the
  nodes before it are not part of the diagram and the link that is kept starts at nothing.
  fermaid follows Mermaid, and a test pins that.
- A composite whose header was a chain of links has no id of its own, as in Mermaid.
- Mermaid looks a class up before trimming its id, so `class a, b hot` loses the block `b`, and
  it throws on a style for an unknown id. pele waits for the block in both cases; fermaid does
  the same, and says so in the code.
- The name Mermaid gives a block without an id of its own ends in a counter, not in random
  digits as Mermaid's own does, so the same text always gives the same names.
- The configuration lives under `block`, as in Mermaid's schema, and holds `padding`, the room
  between the blocks.
- The differential run against Mermaid's own grammar reports no difference over 3,063 inputs,
  the 40 Mermaid spec cases pass (13 of them are not diagrams and are listed in `SKIPPED.md`),
  and 490,480 runs of fuzzing over ten minutes found no panic.
- An architecture diagram is read with Mermaid's Langium grammar, over the shared
  tokenizer. The model holds the services, the groups and the junctions with the edges
  between them, and the layout is pele's own: every node takes a cell of a grid, so a group
  never encloses a node that is not in it.
- A service is an icon square with its title beneath (M7c).
- The differential run against Mermaid's own grammar reports no difference over 3,024
  inputs, the 59 Mermaid spec cases pass (23 of them hold no diagram and are listed in
  `SKIPPED.md`), and ten minutes of fuzzing (1,127,260 runs) found no panic.
- A packet diagram is read with Mermaid's Langium grammar, over a tokenizer of its own. The
  model holds the rows of fields the packet is drawn in: a field names the bit it starts at,
  the bit it ends at, or the number of bits it takes, and what it does not name is filled in
  later.
- Mermaid's own spec expects a field that is split across rows to be drawn on a row of its
  own, but the code of Mermaid 12.1.0 draws it beside the field before it, in the row both
  share. The code is what Mermaid draws, so this follows the code.
- The differential run against Mermaid's own grammar reports no difference over 3,019
  inputs, the 23 Mermaid spec cases pass (1 of them read positions back from a DOM and are
  listed in `SKIPPED.md`), and ten minutes of fuzzing (1,270,660 runs) found no panic.
- A radar chart is read with Mermaid's Langium grammar, over the shared tokenizer. The values
  of a curve are either a list of numbers or a list of `axis: value` pairs, and never a mix
  of the two in one curve: Mermaid's grammar tries the numbers first and falls back to the
  pairs, and here the first value decides which of the two it is.
- The chart is drawn with pele's own layout, as it is for every type: Mermaid draws the chart
  with d3, and the two are not the same picture.
- The differential run against Mermaid's own grammar reports no difference over 3,014
  inputs, the 62 Mermaid spec cases pass (43 of them hold no diagram and are listed in
  `SKIPPED.md`), and ten minutes of fuzzing (1,065,746 runs) found no panic.
- A treemap is read with Mermaid's Langium grammar, over the shared tokenizer. The model holds
  the tree of the diagram: the sections that hold other items and the leaves that hold a
  value. The layout is the squarified treemap, ported from pele.
- The grammar of Mermaid has no rule for `class`, so a treemap that assigns a class to an
  item is refused by Mermaid's own parser, and so it is refused here.
- The differential run against Mermaid's own grammar reports no difference over 3,008
  inputs, the 21 Mermaid spec cases pass (6 of them hold no diagram and are listed in
  `SKIPPED.md`), and ten minutes of fuzzing (1,221,447 runs) found no panic.

### M6c — The rest

- [x] C4 diagrams, behind the `c4` feature.
- [x] Use case diagrams, behind the `usecase` feature.
- [x] Info diagrams, behind the `info` feature.
- [x] Venn diagrams, behind the `venn` feature.
- [x] Tree views, behind the `treeview` feature.
- [x] Cynefin diagrams, behind the `cynefin` feature.
- [x] Ishikawa diagrams, behind the `ishikawa` feature.
- [x] Event modeling diagrams, behind the `eventmodeling` feature.
- [x] Wardley maps, behind the `wardley` feature.
- [x] Railroad diagrams, behind the `railroad` feature.
- [x] Swimlane diagrams, behind the `swimlane` feature.
- [x] Agent flows, behind the `agentflow` feature.

Notes:
- A C4 diagram is read with Mermaid's jison grammar, through a hand-written lexer that walks
  UTF-16 code units, as the jison lexer does. The model holds the shapes with their kinds and
  their labels, the boundaries that hold them and the relations between them; the layout and
  the routes of the relations are pele's own.
- The differential run against Mermaid's own grammar reports no difference over 2,777
  inputs, the 55 Mermaid spec cases pass (13 of them hold a placeholder rather than a
  diagram and are listed in `SKIPPED.md`), and ten minutes of fuzzing (1,181,796 runs) found
  no panic.
- An information diagram is read with Mermaid's Langium grammar, which is small: the keyword,
  the optional `showInfo`, and then any number of accessible texts and titles, each of them
  on a line of its own.
- The differential run against Mermaid's own grammar reports no difference over 3,005
  inputs, the 16 Mermaid spec cases pass, and ten minutes of fuzzing (1,254,040 runs) found
  no panic.
- A Venn diagram is read with Mermaid's jison grammar, through a hand-written lexer that walks
  UTF-16 code units, as the jison lexer does. A token is read only when the grammar has to
  look at one to decide: the lexer asks the model whether an indented `text` continues a set.
- The differential run against Mermaid's own grammar reports no difference over 3,025
  inputs, the 29 Mermaid spec cases pass (13 of them hold no diagram and are listed in
  `SKIPPED.md`), and ten minutes of fuzzing (1,023,835 runs) found no panic.
- A tree view is read with Mermaid's Langium grammar and its value converter, so that a name,
  an annotation and an indentation mean what they mean in Mermaid. A tree may also be written
  with box-drawing characters instead of an indented outline, and Mermaid turns them into an
  indentation before it reads the tree.
- The differential run against Mermaid's own grammar reports no difference over 3,011
  inputs, the 163 Mermaid spec cases pass (1 of them holds no diagram and is listed in
  `SKIPPED.md`), and ten minutes of fuzzing (884,204 runs) found no panic.
- A Cynefin diagram is read with Mermaid's Langium grammar: the four regions with the items in
  them, and the transitions between the regions with their labels. Mermaid's parser is
  hand-written, and pele's port of it reads the same sources the same way; where the two
  differ, this follows Mermaid.
- The differential run against Mermaid's own grammar reports no difference over 3,019
  inputs, the 42 Mermaid spec cases pass (27 of them hold no diagram and are listed in
  `SKIPPED.md`), and ten minutes of fuzzing (462,372 runs) found no panic.
- An Ishikawa diagram is read with Mermaid's `ishikawa.jison` grammar, the way its LALR
  automaton reads it. The grammar is odd in two ways, and Mermaid reads it as it is: a blank
  line right after the keyword is a statement of its own, and a statement must be ended by a
  blank line, a line break or the end of the text, so a blank line between the keyword and
  the effect is refused.
- The differential run against Mermaid's own grammar reports no difference over 3,004
  inputs, the 3 Mermaid spec cases pass, and ten minutes of fuzzing (649,897 runs) found no
  panic.
- An event modeling diagram is read with Mermaid's Langium grammar. Its terminals are tried
  in the order they are written and the keywords come first, so a keyword wins over `EM_ID`
  unless the name is longer: `cmd` is the keyword, `cmdx` is a name. The terminals for
  `title`, `accTitle` and `accDescr` are written after `EM_ID`, so `EM_ID` always wins:
  Mermaid cannot read a title in an event modeling diagram, and fermaid refuses such a line
  as well.
- The differential run against Mermaid's own grammar reports no difference over 3,024
  inputs, the 41 Mermaid spec cases pass (27 of them hold no diagram and are listed in
  `SKIPPED.md`), and ten minutes of fuzzing (1,393,960 runs) found no panic.
- A Wardley map is read with Mermaid's Langium grammar, over the shared tokenizer. The model
  holds the items of the map with the axes they are placed on and the evolution they sit at.
- The differential run against Mermaid's own grammar reports no difference over 3,039
  inputs, the 24 Mermaid spec cases pass (4 of them hold no diagram and are listed in
  `SKIPPED.md`), and ten minutes of fuzzing (1,003,196 runs) found no panic.
- A swimlane diagram is a flowchart whose top-level subgraphs are the lanes, and Mermaid draws
  it with the flowchart's own parser and model: the syntax is the flowchart's, and only the
  layout and the drawing of the lanes are of its own. The `swimlane` feature brings the
  `flowchart` one with it, and the settings come from the `flowchart` section, as Mermaid
  documents.
- The lanes are laid out as pele lays them out: every top-level group is a band that runs the
  length of the diagram along the flow, the ranks are shared by all lanes, so a step sits
  level with the steps that happen at the same time in other lanes, and each node stays
  inside its own lane.
- The differential run against Mermaid's own grammar reports no difference over 3,013
  inputs, the 14 Mermaid spec cases pass (2 of them hold no diagram and are listed in
  `SKIPPED.md`), and ten minutes of fuzzing (803,536 runs) found no panic.
- An agent flow diagram is read with Mermaid's jison grammar, through a hand-written lexer that
  walks UTF-16 code units, as the jison lexer does.
- The metadata of an `@{ … }` block is read the way Mermaid reads it: the body is handed to
  js-yaml as it was written, and a block that is written over several lines is retried once
  with the line-trailing commas stripped. A body that parses into something else than a
  mapping holds no metadata, but it is not an error.
- The differential run against Mermaid's own grammar reports no difference over 3,173
  inputs, the 239 Mermaid spec cases pass (79 are listed in `SKIPPED.md`: 72 are sources
  Mermaid itself refuses and 7 hold no diagram listed in `SKIPPED.md`), and ten minutes of
  fuzzing (676,742 runs) found no panic.
- A use case diagram has no grammar file in Mermaid: its parser is written with Chevrotain,
  in a lexer, a parser and a visitor. The lexer and the parser here make the same decisions
  from the same lookahead, so they accept the same text and name the same token in their
  errors.
- The differential run against Mermaid's own grammar reports no difference over 3,105
  inputs, the 82 Mermaid spec cases pass, and ten minutes of fuzzing (672,820 runs) found no
  panic.
- A railroad diagram has four grammars in Mermaid — its own notation, EBNF, ABNF and PEG — and
  each of them turns its tree into the same model: a rule is a name and a tree of nodes. The
  token types are in the order Mermaid's lexer has them, and the first token type that
  matches wins, except that a keyword that a longer terminal also matches is the terminal.
- The differential run against Mermaid's own grammar reports no difference over 3,090
  inputs, the 117 Mermaid spec cases pass (2 of them hold no diagram and are listed in
  `SKIPPED.md`), and ten minutes of fuzzing (676,308 runs) found no panic.

## M7 — Beyond pele 1.9.0

M6 covers every diagram type Mermaid 12.1.0 builds. M7 covers what Mermaid has built in and
pele has not (yet) implemented, and what pele supports through the host and fermaid does not.

Findings that set the scope (checked against the Mermaid clone, version 12.2.0):
- Every diagram Mermaid 12.2.0 registers is already supported, `flowchart-elk` included, so no
  new diagram type is missing today. `mermaid-zenuml` is an external plugin that needs a
  JavaScript library, and is not built in.
- The 19 jison grammars pele vendors are identical to Mermaid 12.2.0's. The Langium grammars
  of `@mermaid-js/parser` are not compared yet (M7a).
- 12.2.0 adds one feature: `mindmap.colorBy: depth`. The rest are layout fixes.
- pele has what fermaid lacks: the `icons` and `math` resolvers, the five built-in icons of
  architecture diagrams, and the `config`, `links` and `images` options.

### M7a — Follow Mermaid 12.2.0

- [x] Compare the Langium grammars (`packages/parser`) with the ones pele ships. There is no
  difference: the built parser of the Mermaid clone is byte for byte the `@mermaid-js/parser`
  2.0.1 that pele uses.
- [x] Mindmap `colorBy: depth`.
- [x] Run `tools/oracle/diff.mjs` against 12.2.0. It uses the grammars pele vendors, which are
  identical to the clone's (all 19 jison files), so nothing new is reported (mind map: 537
  inputs, 0 unexplained).
- [x] Move the target version to 12.2.0 in this file, the READMEs, `NOTICE` and `CLAUDE.md`.
- [ ] A diagram type that a later Mermaid release adds is handled the same way as M6: grammar,
  model, layout, emitter, spec cases, oracle, fuzz target.

Mermaid is the specification here, and pele has no 12.2.0 behavior to compare with.

Notes:
- `colorBy: depth` colors a node by the number of its ancestors, less one, wrapping after
  eleven levels, as `assignSections` of Mermaid does; the indentation does not count. The
  branch color stays in the model as it was, and the depth color is a second field, so the
  setting only changes how the map is drawn.
- Mermaid colors the nodes and the links by the `cScale` theme variables. fermaid has the
  series colors only, so a level takes `--fermaid-series-N` (eight colors, as the branches do).
- The classic and neo looks, and the `git0`/`cScale*` variables, are Mermaid's themes and are
  not ported.

### M7b — Icons and math through the host

- [x] `RenderOptions::icons` and `RenderOptions::math`: optional callbacks, `None` by default,
  so there is still no dependency. Results are cached for one render, so that output stays
  deterministic.
- [x] Decide how host markup is treated: trusted, as pele does, and documented as such. A
  resolver can only be checked for what the library does around its answer: where it is put,
  the room it takes, an unusable size, and that the text of the diagram reaches the output
  only as a name or as TeX handed to the resolver, or escaped.
- [x] `label.rs` makes room for an icon and typesets `$$…$$`, as pele's `text/label.ts` does,
  instead of leaving them out.
- [x] Connect the types: mindmap, kanban, flowchart, swimlane, usecase and C4 for icons, and
  for math also class, ER, requirement, block, state and sequence. Gantt and cynefin draw
  their labels with `label_svg` and needed nothing of their own.
- [x] Tests with a fake resolver (`tests/host.rs`).

Notes:
- pele keeps the resolvers in module state for the render in progress. Rust has none to borrow
  from, so they travel with the text measurer, which every label is laid out with:
  `TextMeasurer::host` (hidden from the documentation) answers with the resolvers of the
  render, and `render` wraps the measurer in one that does. Titles use a measurer that has no
  math. `Span` holds the icon and the typeset math of a span, so a label draws without asking
  the host again.
- Which types read `$$` is a method of the internal `Diagram` trait, `math`.
- An icon the host has not is a dashed frame in a flowchart icon shape, as it was, and an empty
  square for the icon of a use case actor.

### M7c — Built-in icons of architecture diagrams

- [x] `database`, `server`, `disk`, `internet` and `cloud` as pele draws them (the picture is
  pele's, checked by `render-compare.mjs`); any other name goes to `icons`.

### M7d — Options and packaging

- [x] `RenderOptions::config` (the same as `%%{init}%%`, never able to raise a limit), `links`
  and `images` switches, and the matching CLI flags.
- [x] CI builds the library for `wasm32-unknown-unknown`, to keep the promise that the core does
  no I/O.

### M7e — Viewers without CSS variables

- [x] `RenderOptions::palette` (`Palette::light()` / `dark()`) writes the colors as plain values,
  so resvg shows the diagram; CJK sans-serif fallbacks; `--palette` on the command line.

### Release — 0.4.0

- [x] Publish the library, then the command line tool; tag the version and publish the release.

### Decided against

ZenUML (needs a JavaScript library), ELK and tidy-tree layout plugins, the hand-drawn look, and
pele's browser-only `mount`, `zoom` and `lazy`.
