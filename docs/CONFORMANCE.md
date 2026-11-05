# Official Pele conformance

The reference is Obsidian Pele **1.13.0**, commit
`3a2f075ee0c3d914f0fad3545fc0f193693975ac`, recorded in `upstream.json`.
The official source is kept in a separate checkout, not vendored into the Rust runtime.

## What is compared

The unmodified official non-website Vitest suites pass 6,992 assertions, with 103 skipped and
two TODOs. An external capture plugin records their actual render calls, including stress,
security, randomized inputs, reusable parsed diagrams, custom measurements, icons and math.
After deduplication, the committed corpus has **178,887 render calls**, covering all 33 diagram
types and unsupported input. Another **19,308 utility and layout calls** cover preprocessing,
YAML, URLs, escaping, numeric formatting, style resolution, contrast, palettes and flat/compound
layout, including mutated nodes and edge routes. Every fixture retains its official test origin.

`cargo test --workspace --all-features` replays these fixtures entirely in Rust. Node and the
official checkout are needed only to regenerate them. The original Rust parser compatibility,
security, host integration and snapshot tests remain in place.

The official website's 58 examples have a separate corpus in `tests/website/`: 232 render
references at natural width and `maxWidth` 320, 640 and 1024. `tests/website.rs` compares their
SVG text, styles and geometry directly against the pinned official renderer, including the
volcano emoji mindmap. `tests/scene.rs` independently checks native mixed Latin/emoji font
measurement and embedded color glyphs using redistributable synthetic fonts. Neither test
requires a browser or consuming application.

Successful render comparisons require the same diagram type, links, SVG elements, text,
attributes, classes, colors, styles, markers and geometry. Attribute order and XML quoting are
irrelevant. Coordinates permit 0.011 SVG units for the reference's two-decimal rounding, or a
relative tolerance of 1e-14 for extremely large stress coordinates. Utilities/layout permit
1e-7 absolute or 1e-14 relative numeric error; strings and structures match exactly. Rejected
inputs must return the same error category. Error message wording and diagnostic positions are
not compared by this corpus; original-source diagnostics have separate Rust tests.

This is a public rendering conformance suite, not a translation of every internal JavaScript
assertion. Browser mount/resize/zoom behavior and the website are host responsibilities. Their
render calls are replayed where captured, but their DOM behavior is not implemented here.

## Explicit platform differences

`tests/conformance/inventory.json` records the source pin, counts, all official test names and
statuses, and every excluded captured call. It excludes 8,544 calls:

- 8,471 use lone UTF-16 surrogates, which cannot inhabit a Rust `str`.
- 68 pass JavaScript `NaN` inside a configuration object; the Rust API takes JSON configuration.
- Four test JavaScript module registration before importing any diagrams; Rust uses Cargo features.
- One throws from a JavaScript callback; Rust callbacks use native panic semantics.

`manifest.json` holds portable counts and SHA-256 hashes separately: excluded JavaScript inputs
in the inventory can contain escaped lone surrogates that a Rust JSON string cannot represent.

The native default still enforces a nesting cap of 64. Official deep stress fixtures explicitly
raise that cap and run on 32 MiB test threads. Normal consumers do not need a larger stack.
The deterministic default clock remains the Unix epoch; supply `now` for a live Gantt marker.
Rust's diagnostic columns count Unicode scalar values. Font measurement must match the host's
font and fallback setup for the same visual output; see [native integration](NATIVE.md).

## Reproduce

```sh
git clone https://github.com/obsidianmd/pele ../pele
git -C ../pele checkout 3a2f075ee0c3d914f0fad3545fc0f193693975ac
npm --prefix ../pele ci
node tools/conformance/regenerate.mjs ../pele
cargo test --workspace --all-features
```

Regeneration checks the reference version, commit and tracked source changes before capturing.
It writes compressed JSONL fixtures and an inventory; the temporary raw captures are removed.
Review all fixture and inventory changes when advancing the upstream pin.

For a detailed JSON report of all differences:

```sh
cargo build -p pele-rs --release --example conformance
python3 tools/conformance/compare.py
```

The report is written to the system temporary directory. To inspect selected diagram types,
pass their corpus names, for example `python3 tools/conformance/compare.py flowchart sequence`.

## Repository layout

Like Pele, the Rust library groups parsers, models, layout and renderers under
`crates/pele-rs/src/diagrams/<type>/`, shared SVG/text/layout code under their own modules, and
Mermaid parser cases under `crates/pele-rs/tests/compat/<type>/`. Rust integration tests remain
one file per type. Official public rendering fixtures live in `tests/conformance/`; shared
internal utility/layout replay lives in `src/official_units.rs` behind `cfg(test)`. The CLI is
separate from the library, and browser/website code is not a library dependency.
