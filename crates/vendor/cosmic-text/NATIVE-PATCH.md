# Feishin native optical sizing and font fallbacks

Source: complete cosmic-text 0.19.0 crates.io package, MIT/Apache-2.0.
Archive SHA-256: `be17b688510d934ce13f48a2beba700e11583e281e0fda99c22bb256a14eda73`.
Both upstream licenses are retained and copied into native packages.

Synthetic italic image and outline transforms use Chromium's quarter-unit
shear, rather than rounding its angle to 14 degrees. The existing synthesis
flags, glyph caches and scaler paths are retained. GPUI's independent font-style
checks cover actual synthesized Latin/Japanese/Hebrew raster bounds.

Missing bold faces use an additional cache flag and the existing Swash embolden
operation. Its strength matches Skia's interpolated synthetic stroke width,
including combined italic synthesis, without changing advances. Real bold and
weight-variable faces retain their own outlines. Original Electron comparisons
cover 360 font advances, raster ink extents and weight-dependent pixel changes.

This release already supports variable weight selection and rasterization.
The Feishin patch adds an optional logical optical size to FontSystem and its
font cache. Font constructs its existing HarfRust instance with both wght and
opsz coordinates. The default API keeps the original absent optical size.

Swash image/outline rasterization uses those same normalized axes. CacheKey
retains optical size separately from physical glyph size so different display
densities cannot reuse the wrong optical instance. GPUI uses the text's logical
size for shaping and keeps the actual weight/optical instance in each shaped run.
The original Inter reference checks requested weights and sizes; actual glyph
pixels check that heavier weights remain heavier after shaping and caching.
Ordered font fallback families are retained in Attrs/AttrsOwned and shape-run
cache keys. The shaper tries these families after the primary family and before
platform fallbacks, using the original FontFallbackIter. GPUI passes each
span's existing FontFallbacks list through this field.
Default families use fontdb's CSS nearest-weight/style/stretch query, including
static faces whose weight differs from the request. Missing families advance
to the next configured family rather than skipping directly to platform fonts.

Explicit paragraph direction is accepted by ShapeLine. A first span whose
writing direction opposes its paragraph starts from its logical trailing edge,
so the span is not omitted. Private application fonts remain available through
explicit primary/fallback families and are excluded from automatic platform
fallback. This prevents the bundled Symbola face from changing unrelated emoji
and missing-glyph behavior; the original 128 system-fallback caret cases verify
this against Electron on Linux.

Reimported private faces supersede older faces with the same PostScript name in
family matching and shape caches. The older fontdb entries remain available to
already shaped glyph identifiers. Default-family selection skips these superseded
entries while preserving the private-font exclusion from automatic fallback.


FontSystem can supply optional pre/post shaping context to HarfRust for a wrapped
paragraph fragment. Context-aware runs bypass the normal shape-run cache because
the same fragment bytes can have different Arabic joining forms in different
paragraphs. The caller clears the context after fragment shaping. Normal cached
shaping keeps its existing behavior. Original Electron word-wrap and multiline
editor references verify joining across emergency breaks on Linux.
