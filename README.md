# Feishin: Rust rewrite

**This is a first pass at rewriting Feishin in Rust with GPUI.** Mostly converted by the robots. It is a work in progress, but also very close. I use it day to day as my main player.

**Feel free to contribue!**

Feishin connects to Navidrome, Subsonic-compatible servers and Jellyfin to browse and play your music. This checkout contains the native desktop rewrite, including library browsing, playback, queues, playlists, lyrics, visualizers, themes and remote control. The previous Electron/React application and its build tooling have been removed; its source remains in Git history.

## Build and run

Run these commands from the repository root. Install **Rust 1.91 or newer through rustup**, **CMake 3.21 or newer**, and the platform tools below. Make sure `cargo`, `rustc` and `cmake` are on your `PATH`. Packaging and Python test scripts require **Python 3.11 or newer**.

Cargo builds the Rust crates and the bundled C/C++ libraries, including SoundTouch, TagLib and projectM. The first build downloads dependencies and compiles those libraries, so it can take a while. The desktop build does not require Node, PNPM or a JavaScript package installation.

### macOS

Install full Xcode, open it once to complete setup, and select its developer tools. With Xcode installed at the standard location:

```sh
sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
xcodebuild -downloadComponent MetalToolchain
```

Install the supporting tools, for example with Homebrew:

```sh
brew install cmake pkg-config python
```

Build and open the app:

```sh
sh scripts/bundle-native.sh
open target/Feishin.app
```

The bundle script compiles the optimized GPUI application and the macOS audio-capture helper, copies icons and licenses, and signs the bundle locally. The result is `target/Feishin.app`. Local signing is ad-hoc; this does not produce a notarized distribution release.

To create the release ZIP after building the bundle:

```sh
python3 scripts/package-native-release.py
```

This writes `target/feishin-native-macos-aarch64.zip` on Apple Silicon or `target/feishin-native-macos-x86_64.zip` on Intel.

### Linux

The reference dependency setup uses Ubuntu 24.04. Install the compiler, audio, font and graphics development libraries:

```sh
sudo apt-get update
sudo apt-get install -y build-essential clang cmake pkg-config python3 \
  libasound2-dev libfontconfig-dev libglib2.0-dev libssl-dev \
  libwayland-dev libx11-xcb-dev libxkbcommon-x11-dev libdbus-1-dev dbus \
  libgit2-dev libvulkan1 mesa-vulkan-drivers libgl1-mesa-dev \
  libegl1-mesa-dev libglew-dev libjpeg-dev
export CXXFLAGS=-fPIC
cargo run --locked --release -p feishin-gpui
```

Run the app in a graphical X11 or Wayland session with a working Vulkan driver. Other distributions need equivalent packages. The [native CI workflow](.github/workflows/native.yml) lists additional tools used for automated display and integration tests.

To package the app, keep `CXXFLAGS=-fPIC` set in the same shell and run:

```sh
sh scripts/package-native-linux.sh
python3 scripts/package-native-release.py
```

The package directory is `target/feishin-linux/`, with the executable at `target/feishin-linux/bin/feishin`. The scripts also create `target/feishin-linux.tar.gz` and `target/feishin-native-linux-<architecture>.tar.gz`. These archives use the host's system libraries; they are not self-contained AppImages.

### Windows

The native Windows build and runtime still need validation. Rust cross-checks pass, but they do not verify the complete Windows application.

For an x64 build, install Visual Studio Build Tools with the C++ desktop workload and Windows SDK, the Rust MSVC toolchain, CMake, LLVM/libclang and vcpkg. In a Developer PowerShell, set the paths to your installations and install the native dependencies:

```powershell
$env:VCPKG_INSTALLATION_ROOT = 'C:\vcpkg'
$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'
& "$env:VCPKG_INSTALLATION_ROOT\vcpkg.exe" install glew:x64-windows-static-md zlib:x64-windows-static-md
cargo build --locked --release -p feishin-gpui
.\target\release\feishin.exe
```

The [native CI workflow](.github/workflows/native.yml) contains the Windows archive assembly steps.

### Development builds and optional audio tools

Once platform dependencies are installed, use a debug build for development:

```sh
cargo run --locked -p feishin-gpui
```

To build without opening a window:

```sh
cargo build --locked --release --workspace
```

