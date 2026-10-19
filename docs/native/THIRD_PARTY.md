# Native third-party sources

Feishin is distributed under GPL-3.0-only. Its native dependency versions are
recorded in Cargo.lock. The native build compiles dependencies from their source;
the corresponding dependency sources can be fetched with `cargo fetch --locked`.

References below to legacy source-comparison generators record historical
provenance. Those generators were retired with the Electron application; the
captured fixtures, adapted native code and bundled license texts remain.

- Fuse.js 7.4.2 lyric scoring: Apache-2.0, copyright 2017 Kirollos Risk.
  `crates/feishin-core/src/lyric_match.rs` adapts its Bitap implementation and
  field normalization to the existing lyric search options. The license is in
  `crates/feishin-core/assets/fuse-LICENSE.txt`. The npm archive has SHA256
  25a7e4059b5b33ad8e947afac415f54f92ab91337657f14b3c9d43724e5e8317,
  and its SHA512 integrity was checked against the legacy lockfile when
  the saved reference fixtures were generated.

- validate-color 2.2.4: MIT, copyright 2018 Wallace Sidhrée. The original
  custom-theme validator supplies the expressions in
  `crates/feishin-core/assets/custom-theme-color-patterns.json`; Rust runs them
  with its bounded regex engine. The original loader supplies rejection and
  inheritance fixtures. `scripts/native-tests/custom-theme-colors-reference.cjs`
  reproduces both and the bundled `validate-color-LICENSE.txt`.

- wavesurfer.js 7.12.8: BSD-3-Clause, copyright 2012-2023 katspaugh and
  contributors. `crates/feishin-core/src/waveform.rs` adapts its drawing geometry
  to native rendering. The complete license is bundled in
  `crates/feishin-core/assets/WAVESURFER-LICENSE.txt`. The reference generator
  executes the same version pinned in the original app's lockfile.

- MilkDrop presets: MIT, copyright Jordan Berg. The embedded archive is generated
  from the original preset files in https://github.com/jberg/butterchurn-presets
  at revision c10e2616762f262de898e5d4c1f162a3e01340c1. Its license and source record
  are in `crates/feishin-core/assets/milkdrop-LICENSE.txt` and
  `milkdrop-SOURCE.txt`. `scripts/vendor-native-presets.py` reproduces it and the
  six MIT-licensed textures from the same repository's imageData.js. The cloud
  fallback image is decoded from Butterchurn 3.0.0-beta.5's npm archive, SHA256
  4bf374256652c08ef077727e346dfdb6381e598099bbadc3da4a573b7b037c39.
  Its MIT license is in `crates/feishin-core/assets/butterchurn-LICENSE.txt`. Legacy EEL
  equation whitespace is normalized for projectM-eval without changing values.
- projectm-sys 1.2.2: LGPL-3.0-or-later. Its complete source and LICENSE are
  included in `crates/vendor/projectm-sys`.
- projectM 4.1.8: LGPL-2.1-or-later. Its complete source, including third-party
  license files, is included under `crates/vendor/projectm-sys/libprojectM`.
  The framebuffer API backport and build adjustments are documented in
  `FEISHIN-PATCH.md`. The native binary statically links this library.
- TagLib 2.3.2: LGPL-2.1-or-later (selected from the upstream LGPL/MPL dual
  license). Complete unmodified source and both licenses are in
  `crates/vendor/taglib`. The native binary statically links this library.
  `scripts/vendor-native-taglib.py` reproduces the official release archive,
  SHA256 3ca2d8afaa7f1cf7f6ed10e511ebc368bfacd6dcaa3dbfa690b89e502e8963dc.
  The bundled UTF8-CPP library has a Boost Software License, in
  `crates/vendor/taglib/3rdparty/utfcpp/LICENSE`.
- TagLib-WASM 1.5.1: MIT, copyright Charles Wiltgen. Original tag property names
  supply alias mappings and development-only reference fixtures. Its license is
  in `crates/feishin-core/assets/TagLibWasm-LICENSE.txt`; the WASM library is not
  used by the native runtime.
- Surfman 0.14.0: MIT/Apache-2.0/MPL-2.0; glow 0.17: MIT/Apache-2.0/Zlib. Their
  versions are pinned in Cargo.lock and obtained through crates.io.

