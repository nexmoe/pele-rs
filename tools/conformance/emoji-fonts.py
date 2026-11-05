"""Generate original, redistributable fonts for the native emoji fallback regression.
Requires FontTools. These are synthetic test glyphs, not copies of system fonts.
"""
from pathlib import Path
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import newTable
from fontTools.ttLib.tables.sbixStrike import Strike
from fontTools.ttLib.tables.sbixGlyph import Glyph
import struct
import zlib

root = Path(__file__).resolve().parents[2] / 'crates/pele-rs/tests/fonts'


def rectangle():
    pen = TTGlyphPen(None)
    pen.moveTo((50, 0))
    pen.lineTo((950, 0))
    pen.lineTo((950, 800))
    pen.lineTo((50, 800))
    pen.closePath()
    return pen.glyph()


def font(family, postscript, cmap, advance):
    names = ['.notdef', *dict.fromkeys(cmap.values())]
    builder = FontBuilder(1000, isTTF=True)
    builder.setupGlyphOrder(names)
    builder.setupCharacterMap(cmap)
    builder.setupGlyf({name: rectangle() for name in names})
    builder.setupHorizontalMetrics({name: (advance, 50) for name in names})
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupNameTable({'familyName': family, 'styleName': 'Regular',
                           'uniqueFontIdentifier': postscript, 'fullName': family,
                           'psName': postscript, 'version': 'Version 1.0'})
    builder.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
    builder.setupPost()
    builder.font['head'].created = builder.font['head'].modified = 3406620154
    builder.font.recalcTimestamp = False
    return builder.font


# LastResort claims the complete run; its glyphs must never replace real text.
placeholder = font('Last Resort', 'LastResort',
                   {32: 'box', 65: 'box', 66: 'box', 0x1F30B: 'box', 0x1F913: 'box'}, 900)
placeholder.save(root / 'last-resort.ttf')

# An sbix bitmap like Apple Color Emoji, with a synthetic two-color square.
def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))


rows = b''.join(b'\0' + b'\xff\x40\0\xff' * 16 + b'\0\x40\xff\xff' * 16 for _ in range(32))
png = (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 32, 32, 8, 6, 0, 0, 0))
       + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b''))
emoji = font('Pele Test Emoji', 'PeleTestEmoji', {0x1F30B: 'volcano', 0x1F913: 'nerd'}, 1000)
sbix = newTable('sbix')
sbix.version, sbix.flags = 1, 1
strike = Strike(ppem=32, resolution=72)
strike.glyphs = {name: Glyph(glyphName=name, graphicType='png ', imageData=png)
                 for name in ['volcano', 'nerd']}
sbix.strikes = {32: strike}
emoji['sbix'] = sbix
emoji.save(root / 'emoji.ttf')