The main executable is `target/release/feishin` (`feishin.exe` on Windows). Building the workspace also builds the audio-capture helper, which must sit beside the main executable for macOS system-audio capture. The macOS bundle script handles this arrangement automatically.

Native audio is the default. Install **MPV** to use the optional MPV playback engine, and **FFmpeg** for features such as network-speaker speed/DSP processing and platform-specific audio capture. Both are also used by audio integration tests. On macOS, `brew install ffmpeg mpv` supplies them; on Ubuntu, use `sudo apt-get install ffmpeg mpv`. Configure their executable paths in Settings when needed.

## How it works

The desktop window and controls are rendered by **GPUI**, with shared controls from **gpui-component**. The UI is implemented in Rust. The pinned versions are in [the desktop crate's manifest](crates/feishin-gpui/Cargo.toml).

The GPUI app calls the core crate to authenticate with your music server, fetch and normalize library data, manage the queue, stream audio and save preferences. Network and playback work runs off the UI thread. The default playback engine uses Rodio/Symphonia; the optional MPV engine is controlled through IPC. TagLib handles audio metadata, SoundTouch supports tempo changes, and projectM supplies MilkDrop visualizers.

The browser remote is a small HTML/JavaScript client embedded in the Rust app. When enabled, the app serves it through an authenticated HTTP/WebSocket service so a phone or another browser can control desktop playback. It does not need a separate frontend build or Node server.

Translations, theme definitions, fonts, icons and recorded regression fixtures live in the native workspace. Existing theme/CSS compatibility is implemented by the native styling code. Builds no longer read the retired Electron source tree.

| Path                                                         | Responsibility                                                                                    |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------- |
| [crates/feishin-gpui](crates/feishin-gpui)                   | GPUI views, application state, input handling and desktop integrations                            |
| [crates/feishin-core](crates/feishin-core)                   | Server APIs, library models, playback, metadata, storage, translations, themes and remote service |
| [crates/feishin-audio-capture](crates/feishin-audio-capture) | macOS system-audio capture helper                                                                 |
| [crates/vendor](crates/vendor)                               | Pinned native dependencies and local patches                                                      |
| [assets](assets)                                             | Application icons and font sources used by native tooling                                         |
| [scripts](scripts)                                           | Packaging, integration tests and asset maintenance                                                |

## First launch and existing settings

Add your server address and sign in when the app opens. The rewrite uses a separate **FeishinRust** data directory for settings, caches and saved queue state. Credentials use the operating-system password store. Existing Electron preferences and account definitions can be imported through Settings with a preview and backup; they are not imported automatically.

For a disposable development profile on macOS/Linux:

```sh
FEISHIN_DATA_DIR="$PWD/target/dev-profile" cargo run --locked -p feishin-gpui
```

On Windows, set `$env:FEISHIN_DATA_DIR` before running Cargo. This redirects application files; credentials still use the OS password store.

## Testing and current status

The first pass has automated core, GPUI, protocol, package and virtual-display coverage. macOS and Linux builds have passed the checks recorded in the [parity ledger](docs/native/PARITY.md), including packaged Linux workflows on X11 and Wayland. The [CI workflow](.github/workflows/native.yml) defines checks for all three desktop platforms.

Interactive listening, real devices, some OS integrations, broader server-version coverage and the native Windows build/runtime remain to be checked. See the [interactive acceptance checklist](docs/native/INTERACTIVE-CHECKS.md). Passing automated tests does not establish full feature parity.

```sh
rustup component add rustfmt clippy
cargo fmt --all -- --check
cargo test --locked --workspace -- --test-threads=4
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Browser-remote tests additionally require Node 22 or newer:

```sh
node --test scripts/native-tests/remote.cjs
```

The [native development guide](docs/native/README.md) covers required audio-test tools, extra component and protocol tests, isolated Linux window checks, server fixtures and package validation.

- [Feature parity and validation evidence](docs/native/PARITY.md)
- [Custom themes](docs/CUSTOM_THEMES.md)
- [Contributing](CONTRIBUTING.md)

## License

Feishin is licensed under [GPL-3.0-only](LICENSE). Bundled dependency licenses and source provenance are documented in [THIRD_PARTY.md](docs/native/THIRD_PARTY.md).
