# Deferred interactive acceptance checks

These checks have not been performed. The user deferred interactive checks;
headless tests, protocol fixtures and package verification do not replace them.
Use the original application alongside the native build with the same server
and representative music. Native settings and caches use a separate directory.

- Window and navigation: open the packaged application; check login, Back/Forward,
  minimum-window layout, keyboard focus, table selection, drag/drop, popovers,
  expanded player and custom theme/CSS. Compare English and an RTL language.
- Dialogs: compare search, playlist selection, release notes, player/lyrics/
  visualizer settings and lyric export at enlarged text/zoom and in small windows.
  Check scrolling, entrance/exit motion, Escape, outside clicks, focus restoration
  and custom themes, including reaching Export below the visible viewport.
- Playback: listen to play/pause/seek/stop, gapless and crossfade boundaries,
  ReplayGain, EQ, compressor, speed/pitch and audiobook resume. Repeat with MPV.
  Change output devices during playback and disconnect/reconnect a device.
  Hold a seek preview across playback polls, switch total/remaining time, and
  check scrobble progress, reset and submission against the server dashboard.
- OS integration: verify media keys, tray/menu actions, macOS Dock reopening,
  close/minimize preferences, notifications, Discord presence and sleep inhibition.
  Repeat on each supported desktop OS. Windows native C++ linking and runtime
  still require an actual Windows build and check.
- Visualizers and lyrics: compare synchronization, timed words, scrolling,
  font/layout preferences, analyzer modes and projectM presets during playback.
  Check the player/sidebar action strips, per-song timing, language changes,
  search previews and confirmation after the playing song changes. Compare the
  separate player/sidebar settings and millisecond inputs in English and an RTL locale.
  Check system-audio capture only after granting the platform's capture consent.
- Server workflows: exercise account migration with backups, Quick Connect,
  playlist editing/sharing, artwork changes, radio streams and rating/favorite
  updates against the intended real server versions. Use disposable playlists
  and copies of audio files for tag-writing checks.
- Smart playlists: compare date entry and month/year navigation, including a
  six-week month and full date visibility at enlarged text sizes; search
  fields/operators/sorts, cancel searches, change language,
  edit custom field labels, and check Reset/Clear with unfinished values. Confirm
  that count/percentage drafts remain separate and playlist choices exclude the
  current playlist.
- Network outputs: test actual DLNA/Sonos discovery, grouped speakers, seeking,
  pause/resume, group member volumes and reconnection.
- Remote control: use a physical phone to install the remote app, authenticate,
  search, reorder the queue and control playback; verify reconnect/offline behavior.
- Accessibility: inspect the application with the OS screen reader, including
  ratings, menus, sliders, focus restoration and localized labels.
- Shortcuts: compare recording, local/global dispatch, shifted symbols, a non-US
  keyboard layout and numeric keypad keys on macOS and Windows. Confirm menu
  shortcut labels and that ordinary text editing retains character-based keys.

Record platform, server version, steps and results in [PARITY.md](PARITY.md).
Do not mark feature parity complete until remaining implementation differences
and these acceptance checks have been resolved.
