# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

pele-rs is a pure-Rust library that renders [Mermaid](https://mermaid.js.org) diagram text to SVG — no JavaScript runtime, no headless browser, no Node.js. It is inspired by [obsidianmd/pele](https://github.com/obsidianmd/pele), a small, synchronous, dependency-free TypeScript Mermaid-to-SVG renderer. Pele is the main reference for behavior and scope, and Mermaid's own behavior is the specification:

- Targets Mermaid 12.2.0 syntax, but with its own layout and visual style (pixel-identical output to Mermaid.js is not a goal).
- Each diagram type is a self-contained unit (parser + layout + SVG emitter) that can be registered on its own; the full entry point includes all types.
- Colors and fonts come from CSS variables in the emitted SVG (pele uses `--pele-bg`, `--pele-fg`, `--pele-line`, `--pele-accent`, `--pele-series-1..8`, …), so output can be re-themed without re-rendering.
- Untrusted input is a first-class concern: escape all diagram text, filter links and inline styles, and cap source and output size.
- Parsers are validated against Mermaid's own parser specs and with differential fuzzing.

Read `docs/NATIVE.md` for the current public API. `docs/ROADMAP.md` is the historical fermaid plan; its release claims refer to that upstream project. Key points:

- Algorithms, text metrics and limits are ported from Pele 1.13.0 (`3a2f075`), written as idiomatic Rust (not a line-by-line translation). When behavior is in doubt, run pele's TypeScript sources and compare.
- Official Pele 1.13.0 is the rendering and style reference. `tests/official.rs` and `src/official_units.rs` enforce the pinned baseline; see `docs/CONFORMANCE.md` for platform differences. Do not silently replace a Pele behavior with a Mermaid.js behavior. Historical syntax-only divergences are recorded in `tools/oracle/KNOWN.md`; verify them against the current official corpus before changing a parser.
- Zero runtime dependencies in the default core; optional `scene` uses usvg. `std`, MSRV 1.85 (no APIs stabilized later). One Cargo feature per diagram type; `default = ["all"]`.
- SVG classes and CSS variables use the official `pele-*` / `--pele-*` namespace.
- Output must be deterministic (byte-identical for identical input and options).
- An explicit nesting depth limit is required, because a Rust stack overflow aborts and cannot be caught.

## Architecture

M0 (foundation), M1 (pie), M2 (flowchart), M3 (sequence, differential fuzzing), M4 (state, class, ER, requirement), M5 (gantt, timeline, journey, kanban, mindmap) and M6 (sankey, xychart, quadrant chart, git graph, block, architecture, packet, radar, treemap, c4, usecase, info, venn, treeview, cynefin, ishikawa, eventmodeling, wardley, railroad, swimlane, agentflow) are done: every diagram type of Mermaid 12.1.0 is supported.

- `crates/pele-rs` — the library. `render` checks the source limit, runs `preprocess`, detects the type (`detect.rs`), looks it up (`diagram.rs`, an internal `Diagram` trait; one module per type, feature-gated), renders it and checks the output limit.
  - `diagrams/` — one module per type behind its feature (`pie/`: `parser.rs`, `model.rs`, `render.rs`, and `compat.rs`, which checks the generated Mermaid spec cases; `flowchart/`: a jison-style `lexer.rs` over UTF-16 code units, `parser.rs`, `db.rs` (the model), `graph.rs` (what is drawn), `shapes.rs` (shape names), `render.rs`, `compat.rs`; `sequence/`: `lexer.rs` (UTF-16, as flowchart), `parser.rs` (a flat statement list), `db.rs`, `colors.rs` (CSS color names for boxes), `actors.rs` (participant shapes), `render.rs`, `compat.rs`; `state/`, `class/`, `er/` and `requirement/` follow the same split of `lexer.rs`, `parser.rs`, `db.rs`, `graph.rs`, `render.rs` and `compat.rs`, with `markers.rs` for class and ER line ends; `timeline/` and `journey/` have `lexer.rs`, `parser.rs`, `db.rs`, `render.rs` and `compat.rs`; `kanban/` and `mindmap/` only `db.rs`, `render.rs` and `compat.rs`, over the shared outline parser; `gantt/` has `lexer.rs`, `parser.rs`, `db.rs` (scheduling), `dates.rs` (JavaScript's `Date` in UTC, dayjs formats and strict parsing), `legacy_date.rs` (V8's own date parser, which reads what `new Date(text)` reads), `date_cases.rs` (the dates it reads), `axis.rs` (d3's time ticks and `axisFormat`), `render.rs` and `compat.rs`; `sankey/` has `lexer.rs` (the CSV fields of Mermaid's grammar), `parser.rs`, `db.rs`, `layout.rs` (the columns, the bands and the settling), `render.rs` and `compat.rs`; `xychart/` has `lexer.rs` (the tokens of Mermaid's grammar, over UTF-16 code units), `parser.rs`, `db.rs`, `ticks.rs` (d3's time ticks), `render.rs` and `compat.rs`; `quadrantchart/` has `lexer.rs`, `parser.rs`, `db.rs`, `render.rs` and `compat.rs`; `git/` has `parser.rs` (the tokens of Mermaid's Langium grammar, with the keywords and the names they start with), `db.rs` (the commits, the branches and the lanes), `render.rs` and `compat.rs`. `block/` has `lexer.rs` (Mermaid's jison lexer, with its states for strings, shapes, arrows, classes and styles), `parser.rs` (Mermaid's jison grammar), `db.rs` (the blocks, the links and the classes), `layout.rs` (the grid of a composite), `arrows.rs` (the block arrows), `render.rs` and `compat.rs`. `architecture/` has `parser.rs` (Mermaid's Langium grammar, read by hand), `db.rs` (the services, the groups and the edges), `layout.rs` (the grid of the services) and `render.rs`. `packet/` has `parser.rs`, `tokens.rs` (its own tokenizer, as the shared one is not compiled for it), `db.rs` (the rows of fields), `text.rs`, `render.rs` and `compat.rs`. `radar/` has `parser.rs`, `db.rs` (the axes, the curves and the options) and `render.rs`. `treemap/` has `parser.rs`, `db.rs` (the tree), `layout.rs` (the squarified treemap), `format.rs` and `render.rs`. `c4/` has `tokens.rs` (Mermaid's C4 grammar, read by hand), `parser.rs`, `db.rs` (the shapes, the boundaries and the relations), `layout.rs` (the placement of the boundaries), `route.rs` (the paths of the relations) and `render.rs`. `usecase/` has `tokens.rs` (Mermaid's Chevrotain lexer, read by hand), `parser.rs`, `db.rs` (the actors, the use cases, the boundaries and the notes), `graph.rs` (what is drawn), `json.rs` (the JSON tables) and `render.rs`. `info/` has `tokens.rs` (Mermaid's Langium grammar, read by hand), `parser.rs`, `db.rs` and `render.rs`. `venn/` has `lexer.rs` (Mermaid's jison grammar), `parser.rs`, `db.rs` (the sets), `layout.rs` (the circles) and `render.rs`. `treeview/` has `lexer.rs` (Mermaid's Langium grammar, read by hand), `parser.rs`, `db.rs` (the nodes), `box_drawing.rs` (the tree of box-drawing characters), `icons.rs` (the icons in front of a label) and `render.rs`. `cynefin/` has `lexer.rs` (Mermaid's Langium grammar, read by hand), `parser.rs`, `db.rs` (the domains and the transitions), `boundaries.rs` (the four regions) and `render.rs`. `ishikawa/` has `lexer.rs` (Mermaid's jison grammar), `parser.rs`, `db.rs` (the causes and the spine) and `render.rs`. `eventmodeling/` has `lexer.rs` (Mermaid's Langium grammar, read by hand), `parser.rs`, `db.rs` (the lanes, the frames and the events), `summary.rs` (the summary of a frame) and `render.rs`. `wardley/` has `parser.rs` (Mermaid's Langium grammar, over the shared tokenizer), `db.rs` (the items and the axes) and `render.rs`. `railroad/` has `parser.rs` (Mermaid's four Langium grammars, over the shared tokenizer), `db.rs` (the rules and their parts) and `render.rs`. `swimlane/` reads the flowchart's own parser and model, and adds `lanes.rs` (the lanes), `shapes.rs` and `render.rs`. `agentflow/` has `lexer.rs` (Mermaid's jison lexer, with its states for strings and metadata), `parser.rs`, `db.rs` (the agents, the containers and the edges), `graph.rs` (what is drawn), `metadata.rs` (the YAML of an `@{ … }` block, read the way Mermaid reads it with js-yaml), `shapes.rs` and `render.rs`. `common/ordered.rs` is an insertion-ordered map, as a JavaScript `Map`; `common/generics.rs` reads `~T~` generic types. `common/tokens.rs` is the tokenizer and `Reader` for Langium-defined types (pele's regex token types as hand-written matchers); `common/fit.rs` fits a chart to `RenderOptions::max_width`; `common/literal.rs` breaks long words when a diagram is squeezed; `common/golden.rs` holds helpers of the golden compat tests. `common/scan.rs` is a small tokenizer for jison-style line grammars; `common/chars.rs` holds the characters of a source, as the jison lexers read them on UTF-16 code units; `common/plain.rs` parses the line grammars of timeline and journey; `common/outline.rs` parses the indented outlines of kanban and mindmap.
  - `layout/` — `layered.rs` (pele's Sugiyama layout of one flat graph) and `compound.rs` (groups laid out on their own and placed in their parents, with ports where edges cross borders), and `components.rs` (disconnected parts wrapped into rows, for class diagrams). The arithmetic keeps pele's order, so coordinates match pele's exactly.
  - `url.rs` — link sanitizing and the scheme allowlist; `unicode.rs` — the `\p{L}` table jison lexers use.
  - `preprocess/` — `%%` comments, front matter (`yaml.rs`, a YAML subset), `%%{init}%%` (`json.rs`, strict JSON).
  - `text/` — `Style`, `TextMeasurer`, `MetricsMeasurer` (pele's Arial tables decoded at compile time), `entities.rs` (entity codes), `label.rs` (label parsing, measuring and wrapping; a port of pele's `text/label.ts` with host-provided math and icons).
  - `svg/` — escaping, number formatting, label `<text>` output, theme CSS variables and style resolution, the document root and titles, node `shapes.rs`, `edges.rs` (paths, loops), `markers.rs` (arrowheads) and `cluster.rs` (subgraph titles) and `resolve.rs` (writes the CSS variables out as plain colors when `RenderOptions::palette` is set, for resvg).
  - `error.rs` (`Error` with code, kind, line, column, snippet), `options.rs` (`RenderOptions`, `Limits`, `Rendered`, `Link`), `util.rs` (JavaScript-compatible whitespace, trimming and rounding).
  - `tests/` — one file per type (`pie.rs`, `flowchart.rs`, `sequence.rs`, `state.rs`, `class.rs`, `er.rs`, `requirement.rs`, `timeline.rs`, `journey.rs`, `kanban.rs`, `mindmap.rs`, `gantt.rs`, `sankey.rs`, `xychart.rs`, `quadrantchart.rs`, `git.rs`, `block.rs`, `architecture.rs`, `packet.rs`, `radar.rs`, `treemap.rs`, `c4.rs`, `usecase.rs`, `info.rs`, `venn.rs`, `treeview.rs`, `cynefin.rs`, `ishikawa.rs`, `eventmodeling.rs`, `wardley.rs`, `railroad.rs`, `swimlane.rs`, `agentflow.rs`) (render, security, limits, speed, insta snapshots in `snapshots/`), `options.rs` (the options every type reads: the nesting limit, the fonts, the fitting, the link allowlist), `support/mod.rs` (inert-output check, hostile payloads, mutator; shared by the type tests), `compat/<type>/` (generated `cases.rs`, for most types also the generated `expected.rs`, and a hand-written `SKIPPED.md`).
- `crates/pele-rs-cli` — the `pele-rs` binary, hand-rolled argument parsing.
- `xtask/` — `cargo xtask compat <type> --pele <DIR>` converts pele's `tests/compat/<type>` specs into `cases.rs`.
- `tools/oracle/` — dev-only Node scripts; each needs a pele checkout with `npm ci` done. `golden.mjs` writes what the reference makes of each compat case, as `expected.rs`. `diff.mjs` is the differential run against Mermaid's own parsers (and `@braintree/sanitize-url` for links); pele-rs's side is the ignored `oracle_dump` test of each type, and intended differences are in `KNOWN.md`. `dates.mjs` writes the dates V8's own date parser reads, for the gantt chart dates. `render-compare.mjs` draws the same charts with pele and with pele-rs and compares the two pictures, which is how the layout port is checked. `lib.mjs` holds the pele bundle loader and the model summaries the older types share. `summaries/<type>.mjs` holds the summary and the reference of one type, so that a new type needs no change in the scripts: it exports `<type>Summary`, `<type>Modules`, `<type>Keyword`, `<type>Fragments` and `<type>Reference`.
- `fuzz/` — cargo-fuzz targets (nightly, outside the workspace).
- `.github/workflows/ci.yml` — fmt, clippy, tests on stable and 1.85; a commit of prose alone skips the run. `fuzz.yml` runs the fuzz targets nightly and on demand, `oracle.yml` runs `diff.mjs` nightly and on demand.

Update this file as real architecture lands.

## Commands

```sh
cargo build
cargo test                       # all tests
cargo test <name_substring>      # single test / filter
cargo test -- --nocapture        # show println! output
cargo clippy --all-targets -- -D warnings
cargo fmt --all
cd fuzz && cargo +nightly fuzz run render -- -max_total_time=60   # also: pie, flowchart, sequence, state, class, er, requirement, timeline, journey, kanban, mindmap, gantt, sankey, xychart, quadrantchart, git, block, architecture, packet, radar, treemap, c4, usecase, info, venn, treeview, cynefin, ishikawa, eventmodeling, wardley, railroad, swimlane, agentflow
cargo xtask compat pie --pele ../pele   # regenerate spec cases from a pele checkout
node tools/oracle/golden.mjs ../pele sequence   # regenerate expected.rs (every type; the reference of a type is in tools/oracle/summaries/)
node tools/oracle/diff.mjs ../pele              # differential run against Mermaid's parsers
node tools/oracle/dates.mjs                    # regenerate the dates V8 reads (gantt)
node tools/oracle/render-compare.mjs ../pele xychart   # compare the drawn charts with pele's (xychart, quadrantchart, git)
```

Rust edition is 2024.

The CI runs two toolchains: the latest `stable`, which moves, and `1.85`, the minimum supported
Rust version, which is pinned on purpose. The `stable` job is the one that finds new lints, so
keep the local `stable` in step with it: run `rustup update stable` before trusting a clean
`cargo clippy`, and run the gates again on `1.85` when a change touches the library.

## Language and documentation rules

This is an open-source project for a worldwide audience.

- Code, comments, `CLAUDE.md`, commit messages, issues/PR text, and `README.md` are written in **English**.
- Translated READMEs live alongside it: `README.zh-TW.md` (Traditional Chinese), `README.zh-CN.md` (Simplified Chinese), `README.ja.md` (Japanese). When `README.md` changes in substance, update all three translations in the same change and keep the language-switcher line at the top of each file in sync.

## pele-rs integration changes

`parsed.rs` owns immutable parsed models for every enabled type; diagram modules implement `Diagram::parse` and retain model-render closures. Gantt and agent-flow render from cloned mutable models. `source_map.rs` maps diagnostics lazily. `palette.rs` applies author colors before host theming. `text/cache.rs` provides bounded host measurement caching. `scene` is optional and adds usvg; it is an SVG adapter, not a layout-native IR or a GPUI backend. Default builds must remain std-only. Keep `upstream.json`, legal notices and all translated README introductions accurate.
