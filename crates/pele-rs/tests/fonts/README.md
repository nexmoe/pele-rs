The tiny Pele Test font is an original test fixture generated with FontTools. Each glyph is a rectangle with a 600-unit advance. It contains no third-party font outlines and is covered by this repository's licenses. It makes font and scene tests independent of system fonts.

`last-resort.ttf` and `emoji.ttf` are also original synthetic fixtures. The first mimics a
placeholder font that claims both Latin and emoji; the second has 1,000-unit emoji advances
and embedded two-color PNG glyphs in an sbix table. Loading the placeholder before the real
emoji font reproduces macOS's fallback failure without bundling proprietary system fonts.
Regenerate these two fonts with `python3 tools/conformance/emoji-fonts.py` (FontTools required).
