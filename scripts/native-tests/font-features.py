#!/usr/bin/env python3
"""Read CSS OpenType parsing and original Inter/Poppins metrics in isolated Chromium."""
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
values = {
    'font-variant-numeric': ['normal', 'NORMAL', 'tabular-nums', 'proportional-nums', 'lining-nums', 'oldstyle-nums', 'diagonal-fractions', 'stacked-fractions', 'ordinal', 'slashed-zero', 'ordinal tabular-nums slashed-zero oldstyle-nums diagonal-fractions', r'tabular-\6e ums', '/* comment */ tabular-nums', 'normal tabular-nums', 'tabular-nums proportional-nums', 'lining-nums oldstyle-nums', 'diagonal-fractions stacked-fractions', 'ordinal ordinal', 'tabular-nums tabular-nums', '', 'unknown', 'tabular-nums,ordinal', 'tabular-nums !important'],
    'font-feature-settings': ['normal', 'NORMAL', r'\6e ormal', '"tnum"', "'tnum' on", '"tnum" OFF', '"tnum" 0', '"tnum" 2', '"tnum" +3', '"tnum" -0', '"tnum" 2147483647', '"tnum" 1, "liga" off, "tnum" 0', '"ABCD", "abcd"', '"    " 1', r'"\74 num" on', '/* comment */ "tnum" /* comment */ 1', '', '"abc"', '"abcde"', '"éabc"', 'tnum', '"tnum" -1', '"tnum" 1.5', '"tnum" 1.0', '"tnum" 1e0', '"tnum" yes', 'normal,"tnum"', '"tnum",', ',"tnum"', '"tnum" on off', '"tnum" 1 !important'],
}
variants = ['normal', 'tabular-nums', 'proportional-nums', 'slashed-zero', 'diagonal-fractions', 'stacked-fractions', 'oldstyle-nums ordinal']
settings = ['normal', '"tnum" on', '"tnum" off', '"liga" off', '"zero" on', '"frac" on', '"tnum" 1, "tnum" 0', '"tnum" -1']
samples = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0123456789', '11111', '88888', '1/2', 'office ffi fl', '1st 2nd 3rd']
fonts = [('Inter', 'Inter-VariableFont_opsz,wght.woff2'), ('Poppins', 'Poppins-Regular.woff2')]
css = ''.join('@font-face{font-family:"' + name + '";font-weight:' + ('100 900' if name == 'Inter' else '400') + ';src:url(data:font/woff2;base64,' + base64.b64encode((ROOT / 'assets/fonts' / path).read_bytes()).decode() + ')}' for name, path in fonts)
cases = [{'property': prop, 'value': value} for prop, entries in values.items() for value in entries]
with tempfile.TemporaryDirectory(prefix='feishin-font-features-') as temporary:
    directory = Path(temporary)
    page = directory / 'reference.html'
    page.write_text('''<!doctype html><style>FONTS
.probe{display:inline-block;font-weight:400;white-space:pre}
</style><div id="samples"></div><pre id="result"></pre><script>
const parsers=CASES, variants=VARIANTS, settings=SETTINGS, samples=SAMPLES, fonts=['Inter','Poppins'];
for (const item of parsers) {
    item.valid=CSS.supports(item.property,item.value);
    if(item.valid){const el=document.createElement('span'); el.style.setProperty(item.property,item.value); document.body.append(el); item.computed=getComputedStyle(el).getPropertyValue(item.property); el.remove();}
}
const metrics=[];
for(const font of fonts) for(const numeric of variants) for(const features of settings) for(const text of samples){
    const size=font==='Inter'?14:32;
    const el=document.createElement('span'); el.className='probe'; el.style.fontSize=size+'px'; el.style.fontFamily=font; el.style.fontVariantNumeric=numeric; el.style.fontFeatureSettings=features; el.textContent=text;
    document.getElementById('samples').append(el); metrics.push({font,size,numeric,features,text,el});
}
for(const size of [14,16,20,32]) for(const weight of [100,200,300,400,500,600,700,800,900]) for(const numeric of ['normal','tabular-nums']) for(const text of ['0','0123456789','Feishin music library','office ffi fl']){
    const el=document.createElement('span'); el.className='probe'; el.style.fontSize=size+'px'; el.style.fontWeight=weight; el.style.fontFamily='Inter'; el.style.fontVariantNumeric=numeric; el.textContent=text;
    document.getElementById('samples').append(el); metrics.push({font:'Inter',size,weight,numeric,features:'normal',text,el});
}
window.addEventListener('load', async()=>{
    await document.fonts.ready;
    for(const item of metrics){if(!document.fonts.check(item.size+'px '+item.font)) throw new Error('Font failed to load: '+item.font); item.width=item.el.getBoundingClientRect().width; delete item.el;}
    document.getElementById('result').textContent=JSON.stringify({browser:navigator.userAgent,parsers,metrics});
});
</script>'''.replace('FONTS', css).replace('CASES', json.dumps(cases)).replace('VARIANTS', json.dumps(variants)).replace('SETTINGS', json.dumps(settings)).replace('SAMPLES', json.dumps(samples)))
    result = subprocess.run([chrome, '--headless', '--no-sandbox', '--disable-gpu', '--no-first-run', '--no-default-browser-check', '--user-data-dir=' + str(directory / 'profile'), '--virtual-time-budget=5000', '--dump-dom', page.as_uri()], capture_output=True, text=True, check=True, timeout=30)
    match = re.search(r'<pre id="result">(.*?)</pre>', result.stdout, re.S)
    assert match and match[1], result.stderr + result.stdout[-2000:]
    payload = json.loads(html.unescape(match[1]))
    assert len(payload['metrics']) == 2080
    assert len({item['width'] for item in payload['metrics'] if item['font'] == 'Inter' and item['numeric'] == 'normal' and item['features'] == 'normal' and len(item['text']) == 1}) > 1, 'Chromium did not use the original proportional font'
    path = ROOT / ('target/native-variable-font-audit.json' if '--variations' in sys.argv else 'crates/feishin-gpui/assets/font-features-reference.json')
    previous = json.loads(path.read_text()) if path.exists() else None
    system = platform.system()
    if '--check' in sys.argv:
        assert previous, 'The platform reference has not been generated'
        assert payload['parsers'] == previous['parsers'], 'Chromium CSS parsing differs from saved reference'
        expected = previous['metrics']
        if system in previous.get('platform_widths', {}):
            expected = [dict(item, width=width) for item, width in zip(expected, previous['platform_widths'][system], strict=True)]
        assert payload['metrics'] == expected, 'Chromium font metrics differ from saved platform reference'
    else:
        if previous and '--variations' not in sys.argv:
            # CoreText and HarfBuzz quantize interpolated variable advances differently.
            # Retain independently measured widths rather than relaxing the browser check.
            assert [{k: v for k, v in item.items() if k != 'width'} for item in payload['metrics']] == [{k: v for k, v in item.items() if k != 'width'} for item in previous['metrics']], 'Regenerate every platform after changing the measurement cases'
            if system == 'Darwin':
                payload['platform_widths'] = previous.get('platform_widths', {})
            else:
                widths = [item['width'] for item in payload['metrics']]
                payload = previous
                payload.setdefault('platform_widths', {})[system] = widths
        path.write_text(json.dumps(payload, indent=2) + '\n')
    print('Verified Chromium font features:', len(payload['parsers']), 'CSS values and', len(payload['metrics']), 'original-font measurements')
