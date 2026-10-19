# Architecture

The desktop application is a Rust Cargo workspace.

| Tree | Responsibility |
| --- | --- |
| `crates/feishin-core` | Server APIs, data models, playback, metadata, persistence, translations, themes and HTTP/WebSocket remote service |
| `crates/feishin-gpui` | GPUI views, client state, input handling and desktop integrations |
| `crates/feishin-audio-capture` | macOS system-audio capture helper |
| `crates/vendor` | Pinned dependencies with local native patches |

Keep protocol/backend behavior in the core crate. GPUI code uses the core models and APIs, runs blocking work off the UI thread, and rejects results for obsolete accounts or views. The core crate does not depend on GPUI.

The browser remote is embedded from `crates/feishin-core/src/remote*` and communicates through the native authenticated service. It uses its own browser assets and does not share a React renderer or Electron process.

Build and run with Cargo. Native packaging, platform dependencies and validation commands are documented in `docs/native/README.md` and `.github/workflows/native.yml`.

Runtime assets belong to the native crates or `assets/`. Do not add dependencies on the retired Electron source tree. Preserve the checked-in parity fixtures when changing native behavior.
