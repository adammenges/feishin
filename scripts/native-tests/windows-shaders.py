#!/usr/bin/env python3
"""Compile every Windows HLSL entry point with glslang and validate its SPIR-V.

This catches shader syntax/type errors on Linux. The Windows build still uses
FXC; Direct3D compiler compatibility, reflection and rendering belong to native CI.
"""
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
directory = root / 'crates/vendor/gpui/src/platform/windows'
compiler = shutil.which('glslangValidator')
validator = shutil.which('spirv-val')
disassembler = shutil.which('spirv-dis')
assert compiler and validator and disassembler, 'Install glslang-tools and spirv-tools'
count = 0
with tempfile.TemporaryDirectory(prefix='feishin-windows-shaders-') as temporary:
    for source in sorted(directory.glob('*.hlsl')):
        entries = re.findall(r'^\w+\s+(\w+_(vertex|fragment))\s*\(', source.read_text(), re.M)
        for entry, stage in entries:
            output = Path(temporary) / (entry + '.spv')
            compiled = subprocess.run([compiler, '-D', '-V', '-S', 'vert' if stage == 'vertex' else 'frag',
                                       '-e', entry, str(source), '-o', str(output)],
                                      stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            if compiled.returncode:
                print(compiled.stdout)
            compiled.check_returncode()
            subprocess.run([validator, '--target-env', 'vulkan1.0', '--relax-block-layout',
                            str(output)], check=True)
            if entry == 'polychrome_sprite_vertex':
                reflection = subprocess.check_output([disassembler, str(output)], text=True)
                assert re.search(r'OpMemberDecorate %PolychromeSprite 9 Offset 104\b', reflection)
                assert re.search(r'OpDecorate %_runtimearr_PolychromeSprite ArrayStride 120\b', reflection)
            count += 1
assert count == 20, f'Expected all 20 production Windows shader entry points, found {count}'
print(f'{count} Windows HLSL entry points compiled and SPIR-V validated; Direct3D runtime remains unverified')