- xattr 0.2.3: MIT/Apache-2.0. The complete source and both license texts are in
  `crates/vendor/xattr`. `NATIVE-PATCH.md` records the Linux/Android errno fix
  needed by GPUI 0.2.2 with libc 0.2.190.
- tray-icon 0.26.1: MIT/Apache-2.0. Its source is obtained through crates.io.

- Inter and Poppins: SIL Open Font License 1.1. Native builds decode the same
  `assets/fonts` WOFF2 files used by Electron. Copyright and license texts are
  in `crates/feishin-core/assets/inter-OFL.txt` and `poppins-OFL.txt`.
- Noto Sans JP and Noto Sans Hebrew: SIL Open Font License 1.1, copyright
  Adobe 2014-2021 (reserved name Source) and The Noto Project Authors 2024.
  The original Electron WOFF2 files remain in `assets/fonts`. The native
  `scripts/vendor-native-fallback-fonts.py` uses fonttools 4.61.1 to retain
  their CSS unicode ranges and five fixed font-face weight mappings. Latin
  measurement glyphs are retained for GPUI. `crates/feishin-core/assets/fonts/SOURCE.json`
  records original and derived hashes; `noto-jp-OFL.txt` and `noto-hebrew-OFL.txt`
  retain the original copyright and complete license.
- Symbola 9.00: Unicode Fonts for Ancient Scripts, George Douros 2016.
  The original font's name records permit any use. Their complete permission,
  copyright, source address and unchanged WOFF2 hash are retained in
  `crates/feishin-core/assets/symbola-LICENSE.txt`.
- cosmic-text 0.19.0: MIT/Apache-2.0. The complete crates.io source and both
  licenses are in `crates/vendor/cosmic-text`. Its original variable-weight
  support replaces the older Linux text engine; `NATIVE-PATCH.md` documents
  matching logical optical sizes in shaping, rasterization and glyph caches.
- wuff 0.2.9: MIT/Apache-2.0, https://github.com/nicoburns/wuff. It decodes
  custom WOFF/WOFF2 fonts; decoded fonts are bounded and parsed before loading.

- screencapturekit 11.0.0, apple-cf 0.11.0, apple-metal 0.10.0 and doom-fish-utils
  0.4.1: MIT/Apache-2.0, obtained from crates.io. The macOS capture helper uses
  their platform bridges, isolated from the main player's runtime dependencies.
  ScreenCaptureKit source: https://github.com/doom-fish/screencapturekit-rs.

- Mantine colors-generator 9.3.1 (MIT) and chroma-js 3.2.0 (BSD-3-Clause):
  the native accent shade calculation ports the original generator and its D65/LCh
  saturation adjustment. License texts are in `crates/feishin-core/assets/` as
  `mantine-colors-LICENSE.txt` and `chroma-LICENSE.txt`. The 45 color scale fixtures
  in `accent-reference.json` were generated with these exact npm versions.
- Mantine core 9.3.1: MIT. Its original default color arrays and CSS variable
  maps are embedded in `mantine-defaults.json`; the application adds its own
  TypeScript theme defaults and global CSS aliases during the Rust build.
  `scripts/native-tests/mantine-variables.mjs` generates 276 independent
  color/contrast references from this exact npm package. Its license is bundled
  as `mantine-core-LICENSE.txt`. The rating stars use its original StarIcon
  geometry, reproduced by `scripts/native-tests/rating-reference.cjs`.
  The continuous accent picker adapts its HSV conversion and pointer/keyboard
  behavior in `crates/vendor/gpui-component/src/color_picker/spectrum.rs`;
  `accent-spectrum-reference.cjs` and `accent-spectrum-layout.cjs` reproduce
  its behavior and geometry fixtures from the locked package.

- Player icons are converted from the existing locked react-icons 5.6.0 package.
  Lucide uses ISC/MIT; the repeat/random icons use Remix Icon 4.2.0 under
  Apache-2.0, and the loading icon uses css.gg under MIT.
  The npm SHA512 is checked by `scripts/vendor-native-icons.py`; SVG attribute
  names and filled player controls match the original React rendering. License
  texts are in `crates/feishin-gpui/assets` and included in native packages.

