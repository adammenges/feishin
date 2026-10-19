# Upstream ports

The Rust/GPUI rewrite started from `jeffvli/feishin` commit
`9dc15477c86fbd48d1a98fa878769d09811895e8`. This port covers the 16 subsequent
commits on upstream `development`, through
`3f321069e3126abe9763ceeba2cb57a979606647` (October 18, 2026).

Upstream behavior is implemented in the native crates. The retired Electron
application and its build dependencies are not merged back into this fork.

| Upstream commit | Native implementation or existing coverage |
| --- | --- |
| `947d5a1e` Up next queue | `feishin-core/src/queue.rs`: FIFO Play next entries, preserved identities across shuffle/replacement, consumed entries leave Up next, persistence repair. GPUI main/side queues render section bands with the existing virtual lists. General > Controls has the enabled-by-default setting, searchable and importable from Electron preferences. |
| `985d298a` Queue drop boundaries | Dragging above Up next can pin normal songs there; dragging into the section joins it; moving outside removes priority. Fetching library drops retains the original boundary. Move next uses the first pending song. Browser-remote reorder uses the same core rules. |
| `9f53ee4d` Queue delete focus | Native focus handles already restrict deletion to the active queue and exclude text controls. Backspace now joins Delete. Main and side selections remain independent. |
| `4e3ee968` Playlist album statistics | Consecutive album rows total duration, play count and track count and retain the latest nonempty last-played date. Duplicate tracks and nonadjacent groups keep their identities. |
| `27257096` Native Spotify queries | Spaces become `+` in native Spotify searches; literal plus signs remain percent-encoded. Web links retain their existing encoding. |
| `e5324a17` Row styles | Theme variables provide 3% hover and 6% selection overlays, black in light mode and white in dark mode. Native library, album-track, related, artist and queue rows retain alternate backgrounds and selected styling on hover. Queue section bands use the surface background. |
| `3a02de23` Private new playlists | Already implemented: the playlist editor and Add to playlist creation paths default to private. Existing-playlist edits retain visibility. |
| `dc96747a` Whole-second LRC timestamps | Already implemented in `lyrics::timestamp`; parser tests include `[00:02]`, mixed fractional precision and translations. |
| `319eefe4` Duration click propagation | Already implemented: `progress::duration_view` stops propagation before changing remaining-time display. The native pointer test asserts the full player stays closed. |
| `d76d989b` Zero favorites count | Already implemented: the count badge uses the explicit loading state; a loaded empty list renders `0`. |
| `9d87539f` Duplicate Electron mouse navigation | The removed Electron app-command handler has no native counterpart. Native back/forward navigation uses GPUI input handling. |
| `ae05f232` Docker analytics default | The retired Docker web build is absent; the native app has no analytics sender. No deployment configuration needs migration. |
| `c91cc2a9`, `3cece582`, `36f8e183`, `3f321069` Weblate translations | Changed keys ported into the 10 affected native catalogs. Native-only additions remain intact. Translation reference expectations are refreshed independently with upstream's i18next 25.10.10. |

The translation refresh is reproducible with
`scripts/native-tests/upstream-translations.cjs`; its two optional arguments are
the previous and new upstream commits. Its test-only dependency is installed
under ignored `target/upstream-oracle`, not in the app's dependency tree.

Validation on macOS arm64:

- `cargo test --locked --workspace -- --test-threads=4`: 291 core tests and 430 GPUI tests pass, along with audio-capture, credential-retention and hidden AppKit title-bar checks. The two existing opt-in tests remain ignored (archived TagLib fixtures and a real projectM graphics context).
- `cargo clippy --locked --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check` pass.
- `node --test scripts/native-tests/remote.cjs`: all 8 browser-remote checks pass.
- The optimized `target/Feishin.app` builds and passes strict code-signature verification. `target/feishin-native-macos-aarch64.zip` passes archive, executable-permission and all 58 license checks (SHA-256 `9b00cd9eca336633efa86a0f568bc5d6b1b5652e9e8fe369e7b951439eb40f47`).
- The queue regressions cover FIFO insertion, shuffle, replacement, persistence, duplicate identities, drag boundaries, search, section indexing and saved settings. Existing pointer tests exercise both queue views. Translation fixtures verify the new labels independently against i18next.

Physical desktop/listening acceptance remains deferred as documented in
[INTERACTIVE-CHECKS.md](INTERACTIVE-CHECKS.md).
