#!/usr/bin/env python3
"""Retain the original CSS fallback ranges and five independently verified weights.

Run with fonttools[woff]==4.61.1. These are build assets, with no Python runtime.
"""
import hashlib
import json
from pathlib import Path
import sys

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'crates/feishin-core/assets/fonts'
FONTS = [
    ('NotoSansJP-VariableFont_wght.woff2', 'Noto Sans JP', 'jp', [(0x3000, 0x9fff), (0xff00, 0xffef)]),
    ('NotoSansHebrew-VariableFont_wdth,wght.woff2', 'Noto Sans Hebrew', 'hebrew', [(0x0590, 0x05ff), (0xfb1d, 0xfb4f)]),
]
SOURCE_HASHES = {'NotoSansJP-VariableFont_wght.woff2': 'b1c7d93631058dd351aa323b84e835ad87741400a4355a8b47de222121d1ee9f', 'NotoSansHebrew-VariableFont_wdth,wght.woff2': '5c49d684a536a9b48696911c3119eebf670e8f05877421d9d7d52eb98d10d09d'}
STYLES = ['Regular', 'Medium', 'SemiBold', 'Bold', 'ExtraBold']

DEST.mkdir(parents=True, exist_ok=True)
record = []
for filename, family, stem, ranges in FONTS:
    source = ROOT / 'assets/fonts' / filename
    assert hashlib.sha256(source.read_bytes()).hexdigest() == SOURCE_HASHES[filename], 'Original font checksum differs'
    for weight, style in zip(range(400, 801, 100), STYLES, strict=True):
        font = TTFont(source, recalcTimestamp=False)
        # The web @font-face descriptors map CSS 400/500/600/700/800 to 500/600/700/800/900.
        axes = {'wght': weight + 100}
        if any(axis.axisTag == 'wdth' for axis in font['fvar'].axes):
            axes['wdth'] = 100
        font = instantiateVariableFont(font, axes, inplace=True)
        options = subset.Options()
        options.layout_features = ['*']
        options.name_IDs = ['*']
        options.name_legacy = True
        options.name_languages = ['*']
        # Latin metrics keep GPUI's font measurement helpers usable; fallback selection
        # still gives the configured primary font first choice for those characters.
        characters = set(range(0x20, 0x7f))
        for first, last in ranges:
            characters.update(range(first, last + 1))
        shrink = subset.Subsetter(options=options)
        shrink.populate(unicodes=characters)
        shrink.subset(font)
        font['OS/2'].usWeightClass = weight
        font['OS/2'].fsSelection &= ~(1 | (1 << 5) | (1 << 6))
        font['OS/2'].fsSelection |= (1 << 6) if weight == 400 else (1 << 5) if weight >= 700 else 0
        font['head'].macStyle = 1 if weight >= 700 else 0
        for name in font['name'].names:
            value = {1: family, 2: style, 3: f'Feishin-{stem}-{weight}', 4: family + ' ' + style,
                     6: f'FeishinNoto-{stem}-{style}', 16: family, 17: style}.get(name.nameID)
            if value is not None:
                font['name'].setName(value, name.nameID, name.platformID, name.platEncID, name.langID)
        font.flavor = 'woff2'
        destination = DEST / f'noto-{stem}-{weight}.woff2'
        if '--check' in sys.argv:
            import io
            output = io.BytesIO()
            font.save(output)
            assert destination.read_bytes() == output.getvalue(), f'Stale font: {destination.name}'
        else:
            font.save(destination)
        record.append({'file': destination.name, 'source': filename,
                       'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
                       'sha256': hashlib.sha256(destination.read_bytes()).hexdigest(),
                       'css_weight': weight, 'original_axis_weight': weight + 100,
                       'unicode_ranges': [list(pair) for pair in ranges]})
    # Explicit CSS coordinates override the @font-face defaults. Retain a private
    # variable companion for these requests while keeping ordinary static faces.
    font = TTFont(source, recalcTimestamp=False)
    shrink = subset.Subsetter(options=options)
    shrink.populate(unicodes=characters)
    shrink.subset(font)
    for name in list(font['name'].names):
        value = {1: family + ' Variable', 2: 'Regular', 3: f'Feishin-{stem}-Variable',
                 4: family + ' Variable', 6: f'FeishinNoto-{stem}-Variable',
                 16: family + ' Variable', 17: 'Regular'}.get(name.nameID)
        if value is not None:
            font['name'].setName(value, name.nameID, name.platformID, name.platEncID, name.langID)
    font.flavor = 'woff2'
    destination = DEST / f'noto-{stem}-variable.woff2'
    if '--check' in sys.argv:
        import io
        output = io.BytesIO()
        font.save(output)
        assert destination.read_bytes() == output.getvalue(), f'Stale font: {destination.name}'
    else:
        font.save(destination)
    record.append({'file': destination.name, 'source': filename,
                   'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
                   'sha256': hashlib.sha256(destination.read_bytes()).hexdigest(),
                   'variable': True, 'unicode_ranges': [list(pair) for pair in ranges]})
path = DEST / 'SOURCE.json'
if '--check' in sys.argv:
    assert json.loads(path.read_text()) == record
else:
    path.write_text(json.dumps(record, indent=2) + '\n')
print(f'Verified {len(record)} original CSS fallback faces')