- Fast Average Color 9.5.0: MIT, copyright Denis Seleznev 2025. The native
  artwork backgrounds port the original alpha-weighted square-root and dominant
  calculations, ignored-color thresholds and winner ordering. The pinned npm
  SHA512 is checked by `scripts/native-tests/expanded-album-colors.py` when
  generating original-runtime references. Its license is bundled as
  `fast-average-color-LICENSE.txt`.

- csscolorparser 0.9.0: MIT/Apache-2.0, obtained from crates.io. Its CSS Color 4
  parser uses D50/Bradford conversion for Lab/LCh. License texts are in
  `crates/feishin-core/assets` and are included in native packages.
- moxcms 0.8.1: BSD-3-Clause, copyright Radzivon Bartoshyk, obtained from
  crates.io. Complete source is retained in `crates/vendor/moxcms`; its
  `NATIVE-PATCH.md` records the small CMYK compatibility patch and source hash.
  The shared native artwork decoder uses its RGB/grayscale/CMYK ICC conversion
  into sRGB. Its original license text is included as
  `crates/feishin-gpui/assets/moxcms-LICENSE.txt` and bundled in native packages.
- png 0.18.1: MIT/Apache-2.0, obtained from crates.io. Prepared backgrounds
  encode composited GIF/WebP/APNG frames as animated PNG while retaining their
  display delays. Both original license texts are included as
  `crates/feishin-gpui/assets/png-*-LICENSE.txt` and bundled in native packages.
- zune-core 0.5.3 and zune-jpeg 0.5.15: MIT, obtained from crates.io. CMYK
  and YCCK JPEG artwork retains its original components for ICC conversion.
  The original MIT license texts are bundled as `zune-*-MIT-LICENSE.txt`.
- The Artifex CMYK SWOP profile (copyright Artifex Software 2011), AGPL-3.0-or-later,
  is used only in synthetic JPEG test fixtures, without runtime linkage. Its
  unchanged source, SHA256 and upstream license are retained under
  `scripts/native-tests/assets/ghostscript-CMYK*` and `ghostscript-*`.
  The profile comes from Ghostscript revision
  `22d8a2653ae532c7c23aa9a441e6c120edd9bdef`; its license is also bundled as
  `crates/feishin-gpui/assets/ghostscript-AGPL-LICENSE.txt`.
  The independent fixture generator uses the installed libjpeg encoder and
  Chromium canvas output, rather than the native JPEG decoder.
- The original background-noise SVG is rendered as a 300px PNG by
  `scripts/native-tests/theme-reference.py` in isolated headless Chrome. The same
  script generates independent CSS color and gradient pixel fixtures.

The repository includes all local library modifications. Follow README.md to
rebuild and relink the application with modified library sources. Release source
archives must include the native crates, vendor source, Cargo.lock, scripts and
licenses alongside the rest of the Feishin source. Package scripts include the
Feishin, MilkDrop, projectM and TagLib license texts in the binary artifacts.

- cssparser 0.37.0: MPL-2.0, obtained through crates.io. The native stylesheet
  parser retains CSS tokenization, nested blocks and selector cascade. Its license
  is included as `crates/feishin-core/assets/cssparser-LICENSE.txt`.
- notify-rust 4.18.1: MIT/Apache-2.0, obtained through crates.io. The native player
  uses its macOS, Windows and XDG notification backends. Both license texts are
  included in `crates/feishin-gpui/assets` and native packages.

- regress 0.11.1: MIT/Apache-2.0, https://github.com/ridiculousfish/regress.
  Playback filters use its ECMAScript expressions, including lookaround,
  backreferences and UTF-16 matching. Both license texts are bundled in
  `crates/feishin-core/assets/regress-*-LICENSE.txt`.

- gpui-component 0.5.1: Apache-2.0. Its complete crates.io package source is
  included in `crates/vendor/gpui-component`; `NATIVE-PATCH.md` records the
  CSS interaction-state callbacks, separate context/dropdown classes and fixed
  context previews, hover foreground correction and safe rich
  text URL filtering, plus a weak optional browser dependency for the native
  inspector feature. Its license
  is also included in `crates/feishin-gpui/assets/gpui-component-LICENSE.txt`.

- Skia's synthetic outline antialias filter: BSD-3-Clause, copyright Google Inc.
  The macOS font rasterizer adapts `pack4xHToMask` and its coefficients from the
  original Chromium 150 Skia revision `9c7b2dffb2433f5a0cc2b77f06025a09126807ed`.
  The original license is retained as
  `crates/feishin-gpui/assets/skia-LICENSE.txt` and bundled in native packages.

