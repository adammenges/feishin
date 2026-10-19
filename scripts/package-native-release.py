#!/usr/bin/env python3
"""Package native release assets under the updater's platform-specific names."""
import os
import pathlib
import platform
import subprocess
import zipfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
os.chdir(ROOT)
triple = os.environ.get('CARGO_BUILD_TARGET')
if not triple:
    version = subprocess.check_output(['rustc', '-vV'], text=True)
    triple = next(line.removeprefix('host: ') for line in version.splitlines() if line.startswith('host: '))
arch = triple.split('-')[0]
assert arch in ('aarch64', 'x86_64'), f'Unsupported native release architecture: {arch}'
os_name = {'Darwin': 'macos', 'Windows': 'windows', 'Linux': 'linux'}[platform.system()]
suffix = 'tar.gz' if os_name == 'linux' else 'zip'
destination = ROOT / 'target' / f'feishin-native-{os_name}-{arch}.{suffix}'
if os_name == 'linux':
    import shutil
    shutil.copyfile(ROOT / 'target/feishin-linux.tar.gz', destination)
else:
    source = ROOT / ('target/Feishin.app' if os_name == 'macos' else 'target/feishin-windows')
    assert source.is_dir(), f'Package directory is missing: {source}'
    with zipfile.ZipFile(destination, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        for file in sorted(source.rglob('*')):
            assert not file.is_symlink(), f'Unexpected package symlink: {file}'
            if file.is_file():
                relative = file.relative_to(source.parent if os_name == 'macos' else source)
                archive.write(file, relative)
print(f'Built {destination.relative_to(ROOT)}')
