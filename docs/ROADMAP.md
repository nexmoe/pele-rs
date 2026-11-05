# pele-rs roadmap

pele-rs is maintained as an independent native Rust diagram renderer. Its scope is Mermaid
syntax, Pele-compatible SVG output, reusable parsing and rendering, and integration with
native editors. It does not provide a JavaScript runtime, browser UI or GPUI-specific backend.

## Available today

- All 33 diagram types, with a Cargo feature for each type.
- Reusable immutable parsed models, host text measurement and bounded measurement caching.
- Original-source diagnostics, resource limits and deterministic rendering.
- Themeable SVG and literal palettes for renderers such as resvg.
- Optional usvg-backed vector scenes and matching host font measurement.
- Pinned official conformance fixtures, parser compatibility tests, fuzzing and snapshot tests.
- A separate command-line tool, with default library builds free of runtime dependencies.

See [native integration](NATIVE.md) for API boundaries and [conformance](CONFORMANCE.md) for
the tested reference version, numeric tolerances and platform differences. The current
development version is distributed through Git; it has not been published to crates.io.

## Priorities

These are development directions, not release promises:

1. Track official Pele changes through deliberate reference-pin updates and reviewed fixture
   differences, keeping rendering and style compatibility measurable.
2. Improve native font shaping, fallback and scene integration while preserving explicit
   ownership of fonts, caches and resource loading in the host application.
3. Measure parse, layout and output costs separately; improve large-diagram behavior without
   weakening source, output or nesting limits.
4. Stabilize the public API, document migrations and prepare reproducible package releases
   when the API and compatibility guarantees are ready.

## Maintenance

Keep the supported Rust version, feature combinations, legal notices and translated READMEs
in sync. Record shipped changes in [CHANGELOG.md](../CHANGELOG.md), and retain original
copyright and source provenance in [NOTICE](../NOTICE).
