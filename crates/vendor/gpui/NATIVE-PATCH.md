# Feishin native GPUI patches

Source: the complete gpui 0.2.2 crates.io package (Apache-2.0), with archive
SHA-256 `979b45cfa6ec723b6f42330915a1b3769b930d02b2d505f9697f8ca602bee707`.
The package records Zed revision `69e2130295c2649963eb639fc70b4f2ee8ea1624`.
Its normalized Cargo.toml adds AccessKit 0.24, accesskit_macos 0.26,
accesskit_unix 0.21 and accesskit_windows 0.32. The existing Unicode segmentation dependency is used for accessible word boundaries.
The image dependency's existing moxcms 0.8.1 dependency is also used directly,
with its in-place transform feature, for embedded ICC color profiles.
Other existing dependencies are retained.

Modified source files:

- `src/style.rs`, `src/styled.rs`, `src/element.rs` and `src/elements/div.rs`:
  optional relative paint-scale origins preserve fixed lyric line layout while
  scaling inactive text around its left, center or right alignment. Painting,
  pointer hit areas and accessible bounds share the transform. The default
  origin remains the element center. Native comparisons cover 288 original
  baseline lyric layouts plus 864 optional information layouts, including zoom,
  wrapping and sidebar padding. The public transformed-bounds accessor lets
  rich lyric links map pointer positions back into their shaped text.
- `src/scene.rs`, `src/style.rs`, `src/styled.rs`, `src/window.rs` and the
  Metal/WGSL/HLSL filter shaders add a bounded vertical alpha mask to the
  existing foreground layer pipeline. The lyric viewport uses the original
  15% edge ramps. Eight original pixel references per platform cover layer
  opacity, fractional geometry and transformed mask coordinates.

- `src/platform.rs` and `src/window.rs`: expose an optional `set_visible` hook.
  Unsupported platform implementations return false.
- `src/window.rs` exposes whether keyed element state belongs to the current
  rendered frame, allowing delayed gestures to reject removed controls.
  `src/elements/div.rs` adds an outside-pointer-movement listener independent of
  hover state, which otherwise ends during a held mouse press.
- `src/window.rs`: expose the existing monochrome sprite transform when painting
  shaped glyphs. The default glyph method retains its identity transform; native
  circular analyzer labels rotate around their baseline without rasterizing a
  separate canvas on every frame.
- `src/platform/linux/wayland/window.rs`: retain the window and renderer when
  hiding its surface, suppress frame requests and buffer presentation while
  hidden, and restore title, app ID, minimum size and requested window state
  before the initial empty commit. Showing waits for a fresh xdg_surface
  configure/ack before forcing the first rendered frame. Repeated requests
  preserve state and send no duplicate protocol requests.
- `src/taffy.rs`: make two existing f32 literals explicit for current Rust's
  float literal fallback lint. Values and layout behavior are unchanged.

The hide/show sequence follows the unmapping contract in Wayland's stable
`xdg-shell.xml`. Native macOS, Windows and X11 visibility helpers remain in
the application. All code needed to rebuild this patch is included here.
The package's Apache license is copied into native binary packages.

`scripts/native-tests/wayland.py` runs the actual GPUI example under Weston
inside Xvfb, with a software Vulkan driver. It verifies initial presentation,
null-buffer unmapping, hidden view invalidation, restored attributes, the
empty commit/configure/ack/buffer sequence, repeated visibility requests,
rapid restores and retained view state. No physical desktop is used.

## Calculated layout lengths

`DefiniteLength::Calc` retains a pixel offset and a percentage until layout has
its containing size. The Taffy adapter stores those terms for the lifetime of its
layout tree, using aligned one-based integer handles rather than pointers to Rust
allocations. Dimensions and padding clamp negative results; margins and offsets
retain signed values. Measured leaves use the same resolver as containers.
Native CSS zoom scales the pixel term independently of percentages. Browser
references cover mixed arithmetic and the shipped Glassy expanded-player rule.

## Nested popovers

Deferred drawing accepts nested overlays. Children are prepainted and painted
above their parent in the same order, retaining sibling priority, view identity,
focus ancestry and inherited inert state. The queue stays in the frame during
prepaint so cached subtrees retain valid ranges for nested overlays. Player and
output settings use this path to escape scrolling content clips. Headless checks
exercise nested menus, compact localized Settings, Escape and cached/inert input.

