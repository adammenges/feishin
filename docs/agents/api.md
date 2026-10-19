# Server APIs

Server requests, authentication, backend differences and normalization live in `crates/feishin-core/src/api.rs`, with shared domain types in `model.rs`. Read the existing method and its callers before changing a request or response mapping.

- Reuse the shared API/client instead of issuing one-off requests from GPUI views.
- Preserve account identity, request cancellation, pagination and stale-result rejection.
- Keep Navidrome, Subsonic and Jellyfin response details in the core data layer.
- Validate untrusted responses and preserve credentials and pending mutations across recoverable failures.
- Extend the existing protocol fixture or disposable server integration check when changing backend behavior.

Use in-repo models, implementation and tests first. Consult an upstream API contract only when a required detail is missing or contradictory, and read only the relevant operation. Do not make normal builds depend on a remote schema.

Server integration instructions are in `docs/native/README.md`; disposable current/older backend checks use `scripts/native-tests/live-servers.py`.
