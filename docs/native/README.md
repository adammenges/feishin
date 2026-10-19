# Native Rust development

This is a first pass at rewriting Feishin in Rust and GPUI. Feature parity remains a goal, with outstanding checks recorded in [PARITY.md](PARITY.md). Start with the [root README](../../README.md) for platform setup, build commands and an overview of how the app works.

The former Electron/React application has been removed. Native builds use the assets under the Cargo workspace and `assets/`; they do not read the former `src/` tree.

## Requirements

Use Rust 1.91 or newer through rustup, CMake 3.21 or newer and a C++ compiler. Packaging and Python test scripts require Python 3.11 or newer. CMake builds the bundled SoundTouch, projectM and TagLib libraries.

- macOS: Xcode and Apple's Metal compiler component (`xcodebuild -downloadComponent MetalToolchain`).
- Linux: audio, font, OpenGL/EGL, Vulkan, X11 and Wayland development libraries. The complete package list is in `.github/workflows/native.yml`. Set `CXXFLAGS=-fPIC` when building the bundled native libraries.
- Windows: LLVM/libclang and vcpkg with `glew:x64-windows-static-md` and `zlib:x64-windows-static-md`; set `VCPKG_INSTALLATION_ROOT`.
- Integration tests: FFmpeg and MPV. Node 22 or newer runs the browser remote tests. Chrome, Docker and Linux virtual-display tools are needed only for the corresponding integration checks.

## Run and verify

```sh
cargo build --locked --workspace
cargo run --locked -p feishin-gpui
cargo test --locked --workspace -- --test-threads=4
cargo test --manifest-path crates/vendor/taffy/Cargo.toml --lib --locked
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
node --test scripts/native-tests/remote.cjs
```

Set `FEISHIN_TEST_FFMPEG` and `FEISHIN_TEST_MPV` to the executable paths to require silent audio integration coverage. These tests use generated sources and null output. Interactive listening, real devices and platform permissions remain separate acceptance checks.

`python3 scripts/native-tests/components.py` tests patched shared components. Run it between other Cargo commands: it temporarily includes the component crate in the workspace and restores the manifest and lockfile afterward.

The optional Windows Rust cross-check uses generated projectM headers:

```sh
FEISHIN_PROJECTM_INCLUDE=/path/to/generated/include python3 scripts/native-tests/windows-rust-check.py --clippy
```

This checks Rust code without native Windows C++ linking or runtime validation.

## Profiles and migration

`FEISHIN_DATA_DIR=/path/to/disposable/data` isolates settings, covers, lyrics and queue data. Otherwise the native app uses its separate FeishinRust data directory. Credentials use the operating-system password store and are excluded from settings exports.

Settings supports export, native restore, and importing Electron preferences/account definitions with a preview and backup. Migration compatibility remains supported after removing the old application source. Real-account migration checks remain in [INTERACTIVE-CHECKS.md](INTERACTIVE-CHECKS.md).

`SERVER_TYPE`, `SERVER_NAME`, `SERVER_URL`, `REMOTE_URL` and `LEGACY_AUTHENTICATION` prefill new accounts. A nonempty `DISABLE_AUTO_UPDATES` disables automatic updates while retaining manual checks.

## Native assets and reference data

- `crates/feishin-core/assets/locales/`: the 39 translation catalogs.
- `crates/feishin-core/assets/palettes.json`: bundled theme definitions and stylesheets.
- `assets/icons/macos.png`: the rounded-square macOS render of `media/feishin.icon`, shared by the runtime Dock icon and every bundled icon size.
- `crates/feishin-core/assets/mantine-*.json`: compatibility values for existing theme variables.
- `crates/feishin-core/src/settings_search.rs`: settings search title keys.
- `crates/feishin-gpui/assets/styles/`, `window-controls/` and `service-icons/`: styles and images used directly by the native interface.
- Both native asset directories contain the captured behavior/layout fixtures consumed by Rust tests.

The generators that executed the removed application source have been retired. Their recorded fixtures and licenses remain, and historical reproduction details in [PARITY.md](PARITY.md) describe the source state at the time of capture. The legacy source remains available in Git history.

Asset maintenance scripts for icons, fallback fonts, MilkDrop presets, projectM and TagLib remain under `scripts/vendor-native-*`. Independent Chromium image/font checks and the pinned analyzer reference do not depend on the retired application.

## Linux packaged checks

These checks own private X11/Wayland displays, D-Bus sessions and disposable profiles. They use software Vulkan and do not access the user's desktop, music library or credentials.

```sh
python3 scripts/native-tests/startup.py target/feishin-linux/bin/feishin
python3 scripts/native-tests/startup.py --wayland target/feishin-linux/bin/feishin
python3 scripts/native-tests/startup.py --collapsed target/feishin-linux/bin/feishin
python3 scripts/native-tests/startup.py --wayland --collapsed target/feishin-linux/bin/feishin
python3 scripts/native-tests/startup.py --library target/feishin-linux/bin/feishin
python3 scripts/native-tests/startup.py --wayland --library target/feishin-linux/bin/feishin
```

The library workflow signs in to a disposable localhost fixture using dummy credentials, captures pages, checks silent MPV transport and saved queue state, edits shortcuts and player settings, then exits normally. Screenshots and logs are saved under `target/native-startup*`.

Optional workflow flags cover detail navigation, album controls, themes, accent editing, custom CSS, images, paths, compressor, equalizer and smart playlists. `.github/workflows/native.yml` enables all ten for both display systems.

Additional protocol checks:

```sh
cargo build --locked -p feishin-gpui --example wayland-window
python3 scripts/native-tests/wayland.py target/debug/examples/wayland-window
cargo build --locked -p feishin-gpui --example accessibility
python3 scripts/native-tests/accessibility.py target/debug/examples/accessibility
```

## Remote and server integration

The native HTTP/WebSocket service embeds its remote client from `crates/feishin-core/src/remote*`. It supports authenticated phone control and an installable public shell. It is independent of the removed React remote application.

```sh
cargo build --locked -p feishin-core --example remote-browser
node scripts/native-tests/remote-browser.mjs target/debug/examples/remote-browser
python3 scripts/native-tests/live-servers.py
python3 scripts/native-tests/live-servers.py --legacy
```

The server checks require Docker and FFmpeg and create disposable fixtures. Physical phones, speakers, output devices and screen readers remain in the deferred interactive checklist.

## Packages and updates

```sh
sh scripts/bundle-native.sh
# On Linux instead:
sh scripts/package-native-linux.sh
python3 scripts/package-native-release.py
python3 scripts/native-tests/packages.py
```

macOS output is `target/Feishin.app` and `target/feishin-native-macos-<architecture>.zip`. Linux output is `target/feishin-linux/` and `target/feishin-native-linux-<architecture>.tar.gz`. The native CI workflow also assembles the Windows archive.

Local macOS bundles are ad-hoc signed. Distribution signing and notarization require a separate release setup. The installer and lifecycle checks run without opening an application window:

```sh
python3 scripts/native-tests/updater.py target/release/feishin
python3 scripts/native-tests/lifecycle.py target/release/feishin
```

Use the `.exe` suffix on Windows. Package checks validate executable formats/modes, archive contents, checksums and bundled licenses. Updater checks cover checksum rejection, replacement, cleanup and preservation of settings.

See [PARITY.md](PARITY.md) for current verification evidence and [INTERACTIVE-CHECKS.md](INTERACTIVE-CHECKS.md) for deferred acceptance. Removing the legacy source does not establish full parity.
