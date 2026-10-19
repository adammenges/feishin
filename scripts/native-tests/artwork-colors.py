#!/usr/bin/env python3
"""Generate synthetic ICC-tagged artwork and read its actual Chromium canvas colors."""
import base64
import html
import json
import os
import platform
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
chrome = os.environ.get('CHROME_BIN') or (('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome') if platform.system() == 'Darwin' else shutil.which('google-chrome') or shutil.which('chromium'))
assert chrome, 'Set CHROME_BIN to an installed Chromium browser'
with tempfile.TemporaryDirectory(prefix='feishin-artwork-colors-') as temporary:
    directory = Path(temporary)
    (directory / 'src').mkdir()
    (directory / 'Cargo.toml').write_text('''[package]
name = "feishin-artwork-color-reference"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
moxcms = "=0.8.1"
image = "=0.25.10"
serde_json = "1"
base64 = "0.22"
''')
    (directory / 'src/main.rs').write_bytes((ROOT / 'scripts/native-tests/artwork-color-sources.rs').read_bytes())
    environment = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target/native-color-reference/target'))
    sources = json.loads(subprocess.check_output(['cargo', 'run', '--offline', '--quiet', '--manifest-path', str(directory / 'Cargo.toml')], env=environment, text=True))
    encoder = directory / 'cmyk-encoder'
    flags = shlex.split(subprocess.check_output(['pkg-config', '--cflags', '--libs', 'libjpeg'], text=True))
    subprocess.run(['cc', '-std=c99', '-Wall', '-Wextra', '-Werror', str(ROOT / 'scripts/native-tests/artwork-cmyk-sources.c'), *flags, '-o', str(encoder)], check=True)
    subprocess.run([str(encoder), str(ROOT / 'scripts/native-tests/assets/ghostscript-CMYK.icc'), str(directory)], check=True)
    for path in sorted(directory.glob('*.jpeg')):
        sources.append({'profile': path.stem, 'format': 'jpeg', 'width':16, 'height':24, 'bytes': base64.b64encode(path.read_bytes()).decode()})
    page = directory / 'reference.html'
    images = ''.join('<img hidden id="image-' + str(index) + '" src="data:image/' + ('jpeg' if source['format'] == 'jpeg' else 'webp' if source['format'] == 'webp' else 'png') + ';base64,' + source['bytes'] + '">' for index, source in enumerate(sources))
    page.write_text('''<!doctype html><pre id="result"></pre>IMAGES<script>
window.addEventListener('load', () => {
    const canvas=document.createElement('canvas'); canvas.width=2; canvas.height=3;
    const context=canvas.getContext('2d',{willReadFrequently:true,colorSpace:'srgb'});
    const sources=SOURCES, images=[];
    for (let index=0; index<sources.length; index++) {
        const image=document.getElementById('image-'+index);
        if (!image.naturalWidth) throw new Error('Artwork failed to load: '+sources[index].profile);
        canvas.width=sources[index].width||2; canvas.height=sources[index].height||3;
        context.drawImage(image,0,0);
        images.push({...sources[index],rgba:[...context.getImageData(0,0,canvas.width,canvas.height).data]});
    }
    document.getElementById('result').textContent=JSON.stringify({browser:navigator.userAgent,images});
});
</script>'''.replace('IMAGES', images).replace('SOURCES', json.dumps(sources)))
    result = subprocess.run([chrome, '--headless', '--no-sandbox', '--disable-gpu', '--no-first-run', '--no-default-browser-check', '--force-color-profile=srgb', '--user-data-dir=' + str(directory / 'profile'), '--virtual-time-budget=5000', '--dump-dom', page.as_uri()], capture_output=True, text=True, check=True, timeout=30)
    match = re.search(r'<pre id="result">(.*?)</pre>', result.stdout, re.S)
    assert match and match[1], result.stderr + result.stdout[-2000:]
    payload = json.loads(html.unescape(match[1]))
    assert len(payload['images']) == 21
    path = ROOT / 'crates/feishin-gpui/assets/artwork-color-reference.json'
    if '--check' in sys.argv:
        assert payload['images'] == json.loads(path.read_text())['images'], 'Chromium artwork colors differ from the saved reference'
    else:
        path.write_text(json.dumps(payload, indent=2) + '\n')
    print('Verified Chromium artwork colors:', [(image['profile'], image['format'], image['rgba'][:4]) for image in payload['images']])