## Physical shortcuts

Keystrokes retain optional physical key identity and modifiers before character
normalization. macOS uses hardware key codes, Windows uses extended scan codes,
and X11/Wayland share XKB position names. Unknown hardware positions are marked
unidentified so they cannot accidentally become ANSI letter shortcuts. Synthetic
test strokes can infer ANSI positions when platform metadata is absent.
`KeyBinding::with_physical_keys` opts into position matching while retaining the
existing readable key for menu and tooltip labels. Ordinary bindings and typed
characters keep their existing matching behavior. The host's tests cover the
original recorder's 102 keys, shifted punctuation, layouts, keypad separation,
saved recording and legacy bindings; its private Linux display workflow exercises
the actual platform input path.

## Accessibility

Backported from Zed commit `1d029c5ff5654fb1b1e8caf4462993c8ee13a133`
(the upstream GPUI AccessKit integration, Apache-2.0), adapted to GPUI 0.2.2:
`src/element.rs`, `src/elements/div.rs`, `src/elements/text.rs`, `src/gpui.rs`,
`src/platform.rs`, `src/window.rs`, `src/window/a11y.rs`, `src/_accessibility.rs`,
macOS `window.rs`, Linux Wayland/X11 `window.rs`, and Windows `window.rs`/`events.rs`.

The platform adapters publish native accessibility trees and dispatch actions.
The Feishin adaptation uses existing futures channels, releases macOS adapters
outside window locks, and holds Windows accessibility state separately from its
rendering state to permit UI Automation reentrancy. Ordinary and styled text
receive parent-scoped positional IDs; explicit IDs are recommended for controls.
Hidden subtrees, disabled actions, stale IDs and unsupported requests are filtered.
Explicit presentational containers retain accessible children without exposing
event-propagation wrappers as buttons. Live regions publish the message itself
as their accessible label and request polite announcements.
Only existing nodes may become accessibility focus targets. Test-support methods
activate, inspect and exercise the production tree and action path headlessly.

Editable controls publish bounded text runs, UTF-8 character lengths, word
boundaries, CRLF line breaks, links between long-line chunks and current selection.
Native selection requests are translated back to editor offsets only after both
endpoints are validated against the requesting control. Password documents are
empty and do not expose their contents. Controls remain editable through native
keyboard input after an accessibility focus request.

## Native artwork decoding

`src/assets.rs`, `src/elements/img.rs` and `src/platform.rs` share the raster
frame decoder. Both file resources and byte images apply EXIF rotations and
reflections, retain GIF/WebP/APNG frames and delays, and convert RGBA into native BGRA.
APNG subframes are composited by the existing image decoder; separate default
poster images are excluded from the animation.
Invalid orientation metadata falls back to the untransformed decoded image.
Still-image allocation limits remain enforced. The shared decoder accepts
caller-supplied dimensions and allocation limits, bounding cumulative animation
frame storage as well as each decoded frame. Direct SVG images use the same
straight-alpha BGRA conversion as file resources; `src/color.rs` rounds the
unpremultiplication rather than truncating a fully saturated channel to 254.
RGB and grayscale ICC profiles are converted into the renderer's sRGB space,
including alpha preservation and animated WebP frames. Invalid profiles retain
decoded pixels. CMYK and YCCK JPEG retain their four original components through
the existing zune decoder. Their black component is combined before ICC conversion,
matching Chromium's integer division and legacy Lab handling. Color conversion
reuses the decoded buffer and bounds temporary staged storage to one scanline.
The small moxcms compatibility patch is documented in its NATIVE-PATCH.md.
The host application's image tests cover all eight orientations for PNG/JPEG/WebP,
missing/invalid metadata, oriented animated WebP, GIF timing and SVG alpha/color,
plus independent Chromium pixels for sRGB, Display P3, Adobe RGB and grayscale ICC
profiles plus tagged/untagged CMYK/YCCK JPEG in file/byte images and prepared
backgrounds.

## Native CSS text decoration patterns

