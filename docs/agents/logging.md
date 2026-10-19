# Logging

Use Rust's `log` macros for diagnosable failures, fallbacks, engine lifecycle and connection state. Preserve the application's redacted rotating-log and diagnostics-export behavior.

- `error`: failed operation; include enough context to identify it.
- `warn`: degraded operation or an unexpected fallback.
- `info`: meaningful lifecycle changes.
- `debug`: diagnostic detail gated by the configured level.

Avoid noisy rendering, polling and ordinary success logs. Never log passwords, tokens, authorization headers, private stream URLs or values excluded by the diagnostics sanitizer. Keep user-facing errors separate from detailed diagnostics.
