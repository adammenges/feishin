#!/usr/bin/env python3
"""Make two revisions of one real font face for cache/reload regression checks.

Requires the same fonttools[woff]==4.61.1 as the existing font asset generator.
"""
import io
from pathlib import Path
import sys
from fontTools import subset
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont

root = Path(__file__).resolve().parents[2]
for revision in [1, 2]:
    font = TTFont(root / 'assets/fonts/Poppins-Regular.woff2', recalcTimestamp=False)
    options = subset.Options()
    options.name_IDs = ['*']
    options.name_languages = ['*']
    shrink = subset.Subsetter(options=options)
    shrink.populate(unicodes=range(32, 127))
    shrink.subset(font)
    for name in font['name'].names:
        value = {1: 'Feishin Reload Probe', 2: 'Regular', 3: 'FeishinReloadProbe-Regular',
                 4: 'Feishin Reload Probe Regular', 6: 'FeishinReloadProbe-Regular',
                 16: 'Feishin Reload Probe', 17: 'Regular'}.get(name.nameID)
        if value is not None:
            font['name'].setName(value, name.nameID, name.platformID, name.platEncID, name.langID)
    if revision == 2:
        glyph = font.getBestCmap()[ord('H')]
        pen = TTGlyphPen(font.getGlyphSet())
        font.getGlyphSet()[glyph].draw(TransformPen(pen, (2, 0, 0, 1, 0, 0)))
        font['glyf'][glyph] = pen.glyph()
        advance, bearing = font['hmtx'][glyph]
        font['hmtx'][glyph] = (advance * 2, bearing * 2)
    font.flavor = None
    output = io.BytesIO()
    font.save(output)
    destination = root / f'crates/feishin-gpui/assets/font-reload-{revision}.ttf'
    if '--check' in sys.argv:
        assert destination.read_bytes() == output.getvalue(), destination.name
    else:
        destination.write_bytes(output.getvalue())
print('Two same-family, same-PostScript-name font revisions verified')