`UnderlineStyle` and `StrikethroughStyle` add a serde-defaulted line pattern.
The existing solid and wavy underline behavior remains available; native CSS can
also draw dotted, dashed and double lines, including wavy strikethroughs. Dots and
dashes use existing rounded quads, and double lines use existing underline
primitives, without changing platform shaders. Segment work is limited to the
visible horizontal range and a minimum physical pixel thickness. Text runs retain
the pattern across wrapping and color changes. Existing explicit style literals
select the solid default.

The host's headless CSS tests check shorthand/longhand cascade and actual painted
line/quad geometry for wrapped text, including style removal. A test-support-only
window snapshot exposes the painted bounds, colors, rounding and wave flags.

Overlines are carried independently through TextStyle, HighlightStyle, TextRun
and DecorationRun. The line painter positions them at the font ascent and retains
color/pattern changes across styled and wrapped runs. CSS can combine all three
line decorations or clear their inherited native styles. Ordinary explicit run
and highlight literals retain an absent overline by default.

TextStyle stores the optional overline through indirection so adding it does not
inflate the nested element render stack. The full screen-rendering regression
passes with the ordinary test stack, without a stack-size override.

## OpenType font features

`Styled::font_features` applies the existing inherited FontFeatures value without
changing the element's other font properties. The cached WindowTextSystem
constructor is public so the actual platform shaper can be exercised under
Application::headless without creating a window. CoreText feature dictionaries
retain their strings/numbers until construction; temporary feature/fallback
arrays are released after the descriptor dictionary retains them.

The native root restores the original tabular-number default. Custom numeric
variants and low-level settings cascade and inherit separately, including
explicit clearing and low-level tag precedence. Independent Chromium fixtures
check 54 CSS values and 2,080 Inter/Poppins measurements; the example uses the
real native platform engine. Variable Inter weights and optical sizes are retained
through CoreText and Linux shaping/rasterization. Actual glyph pixels verify nine
increasing weights and distinct logical sizes at equal physical size. Linux's
integer font-unit advance rounding is bounded independently of browser layout
quantization; macOS retains the strict 0.02px measurement threshold.

`Styled::font_fallbacks` exposes the existing inherited fallback list. Missing
primary families now try this list before the platform defaults, retaining the
requested weight, style and features. CoreText cascade descriptors resolve
actual embedded faces and their requested weights, rather than looking up
private fonts by a system-only family name. DirectWrite mappings also use the
custom collection before the system collection. Linux spans retain ordered
fallback families through the small cosmic-text attribute patch, and include
them in font selection and glyph instance caches.

The Linux text dependency is pinned to cosmic-text 0.19.0 for its original variable
weight support, with the small optical-size patch documented in that package.
Windows text formats opt into automatic optical sizing when DirectWrite supports
it. Requested weight/style remain separate from the initially matched face;
shaped variable faces are retained with their actual axis values in the glyph
cache. Windows API calls cross-compile; actual Windows rendering remains pending.

Private font registration invalidates cached family lookups, including previous
missing-family fallbacks. Reimporting a face with the same PostScript name replaces
its selection entry while keeping earlier font identifiers alive for cached frames.
Input wrapping checks the font registration revision even when the family and style
are unchanged. The headless `font-reload` example checks late import, changed metrics
and glyph pixels, and retained old layouts using the actual platform font system.
An input regression additionally verifies rewrapping without changing its text.


## CJK punctuation and caret cells

`TextSpacingTrim` inherits the four Chromium CSS modes through `TextStyle` and
`TextRun`; GPUI defaults to `SpaceAll` and Feishin explicitly selects `Normal`.
Existing Unicode property and line-break dependencies identify contextual
punctuation and legal CJK boundaries. Selected font runs retain all original
features while adding OpenType `halt`; explicit exclusive spacing features keep
priority. CoreText fallback descriptors and Linux span compatibility retain
those features. Adjacent text spans resolve their actual font before merging.

Wrapped CJK lines reuse cached fragment shaping, reshape their physical starts
and ends, and retain logical character cells independently of glyph drawing
offsets. CoreText opening offsets and COSMIC glyph offsets reach the existing
paint path without moving caret, selection or decoration positions. Soft-line
caret boundaries use the next line and final Unicode glyph cells support nearest
caret selection. Wrapped layout cache keys include the requested line limit.

