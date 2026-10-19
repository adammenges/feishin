#!/usr/bin/env python3
"""Verify native archives, executable modes and every bundled license."""
import hashlib
import pathlib
import re
import stat
import sys
import tarfile
import zipfile

root = pathlib.Path(__file__).resolve().parents[2]
licenses = {path.name: path for pattern in ['crates/feishin-core/assets/*LICENSE.txt', 'crates/feishin-core/assets/*OFL.txt', 'crates/feishin-gpui/assets/*LICENSE.txt', 'crates/feishin-gpui/assets/*AUTHORS.txt'] for path in root.glob(pattern)}
licenses.update({name: root / source for name, source in {
    'Feishin.txt': 'LICENSE',
    'projectm-sys.txt': 'crates/vendor/projectm-sys/LICENSE',
    'projectM.txt': 'crates/vendor/projectm-sys/libprojectM/src/libprojectM/COPYING',
    'xattr-MIT.txt': 'crates/vendor/xattr/LICENSE-MIT',
    'xattr-Apache.txt': 'crates/vendor/xattr/LICENSE-APACHE',
    'cosmic-text-MIT.txt': 'crates/vendor/cosmic-text/LICENSE-MIT',
    'cosmic-text-Apache.txt': 'crates/vendor/cosmic-text/LICENSE-APACHE',
    'TagLib-LGPL.txt': 'crates/vendor/taglib/COPYING.LGPL',
    'TagLib-MPL.txt': 'crates/vendor/taglib/COPYING.MPL',
    'utfcpp.txt': 'crates/vendor/taglib/3rdparty/utfcpp/LICENSE',
    'THIRD_PARTY.md': 'docs/native/THIRD_PARTY.md',
}.items()})
archives = [pathlib.Path(argument) for argument in sys.argv[1:]] or sorted((root / 'target').glob('feishin-native-*'))
assert archives, 'No native release archives found'
for path in archives:
    match = re.fullmatch(r'feishin-native-(macos|linux|windows)-(aarch64|x86_64)\.(zip|tar\.gz)', path.name)
    assert match, f'Unexpected native archive name: {path}'
    system, architecture, _ = match.groups()
    mac = system == 'macos'
    prefix = 'Feishin.app/Contents/Resources/licenses/' if mac else 'feishin-linux/share/licenses/feishin/' if system == 'linux' else 'licenses/'
    executable = 'Feishin.app/Contents/MacOS/feishin' if mac else 'feishin-linux/bin/feishin' if system == 'linux' else 'feishin.exe'
    if path.suffix == '.zip':
        with zipfile.ZipFile(path) as archive:
            assert archive.testzip() is None, f'Archive CRC failure: {path}'
            files = {entry.filename: entry for entry in archive.infolist()}
            for name, entry in files.items():
                member = pathlib.PurePosixPath(name)
                assert not member.is_absolute() and '..' not in member.parts
                assert not stat.S_ISLNK(entry.external_attr >> 16), name
            for name, source in licenses.items():
                assert archive.read(prefix + name) == source.read_bytes(), f'Stale or missing license: {name}'
            data = archive.read(executable)
            if mac:
                for name in [executable, 'Feishin.app/Contents/MacOS/feishin-audio-capture']:
                    assert files[name].external_attr >> 16 & 0o111, f'Missing executable mode: {name}'
                assert data[:4] in [b'\xcf\xfa\xed\xfe', b'\xfe\xed\xfa\xcf', b'\xca\xfe\xba\xbe']
            else:
                assert data[:2] == b'MZ'
    else:
        with tarfile.open(path, 'r:gz') as archive:
            files = {entry.name: entry for entry in archive.getmembers()}
            for name, entry in files.items():
                member = pathlib.PurePosixPath(name)
                assert not member.is_absolute() and '..' not in member.parts
                assert entry.isfile() or entry.isdir(), name
            for name, source in licenses.items():
                assert archive.extractfile(prefix + name).read() == source.read_bytes(), f'Stale or missing license: {name}'
            assert files[executable].mode & 0o111, 'Missing executable mode'
            data = archive.extractfile(executable).read()
            assert data[:5] == b'\x7fELF\x02'
            assert int.from_bytes(data[18:20], 'little') == (183 if architecture == 'aarch64' else 62)
    with path.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    print(f'{path.name}: archive, executable and {len(licenses)} licenses verified; SHA-256 {digest}')
