# Feishin media-control integration

Source: Souvlaki 0.8.3 from crates.io, MIT licensed.
Original archive SHA-256:
`5855c8f31521af07d896b852eaa9eca974ddd3211fc2ae292e58dda8eb129bc8`.
The original license, source, manifest and examples are retained.

The zbus backend now supports writable Shuffle and LoopStatus, Seeked signals,
unique track identities with stale SetPosition rejection, empty metadata, and
the extended Xesam fields used by Feishin's original Linux integration. Metadata
and status notifications are emitted only when the associated property changes;
ordinary progress updates do not announce a playback-status change. Duration
conversion saturates instead of panicking on overflow.

Additional typed metadata is ignored on macOS and Windows. Both continue using
the existing cover URL API. Windows clears previous fields before publishing a
replacement and decodes file URLs with the existing `url` dependency, preserving
drive letters, Unicode and escaped spaces in artwork paths. The manifest declares
the legacy Objective-C macro's
`cargo-clippy` cfg so current Rust can check other cfg values without warnings.

Windows retains and removes both button and position callback tokens when
detaching; reattachment first removes previous handlers. macOS likewise removes
old handlers before reattachment, registers Stop and reads the documented
`positionTime` property instead of a private instance variable. Invalid position
durations are ignored. These callbacks compile against their platform APIs;
physical media-control testing remains on the deferred interactive checklist.

The application uses `use_zbus`; the unused `use_dbus` backend is unchanged.
`mpris-reference.cjs` evaluates the original bridge. Native metadata and command
tests consume that reference, and the packaged Linux library workflow exercises
the real MPRIS service on a private D-Bus with silent audio and MPV's null output.