The pinned original Electron 43.7.0 restores paragraph-final closing advances
after line-fit checks. Linux fits also retain Chromium's ceiling of cached
paragraph positions to 1/64px and FreeType's 16.16 scaling of 26.6 pixel sizes,
including reshaped starts and closing/opening boundaries. These behaviors are
checked independently against original Electron, rather than newer Chromium.
References: Chromium tag 150.0.7871.250 `shaping_line_breaker.cc` and
`shape_result.cc`, plus FreeType's sizing/scaling and linear-advance documentation.

`scripts/native-tests/font-spacing.py`, `font-probe.cjs` and the two native
font-spacing examples cover 2,928 single-line and 3,264 wrapped measurements on
macOS/Linux, actual opening-glyph pixels/displacement, caret cells and changing
title line limits. The Electron probe uses a standalone WebContentsView without
native windows; Linux runs it inside an isolated Xvfb display.

## Paragraph direction and logical text cells

Missing italic faces use CoreText's native font matrix, Swash's existing
synthesis flags or DirectWrite's simulated font-face reference. Chromium's
quarter-unit shear is retained on macOS/Linux. Requested Linux styles reach
shaping and raster caches independently; DirectWrite identifiers additionally
include simulation flags. Native font-styles checks cover 126 independent
Chromium advances and ink extents, including Japanese/Hebrew fallbacks. They
separate one-pixel synthetic raster bounds from the existing two-pixel upright
FreeType/Swash difference. Windows synthesis calls compile against actual
DirectWrite bindings; runtime rendering remains pending.

Monochrome CoreText glyphs use native black-on-white color-surface font smoothing
before extracting grayscale coverage. Alpha-only surfaces omit this smoothing,
which lost outer pixels on variable italic faces. Raster bounds use the actual
drawing size, including optical axes, and retain room for smoothing.

Missing bold faces additionally use distinct raster identities on all platforms.
Real bold faces and fonts with a weight axis bypass synthesis. CoreText outline
strokes retain Skia's size-dependent width and horizontal four-sample grayscale
filter from `pack4xHToMask` in the original Chromium 150 Skia revision
`9c7b2dffb2433f5a0cc2b77f06025a09126807ed`. The filter's BSD-3-Clause license is
retained in `crates/feishin-gpui/assets/skia-LICENSE.txt` and native packages.
Mac fallback runs carry the requested bold state; their native font and synthesis
flag form the raster cache identity. Swash and DirectWrite combine bold and italic
flags. Original Electron checks cover 360 regular-only, real-bold and variable
font advances, ink extents and pixel changes. Physical Windows checks are pending.

TextLayout::paint_with_color repaints cached shaped rows in a new color without
changing their wrapping, fonts, alignment or glyph positions. Native karaoke
clips this repaint to the word progress; headless glyph/clip checks cover Latin
and CJK/emoji long words across all lyric alignments and progress positions.

TextDirection inherits through TextStyle/TextRun and font-run cache keys.
TextAlign Start/End resolve against the paragraph direction. Independently
reshaped rows retain the full Unicode bidi paragraph context when wrapping.
Grapheme cells carry logical leading/trailing edges and dual insertion edges;
pointer hits preserve the selected edge at a change of writing direction.
Selection backgrounds and decorations use the selected physical intervals,
including disjoint ranges, instead of assuming glyph source indices increase.

macOS uses the existing native font for rasterization and HarfRust for complex
clusters, RTL runs and emoji advances. Its font bytes are cached by PostScript
name, with native variable axes and logical optical size. Combining-mark
insertion edges follow the cluster's total advance. Invisible bidi controls
retain zero-width cells in their resolved visual runs. Linux uses COSMIC bidi
levels and grapheme clusters. DirectWrite uses the actual run origins, reading
direction and signed glyph offsets; its new calls and glyph loop compile against
the official Windows bindings, while Windows runtime checks remain pending.

The text-cells and text-wraps examples compare 144 single-line and 192 wrapped
cases with original Electron 43.7.0 on macOS/Linux. Another 128 cases cover
Arabic, Persian, emoji sequences and invisible bidi controls. Input event tests
use the native font shaper in deterministic test windows, including clicked
insertion edges and the tessellated area of disjoint selection highlights.


