#!/usr/bin/env python3
"""Reproduce the original app's icons from the locked npm package."""
import base64
import hashlib
import io
import json
from pathlib import Path
import re
import tarfile
import urllib.request
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'crates/feishin-gpui/assets'
INTEGRITY = 'RH93p5ki6LfOiIt0UtDyNg/cee+HLVR6cHHtW3wALfo+eOHTp8RnU2kRkI6E+H19zMIs03DyxUG/GfZMOGvmiA=='
ICONS = {
    'previous': ('lu', 'LuSkipBack'), 'next': ('lu', 'LuSkipForward'),
    'play': ('lu', 'LuPlay'), 'pause': ('lu', 'LuPause'),
    'stop': ('lu', 'LuSquare'), 'shuffle': ('lu', 'LuShuffle'),
    'step-back': ('lu', 'LuStepBack'), 'step-forward': ('lu', 'LuStepForward'),
    'repeat': ('ri', 'RiRepeat2Line'), 'repeat-one': ('ri', 'RiRepeatOneLine'),
    'album': ('lu', 'LuDisc3'), 'song': ('lu', 'LuMusic'),
    'artist': ('lu', 'LuUserPen'), 'folder': ('lu', 'LuFolderOpen'),
    'folder-closed': ('lu', 'LuFolderClosed'),
    'folders-expand': ('lu', 'LuChevronsUpDown'), 'folders-collapse': ('lu', 'LuChevronsDownUp'),
    'playlist': ('lu', 'LuListMusic'), 'genre': ('lu', 'LuFlag'),
    'radio': ('lu', 'LuRadio'), 'library': ('lu', 'LuLibrary'),
    'keyboard': ('lu', 'LuKeyboard'), 'more': ('lu', 'LuEllipsis'),
    'close': ('lu', 'LuX'), 'cast': ('lu', 'LuCast'),
    'favorite': ('lu', 'LuHeart'), 'favorite-filled': ('lu', 'LuHeart'),
    'arrow-down': ('lu', 'LuChevronDown'),
    'arrow-up': ('lu', 'LuChevronUp'), 'arrow-left-right': ('lu', 'LuArrowLeftRight'),
    'arrow-forward': ('lu', 'LuArrowRight'),
    'error': ('lu', 'LuShieldAlert'), 'info': ('lu', 'LuInfo'),
    'wifi-off': ('lu', 'LuWifiOff'), 'refresh': ('lu', 'LuRotateCw'),
    'play-next': ('lu', 'LuCornerUpRight'), 'play-last': ('lu', 'LuChevronLast'),
    'random': ('ri', 'RiPlayListAddLine'),
    'spinner': ('cg', 'CgSpinnerTwoAlt'),
    'arrow-left': ('lu', 'LuChevronLeft'), 'arrow-right': ('lu', 'LuChevronRight'),
    'upload': ('lu', 'LuUpload'), 'download': ('lu', 'LuDownload'),
    'file-json': ('lu', 'LuFileJson'),
    'delete': ('lu', 'LuTrash'), 'go-current': ('lu', 'LuCornerDownRight'),
    'playlist-add': ('lu', 'LuListPlus'), 'settings': ('lu', 'LuSettings2'),
    'search': ('lu', 'LuSearch'), 'menu': ('lu', 'LuMenu'),
    'filter': ('lu', 'LuListFilter'), 'layout-grid': ('lu', 'LuLayoutGrid'),
    'layout-detail': ('lu', 'LuLayoutList'), 'layout-table': ('lu', 'LuTable'),
    'sort-ascending': ('lu', 'LuArrowUpNarrowWide'), 'sort-descending': ('lu', 'LuArrowDownWideNarrow'),
    'collection': ('lu', 'LuPackage2'),
    'metadata': ('lu', 'LuBookOpen'), 'languages': ('lu', 'LuLanguages'),
    'minus': ('lu', 'LuMinus'), 'plus': ('lu', 'LuPlus'),
    'duration': ('lu', 'LuClock3'), 'user': ('lu', 'LuUser'),
    'disc': ('lu', 'LuDisc'), 'hash': ('lu', 'LuHash'), 'drag-vertical': ('lu', 'LuGripVertical'),
    'expand': ('lu', 'LuExpand'), 'shrink': ('lu', 'LuShrink'),
    'visualizer': ('lu', 'LuAudioLines'),
    'microphone': ('lu', 'LuMicVocal'),
    'queue': ('lu', 'LuList'), 'related': ('lu', 'LuSparkle'),
    'sliders': ('lu', 'LuSlidersHorizontal'),
    'group': ('lu', 'LuGroup'), 'star': ('lu', 'LuStar'), 'circle-slash': ('lu', 'LuCircleSlash'),
    'timer': ('lu', 'LuTimer'), 'timer-off': ('lu', 'LuTimerOff'),
    'panel-open': ('lu', 'LuPanelRightOpen'), 'panel-close': ('lu', 'LuPanelRightClose'),
    'volume-low': ('lu', 'LuVolume1'), 'volume-high': ('lu', 'LuVolume2'),
    'volume-muted': ('lu', 'LuVolumeX'),
    'window-minimize': ('ri', 'RiSubtractLine'),
    'window-maximize': ('ri', 'RiCheckboxBlankLine'),
    'window-close': ('ri', 'RiCloseLine'),
}
SIDEBAR_ICONS = ['RiAlbumFill', 'RiAlbumLine', 'RiFlag2Fill', 'RiFlag2Line', 'RiFolder3Fill', 'RiFolder3Line', 'RiHeartFill', 'RiHeartLine', 'RiHome6Fill', 'RiHome6Line', 'RiMusic2Fill', 'RiMusic2Line', 'RiPlayFill', 'RiPlayLine', 'RiPlayListFill', 'RiPlayListLine', 'RiRadioFill', 'RiRadioLine', 'RiSearchFill', 'RiSearchLine', 'RiSettings2Fill', 'RiSettings2Line', 'RiUserVoiceFill', 'RiUserVoiceLine']
ICONS.update({name: ('ri', name) for name in SIDEBAR_ICONS})
PLAYER_FILLED = {'previous', 'next', 'play', 'pause', 'stop', 'step-back',
                 'step-forward', 'repeat', 'repeat-one', 'favorite-filled'}

