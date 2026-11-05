# Changelog

All notable changes are recorded here. pele-rs follows [Semantic Versioning](https://semver.org);
before 1.0, a minor version may break the API.

## [Unreleased]

### Added

- Rust replay of 178,887 official Pele render calls and 19,308 utility/layout calls, with pinned provenance, explicit exclusions and reproducible fixture capture.
- Official text emphasis options, adaptive rendering, contrast-aware labels and edge casings, built-in vector tree icons, and complete block link chains.

### Changed

- Establish pele-rs documentation and licensing summaries around its independent project identity; archive inherited release notes and design plans while preserving original attribution.
- Align layout, SVG structure, shapes, markers, colors and styles with Pele 1.13.0 at `3a2f075ee0c3d914f0fad3545fc0f193693975ac`.
- Use official `pele-*` CSS classes and `--pele-*` variables instead of `pele-rs-*`; consumers with custom styles must update their selectors.
- `RenderOptions::responsive` and `id_prefix` now use `Option` to distinguish inherited configuration from an explicit override; `monospace` selects label categories.
- Render event-modeling notes/specifications and content-sized Venn diagrams like the official library; Sankey names now accept Unicode.

### Fixed

- Match reference floating-point evaluation in iterative Venn layout, YAML metadata parsing and layered graph ordering.
- Keep the native API dependency-free by default, deterministic and bounded by explicit nesting limits.

## 0.6.0 - 2026-11-05

### Added

- Native Rust rendering library and CLI for Mermaid diagrams.
- Reusable, immutable `ParsedDiagram` models for every diagram type, safe to share across workers.
- Bounded host font measurement caching and optional usvg-backed `FontMeasurer` / `SvgScene` adapters for native painters.
- Original-source error mapping through front matter, comments, directives, entities and line-ending normalization.
- Mermaid chart palette keys, including author colors preserved under literal host palettes.
- Validation of non-finite geometry and invalid host font measurements.

### Fixed

- Subnormal Sankey flows no longer overflow during normalization and silently collapse to zero-width bands.
- EOF diagnostics point at actual source tokens rather than synthetic trailing lines.

### Changed

- Package/import names are `pele-rs` / `pele_rs`; CLI and SVG namespaces are `pele-rs`.
- `scene` is opt-in; default builds retain zero runtime dependencies. This version is distributed through Git only.