## Browser-compatible wrapping and editing

TextStyle/TextWrap exposes Normal, BreakWord, BreakAll, KeepAll and
KeepAllBreakWord and reaches the shared
paragraph shaper. BreakWord reshapes emergency grapheme breaks in full paragraph
context; BreakAll preserves forbidden opening/closing punctuation boundaries.
Normal line breaking retains Blink's internal hyphen/slash behavior. The original
Electron reference verifies 576 layouts across fonts, sizes, directions and modes.
Another 1,008 original layouts verify keep-all and its overflow-wrap combinations.
Letter and number units stay together while punctuation and emoji retain their
original boundaries. Emergency breaks retain paragraph punctuation spacing;
trailing zero-width controls attach to their original visual item in RTL text.
WrappedLine::shaped_rows extracts the painted physical rows, source text, colors,
fonts, decorations, bidi levels and grapheme cells without reshaping them again.
L1 reorders trailing whitespace while editing retains the original item direction
and bidi level. Logical editing carets follow Chromium 150's legacy caret/hit-test
boundary adjustment. The macOS native fallback registry retains default font
features for discovered fallback faces, including punctuation and emoji.
Linux paragraph fragments pass pre/post text context to the platform shaper so
Arabic joining forms survive emergency breaks. Windows runtime checks remain
pending. Chromium source reference: tag 150.0.7871.250 bidi_adjustment.cc,
selection_modifier_line.cc, caret_rect.cc and line_breaker.cc.

## Raster element backgrounds

Fill::Image accepts a shared runtime renderer for size-dependent backgrounds.
Style::paint draws the solid underlay and raster before children, using the
element's corners, clipping and inherited opacity. ImageFill records whether an
image-only refinement preserves the element's existing background color.
The native application reuses its gradient rasterizer and bounded display-scale
image cache rather than duplicating gradient math in platform shaders.
Window's headless helpers expose raster and glyph bounds, clips and opacity;
VisualTestContext::debug_bounds accepts temporary selectors without requiring
a static lifetime. 44 independent Chromium background references cover both
normal and Retina resolution.

## Backdrop filters

