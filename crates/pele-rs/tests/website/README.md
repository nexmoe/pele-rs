# Official website examples

`examples.md` is the unmodified Markdown from https://pele.run/examples.md, downloaded on
2026-11-05. It contains 58 examples across 32 types, including the `Unicode works too: 🌋`
mindmap. The reference website does not list every library type.

`renders.jsonl.gz` records the outputs of the pinned, unmodified official Pele 1.13.0 for
these sources at their natural width and with `maxWidth` 320, 640 and 1024: 232 references.
`manifest.json` records the upstream commit, source hash and counts. Regenerate with:

```sh
node tools/conformance/website.mjs ../pele
cargo test -p pele-rs --test website
```

The test compares SVG text, styles, elements, attributes and geometry using the same
comparison rules as the official suite. It runs without Node, a browser or a host app.
Native emoji fallback is covered separately in `tests/scene.rs` with synthetic font fixtures.

The example sources and reference SVGs originate from Obsidian Pele, Copyright (c) 2026
kepano, under MIT; see `NOTICE` and `LICENSE-MIT` at the repository root. The underlying
Mermaid documentation examples retain their existing upstream attribution in `NOTICE`.
