#!/usr/bin/env python3
"""Cross-check all Windows Rust code without linking native C++ libraries.

Uses the actual projectM headers and bindgen, plus the real Windows API crates.
TagLib/projectM C++ compilation and Windows runtime checks belong to native CI.
The production workspace and build scripts are left unchanged.
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess

root = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--clippy', action='store_true', help='Run strict Clippy instead of cargo check')
args = parser.parse_args()
check = root / 'target/native-windows-rust-check'
include = Path(os.environ['FEISHIN_PROJECTM_INCLUDE']).resolve()
assert (include / 'projectM-4/projectM.h').is_file(), 'Use the generated projectM API headers'
check.mkdir(parents=True, exist_ok=True)

def link(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    if not destination.exists():
        destination.symlink_to(source.resolve(), target_is_directory=source.is_dir())

for name in ['assets', 'LICENSE']:
    link(root / name, check / name)
for name in ['Cargo.toml', 'Cargo.lock']:
    shutil.copyfile(root / name, check / name)
for source in (root / 'crates').iterdir():
    if source.name not in ['vendor', 'feishin-core']:
        link(source, check / 'crates' / source.name)
for source in (root / 'crates/vendor').iterdir():
    if source.name != 'projectm-sys':
        link(source, check / 'crates/vendor' / source.name)

core = check / 'crates/feishin-core'
core.mkdir(parents=True, exist_ok=True)
for source in (root / 'crates/feishin-core').iterdir():
    if source.name != 'build.rs':
        link(source, core / source.name)
build = (root / 'crates/feishin-core/build.rs').read_text()
assert build.count('    build_taglib(&root);') == 1
build = build.replace('    build_taglib(&root);', '    // Rust metadata check: no TagLib library is linked.')
build = build.replace('fn build_taglib(', '#[allow(dead_code)]\nfn build_taglib(')
(core / 'build.rs').write_text(build)

projectm = check / 'crates/vendor/projectm-sys'
projectm.mkdir(parents=True, exist_ok=True)
for source in (root / 'crates/vendor/projectm-sys').iterdir():
    if source.name != 'build.rs':
        link(source, projectm / source.name)
(projectm / 'build.rs').write_text('''
fn main() {
    let headers = std::env::var("FEISHIN_PROJECTM_INCLUDE").unwrap();
    let bindings = bindgen::Builder::default()
        .header("bindgen/default.h")
        .clang_arg(format!("-I{headers}"))
        .allowlist_function("projectm_.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate().expect("Cannot parse the real projectM API headers");
    bindings.write_to_file(std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap())
        .join("bindings.rs")).unwrap();
}
''')
environment = dict(os.environ, FEISHIN_PROJECTM_INCLUDE=str(include))
# This workspace deliberately changes native build scripts, including host build
# dependencies. Never let its linker metadata replace the production artifacts.
environment['CARGO_TARGET_DIR'] = str(check / 'target')
compiler = shutil.which('x86_64-w64-mingw32-g++')
assert compiler, 'Install the MinGW C++ cross-compiler'
search = subprocess.run([compiler, '-E', '-Wp,-v', '-x', 'c++', '-'],
                        input='', text=True, capture_output=True, check=True).stderr
paths = search.split('#include <...> search starts here:\n', 1)[1].split('End of search list.', 1)[0]
key = 'BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_gnu'
environment.setdefault(key, ' '.join('-isystem ' + path.strip() for path in paths.splitlines() if path.strip()))
command = ['cargo', 'clippy' if args.clippy else 'check', '--workspace', '--all-targets',
           '--target', 'x86_64-pc-windows-gnu', '--manifest-path', str(check / 'Cargo.toml')]
if args.clippy:
    command.extend(['--', '-D', 'warnings'])
subprocess.run(command, env=environment, check=True)
print('All Windows Rust targets checked; native C++ linking and runtime remain unverified')
