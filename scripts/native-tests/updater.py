#!/usr/bin/env python3
"""Exercise the real installer command against temporary application fixtures."""
import hashlib
import json
import pathlib
import platform
import subprocess
import sys
import tempfile

binary = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else 'target/debug/feishin').resolve()
assert binary.is_file(), f'Native binary is missing: {binary}'
with tempfile.TemporaryDirectory(prefix='feishin-native-installer-') as directory:
    root = pathlib.Path(directory)
    mac = platform.system() == 'Darwin'
    target = root / ('Feishin.app' if mac else 'feishin.exe' if platform.system() == 'Windows' else 'feishin')
    staged = root / ('New.app' if mac else 'new.exe' if platform.system() == 'Windows' else 'new')
    executable = lambda path: path / 'Contents/MacOS/feishin' if mac else path
    for path, data in [(executable(target), b'old application'), (executable(staged), b'new application')]:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    settings = root / 'state.json'
    settings.write_text('{"saved":"user data"}')
    parent = subprocess.Popen([sys.executable, '-c', 'pass'])
    parent.wait(timeout=10)
    job = {'target': str(target), 'staged': str(staged), 'checksum': 'wrong', 'parent': parent.pid, 'restart': False}
    job_file = root / 'job.json'
    job_file.write_text(json.dumps(job))
    failed = subprocess.run([str(binary), '--apply-native-update', str(job_file)], capture_output=True, text=True, timeout=20)
    assert failed.returncode != 0 and 'changed after' in failed.stderr, f'Installer exit {failed.returncode}: {failed.stderr}'
    assert executable(target).read_bytes() == b'old application'
    job['checksum'] = hashlib.sha256(b'old application').hexdigest()
    job_file.write_text(json.dumps(job))
    passed = subprocess.run([str(binary), '--apply-native-update', str(job_file)], capture_output=True, text=True, timeout=20)
    assert passed.returncode == 0, passed.stderr
    assert executable(target).read_bytes() == b'new application'
    assert settings.read_text() == '{"saved":"user data"}'
    assert not list(root.glob('.feishin-*'))
print('Native installer CLI: checksum rejection, replacement, cleanup and settings preservation passed')