BackdropBlur primitives fence scene batches before their own backgrounds and
children. Metal, Blade and Direct3D snapshot the already painted scene, apply a
bounded separable Gaussian kernel and composite through the original bounds,
corner radii, content mask and inherited opacity. Large kernels first average
successive 2x2 texture levels, preserving fine source detail. Texture levels are
reused; scenes without filters retain their direct presentation path. Ordered
brightness, contrast, saturation, grayscale, sepia, hue rotation, inversion and
opacity follow the [CSS Filter Effects matrices](https://www.w3.org/TR/filter-effects-1/).
Consecutive color operations execute in one pass, retaining float precision and
clamping after each function. Repeated blur and arbitrary color/blur ordering
use a shared pass plan; full-size work textures prevent read/write aliasing.
The existing blur-plus-saturation path retains its original pipeline.

Opacity scopes snapshot their base, paint overlapping primitives at full opacity
and interpolate the group once. An element's own backdrop executes before that
opacity group; its completed effect refreshes the group's base. Nested ordinary
opacity groups retain independent snapshots. Backdrop-root isolation remains
in the CSS audit.
Premultiplied source-over composition preserves transparent backdrops. Uniform
background alpha accounts for the following rounded background's edge coverage;
normal GPU alpha blending and fractional borderless rectangles retain source
coverage. Varying-alpha gradients have not been verified with rounded filters.

The original pinned Electron offscreen probe captures 72 independent scenes per
platform and 101 CSS cascades. Windowless tests exercise the production Metal and
Blade pipelines on macOS/Linux, including overlapping filters, sharp controls,
parent clipping, fractional bounds, translucent artwork and two-pixel stripes.
Whole-image RMS error is limited to one channel level for blur and color-only
cases. Combined blur/color effects scale this bound by color gain to account for
amplification of the Gaussian/software approximation difference. A separate
48-level legacy and 72-level high-contrast color perimeter bounds cover SDF
versus Chromium path antialiasing. Color-only interior samples have a separate
one-level bound. The full
Windows GPUI crate cross-compiles against the real bindings and affected HLSL
entry points compile with glslang; Direct3D runtime rendering remains pending.

## Uniform visual scaling and headless sprites

`Style.paint_scale`, `Styled.paint_scale` and the element drawing wrapper apply a
centered uniform visual scale during prepaint and paint while retaining Taffy
layout. Pointer and AccessKit bounds use the same transform. Quad/image geometry,
rounded corners, shadows, backdrop blur, paths and decorations scale uniformly;
monochrome glyph/SVG transforms compose with existing transforms and scene
ordering uses their transformed bounds. This supports the original play controls'
hover/press transitions. Arbitrary browser transforms remain in the CSS audit.

Test-only atlas snapshots retain source bytes and remap fake tiles into actual
Metal/Blade atlases. The windowless render helper now exercises actual SVG/glyph
and raster pixels alongside quads, shadows, decorations and backdrop/opacity
groups. `TestAppContext.set_assets` installs the real application's embedded
assets for these checks. Headless Blade path/surface batches remain unsupported.
The transition reversal rule follows
[CSS Transitions 1, section 3](https://www.w3.org/TR/css-transitions-1/#starting).

Element animation uses the background executor's clock, which retains the
production monotonic clock and permits deterministic headless animation frames.
Test sprite helpers expose the actual rotation/scale matrix.

## Foreground filter layers

`src/style.rs`, `src/styled.rs`, `src/window.rs` and `src/scene.rs` add ordered
foreground filter groups around the complete element paint. Source content is
painted into a transparent layer before its parent's clipping is applied. Filter
lengths follow the paint and device scales. The existing Metal, Blade and Direct3D
filter pipelines retain one shared pass plan for the ten supported CSS functions.
Foreground blur samples transparent pixels outside the source; backdrop blur
retains reflected border sampling. Nested filter and opacity scopes retain their
own snapshots and composite premultiplied colors in order.

`scripts/native-tests/foreground-filters.cjs` captures 78 original windowless
Electron scenes on each of macOS and Linux, plus 134 independent CSS cascades.
Actual GPU comparisons cover overlapping alpha, filter order, repeated blur,
clipped parents, transforms, actual cover-cropped artwork, and nested filter/opacity layers.
Drop shadows preserve their input, translate and convolve its alpha, then composite
the input above the colored mask. Each later shadow includes the previous result.
Current colors resolve against the element using the filter, including inheritance.
SVG filters are not implemented by this patch. Source painting outside the window
surface remains bounded to that surface.

Image atlas sampling clamps each tile to its own edge pixels. Raster uploads
premultiply RGB before linear sampling; ordinary raster sprites identify that
encoding using their otherwise unused backdrop parameter slot. Emoji sprites
retain their existing encoding. Cover images sample a normalized source region
into their visible rectangle, and fractional image bounds retain their geometry
with the same half-pixel antialiasing fringe used by other quads. Sharp and blurred
original artwork controls verify edge colors, transparent interpolation and
cover cropping at normal and 1.1 scales.

Source coordinates use four scalar floats on all backends. Compile-time Rust
assertions retain offset 104 and stride 120; the independent HLSL check verifies
the same layout in generated SPIR-V. `scripts/native-tests/windows-shaders.py`
compiles all 20 production entry points with glslang and validates their output,
including relaxed HLSL buffer layout. Native Windows FXC/runtime checks remain
separate requirements.

## CSS normal line-height metrics

The text system exposes the existing ascent, absolute descender and line gap as
one normal line-box measurement. It rounds each metric at the platform's layout
scale before summing: device scale on macOS and logical pixels on Linux. Font
units use double precision until rounding so exact half-pixel ties survive.
The application uses its selected font and CSS weight/style, keeps inherited
`normal` relative to each element's font, and invalidates computed CSS when
private font registration changes. The original pinned Electron renderer supplies
224 normal/initial/unset/inherit measurements per platform at two zoom levels.
Actual platform metrics and rendered native text boxes compare against these
fixtures. Windows font-line-box rendering remains unverified.

The test-only glyph inspector also exposes inherited opacity scopes, alongside
its original colors before compositing. Native lyric-status checks use the
composed values to compare the original half-second empty-message fade.

Inline links use the shaped text's UTF-8 range boxes, with wrapping,
alignment and bidi fragments retained. `TextLayout::range_bounds` exposes those
boxes after prepaint. `TextLayout::link_controls` shares the lyric interaction
with biography and release-note rich text. Transparent controls provide stable focus and
accessibility nodes without reshaping the visible text. `Window::dispatch_mouse_click`
activates their ordinary pointer path so the enclosing synchronized line still
receives its seek click. Twelve windowless original browser cases verify Tab,
Enter, Space and delegated seeking; native tests also exercise accessibility
activation, scaled rows, wrapped labels and Arabic text.

Tracked scroll containers expose a scoped `ScrollReveal` during descendant
prepaint. Keyboard and accessibility focus requests have a generation so a
focused link reveals itself once, preserving later manual scrolling. Revealing
content uses the minimum bounded offset and can notify a container to pause its
own automatic following. The captured offset accounts for repeated activation
before repaint. The headless platform does not deliver native frame callbacks;
`draw_next_frame_for_test` executes the production callback-before-draw ordering
without a physical window. Offscreen link checks cover that sequence, repeated
Enter, preserved manual scrolling and accessibility activation.

`HitboxBehavior::IgnoreMouse` and `InteractiveElement::mouse_passthrough` retain
keyboard focus and activation while excluding the element from pointer hit tests,
hover and tooltip handling. The shared search control uses this for its fading
search button, matching CSS pointer-events:none. Native tests cover overlapping
clear buttons, underlying input clicks, Tab/Enter and rendered opacity frames.

- `src/platform/test/window.rs` and `src/app/test_context.rs`: headless windows
  expose pixel-density changes through the same resize callback as platform
  windows. The default remains 2x. Native genre references use the original
  browser display scale, and exercise moving between 1x and 2x displays.

## Shared translation and uniform scale

The existing visual scale scope accepts a parent-coordinate translation. Its
composition follows the current ancestor scale, and applies in prepaint and paint
so hitboxes, accessibility, clips, quads, sprites and descendant content move
together without changing layout. Style refinements retain separate pixel and
element-relative translation/origin values until layout bounds are available.

Built-in styled elements expose their style to the common `Element` defaults:
Div, Img, Svg, Canvas, Surface, List, UniformList and ImageCache. Stateful wrappers
forward these methods. Previously only Div exposed visual scale, which caused
custom styles on artwork to be ignored. Native regressions check direct image,
SVG and canvas painting as well as 496 original-browser geometry cases, clicks
and accessibility bounds at two zoom levels. Zero scale produces empty primitive
bounds, which the scene already culls. Hit testing also excludes empty bounds,
including their exact origin. This remains a nonnegative uniform
scale path; arbitrary affine or perspective matrices are not introduced here.

## X11 accessibility screen coordinates

The X11 adapter obtains the drawable's actual root position with the existing
TranslateCoordinates helper. Accessible node bounds already include client-drawn
shadows, so the adapter does not add the client inset a second time. This also
avoids treating WM-relative ConfigureNotify coordinates as screen coordinates.
The packaged saturation-control workflow clicks the reported screen center and
checks preview/release values. The old mapping failed this check by eight pixels
horizontally and twelve vertically on the isolated X server.

## Focus indication

Window input tracks keyboard versus pointer focus indication independently of
scroll-reveal requests. Tab and accessibility focus restore the indicator;
pointer presses hide it and ordinary keyboard input restores it. Wrapped native
buttons use this state for their focus-visible style while text inputs retain
their existing focus behavior. The original-browser focus fixture covers pointer,
disabled, Tab, Enter and Space interactions.

## CSS image fitting

Style refinements can override image fitting without changing the existing
ImageStyle default. Explicit CSS none centers the intrinsic image; contain,
none and scale-down clip inside the content box and retain rounded box clipping
instead of rounding the smaller letterboxed image. Cover sampling clamps linear
interpolation to the cropped source region on Metal, Vulkan and Direct3D, avoiding
color bleeding from pixels outside that crop. The inset is bounded for source
regions narrower than one pixel. Native browser comparisons cover all fit modes,
large/small images, rounded corners and CSS reset/inheritance/variable rules.