- GPUI 0.2.2: Apache-2.0. The complete crates.io source package is included in
  `crates/vendor/gpui`; `NATIVE-PATCH.md` records its archive digest and retained
  Wayland window changes. The license is also bundled as
  `crates/feishin-gpui/assets/gpui-LICENSE.txt`.

- ICU4X icu_plurals/icu_locale_core/icu_segmenter/icu_segmenter_data/icu_collator/icu_collator_data,
  icu_datetime/icu_calendar/icu_decimal/icu_casemap and their compiled data 2.3.0,
  icu_time 2.3.0/icu_time_data 2.3.1 and fixed_decimal 0.7.2: Unicode-3.0,
  obtained through crates.io. Their compiled Unicode plural data implements the
  original catalog's language-specific number forms; compiled segmentation
  dictionaries retain language-aware lyric cue boundaries without a network
  service. Compiled collation data preserves locale-aware tag ordering, including
  numeric server values and locale-specific casing. These ICU crates require
  Rust 1.88, below this workspace's 1.91
  minimum. The license is bundled as
  `crates/feishin-core/assets/icu-LICENSE.txt`. i18next 25.10.10 is used only to
  generate the independent catalog reference fixtures from the lockfile version.

- audioMotion-analyzer 4.5.4 (AGPL-3.0-or-later), copyright Henrique Avila
  Vianna: `scripts/native-tests/spectrum-reference.mjs` executes the original
  npm dependency in an isolated Node VM to generate analyzer parity expectations.
  The archive integrity was checked against the legacy lockfile; the dependency is used by the
  reference generator and is not linked into the native application. The five
  built-in gradient definitions are preserved as normalized reference data.
  The Rust FFT and GPUI drawing implementation is checked against those
  expectations.

- AccessKit 0.24.1, accesskit_consumer 0.35.0/0.36.0/0.38.0 and the native
  macOS 0.26.3, Unix 0.21.1 and Windows 0.32.1 adapters: MIT OR Apache-2.0, with Chromium-derived
  BSD code. License texts and AUTHORS are copied verbatim from AccessKit revision
  `a55d3e1a18bb9ef0e4bccc9083fb13c3e0ad8969` into the native app's assets and all
  binary packages. The upstream GPUI adapter integration is backported into the
  vendored Apache-2.0 GPUI source; see its `NATIVE-PATCH.md` for provenance.

- Taffy 0.9.0: MIT. The complete crates.io package is included in
  `crates/vendor/taffy`; `NATIVE-PATCH.md` records archive provenance and ancestor
  layout cache invalidation. Its unchanged license is also included as
  `crates/feishin-gpui/assets/taffy-LICENSE.txt` and in native binary packages.

- Kuroshiro 1.2.0: MIT. Its original Hepburn character table and ordered
  conversion rules are retained for lyric pronunciation parity; the 357-entry
  table reproduces from the pinned reference package. The upstream license is
  bundled as `crates/feishin-core/assets/kuroshiro-LICENSE.txt`.

- string-to-color 2.2.2, colornames 1.1.1 and lodash.words 4.2.0: MIT.
  The native genre badges retain the original seeded hash and named-color mixing.
  `scripts/native-tests/genres-reference.cjs` reproduces the Unicode word pattern,
  named-color RGB lookup and original-runtime fixtures from pinned npm packages.
  Complete licenses are bundled as `STRING-TO-COLOR-LICENSE.txt`,
  `COLORNAMES-LICENSE.txt` and `LODASH-WORDS-LICENSE.txt` in core assets.

- Day.js 1.11.21: MIT, copyright iamkun. The native metadata formatter uses
  locale patterns and relative-time vocabulary generated from the original
  dependency. The locale reference executes the original helpers and components
  in pinned Chromium. The license is bundled as
  `crates/feishin-core/assets/dayjs-LICENSE.txt`.

- Souvlaki 0.8.3 (MIT) is retained in `crates/vendor/souvlaki` with a small
  zbus extension for the original Linux media controls and metadata. Its
  `NATIVE-PATCH.md` records the source digest and changes; packages include
  `SOUVLAKI-LICENSE.txt`.
