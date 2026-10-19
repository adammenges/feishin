#!/usr/bin/env python3
"""Test native component patches with the application's pinned dependency graph.

Run between Cargo commands: the vendored dependency is temporarily a workspace
member so Cargo can include its test-only dependencies, then the manifest and
application lockfile are restored byte for byte even when a test fails.
"""
from pathlib import Path
import argparse
import subprocess

ROOT = Path(__file__).resolve().parents[2]
manifest = ROOT / 'Cargo.toml'
original = manifest.read_bytes()
lockfile = ROOT / 'Cargo.lock'
original_lock = lockfile.read_bytes()
component = '"crates/vendor/gpui-component"'
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--filter', action='append', help='Run only this component test filter; may be repeated.')
args = parser.parse_args()
lines = original.decode().splitlines(keepends=True)
try:
    for index, line in enumerate(lines):
        if line.startswith('members = '):
            lines[index] = line.replace(']\n', f', {component}]\n')
        elif line.startswith('exclude = '):
            lines[index] = line.replace(f', {component}', '')
    manifest.write_text(''.join(lines))
    for test_filter in args.filter or [
        'remote_markdown_filters_unsafe_links_images_html_and_references',
        'blink_cursor_follows_the_platform_clock_and_cancels_stale_cycles',
    ]:
        result = subprocess.run(['cargo', 'test', '-p', 'gpui-component', test_filter], cwd=ROOT)
        if result.returncode:
            break
finally:
    manifest.write_bytes(original)
    lockfile.write_bytes(original_lock)
raise SystemExit(result.returncode)
