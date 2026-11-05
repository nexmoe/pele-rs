# pele-rs

**English** | [繁體中文](README.zh-TW.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md)

An independent derivative of [fermaid](https://github.com/tenyi/fermaid), optimized for native editors. The original Git history, authorship and licenses are retained; this is not an official Obsidian project. Install this version from Git:

```toml
[dependencies]
pele-rs = { git = "https://github.com/nexmoe/pele-rs" }
```

New in this derivative: reusable `parse` / `ParsedDiagram::render` for every diagram type; bounded `CachingMeasurer`; original-source diagnostics; chart palette compatibility; numeric validation; and optional `scene::SvgScene` / `scene::FontMeasurer` for native painters. The `scene` feature uses usvg; default builds remain dependency-free. See [native integration](docs/NATIVE.md) for API boundaries and examples. This version has not been published to crates.io.


pele-rs renders [Mermaid](https://mermaid.js.org) diagrams to SVG in pure Rust — no JavaScript runtime, no headless browser, no Node.js. It is a port of [pele](https://github.com/obsidianmd/pele), a small, dependency-free TypeScript renderer, and targets Mermaid 12.2.0 syntax with pele's own layout and visual style.

> **Status:** alpha. Every diagram type of Mermaid 12.2.0 is supported; the table below says what each one reads. See the [roadmap](docs/ROADMAP.md).

| Diagram type | Status |
|---|---|
| Pie (`pie`) | Supported: `showData`, `title`, `accTitle`, `accDescr`, and the `pie` config keys `textPosition`, `donutHole` and `legendPosition` |
| Flowchart (`flowchart`, `graph`) | Supported: every node shape, including `@{ shape: … }`, all edge kinds and lengths, edge ids, subgraphs with their own direction, `classDef`, `class`, `style`, `linkStyle`, `click` links and tooltips, markdown labels, and the `flowchart` config keys `curve`, `nodeSpacing`, `rankSpacing` and `wrappingWidth` |
| Sequence (`sequenceDiagram`) | Supported: participants and actors of every kind, every arrow kind, activations, notes, `loop`, `alt`, `opt`, `par`, `critical`, `break` and `rect` blocks, boxes, `autonumber`, created and destroyed participants, and the `sequence` config keys. Participant menus (`link`, `links`) are not drawn as links |
| State (`stateDiagram`, `stateDiagram-v2`) | Supported: states with descriptions, composite states with their own direction, concurrency regions, start and end, choice, fork and join, notes, `classDef`, `class`, `style` and `click` |
| Class (`classDiagram`) | Supported: classes with members, generic types and annotations, relations with markers and cardinalities, namespaces, notes, lollipop interfaces, `classDef`, `cssClass`, `style`, `link`, `click` and `callback` |
| ER (`erDiagram`) | Supported: entities with attributes, keys and comments, relationships with crow's foot cardinalities, aliases, subgraphs, `classDef`, `class` and `style` |
| Requirement (`requirementDiagram`) | Supported: requirements of every kind and elements with their fields, relationships, `classDef`, `class` and `style` |
| Gantt (`gantt`) | Supported: tasks with dates, durations, `after` and `until`, sections, `done`, `active`, `crit`, `milestone` and `vert`, `excludes`, `includes` and `weekend`, `dateFormat`, `axisFormat`, `tickInterval`, `weekday`, `todayMarker`, `topAxis`, compact mode and `click` links. Dates are UTC, and today is `RenderOptions::now` |
| Timeline (`timeline`) | Supported: title, sections, periods with their events |
| User journey (`journey`) | Supported: title, sections, tasks with scores and actors |
| Kanban (`kanban`) | Supported: columns and cards with shapes, `@{ … }` metadata (`assigned`, `ticket`, `priority`, `label`), classes, and ticket links through `ticketBaseUrl` |
| Mind map (`mindmap`) | Supported: every node shape, icons, classes, and branch colors (by branch, or by depth with `colorBy`); laid out across, or down for a narrow host |
| Sankey (`sankey`, `sankey-beta`) | Supported: the CSV form of Mermaid, `source,target,value` per line, with quoted fields; and the `sankey` config keys `width`, `height`, `nodeWidth`, `nodePadding`, `nodeAlignment`, `nodeColors`, `linkColor`, `showValues`, `prefix`, `suffix` and `labelStyle` |
| XY chart (`xychart`, `xychart-beta`) | Supported: `title`, `x-axis` with categories or a range, `y-axis` with a range, `line` and `bar` series with point labels, `accTitle` and `accDescr`, the `horizontal` and `vertical` orientation, and the `xyChart` config keys `width`, `height`, `chartOrientation`, `showTitle`, `showLegend`, `showDataLabel`, `xAxis.showTitle`, `xAxis.showTick`, `xAxis.showLabel`, `xAxis.showAxisLine` and the same four under `yAxis` |
| Quadrant chart (`quadrantChart`) | Supported: `title`, `x-axis` and `y-axis` with a `-->`, `quadrant-1` to `quadrant-4`, points with `radius`, `color`, `stroke-color` and `stroke-width`, `classDef` and the classes of a point, `accTitle` and `accDescr`; and the `quadrantChart` config keys `chartWidth`, `chartHeight`, `titlePadding`, `xAxisLabelPadding`, `yAxisLabelPadding`, `xAxisPosition`, `yAxisPosition`, `quadrantPadding`, `quadrantTextTopPadding`, `pointRadius` and `pointTextPadding` |
| Git graph (`gitGraph`) | Supported: `title`, `accTitle` and `accDescr`, `commit` with an `id`, a `msg`, `tag`s and a `type` of `NORMAL`, `REVERSE` or `HIGHLIGHT`, `branch` with an `order`, `checkout` and `switch`, `merge` and `cherry-pick` with a `parent`; and the `gitGraph` config keys `mainBranchName`, `mainBranchOrder`, `showCommitLabel`, `showBranches`, `rotateCommitLabel` and `parallelCommits` |
| Block (`block`, `block-beta`) | Supported: blocks in a grid with `columns`, composites with `block:` and `end`, `space`, every shape of Mermaid, block arrows with their directions, links with their labels and their arrow ends, `classDef`, `class` and `style`; and the `block` config key `padding` |
| Architecture (`architecture`, `architecture-beta`) | Supported: services in groups, with the edges between them and their labels, junctions, `service`, `group` and `junction` with their icons; and the `architecture` config keys `iconSize`, `padding` and `fontSize`. a service is an icon square with its title beneath; `database`, `server`, `disk`, `internet` and `cloud` are built in, other names go to the `icons` option |
| Packet (`packet`, `packet-beta`) | Supported: `title`, `accTitle` and `accDescr`, rows of fields with their bit ranges, a field of `+bits`; and the `packet` config keys `rowHeight`, `bitWidth`, `bitsPerRow`, `showBits`, `bitOrder`, `paddingX` and `paddingY` |
| Radar chart (`radar-beta`) | Supported: `title`, `accTitle` and `accDescr`, `axis` and `curve` with their entries, and the options `showLegend`, `ticks`, `min`, `max` and `graticule`; and the `radar` config keys `width`, `height`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `axisScaleFactor`, `axisLabelFactor` and `curveTension` |
| Treemap (`treemap`) | Supported: a tree of sections and leaves with their values, and `classDef` and `class` |
| C4 diagram (`C4Context`, `C4Container`, `C4Component`, `C4Dynamic`, `C4Deployment`) | Supported: `title`, `accTitle` and `accDescr`, persons, systems, containers and components with their boundaries and their relationships, and the `UpdateLayoutConfig` statement |
| Use case diagram (`usecase-beta`) | Supported: `title`, `accTitle` and `accDescr`, actors and use cases with the relations between them; and the `usecase` config keys `nodeSpacing` and `rankSpacing` |
| Information diagram (`info`) | Supported: `title`, `accTitle` and `accDescr`, the version and the lines of text |
| Venn diagram (`venn-beta`) | Supported: the circles with their labels; and the `venn` config keys `width`, `height` and `padding` |
| Tree view diagram (`treeView-beta`) | Supported: the tree drawn with box-drawing characters, the icons in front of a label, `classDef` and `class`; and the `treeView` config keys `rowIndent`, `paddingX`, `paddingY`, `lineThickness`, `showIcons`, `defaultIconPack`, `filenameIcons` and `extensionIcons` |
| Cynefin diagram (`cynefin-beta:`) | Supported: `title`, `accTitle` and `accDescr`, the items in the four regions and the transitions between them; and the `cynefin` config keys `width`, `height`, `showDomainDescriptions`, `boundaryAmplitude` and `seed` |
| Ishikawa diagram (`ishikawa`) | Supported: the causes on the levels and the spine |
| Event modeling diagram (`eventmodeling`) | Supported: the lanes, the frames, the relations and the notes |
| Wardley map (`wardley-beta`) | Supported: `title`, `accTitle` and `accDescr`, the items with their axes and their evolution; and the `wardley` config keys `width`, `height` and `showGrid` |
| Railroad diagram (`railroad-beta`, `railroad-ebnf-beta`, `railroad-abnf-beta`, `railroad-peg-beta`) | Supported: the four notations of Mermaid, with their rules and their terminals; and the `railroad` config keys `padding`, `verticalSeparation`, `horizontalSeparation`, `arcRadius`, `fontSize`, `strokeWidth`, `showMarkers` and `markerRadius` |
| Swimlane diagram (`swimlane-beta`) | Supported: the flowchart syntax, with the top-level subgraphs drawn as the lanes. The settings come from the `flowchart` section, as Mermaid documents |
| Agent flow diagram (`agentflow-beta`) | Supported: `title`, `accTitle` and `accDescr`, agents, containers and the edges between them, `classDef` and `class`; and the `agentflow` config keys `nodeSpacing`, `rankSpacing`, `wrappingWidth` and `minNodeWidth` |

## Goals

- Pure Rust and synchronous, with no I/O. Default builds have no runtime dependencies; the optional scene adapter uses usvg.
- Mermaid syntax compatibility, with its own clean layout and visual style.
- Deterministic output: the same input and options give byte-identical SVG.
- Themeable output: colors and fonts are CSS variables in the SVG.
- Safe for untrusted input: escaped text, filtered links and styles, size and nesting limits.

## Library

```rust
let options = pele_rs::RenderOptions {
    font_size: Some(14.0),
    ..Default::default()
};
match pele_rs::render("pie\n  \"Rust\": 70\n  \"Other\": 30", &options) {
    Ok(rendered) => println!("{}", rendered.svg),
    Err(e) => eprintln!("{}:{}: {}\n{}", e.line, e.column, e.message, e.snippet),
}
```

`render` returns the SVG with its size and links, or an `Error` with a code (`Unsupported`, `Syntax`, `Semantic`, `Limit`), a line and column, and a snippet that points at the problem. `detect_type` tells the diagram type without rendering. Set `max_width` to the width the host has, and a diagram that can draw itself narrower does (a pie chart moves its legend below the chart). Links from `click` statements are sanitized and kept only when their scheme is allowed (`link_schemes`, by default `http`, `https`, `mailto` and `tel`); a flowchart that runs across turns to run down when it does not fit `max_width`. Each diagram type is a Cargo feature (`flowchart`, `pie`, …); the default feature `all` includes every type. A gantt chart dates "today" from `now`, in milliseconds since 1970-01-01 UTC; it defaults to 1970-01-01 so that output never depends on the clock.

Icons and math come from the host, as pele-rs has neither an icon set nor a typesetter. `RenderOptions::icons` gives the SVG markup of an icon by its name (`fa:fa-car` in a label, the `icon` of a node), and `RenderOptions::math` typesets the TeX between `$$` in the labels of flowcharts, swimlanes, state, class, ER, requirement, block, sequence, use case, kanban and mind map diagrams. Without them an icon is left out and the TeX stays as written. What the host returns is put into the SVG as it is, so it has to come from a source you trust.

## Command line

```sh
cargo install --git https://github.com/nexmoe/pele-rs pele-rs-cli
cargo install --path crates/pele-rs-cli  # from a checkout
pele-rs diagram.mmd -o diagram.svg
cat diagram.mmd | pele-rs --font-size 14 --max-width 360 --id-prefix fig1 > diagram.svg
pele-rs plan.mmd --now 1700000000000 -o plan.svg   # a gantt chart with today's marker
pele-rs page.mmd --config '{"theme": "dark"}' --no-links --no-images -o page.svg
pele-rs page.mmd --palette dark -o page.svg   # plain colors, for resvg and other viewers without var()
```

## Theming

The SVG takes its colors and fonts from CSS custom properties, with built-in fallbacks, so a page can re-theme a diagram without rendering it again:

```css
svg {
  --pele-rs-bg: #1e1e1e;
  --pele-rs-fg: #dcddde;
  --pele-rs-line: #999;
  --pele-rs-accent: #7f6df2;
  --pele-rs-font: "Inter", sans-serif;
}
```

Available variables: `--pele-rs-bg`, `--pele-rs-fg`, `--pele-rs-muted`, `--pele-rs-line`, `--pele-rs-surface`, `--pele-rs-surface-alt`, `--pele-rs-border`, `--pele-rs-accent`, `--pele-rs-radius`, `--pele-rs-font`, `--pele-rs-font-mono`, and `--pele-rs-series-1` to `--pele-rs-series-8` for the colors of chart series such as pie slices.

### Viewers without CSS variables

resvg, librsvg and other SVG renderers do not read `var()`, so a default SVG is drawn black in them. Set `RenderOptions::palette` and every color is written out as a plain value, with no `var()` left in the SVG (the output stays deterministic, and without a palette it is exactly as before):

```rust
let options = pele_rs::RenderOptions {
    palette: Some(pele_rs::Palette::dark()), // or Palette::light(), or your own colors
    ..Default::default()
};
```

A `Palette` has the colors of the variables above (`bg`, `fg`, `muted`, `line`, `surface`, `surface_alt`, `border`, `accent`, `critical` and the eight `series`); a color may hold only letters, digits and `# % . , ( ) -`, and any other is replaced by the light one. The same mode also names sans-serif CJK fonts (Noto Sans CJK, PingFang, Microsoft JhengHei, …) after the generic family, since resvg would otherwise draw CJK text in a serif font; the text widths assume that a CJK character is one em wide. On the command line it is `--palette light|dark`.

## Development

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all
cargo +nightly fuzz run render    # needs cargo-fuzz; every diagram type has a target of its own
cargo xtask compat pie --pele ../pele   # regenerates the Mermaid spec cases from a pele checkout
node tools/oracle/golden.mjs ../pele flowchart   # regenerates what pele makes of them
node tools/oracle/diff.mjs ../pele   # differential fuzzing against Mermaid's own parsers
```

The minimum supported Rust version is 1.85.

## Credits

pele-rs's algorithms, text metrics and limits are ported from [pele](https://github.com/obsidianmd/pele) by kepano (MIT), from the Obsidian team. The diagram syntax and the parser specifications used as tests come from [Mermaid](https://github.com/mermaid-js/mermaid) (MIT). See [NOTICE](NOTICE).

## License

Multi-licensed under your choice of:
- [MIT](LICENSE-MIT)
- [Apache-2.0](LICENSE-APACHE)
- [BSD-3-Clause](LICENSE-BSD3)
- [0BSD](LICENSE-0BSD)

See [LICENSE](LICENSE) and [NOTICE](NOTICE) for details.