def read(url):
    with urllib.request.urlopen(url, timeout=30) as response:
        return response.read()

def node(data):
    element = ET.Element(data['tag'], {(key if key in {'viewBox', 'preserveAspectRatio'} else re.sub(r'[A-Z]', lambda match: '-'+match[0].lower(), key)): str(value) for key, value in data.get('attr', {}).items()})
    for child in data.get('child', []):
        element.append(node(child))
    return element

def main():
    archive = read('https://registry.npmjs.org/react-icons/-/react-icons-5.6.0.tgz')
    assert base64.b64encode(hashlib.sha512(archive).digest()).decode() == INTEGRITY
    (DEST / 'icons').mkdir(parents=True, exist_ok=True)
    with tarfile.open(fileobj=io.BytesIO(archive)) as files:
        for name, (family, function) in ICONS.items():
            source = files.extractfile(f'package/{family}/index.mjs').read().decode()
            match = re.search(r'export function ' + function + r'\s*\(props\)\s*\{\s*return GenIcon\((.*?)\)\(props\);', source, re.S)
            assert match, function
            element = node(json.loads(match[1]))
            element.set('xmlns', 'http://www.w3.org/2000/svg')
            element.set('width', '24'); element.set('height', '24')
            if name in PLAYER_FILLED:
                element.set('fill', 'currentColor')
            notice = '<!-- Converted from react-icons 5.6.0; Lucide ISC/MIT or Remix Icon 4.2.0 Apache-2.0. Root size and fill match the app player controls. -->\n'
            if name not in PLAYER_FILLED and name != 'shuffle':
                notice = '<!-- Converted from react-icons 5.6.0; Lucide ISC/MIT. Geometry and fill match the original app icon. -->\n'
            if family == 'ri' and name not in PLAYER_FILLED:
                notice = '<!-- Converted from react-icons 5.6.0; Remix Icon 4.2.0 Apache-2.0. Geometry and fill match the original app icon. -->\n'
            if name == 'spinner':
                notice = '<!-- Converted from react-icons 5.6.0; css.gg MIT. Geometry and fill match the original app loading icon. -->\n'
            (DEST / 'icons' / f'{name}.svg').write_text(notice + ET.tostring(element, encoding='unicode') + '\n')
        (DEST / 'react-icons-LICENSE.txt').write_bytes(files.extractfile('package/LICENSE').read())
    (DEST / 'lucide-LICENSE.txt').write_bytes(read('https://raw.githubusercontent.com/lucide-icons/lucide/main/LICENSE'))
    (DEST / 'remix-4.2.0-LICENSE.txt').write_bytes(read('https://raw.githubusercontent.com/Remix-Design/RemixIcon/v4.2.0/License'))
    (DEST / 'css-gg-LICENSE.txt').write_bytes(read('https://raw.githubusercontent.com/astrit/css.gg/main/LICENSE.md'))
    print(f'Recorded {len(ICONS)} icons from verified react-icons 5.6.0')

if __name__ == '__main__':
    main()
