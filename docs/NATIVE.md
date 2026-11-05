# Native editor integration

pele-rs is an independent derivative of [fermaid 0.5.1](https://github.com/tenyi/fermaid),
retaining its Git history and legal notices. The upstream layout reference remains Pele 1.9.0;
this is not an official Obsidian project or a claim of full parity with a newer Pele release.
The package is `pele-rs`, the Rust import is `pele_rs`, and the binary is `pele-rs`.
Version 0.6.0 is available from Git; it has not been published to crates.io.

## Parse and render separately

```rust
use pele_rs::{parse, ParseOptions, RenderOptions};

let diagram = parse("flowchart LR\nA[中文] --> B[Editor]", &ParseOptions::default())?;
let options = RenderOptions { max_width: Some(640.0), ..Default::default() };
let rendered = diagram.render(&options)?;
# Ok::<(), pele_rs::Error>(())
```

Every diagram type has an owned parsed model. `ParsedDiagram` is `Clone + Send + Sync`:
clones share the model and source; render callbacks and fonts belong to each call. Gantt and
agent-flow rendering work on copies of mutable scheduling/model data. Repeated renders cannot
change the cached model, including when widths or dates differ on concurrent workers.

Limits and host JSON configuration belong to `ParseOptions`. A parsed diagram accepts new
font, width, palette, link, image and time settings through `RenderOptions`. Changing effective
parse configuration or tightening structural limits requires parsing again and returns an
explicit error. Output and source limits are checked on every render.

## Cache at the right stage

Keep these objects separately in the application:

- Parsed models, keyed by source, parsing configuration and structural limits.
- SVG output or `SvgScene`, keyed by the model, layout width, fonts, appearance, resolvers and
  relevant clock input. Resolver outputs must be stable or have a host-controlled revision.
- Raster images, keyed by SVG/scene revision, device pixel ratio and raster scale.

Zoom and pan do not change the diagram model. Reusing SVG avoids re-running layout when only
the raster resolution changes. Reusing a scene avoids re-parsing SVG as well. Hosts decide
whether a viewport resize reflows a diagram or only fits an existing layout. Keep editor block
height stable while a job runs, retain the previous successful preview, and discard stale jobs.
These are view responsibilities and are not global state in this library.

## Match font measurement to painting

The default `MetricsMeasurer` is deterministic, dependency-free and approximate. A native
host can implement `TextMeasurer` with its own shaper. `CachingMeasurer::new(measurer, 4096)`
provides a bounded FIFO cache keyed by text, font size and emphasis. Use a separate cache for
each font/fallback configuration and clear it when fonts change. Capacity zero disables caching.
The host callback runs outside the lock; invalid widths are not cached and a render that
receives an invalid width returns `ErrorCode::Limit`.

With the optional `scene` feature, `scene::FontMeasurer` uses usvg's shaping and fallback rules,
matching resvg. The host supplies an `Arc<usvg::fontdb::Database>` loaded from its own font bytes.
No font discovery or file loading is performed by pele-rs. Wrap this measurer in
`CachingMeasurer`: a cache miss parses a small text-only SVG, while hits reuse its advance.
Use the same family names and font database for layout, scene construction and painting.

## Optional vector scene adapter

Enable `features = ["scene"]` to use `ParsedDiagram::scene` or `scene::SvgScene::new`.
This feature adds usvg as an optional dependency; default builds still have no runtime
dependencies. The adapter converts finished SVG into a normalized, reusable vector tree.
It is an SVG-based adapter, **not a layout-native IR**, and does not include a GPUI renderer.

`SvgScene::tree()` exposes groups, paths, text, transforms, clipping, gradients and opacity.
Text has both positioned glyphs and flattened outlines through usvg. A painter must preserve
the group semantics and paint at the target zoom/device scale; stretching a cached texture or
path mask can still blur it. `rendered()` retains the SVG export and dimensions.

Scenes resolve CSS variables to concrete colors. Pass a palette when rendering for a dark or
custom appearance. Scenes containing text require a nonempty host font database. Images and
HTML math are rejected explicitly; SVG math and host-provided SVG icons are supported. Resource
resolvers are disabled, so scene construction never reads external files or URLs.

A future layout-native scene can eliminate SVG serialization and parsing. That requires
changing each diagram's geometry emitter; the current adapter deliberately makes no such claim.

## Diagnostics and compatibility

Parser diagnostics are mapped back through front matter, init directives, comment removal,
entity placeholders and normalized line endings. The mapping is built only on errors.
Columns count Unicode scalar values, as the inherited Rust API does. EOF errors point at the
last diagram token instead of an artificial appended blank line.

Mermaid author palette keys `pie1..8`, `git0..7`, `cScale0..7` (timeline, radar, treemap),
`cScale1..8` (mindmap) and `venn1..8` are applied before host palette resolution. Colors are
filtered before reaching SVG; labels containing variable-like text are left untouched.
SVG classes and CSS variables use the `pele-rs` prefix.

The existing snapshots, parser compatibility cases and differential harness are retained.
The fixed parser/layout reference is recorded in `upstream.json`; updating it is a deliberate
compatibility task. The historical upstream roadmap is retained separately in `ROADMAP.md`.

## Verification

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo test -p pele-rs --no-default-features
cargo run -p pele-rs --release --example reuse_bench
```

The benchmark includes full SVG output, warms the code paths and reports the median of seven
batches. It compares one-shot rendering with reuse on the same machine and source; it does not
measure GPUI frame times or imply a speedup for every diagram type.
