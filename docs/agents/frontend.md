# Native interface

Desktop views live in `crates/feishin-gpui/src` and implement GPUI rendering. Reuse the shared styles, appearance, input, menu and scrolling helpers and the pinned `gpui-component` controls before adding a new component.

Keep application state in the existing `Feishin` view and core models. Use GPUI entities and subscriptions for editable control state. Preserve unfinished input and focus across updates; discard obsolete asynchronous work when navigation or accounts change.

Use the native translation helpers and catalogs in `crates/feishin-core/assets/locales`. Reuse the native icon registry. Theme compatibility data is in `crates/feishin-core/assets`; native built-in styles are in `crates/feishin-gpui/assets/styles`.

Keep keyboard behavior, accessibility labels, disabled/expanded states, scrolling and right-to-left layouts working. Use the existing headless GPUI tests and captured fixtures for meaningful regressions. Physical interaction and screen-reader acceptance are tracked separately.

The browser remote under `crates/feishin-core/src/remote*` uses plain JavaScript, HTML and CSS served by the Rust application. Its tests are `scripts/native-tests/remote.cjs` and `remote-browser.mjs`.
