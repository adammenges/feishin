#!/usr/bin/env python3
"""Measure original upright and synthesized italic fonts in isolated Chromium."""
import base64
import html
import json
import os
import platform
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
chrome = os.environ.get('CHROME_BIN') or ('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' if platform.system() == 'Darwin' else shutil.which('chromium') or shutil.which('google-chrome'))
assert chrome, 'Set CHROME_BIN to an installed Chromium browser'
faces = [('Inter', 'Inter-VariableFont_opsz,wght.woff2'), ('Poppins', 'Poppins-Regular.woff2')]
css = ''.join('@font-face{font-family:"' + name + '";font-weight:' + ('100 900' if name == 'Inter' else '400') + ';src:url(data:font/woff2;base64,' + base64.b64encode((ROOT / 'assets/fonts' / path).read_bytes()).decode() + ')}' for name, path in faces)
css += ''.join('@font-face{font-family:"' + name + '";font-weight:400;src:url(data:font/woff2;base64,' + base64.b64encode((ROOT / 'crates/feishin-core/assets/fonts' / path).read_bytes()).decode() + ')}' for name, path in [('Noto Sans JP', 'noto-jp-400.woff2'), ('Noto Sans Hebrew', 'noto-hebrew-400.woff2')])
cascades = [
    'span { font-style: normal; }',
    'span { font-style: italic; }',
    'span { font-style: OBLIQUE; }',
    r'span { font-style: \69 talic; }',
    'span { font-style: /* note */ italic; }',
    'span { font-style: italic; font-style: unknown; }',
    'span { font-style: oblique; font-style: 12; }',
    'span { font-style: italic; font-style: "normal"; }',
    'span { font-style: italic; font-style: normal bold; }',
    'body { font-style: italic; } span { font-style: normal; font-style: inherit; }',
    'body { font-style: italic; } span { font-style: unset; }',
    'body { font-style: italic; } span { font-style: revert; }',
    'body { font-style: italic; } span { font-style: initial; }',
    'body { --slant: oblique; } span { font-style: var(--slant); }',
]
with tempfile.TemporaryDirectory(prefix='feishin-font-styles-') as temporary:
    directory = Path(temporary)
    page = directory / 'reference.html'
    page.write_text('''<!doctype html><meta charset="utf-8"><style>FONTS</style><pre id="result"></pre><script>
window.addEventListener('load',async()=>{
    for(const family of ['Inter','Poppins']) await document.fonts.load('64px '+family);
    await document.fonts.load('64px "Noto Sans JP"','漢');
    await document.fonts.load('64px "Noto Sans Hebrew"','א');
    const metrics=[];
    const cases=[];
    for(const family of ['Inter','Poppins']) for(const text of ['H','I','n','A','f','0']) cases.push({family,text});
    for(const [fallback,text] of [['Noto Sans JP','漢'],['Noto Sans Hebrew','א']])cases.push({family:'Inter',fallback,text});
    for(const {family,fallback,text} of cases) for(const style of ['normal','italic','oblique']) for(const size of [14,32,64]) {
        const canvas=document.createElement('canvas');canvas.width=canvas.height=256;
        const c=canvas.getContext('2d');c.font=style+' '+size+'px '+family+(fallback?',"'+fallback+'"':'');
        c.fillText(text,64,128);
        const alpha=c.getImageData(0,0,256,256).data;
        let left=256,right=-1,top=256,bottom=-1;
        for(let y=0;y<256;y++)for(let x=0;x<256;x++)if(alpha[(y*256+x)*4+3]){left=Math.min(left,x);right=Math.max(right,x);top=Math.min(top,y);bottom=Math.max(bottom,y);}
        metrics.push({family,style,size,text,...(fallback?{fallback}:{}),width:c.measureText(text).width,ink:[right-left+1,bottom-top+1]});
    }
    const cascades=CASCADE.map(source=>{
        const sheet=document.createElement('style');sheet.textContent=source;document.head.append(sheet);
        const span=document.createElement('span');span.textContent='probe';document.body.append(span);
        const style=getComputedStyle(span).fontStyle;span.remove();sheet.remove();return {source,style};
    });
    document.getElementById('result').textContent=JSON.stringify({metrics,cascades});
});</script>'''.replace('FONTS', css).replace('CASCADE', json.dumps(cascades)))
    result = subprocess.run([chrome, '--headless', '--no-sandbox', '--disable-gpu', '--no-first-run', '--no-default-browser-check', '--user-data-dir=' + str(directory / 'profile'), '--virtual-time-budget=5000', '--dump-dom', page.as_uri()], capture_output=True, text=True, check=True, timeout=30)
    match = re.search(r'<pre id="result">(.*?)</pre>', result.stdout, re.S)
    assert match and match[1], result.stderr + result.stdout[-2000:]
    payload = json.loads(html.unescape(match[1]))
    assert len(payload['metrics']) == 126 and len(payload['cascades']) == 14
    path = ROOT / 'crates/feishin-gpui/assets/font-styles-reference.json'
    reference = json.loads(path.read_text()) if path.exists() else {'platform_metrics': {}}
    name = platform.system()
    if '--check' in sys.argv:
        assert reference['platform_metrics'][name] == payload, 'Chromium font-style geometry changed'
    else:
        reference['platform_metrics'][name] = payload
        path.write_text(json.dumps(reference, indent=2) + '\n')
    print(f'126 original font-style measurements and 14 CSS cascades verified on {name}')
