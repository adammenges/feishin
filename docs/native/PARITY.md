# Feature parity ledger

Status reflects implementation and the evidence available. A passing protocol
fixture does not establish compatibility with every server version. Interactive
checks were deferred by the user; the checklist is in [INTERACTIVE-CHECKS.md](INTERACTIVE-CHECKS.md). Full feature parity has not yet been achieved.
Checkpoint entries are chronological; later entries supersede earlier gaps,
validation results and package hashes.

Current checkpoint: EV removes the legacy Electron/React application and its
build, packaging and source-comparison tooling at the user's request. Native
assets and saved regression fixtures are preserved. The native core no longer
parses TypeScript or reads the retired source tree during a build. Core tests,
affected GPUI tests, strict checks and rebuilt packages pass on macOS/Linux;
Windows Rust cross-checks pass. Both complete packaged Linux workflows pass on
X11 and Wayland with all ten optional checks enabled. EV supersedes EU's archives.

Historical generator commands below describe the pre-removal source state.
Their fixtures remain in the native assets; removed generators are no longer
part of the active test workflow. Removal does not certify full feature parity.

EU gives calendar-backed date fields their own text width,
so the calendar trigger no longer clips the year. The ET screenshots revealed
the issue after both packaged workflows passed. Both affected checks pass on
macOS/Linux (14.58 s / 16.90 s), followed by strict checks (11.85 s / 11.30 s)
and Windows Rust checks (14.57 s). Both EU archives pass package/updater
validation. The refreshed packaged library workflow passes on X11 and Wayland
with smart-editor checks enabled; both screenshots show the full date without
clipping. EU supersedes the ET archives below.

ET restores smart-rule Reset/Clear, blank rule drafts,
independent structural edits, integer numeric values and current-playlist exclusion.
Three affected core tests and six interface checks pass on macOS (20.85 s), followed
by strict checks (14.19 s). Linux passes the three affected core checks and all
six interface checks (22.67 s), strict checks (16.89 s), and Windows Rust checks
(16.85 s). Both ET archives pass package/updater validation. Complete packaged
Linux library workflows pass on X11 and Wayland with all ten optional detail,
settings and smart-editor checks enabled. Both apps paint their windows and exit
normally. The macOS bundle is locally ad-hoc signed; its seal, archive and
58-license inventory pass validation.
ES's complete calendars pass both affected checks on macOS/Linux (12.42 s / 14.46 s),
strict checks (12.12 s / 13.96 s), and Windows Rust checks (17.75 s).
ER's seven selector/editor checks pass on macOS/Linux (29.71 s / 30.45 s), with
strict checks on both platforms and Windows Rust checks.

EQ adds complete smart-playlist date operators, calendar entry, searchable
choices, all 133 sort fields and localized operators. Its core comparisons pass
90 original date mappings across 39 operator locales; five macOS interface
checks pass (19.16 s), including 12 original limit states. Strict checks pass
(16.74 s). The EQ macOS archive passed bundle/archive/updater checks; ET supersedes it. Linux EQ passes its three affected core tests and five interface
checks (18.44 s), followed by strict checks (26.52 s) and Windows Rust checks
(24.36 s).

EP's 32 image fitting scenes and nine affected image/CSS/context/drag checks pass
on both platforms, followed by strict and Windows Rust checks; all 20 Windows
shader entries compile and validate. Its Linux archive/updater and full X11
workflow pass. The full Wayland rerun also passes within the expanded ten-minute
harness budget, including player configuration and normal shutdown. The EP Linux archive is superseded by ET.

The earlier complete EF interface suites each finished with 407 passes, two
failures subsequently fixed and verified by EH/EJ, and one opt-in projectM
exclusion. Linux EM passes all 284 core unit tests; EN's 15 affected migration
checks pass on both platforms. Windows native linking/runtime and the user's
deferred interactive acceptance remain outstanding. Later checkpoint entries
supersede earlier validation results and package hashes.

| Area | Implemented in Rust | Remaining |
| --- | --- | --- |
| Servers | Bounded Jellyfin UDP discovery with streamed selection and cancellation; original environment defaults for new accounts; opt-in certificate exceptions with strict defaults and live API/audio/waveform/speaker policy; Navidrome, Subsonic, Jellyfin password login and Quick Connect; code copying, approval links, cancellation and five-second polling; expired-session sign-in recovery and account Logout with saved server/queue preservation and durable session-restore disabling; secure sessions bound to account addresses; restored-account checks, refreshed administrator permissions, bounded connection retry and localized recovery; editing/selection/removal; local/remote URLs; library selection; Jellyfin audiobooks/Instant Mix; legacy Subsonic auth; extension detection, Navidrome token renewal/rotation and Jellyfin saved-password renewal; password visibility controls | broader backend version coverage |
| Home/navigation | Per-visit back/forward scroll and loaded-page restoration; native command palette with debounced album/album-artist/track search, independent paging, persistent collapse choices, keyboard selection, inline queue/playlist actions and navigation/server commands; configurable seven home sections, backend-specific album/song histories, featured albums, section refresh/paging, order/visibility and preference import; configurable sidebar items, collapse/width, filtered/shared playlists and local playlist order; resizable queue/lyrics/visualizer panels, right/bottom layouts, combined visuals/lyrics and queue drawer | Home/panel artwork and presentation comparison, broader navigation comparison |
| Library | Search tabs for tracks/albums/album artists with 200 ms debounce, server paging and shared saved list layouts; albums, artists and album artists, roles, ordered/hideable artist overview sections with local search and community/personal/favorite/rated modes, biographies, discography/similar/top-song tabs, similar-song browsing, songs, genres, lazy library-folder trees with responsive breadcrumbs, shared song-table settings and displayed-track playback, search, favorites, ratings, sort, paging, recursive play all, covers, typed and multiple-selection filters, AND/OR artist matching, year ranges/custom tags, pinned filters, per-server saved views and named collections, local text filters, release-type groups and ordering/visibility, configurable consecutive album groups with aggregated metadata and album actions, per-view table/card/detail layouts, proportional auto-fit, spare-space column sizing and pagination, artist favorite/rated tabs and genre album/song views, configurable favorites/ratings, grid range/additive selection, explicit-image censoring, artist radio and default play actions | backend-version checks and detail presentation comparison |
| Queue | Duplicate entry identities, Now/Next/End, shuffle, repeat, reorder, remove, persisted queue, local text filter, server queue by index when supported, Auto DJ track/album prefetch, similarity/random strategies, duplicate policy and library limits, drag/drop, multi-track moves and permanent queue shuffle, saved columns/size/borders/pinning, automatic column sizing, display sorting and pagination, virtualized side queues with lazy artwork, current-track following, side-panel column drag/resize, optional queue restoration and automatic server saves | visual checks |
| Playback | Native streaming/seek/pause, server-side Navidrome/Subsonic Jukebox recovery and transport, preloading/gapless/crossfade, local device selection, MPV IPC/gapless/error replies and configurable properties, optional/custom transcoding format, bitrate and sample-rate limits; pitch-preserving speed, mute/volume, 12-band equalizer/preamp/compressor and built-in/custom presets, ReplayGain, 300 ms play/pause/stop fades, custom hours/minutes/seconds and preset sleep timers with paused countdowns and track/album end modes, configurable scrobble thresholds/server reporting; playback state/engine/scrobble display, configurable skip buttons and cached waveform seek bar with cancellation and slider fallback; audiobook checkpoint resume, semitone/10-cent speed controls and configurable volume wheel/width | listening/output checks |
| Playlists | Track/album views with consecutive duplicate-preserving groups, group playback/selection/downloads; CRUD, add/remove/move tracks, bulk track removal and ordered block moves with stale-playlist checks; local track drafts with Save, Cancel, Save As and held row moves; graphical/raw Navidrome smart rules, nested groups and typed values, sorting/limits, public visibility, folder tree/list/navigation and drag/drop, ownership/sync preservation, save queue to playlist, append/replace workflows | backend integration checks |
| Artwork/sharing | Navidrome share options and clipboard URL; artist/playlist/radio artwork upload/removal, per-station cache persistence for Navidrome/Jellyfin; library/album-track bulk favorite/rating/share/playlist actions, detail information and downloads; imported calendar-based share expiry with server-default/never options | server integration checks, remaining artwork visual comparisons |
| Radio | Stations, HTTP streaming and live ICY/MPV metadata, create/edit/remove, local Jellyfin persistence, homepage editing and artwork; independent live transport preserves the music queue/resume position | listening and live-station integration checks |
| Lyrics | Server lyrics; LRCLIB/NetEase/Genius/SimpMusic; original Fuse candidate ranking and primary-artist queries; provider selection/search; persistent overrides/cache; LRC import/export; timed lines/words with duration-based highlighting, RichSync singer cues and selectable server overlay layers; configurable player/sidebar text alignment/font/opacity/spacing; follow alignment/lead time and manual-scroll pause; all server languages/variants and UTF-8 word cues; automatic-fetch/local-preference controls and policy-aware caches; NetEase and Azure/Google translation; offline Japanese furigana/romaji | synchronization behavior and visual checks |
| Tags | Native file/batch editing; arbitrary and multivalue tags; typed numeric/boolean/text controls, mixed values and per-field undo; artwork; path mapping; preservation/read-back safeguards, saved field types/custom suggestions/server sources, progress, image drag/drop and admin metadata refresh | interactive checks |
| Appearance | Existing theme palettes/locales embedded; custom theme inheritance/watch; original accent shade generation, separate system dark/light choices, root font size, content width, scrollbar colors/dimensions/radii, multistop overlay gradients and scoped custom CSS with responsive media conditions and app/color/size variables and original Mantine defaults; popup/button/input interaction styles; built-in Inter/Poppins, system fonts, validated custom TTF/OpenType/WOFF/WOFF2 and file-change reload | Remaining browser CSS effects, remaining localization, RTL and physical screen-reader comparisons |
| Remote | Authenticated HTTP/WebSocket listener, mobile controls, artwork/fullscreen artwork, debounced searches and automatic library paging, virtual queue/library rows, touch hold/swipe/reorder with automatic scrolling and cancellation, duplicate-safe target-edge insertion, play menus, track radio, playlist additions, favorite/rating, queue removal/clear, reply correlation, system/saved themes and live original light/dark accent settings, persistent credential/port configuration; install manifest/icons and a network-first offline shell with private-route cache exclusion | Physical mobile installation and broader visual comparison |
| Integrations | Original nonempty DISABLE_AUTO_UPDATES override with manual checks retained; Windows app identity; single-instance activation and crash recovery; retained macOS windows and Dock reopen; native app menus with focus-sensitive playback, edit roles and GPUI inspection; preference-safe restart; platform media controls, delayed track notifications, configurable track window titles, optional Discord presence, private mode and playback sleep inhibition, native tray transport/menu, macOS Dock playback actions, Windows taskbar playback buttons; latest/beta/alpha native update checks, verified downloads, staged install/restart and install on quit; close/minimize-to-tray on macOS/Windows/X11/Wayland and minimized startup | Actual platform/Discord/tray checks |
| Network speakers | Direct output selector; DLNA discovery/transport, Sonos grouping/properties, group-wide master volume/mute with Shift for individual control, member sliders/mute and coalesced writes; stereo-pair detection; existing playback adoption, normal/passive disconnect and local-volume/position restoration; next URI, seekable FFmpeg speed/DSP proxy and revocable live radio streaming; UPnP GENA subscriptions with renewal, expiry recovery and polling fallback | physical speaker checks |
| Windows/display | Native window persistence/restore, expanded player/fullscreen with metadata order/visibility, title scrolling/multiline controls, artwork size/aspect, dynamic color/blurred artwork backgrounds and lyrics/queue/related tabs, idle expanded-player opening and playerbar click preference; configurable album/artist header backgrounds; editable local/global shortcuts; bounded native stereo PCM capture, FFT spectrum graph/bars/radial/stereo layouts and four existing presets, typed settings, saved presets and custom gradient/color-stop editing; native projectM MilkDrop, 349 original presets, cycling/selection/exclusions/imports, offscreen resize and blend; consent-controlled MPV system-audio capture with macOS helper, WASAPI loopback and PulseAudio/PipeWire monitor | Physical comparison with original analyzer/projectM visuals, physical capture and OS integration |
| Data/release | Separate native data; atomic writes; settings export; native data restore and Electron preferences/account definitions import with preview/backup and transactional keychain credential imports; redacted diagnostics ZIP and rotating logs; optimized macOS bundle; macOS/Linux package scripts; macOS/Linux/Windows CI workflow | Real-account migration checks, remote CI execution, signed releases/web distribution |

## Automated evidence

- Featured carousels match the original container breakpoints, cyclic card slots,
  Next/Previous wraparound and Shift-wheel threshold/cooldown. The original
  callbacks supply 900 navigation cases, 60 wheel cases and 120 weighted title
  sizes. Sixty windowless Electron layouts on macOS/Linux verify single/multiple
  card geometry, overlaid navigation and the play group across 15 widths, within
  GPUI's physical-pixel rounding. Rendered native cards
  verify artwork-derived dominant colors, censored artwork, accessible
  Next/Play/Last actions, default-play preference independence, hold/release
  suppression, rounded artwork, hover dimming and detail navigation. Single-feature artwork also retains its three-rem foreground blur, 1.1 scale,
  0.8 opacity and parent clipping. Artwork retains ordered alpha-mask shadows,
  including the original default, multiple-card hover and single-card behavior.
  Page entry/exit retain the original 200 ms ease-out and 300 ms ease-in
  opacity fades. The pinned React/Motion runtime supplies 222 frames across
  24 complete, interrupted, returning and completed-return scenarios, including
  overlapping layout, insertion order, pointer targeting and final page removal.
  Native pointer tests verify painted cover opacity and overlapping geometry,
  and prevent clicks from reaching a covered page.

- Carousel metadata uses the original transparent genre/year badges, uppercase
  tracking, dimensions and wrapping. Seventy-two original Mantine layouts per
  platform cover standalone labels and the actual intrinsic metadata stack,
  narrow cards, long genres and two root font sizes. Fractional text advances
  retain later label positions; single-card separators omit collapsed trailing
  whitespace. Single artist headings wrap while multiple-card artists retain
  their one-line clamp. Title, artist and genre/year text retain their original
  white foreground and eight-pixel black shadow. Forty-eight original shadow
  pairs cover wrapping, two root font sizes and browser zoom. Shadow-only samples
  exclude glyph antialiasing differences, with RMS bounded at 0.7 and individual
  channel differences at eight levels; glyph bounds remain within one CSS pixel.
  Custom CSS aliases remain in the visual audit.
  Page zoom retains the unzoomed optical size while scaling variable-font
  glyphs. The same mapping reaches independent input wrappers; zoom changes
  invalidate retained text caches without changing previously shaped font IDs.

- Custom text shadows inherit independently of element filters and backgrounds.
  Thirty-four pinned Electron reference pairs per platform verify colored and
  transparent shadows, offsets, independent layer ordering, currentColor,
  inheritance/resets, variables and rejected negative blur/spread/inset/percentage
  values. Shadows remain visible on transparent glyphs, including emoji; borders
  and backgrounds do not cast glyph shadows. Standalone shaped lines use the same
  paint path as wrapped text. Native foreground bounds stay within one CSS pixel.
  Shadow-only RMS is bounded at 0.7 on macOS and 1.1 on Linux, with individual
  differences bounded at 20 and 22 levels. Unblurred font-edge differences are
  confined to a one-device-pixel perimeter, bounded at 40 levels on macOS and
  112 on Linux; the Linux sharp-shadow RMS bound is 2.6. These bounds account for
  native font antialiasing and do not imply pixel-identical rasterization.
  CSS decoration shadows retain font-relative automatic/from-font thickness and
  browser positioning; line-through paints above glyphs. The reference includes
  original automatic decoration thickness and positioning. CoreText color glyphs
  request their actual device size instead of scaling a smaller bitmap strike.
  Applying CSS text refinements preserves existing font weight, size and line
  height. Automatic-direction wrapping uses shaped advances, preserving kerning
  and ligatures instead of summing isolated character widths.

- Carousel arrows use the original 200 ms ease transform/border transitions and
  single-card opacity fade. Artwork shadows, dimming and the play overlay retain
  300 ms ease timing. 154 original CSS transition frames and 162 interrupted
  reversal frames check values and shortening. Headless pointer frames verify
  painted scale, icon/layout/accessibility geometry and retained explicit opacity;
  actual artwork hover/leave checks advance the same controllable clock.

- Play random retains saved song/album counts, enabled year bounds, genre,
  library and Jellyfin played choices. Sixty-six request references execute
  the original controllers, including audiobook libraries and single-bound
  Jellyfin behavior; separate fixtures preserve duplicate songs and album blocks.
  Headless mouse tests exercise genre selection, persistence, next/now/append,
  held shuffle, invalid values and cancellation. Electron options import with
  preview and validation.

- Shared circular play controls retain the original icon assets, 32/48/32 sizes,
  spacing and translated accessible names. Thirteen windowless original Electron
  frames execute the original React hooks and verify the held control's 100 ms
  delay, 300 ms expansion, opacity and removal at the 500 ms hold deadline.
  Another 64 original transition frames and 144 interrupted frames verify the
  hook's 200 ms ease-in-out hover/press scaling and shorter reversals. Layout,
  icon and accessibility geometry, expanded hit areas and disabled/loading
  states have headless checks, including actual windowless GPU opacity samples.
  Pointer exit, release, disabling and removal clear held feedback.
  Thirty-six original loading frames also verify the css.gg spinner, one-second
  linear rotation and original 18-pixel font token with zoom/custom overrides.
  Both native button sizes replace the play glyph and preserve centered layout.

- Ordered foreground blur, brightness, contrast, saturation, grayscale, sepia,
  hue rotation, inversion, opacity and alpha-mask drop shadows operate on an isolated element layer.
  188 original Electron pixel scenes on macOS/Linux and 134 independent
  CSS cascades cover transparent edges, overlapping translucent sources,
  repeated/ordered functions, parent clipping, scaling and nested filter/opacity
  groups. Shadow checks cover repeated and reordered chains, negative offsets,
  transparent colors, artwork, currentColor and inheritance. Translating the mask
  before Gaussian convolution retains coverage above positive offsets.
  Rounded ancestor/source clips include scaled artwork and nested opacity/filter
  groups; backgrounds and descendants share their overflow clip. The largest
  sharp curved-edge differences are 36 channel levels on macOS and 57 on Linux,
  confined to a one-pixel antialiasing perimeter. Interior bounds remain strict;
  the Linux sharp rounded-image RMS bound is 0.65 and other sharp scenes 0.55.
  The actual Metal/Vulkan pipelines use the same filter pass plan;
  color-only interior samples agree within one channel level.
  SVG filters and rendering beyond the window surface remain in the audit.
  Windows runtime rendering remains unverified.

- All 20 Windows HLSL entry points compile and pass SPIR-V validation on Linux.
  Scalar source-coordinate arrays retain the image sprite's 104-byte field offset
  and 120-byte stride; Rust has matching compile-time layout assertions. These
  checks complement Windows Rust checks and do not establish Direct3D runtime
  rendering or FXC compatibility.

- Ordered backdrop blur, brightness, contrast, saturation, grayscale, sepia,
  hue rotation, inversion and opacity are checked against 72 original Electron
  offscreen scenes on macOS/Linux. The actual GPU pipelines cover repeated and
  reordered filters, translucent artwork, overlapping and nested opacity groups,
  sharp foreground controls, clipping, fractional edges and original preview
  filters. Consecutive color functions retain float precision and clamp in order.
  101 independent CSS cascades verify units, defaults, validation, variables,
  inheritance and resets. Whole-image error is bounded separately from rounded
  edge antialiasing; tested color-only interior samples differ by at most one
  channel level. SVG/drop-shadow effects, backdrop roots and
  rounded varying-alpha background composition remain in the CSS audit.
  Windows runtime rendering remains unverified.

- 184 invalid CSS declarations match the original Electron cascade across 71
  properties. Rejected lengths, borders, flex values and text decorations preserve
  earlier valid declarations, including declarations marked important.

- 1,124 original variable cascades distinguish rejected variable syntax from values
  that become invalid after substitution. Forty supported text, layout and paint
  properties restore their parent or initial value; empty, cyclic and malformed
  values, escaped/uppercase functions and literal strings have independent checks.
  Custom properties resolve before inheritance, preserve aliases across child
  overrides, honor resets and reject active cycles even with internal fallbacks.
  Unused fallback branches remain unevaluated, matching the original runtime.
  Substitution preserves token boundaries, including adjacent identifiers, numbers,
  empty replacements and parentheses that would form a different value if
  concatenated as text. Layout and paint values reset to their initial values;
  gradient comparisons verify actual generated pixels.
  Other properties and broader variable syntax remain in the audit.

- 34 original flex shorthand cases verify length/percentage bases, basis-first
  ordering, numeric grow/shrink values and resets. Invalid duplicate bases,
  interleaved values and bare nonzero bases preserve the earlier valid cascade.

- Native drag previews retain the first item's title, cover/hash or original icon,
  translated additional-item count and folder path. Headless mouse down/move/drop
  checks preserve duplicate queue keys and order and update painted French text.
  The floating library selection dialog retains the original CSS filter, bottom
  centering, translated count, keyboard help, clear control and full context menu.
  Actual headless clicks verify duplicate-safe information actions. The constant
  drag transform enlarges artwork and icons by 5% about their center without
  changing layout space. The original flattened 3D ancestors add no perspective
  displacement; no transform state changes trigger its declared transition.
  Thirty-two original windowless layouts per platform cover images/icons, item
  counts, root font sizes, zoom and both directions. Native checks compare painted
  bounds and text placement within one native layout pixel.

- The expandable album preview persists beneath library and detail routes, with
  its own track selection, virtualized scrolling, context actions and drag previews.
  Playlist order and duplicate entry identities survive play, next and append;
  double-click starts the complete album at the chosen entry. The original three
  play buttons support a 500 ms hold to shuffle, cancel on pointer exit, and avoid
  a second action on release. Headless mouse tests cover expansion, close, stale
  responses and selection independence. Eight layouts measured from the original
  CSS cover its container breakpoints; 48 original-runtime artwork cases cover
  alpha thresholds, ignored colors and fixed-point speed sampling. Artwork and
  its gradient receive a two-pixel native backdrop blur. Motion and physical
  presentation comparison remain deferred.

- Detail and fullscreen artwork use the original dominant-color calculation,
  including 24-step grouping, alpha weighting, ignored colors, first-to-highest
  winner ordering and canvas speed sampling. Forty-eight independent original
  cases cover transparent pixels, threshold boundaries, ties and fractional
  downscales. The preview's typography, button and icon sizes, panel height and
  blur also follow application zoom; a headless check verifies 150% scaling.
  Separate album-artist labels retain the original separator and wrapping.
  Original preview and play-control CSS classes retain nested ancestor matching;
  live headless overrides change row heights, artist fonts, button sizes and gaps
  and restore the defaults without losing selection or queue state.

- Library, home, detail and whole-list play controls share the original 500 ms
  hold-to-shuffle behavior, including Now/Next/Last preferences, selection,
  duplicate identities and the clicked start entry. Artist-page holds retain
  filtered release groups. Fetching/loading controls block additional actions.
  Headless checks cover the deadline, redraws, release suppression, pointer exit
  and return, removed/disabled/loading controls and ordinary keyboard activation.

- Explicit and clean badges use the original Symbola glyphs, theme sizes and spacing.
  Headless rendered tables, cards and duplicate-safe queues verify both accessibility
  labels and omission of unknown statuses. Automatic title sizing includes the badge
  and invalidates when its CSS font or spacing changes.

- Continuous gradient colors match 1,680 original canvas samples across all five
  palettes, channel layouts, luminance, radial and split-gradient modes. Native
  radial graphs use radius-based coloring; LED rows sample their physical
  position, and outlined bars preserve gradient fills and border colors.
- Amplitude labels and dashed grids match 144 original dB/percentage cases;
  circular frequency/note labels, positions, rotations and colors match 768
  original cases. Native glyph transforms retain the circular scale's rotation.

- X-axis labels, octave/natural-note selection, high-frequency label density, ANSI numeric scales, log/linear/Bark/Mel positions, highlight colors and horizontal mirroring match 288 original scale-rendering cases. GPUI measures the selected native font and retains the original maximum label widths and scale background overlay.

- Real isolated headless Chrome verifies the production remote server's manifest, failed/successful authentication, exact public-shell cache contents, credential/artwork/query exclusion, offline document/script reload, WebSocket shutdown and reconnecting to a restarted server's changed snapshot. Desktop, tablet and phone viewports fit without horizontal overflow; the test saves a mobile screenshot.

- audioMotion 4.5.4 reference generator checks 480 canvas/channel/LED/bar geometry cases, 48 actual radial drawing cases and 24 reflection drawing calls, including odd canvas dimensions. Native GPUI paints nonzero frames for these radial/reflection options plus LED peaks and high-density layouts; reflections preserve the underlying graph or LED geometry.

- Real Wayland protocol fixture under isolated Weston/Xvfb verifies initial presentation, null-buffer unmapping, hidden view invalidation, restored title/app ID/minimum size, empty commit/configure/ack/buffer ordering, idempotent visibility requests, rapid restores and retained view state. Linux CI runs the fixture with software Vulkan; physical tray interaction remains deferred.

- Backend authentication, normalization and pagination fixtures for all three backends.
- Real disposable Navidrome 0.64.2, Jellyfin 12.2.0 and Gonic 0.22.0 containers pass login, music/artist/genre lists, full album metadata and artist/genre recommendation filters, artwork, original FLAC streaming, favorites, supported ratings, playlist metadata, both move directions, single duplicate removal, replacement, append and deletion. Navidrome preserves the duplicate queue index; Gonic's song-ID queue restores the first matching occurrence as in the original application. Images are pinned by digest and account/music data are generated for each run.
- Duplicate playlist entries, playlist moves, public updates, artwork requests and shares.
- Queue persistence by index and form POST/playback-report extension behavior.
- Credential-safe redirects, server/path validation and protected remote access.
- Real WebSocket authentication, request/reply, state update and shutdown tests.
- HTTP audio decoding and seeking without a sound device; equalizer/ReplayGain signals.
- ICY metadata/audio separation and live FFmpeg HEAD/range/revocation checks.
- Continuous PCM gapless sample counts, crossfade curves, cancellation and seek behavior.
- Real MPV IPC with a silent output, including duplicate-track gapless playback and property error replies.
- SoundTouch pitch/duration/stereo/seek checks at half and double speed; live speed changes.
- Remote artwork access and credential isolation; client request correlation and browser-flow DOM tests.
- Offline Japanese IPADIC readings, original-text preservation and word times.
- Native/Electron import validation, omitted-field reports, scrobble thresholds and secret exclusion.
- The complete original default preferences execute in Node with the original enum values and table/grid column builders. CI checks the generated fixture; Rust validates all mapped settings and an exact native export/import round trip. Omissions are explicitly limited to empty secret slots, the browser Web Audio pipeline switch and the Electron settings-tab route.
- FFmpeg speed/equalizer/compression output decoding and cleanup; proxy ranges and HEAD.
- SOAP discovery/transport/grouping fixtures and playback worker lifecycle, proxy URL/seek scaling.
- Headless GPUI rendering, keyboard focus/action and numeric-input checks; audio preset persistence.
- Metadata/artwork round trips preserve decoded WAV, FLAC, MP3, Ogg, AAC/M4A, ALAC, AIFF, Opus, WavPack, WMA, Matroska, TrueAudio, DSF and DFF audio and unedited tags. Unsupported ASF custom/multivalue edits preserve the original file.
- Theme/locale reuse, durable settings, LRC parsing, provider payload parsing and overrides.
- Bounded Discord IPC, payload privacy/timestamps, sleep-inhibition lifecycle and shortcut reassignment/input focus. Physical integration checks are still pending.

- Nested smart-rule editing and JSON round trips; folder path remapping, metadata preservation, sidebar mouse collapse and persisted view state.
- Playback status fade endpoints, rapid resume cancellation and stale stop cancellation with silent MPV output.
- Diagnostics ZIP content and redaction of passwords, tokens, stream URLs and headers.

- Auto DJ track/album strategy fixtures, duplicate/library/genre controls, queue replacement protection and enabled playback exclusions, and appending after queue exhaustion.

- Radio queue separation, disabled music shortcuts, remote radio controls, and preserved resume positions.
- Exhaustive ordered playlist block moves and bulk removal fixtures for duplicate entries on all three backends.
- Server URL/library options, Jellyfin audiobooks/Instant Mix, and server-edit persistence checks.
- Native data restore, import backups, stable Electron server identity mapping, and stored credential address binding.

- Saved per-view table/card settings, column pinning under horizontal scrolling, playlist drag identities, paginated indices and queue display sorting without changing playback order.
- Tag field configuration/import, bounded server suggestions, all three backend metadata refresh requests, and audio/artwork preservation across common formats.

- Typed list filters, false boolean values, year ranges, custom tags, legacy Navidrome multiple-selection fallbacks and Subsonic sorting before paging.
- Saved collection import, per-server view persistence, rename/overwrite/delete and navigation history.
- Mouse column reorder/resize, original proportional/spare-space width allocation and album-detail track selection/dragging.
- Native PCM capture preserves samples, bounds memory and resolves independent stereo FFT tones; all four built-in spectrum presets render headlessly.
- Lyric cache policies, disabled fetch, fallback failures, all server languages, UTF-8 word cues and scrolling lead/alignment.
- Azure/Google local HTTP translation fixtures preserve blank lines and batches, verify credentials/region and keep secrets out of settings exports.
- Headless GPUI lyric variant selection, typed settings, follow scrolling/manual pause and translation rendering.
- Typed metadata keyboard/step/boolean/mixed-value controls and invalid numeric rejection before writing files.
- Group member SOAP volume reads/writes and validation; stale topology reads cannot overwrite local volume edits.
- Native projectM offscreen GPU rendering of all 349 presets, nonempty pixels, resized surfaces and blends; preset pack ordering/cycling, texture installation and GPUI controls.

- Typed spectrum settings, gradient positions/levels, rename selection preservation, deletion fallback, saved presets and disk round trips.
- RichSync UTF-8 singer cues and word end times, cache compatibility, native partial-word mask geometry, overlay mouse toggles/persistence and paused/buffering interpolation.
- Tray menu command validation, duplicate queue transport/stop behavior and native/Electron window preference persistence. Actual tray/window checks remain deferred.

- Real UPnP event subscribe/notify/renew/expiry/re-subscribe/unsubscribe fixtures, callback authentication and sequence ordering; event-driven gapless playback while SOAP polling remains stale.

- Radial graph and retained-peak geometry, intermediate gradient colors, zero-width lines, frequency/note labels, LED backgrounds and 16 stereo/graph/radial headless combinations. Analyzer visual equivalence still needs interactive comparison.
- RichSync Japanese duet text preservation, individual word times, two singer furigana and romaji rows in headless GPUI.
- Fullscreen typed preferences, metadata order/visibility, artwork aspect, title scrolling geometry, dominant-color/blur processing, Electron/native imports, mouse queue tabs and persistence.
- Silent HTTP waveform decoding, bounded peak memory, stereo transients/normalization, mouse seeking, clamping, generation cancellation and native preference persistence.

- macOS and Linux core/headless GPUI suites; isolated Linux compilation catches dependency and toolchain differences. New features continue to be tested before final release builds.
- Home query selection, section order/visibility imports, rendered mouse paging and album navigation; public-shell manifest/icon routes and service-worker online/offline/private-cache behavior.
- Existing WOFF2 assets and OpenType parsing, expansion limits, custom-font file reload, invalid fallback, settings import and persistence.
- Actual macOS/Linux font registration checks cover importing a previously missing
  family and replacing a face with the same name. New metrics and glyph pixels take
  effect, existing layouts retain valid font identifiers, and multiline inputs
  rewrap their unchanged text after registration.
- Original tabular digits, inherited numeric variants and low-level OpenType overrides match 54 independent Chromium CSS values. Independent Inter/Poppins references include 2,080 measurements across feature combinations, nine variable weights and four optical sizes, with separately measured macOS/Linux widths. Actual glyph pixels verify increasing weights and distinct logical sizes at equal physical size. Nested GPUI text tests verify inheritance and clearing; Windows rendering remains pending. Linux comparisons bound the text engine's integer font-unit rounding separately from Chromium's layout quantization.
- Japanese/Hebrew fallback faces retain the original unicode ranges and CSS weight-to-axis mappings. On macOS/Linux, 816 independently measured Chromium font cases verify mixed scripts, combining marks, missing primary families and Symbola symbols; actual glyph pixels verify embedded face selection and all five mapped weights. Static fallback advances use an explicit half-font-unit rounding bound. These fallback font checks isolate their metrics with full punctuation spacing. The derived assets reproduce byte for byte from the original files. Twenty-three independent CSS font-family cases verify quoted names, commas, escapes and invalid values. DirectWrite's modified private-collection fallback method compiles against official Windows bindings; Windows rendering remains pending.
- Missing italic faces use native synthesis, including Japanese and Hebrew
  fallbacks. Independent Chromium references verify 126 upright/italic/default
  oblique advances and ink extents on macOS/Linux, plus 14 CSS inheritance,
  variable, escape, reset and invalid-declaration cases. Synthesized ink extents
  have a one-pixel rasterization bound; upright Linux glyphs retain a separate
  two-pixel FreeType/Swash bound. DirectWrite's synthesis method compiles against
  official bindings and keeps simulated faces distinct in its font cache;
  Windows rendering remains pending.

- Missing bold faces retain their original advances and synthesize glyph ink at
  weights 600 and above, including combined italic synthesis. Pinned original
  Electron references verify 360 advances, ink extents and weight-dependent pixel
  changes on macOS/Linux. Real Poppins bold faces and the variable Inter face are
  controls against extra synthesis. The existing one-pixel ink bound and separate
  two-pixel upright Linux bound are retained. DirectWrite supports combined bold
  and oblique simulations; Windows rendering remains pending.

- Original tracking has 356 single-line/caret and 384 wrapped-row measurements
  per platform across Latin, Japanese, Hebrew, Arabic, combining marks, emoji
  and bidi controls, including explicit ligature settings and negative spacing.
  Tracking preserves cursive joins, suppresses optional ligatures, skips format
  controls and follows actual shaped advances when truncating. Linux contextual
  row shaping applies spacing to spaces; macOS system fallbacks select the
  requested weight independently of the primary font's default cascade.
  Percentage tracking is covered by the later relative-font checkpoint below.

- Original Electron 43.7.0 punctuation references verify 2,928 single-line layouts and caret cells plus 3,264 wrapped/forced-line layouts across four font sizes, exact/fractional/neighboring widths and all four spacing modes on macOS/Linux, including explicit OpenType spacing overrides. Static font-unit rounding has a separate horizontal bound; line breaks remain strict. Native punctuation drawing retains actual glyph offsets; inherited CSS and live title line-count changes have headless checks. Original paragraph-final bracket spacing, Linux FreeType scaling and paragraph-position rounding are preserved. Adjacent same-color spans retain separate fonts. Windows runtime checks remain pending.
- Original Electron word-wrap references verify 576 paragraph layouts in Normal,
  BreakWord and BreakAll modes on macOS/Linux. Another 1,008 original layouts
  verify KeepAll with normal, break-word and anywhere overflow wrapping, mixed
  CJK/Latin/Hebrew words, numbers, punctuation, emoji, spaces, hyphens, slashes
  and zero-width break markers in both directions. Actual input events match 1,200
  multiline actions on each platform, covering Home/End, vertical movement,
  Shift selection, forced newlines, Arabic ligatures and hanging RTL spaces.
  Wrapped pointer checks retain the clicked physical row and bidi edge, with IME
  candidates anchored to the painted cursor. CSS wrap modes inherit and resolve
  variables, resets and invalid declarations.
- Original Electron references additionally cover 240 after-insertion navigation
  cases per platform. Native checks exercise committed and IME insertion through
  the same layout revision handling. Multiline placeholders wrap long words and
  preserve the empty paragraph caret. Masked Unicode IME ranges and point hits
  map displayed password cells back to the original UTF-16 positions.
- Long karaoke words wrap within the lyric surface. The progressing highlight
  clips the same shaped text used by the base word, preserving wrapped glyph
  positions across three alignments and three progress positions for Latin and
  CJK/emoji lyrics. Translations and cue text use the same BreakWord behavior.

- Original Electron references verify 144 mixed-direction caret layouts, 192 wrapped
  paragraphs and 128 system-fallback layouts on macOS/Linux, including Arabic and
  Persian marks, Hebrew, emoji sequences and bidi isolates. Pointer edges preserve
  their alternate insertion positions. The original 112 input-arrow cases match
  on both platforms; actual headless input painting checks right-aligned RTL
  cursors, clicked edges and disjoint selection highlights. Linux private fonts
  retain explicit fallback order without entering unrelated automatic fallback.
  DirectWrite's paragraph/glyph changes compile against official Windows bindings;
  Windows runtime and physical screen-reader checks remain pending.

- Sidebar width, collapse, visibility, shared/regex playlist filtering and persisted local ordering; native data restore preserves saved playlist order.
- Sparse-artwork home paging preserves every newly fetched album and bounds automatic prefetch.
- Playback-panel visibility/order/imports, combined lyrics/visuals, independent queue search and drawer mouse controls; headless rendering reuses the main lyric and visualizer surfaces.
- Validated planar/interleaved stereo PCM, endianness, bounded buffers and streaming 44.1-to-48 kHz conversion; a silent synthetic FFmpeg process verifies capture readiness and worker/process cleanup. Physical system-audio capture remains deferred.

- Original Butterchurn headless reference renders all 349 presets with synthetic audio; strict projectM GPU checks compile every actual shader and require visible pixels for every preset. Legacy noise/sample names and the original cloud image fallback are preserved.
- All Windows Rust application, library, test and example targets cross-compile
  and pass strict Clippy against the actual Windows API crates and projectM headers. Native C++ linking
  and Windows runtime checks remain pending.

- Native update channel/architecture selection, release ordering, missing checksums, bounded HTTP reads, same-size corrupt downloads and partial-file cleanup; archive traversal rejection, temporary app replacement and failure preservation. The real installer command is tested against temporary apps without opening a window.
- Release history retains the original 30-release selector, stable/beta notes and alpha development comparison. Local HTTP fixtures verify endpoints and missing/current releases; mouse tests switch from rich notes to commits and persist installed-version dismissal. Rich text tests reject unsafe links/images across Markdown references, tables and HTML.
- Mouse-tested update channel/automatic settings and Electron preference imports; macOS/Linux strict Clippy and Windows updater API cross-checks pass. Published native assets and real application replacement remain unverified.

- Imported built-in theme layout tokens, inherited defaults, zoom invalidation and system dark/light preference migration. Existing locale catalogs drive native labels/tooltips; mouse-tested Arabic mirroring and live French switching preserve language preferences.
- Current-track following across virtual side queues and paginated main queues, disabled-follow behavior and manual scrolling/paging preservation; side-panel column resizing/reordering persists independently of the main list.

- PNG and WebP cover replacement/readback across all fourteen audio/DSD format fixtures, including replacing existing MP4 pictures; truncated WebP and unsupported ASF fields leave originals untouched.

- Jellyfin audiobook checkpoints apply once per queue entry, retain saved local positions, and bypass incompatible gapless preloading; ordinary songs and invalid checkpoints remain at zero.
- Queue restore disabling keeps the live queue intact while clearing the persisted snapshot; automatic server saves count entry identities, serialize requests and preserve duplicate-track indices.
- Provider links encode artist/album names, validate MusicBrainz IDs, respect per-provider visibility and support native Spotify URLs.
- All 450 accent shades for 45 reference colors match the exact Mantine/chroma generator; inherited dark/light shade overrides and runtime palette changes are checked.
- All 282 Mantine color-variable cases match the original resolver, including both modes, shade endpoints, named primary colors, partial arrays, contrast disabling and threshold/black/white overrides. A native app test checks the chosen scale and original global CSS aliases after live theme changes.
- Grid additive/range selection retains insertion order; censored artwork preserves the source and never exposes an uncensored fallback. Favorite controls are checked for both visible and hidden rendering.
- Idle opening uses playback/buffering/activity state; mouse movement and wheel events reset the timer, volume wheel steps and width persist, and control clicks do not trigger playerbar opening.
- Default play now/next/last/shuffle actions preserve selected-track identities and existing queue positions; live radio always starts immediately. Artist radio uses Jellyfin Instant Mix and Subsonic similar-song endpoints.
- Musical speed adjustments match semitone/cent ratios, clamp to the original range, reject nonfinite input and persist. Virtual queue artwork fetches are bounded to rendered rows while scrolling.

- Calendar share expiration covers seconds through years, month ends and leap years, server defaults, invalid/overflow rejection, Electron imports and typed native preference persistence.

- Consecutive album groups retain duplicate playlist rows, aggregate duration/size/genres in configured order and import artwork/layout/control preferences. Headless mouse clicks preserve the list's queue context; album favorite/rating updates refresh the cached album, and stale fetches are discarded after navigation.

- Artist overview section imports preserve order/visibility. Headless mouse tests verify section-context playback and local search; configured release types filter/group the album section, virtual song rows load artwork lazily, and section requests reject stale generations. Visibility checks measure current layout because GPUI 0.2.2 retains historic debug selector bounds.

- ThumbHash and BlurHash decode all 110 independent original JavaScript fixtures pixel for pixel; malformed hashes fall back to the next source and the decoded cache stays bounded. Backend request fixtures cover per-view sizes and original-resolution requests. Headless checks cover artwork preference persistence, placeholders, and cache refresh after replacement.

- Playlist album groups preserve original song positions and disc/track play order; headless mouse playback retains duplicate entries, selection/drag operations expand to original tracks, and failed group ZIP downloads preserve existing files.

- Native scrollbar track clicks, pointer drags outside the viewport, axis isolation and drag release; player/sidebar lyric settings remain independent and survive imports and restarts.
- Large-list fetch confirmation, stale confirmation rejection, cancellation and persisted confirmation preferences; casting restores local volume when disconnected or restarted.
- Transactional keychain imports restore existing credentials and remove new credentials on failure; exported settings omit remote-control, translation and Last.fm secrets.
- Multi-artist normalization on all three backends preserves the first artist identity for Discord links and artwork lookup.

- Playback exclusions match 277 cases executed by the original TypeScript player functions, including OR rules, zero/false values, millisecond durations, path prefixes, invalid expressions, lookaround, backreferences and UTF-16 surrogate semantics. Headless controls add, edit, disable and remove persisted rules; imports preserve legacy disabled rules.
- Library and queue right-click menus preserve duplicate queue identities and selected order for information and playlist actions; selected shuffling leaves unselected positions intact. Album radio uses the original backend-specific similar-song/Instant Mix endpoints.

- Painted headless CSS checks exercise normal, hovered, pressed, checked, disabled and focused input colors. The native component callback preserves compound interaction selectors and fixes the upstream hardcoded button hover foreground.
- Translation lookup, nested references, interpolation, regional fallbacks and plural forms match 75,936 original i18next cases across all 39 catalogs. Native action menus and counted timer presets use these translations; headless Arabic/French checks cover locale changes and mirrored sidebar layout.
- Browser media-query comparisons cover ranges, density, orientation, theme, boolean conditions and unsupported-feature negation. A headless resize/zoom/theme test verifies native layout changes and cache invalidation.
- Painted popup checks isolate the original dropdown/context-menu classes, descendant selectors, keyboard selection, checked/disabled items and mouse press. Fixed context previews retain the first item and duplicate-aware additional count while actions scroll. CSS sizes and preview rules come from the original sources; inherited Mantine overrides, scoped variables, native gradients and shorthand cascade precedence reach native layout.
- CSS element backgrounds match 44 independent Chromium references at normal and Retina resolution, including multistop and double-position gradients, pixel/percentage stops, decreasing and coincident positions, magic corners, transparency and original theme noise. Headless paint checks verify rounded corners, opacity, resizing, bounded image caching and background-color/image shorthand behavior. Image-only declarations retain native widget colors through the shared component styling path.
- The shared song path resolver matches original prefix behavior when the replacement prefix matches, is absent or does not match. Tag editing, file reveal and playback exclusions use the same resolver.

- Album tag actions expand selected albums into tracks before resolving local paths; a headless fixture loads a real WAV and verifies that opening the editor leaves its bytes unchanged. Artist download and rating menus expose the existing backend operations.
- Navidrome/Subsonic Jukebox fixtures recover server queues without replacing them, exercise pause/gain/integer-second seek, preserve duplicate-track advancement and reject foreign-server tracks. The real playback worker opens no audio device, and an immediate selection/play regression checks command ordering. Headless mouse selection preserves the current duplicate entry, saves the engine preference and restores local volume on disconnect.
- Lyric scoring and result order match 765 cases generated by the original TypeScript with the exact lockfile Fuse.js version. Local HTTP fixtures verify primary-artist queries and automatic selection through the same ranking. Synchronized-result priority, the 0.55 threshold, Unicode and long-pattern chunking are retained.

- Linux password-store preferences import and persist all original libsecret/KWallet 4/5/6 choices. Private D-Bus fixtures verify daemon paths, credential read/write/remove, Unicode and empty values, failed-write preservation, denial, cleanup and error redaction; headless mouse selection saves the choice for restart. Credential imports target the selected store and retain transactional rollback. Actual desktop wallet checks remain deferred.

- Confirmed query/full cache controls cover cancellation, queue/settings preservation, local radio artwork and imported lyrics. Disk fixtures preserve unrelated user files and avoid following cache symlinks. Saved cache revisions reject older lyric results and artwork downloads; waveform cancellation and background callbacks use the same invalidation.

- Playlist context editing and bulk deletion use the original public ownership/admin rules. Headless menu clicks exercise single editing, foreign/mixed selections, cancellation, duplicate removal and partial deletion failures through all three backend endpoints. Server radio controls require an administrator; local Jellyfin station permissions remain independent.
- Settings search embeds option title keys from the original source and matches translated titles, including the distinct French application/lyric language labels. Headless typing and clicks find the setting and navigate to its picker. Confirmed reset updates live inputs and defaults, preserves queue/transport/library data, saves the previous preferences, keeps a Last.fm key inactive after restart, and leaves memory unchanged on a failed save. Mouse back/forward events preserve both history stacks.

- Library scan protocol fixtures preserve selected Jellyfin music libraries and metadata/image replacement options, exclude unrelated video/book libraries, and normalize running/cancelling/completed statuses. Headless sidebar clicks prevent duplicate scan requests; completion invalidates library views while preserving queue and lyrics. Delayed starts, quick completion, unreported start timeouts and stale server/generation results are covered. Successful tag writes watch the metadata refresh through the same worker.
- Detail pages fetch full album/artist/playlist metadata on entry and refresh, reject stale navigation responses, and preserve favorites/ratings changed during a request. Album pages include disc selection/subtitles, metadata, record labels, grouping filters and related albums from the primary album artist and first two genres. Headless mouse checks cover refresh, additive disc selection, excluded-album paging and record-label navigation. Twenty-four album metadata cases execute the original three TypeScript normalizers to check partial dates, year fallbacks, tags, labels, versions and disc titles.
- Artist biographies preserve HTML paragraphs, emphasis, lists and links in the native rich-text renderer. DOM parsing removes embedded script content and event handlers; native renderer tests reject unsafe links and local-file images. Headless mouse checks cover expansion/collapse, full-detail biography fallback and genre navigation. Navidrome artist metadata retains biographies and selects the original maximum album/track counts across artist roles.
- Older Navidrome 0.54.5, Jellyfin 10.9.11 and Gonic 0.16.4 containers run the same real integration suite. Classic Subsonic genre membership comes from the server even when album lists omit genre metadata; protocol tests cover OR/AND, deduplication, pagination and remaining favorite/year/search predicates. Genre names containing query punctuation are safely encoded. Jellyfin saves edited descriptions after playlist metadata changes and ordinary renames avoid elevated metadata writes. Headless rescan controls require server administrator permissions, as in the original app.

- Fresh macOS/Linux optimized packages include all native font and dependency licenses. Real installer CLI checks pass on both packages without opening a window; repeated macOS bundle builds replace executable inodes and refresh/verify the ad hoc seal. Developer ID signing and distribution remain pending.

- Jellyfin Quick Connect protocol tests cover unauthenticated initiation, pending/approved states, authorized account names and permissions, encoded secrets, malformed/expired responses and cancellation. Native headless mouse tests copy codes, cancel, switch methods, edit addresses, navigate away and reuse an existing account ID. Secrets remain outside the state file. Approval is exercised against the disposable Jellyfin 12.2 and 10.9.11 accounts.
- Logout revokes shared API clients and rejects an in-flight library response, new streams and artwork reads. A headless sidebar mouse test preserves both server definitions, every queue identity and position, and the other account's session; it persists the disabled restore marker and clears it after signing in again. Password login also rejects completion after a form change or navigation.
- Jellyfin playlist writes share a per-playlist lock and verify complete track order plus stable entry identities after each mutation. Appends verify each 50-track chunk; invalid input is rejected before any request. Protocol fixtures exercise transient stale reads, regenerated identities, concurrent appends and server failures without replaying a write or accepting incorrect order. All six real server versions pass the same immediate mutation/readback checks through these production methods.

- Expired Jellyfin HTTP 401 and numeric/string Subsonic error 40 revoke shared clients and return the affected account to sign-in. HTTP 403 and temporary Navidrome renewal failures preserve the session; successful renewal stays automatic. Artwork uploads use the same checks, and unlinked provider error 0 retains the original empty-biography fallback. Headless recovery preserves the queue and other accounts, persists disabled auto-restore and retains the expiration message after credential cleanup.
- Command palette headless keyboard/mouse tests cover opening from another focused input, nested navigation and Escape, 400 ms debounce, stale-query cancellation, separate four-result pages, section collapse persistence, inline queue append, unsigned-account sign-in and playlist creation. Native imports preserve the section choices, and original Electron app-store collapse preferences are accepted.

- Self-signed HTTPS fixtures verify strict defaults, opt-in exceptions and re-enabling verification for API clients, native waveform decoding, silent MPV and FFmpeg speaker transcoding. Headless mouse toggles update live clients, future sign-in forms and persisted definitions; original boolean/string imports and resetting settings preserve the policy.
- Cross-process single-instance tests cover second-launch activation, invalid and stale messages, profile isolation, private endpoint permissions, normal exit and crash recovery. Native CLI fixtures also check deferred restarts, invalid arguments and preservation of the locked profile without initializing audio, credentials or a window.
- App menu fixtures check original commands, edit roles, active playback labels, configurable shortcut fallback and focus-sensitive disabling. Headless GPUI tests exercise settings, server, private, sidebar and playlist actions. Settings restart checks preserve preferences and reject failed saves. Physical menu/window activation remains deferred.

- Jellyfin UDP fixtures verify the original discovery query, streamed normalized results, deduplication, invalid-address rejection and cancellation. Headless selection fills a new account without changing saved accounts; leaving the page cancels the scan.
- Original server environment defaults are tested for case-insensitive types, legacy authentication and names/addresses. Native form tests preserve saved account values and apply defaults only to new accounts. Update overrides preserve the original nonempty-value semantics, including `false` and `0`. The Windows app identity compiles against the actual Windows APIs.

- Analyzer checks execute audioMotion 4.5.4 for 216 amplitude conversions, 24 mode combinations, 48 frame-by-frame peak traces, 160 frequency-band cases across all supported modes/scales 24 radial geometries and 120 renderer color selections. Native drawing uses measured surface dimensions, pixel density and the original stereo/mirror geometry; headless rendering and preset tests cover those paths. Interactive visual comparison remains deferred.

- Native accessibility uses the upstream GPUI AccessKit integration adapted to the
  vendored framework. Buttons, checkboxes, switches, editable text, sliders,
  navigation, library rows, column headers and menus expose their names, values,
  selection/toggle/disabled states and actions. Ordinary text is included; hidden
  subtrees and password contents are excluded. Headless tests validate actual
  AccessKit trees, real form edits and persisted volume, Unicode/CRLF selections,
  long text, undo/redo, named buttons across 21 native screens,
  stale/foreign/unsupported actions and disabled controls. Linux's
  virtual-display AT-SPI fixture exercises discovery, input focus and typing,
  text/caret/selection reading, checkbox activation, live-region announcements
  and slider updates through D-Bus.
  macOS/Linux adapters compile, and the Windows adapter methods compile against
  the actual Win32/AccessKit bindings. Physical screen-reader checks remain deferred.
- Analyzer rotation follows nonzero spectral energy, including system capture,
  preserves phase when changing speed and resets at zero speed. Its FPS indicator
  uses the original bold green text, canvas-relative size and upper-right baseline.
- Custom CSS tests cover native cursor shapes, self/content alignment, solid/wavy
  underlines, overlines and strikethroughs, dotted/dashed/double patterns, wavy strikes,
  independent longhands, shorthand reset/cascade and clearing inherited
  decorations. Wrapped-text paint snapshots verify the actual line/quad geometry.
- Native captions reuse original action/settings translations and interpolate
  Discord, last.fm, MusicBrainz and ReplayGain names. Checks cover all catalogs,
  branded labels and changing values beside translated option titles. Home
  configuration, playlist actions and filters, empty results, lyric availability
  and remote passwords reuse the corresponding original translations in every
  catalog.
  Additional carousel, artwork, lyric, Discord and queue labels reuse original
  keys in all 39 languages. Settings save buttons use explicit Save keys, and
  library queue actions use Add last independently of queue navigation.
- Live Arabic-to-French form switching updates placeholders and accessible names
  without rebuilding controls; headless checks preserve the typed username and
  focus. User-defined metadata field names are excluded from label translation.
- Password reveal/hide accepts mouse, Space/Enter and native accessibility
  activation. Disabled fields reject activation; hiding removes the plain-text
  accessibility document. Navigation masks server, remote, Last.fm and translation
  secrets again while preserving their contents. Masked copy/cut leaves the
  clipboard and text unchanged; context menu items are disabled. Revealed
  copy/cut and undo remain available, matching [Chromium's password rules](https://github.com/chromium/chromium/blob/main/third_party/blink/renderer/core/editing/editor.cc).
- macOS workspace tests and strict Clippy pass after these changes. Linux CI pins
  the verified Rust 1.91 minimum rather than testing only the latest compiler.
- File and byte artwork apply all eight EXIF orientations in PNG, JPEG and WebP;
  33 encoded metadata cases also verify missing and invalid tags. Prepared
  backgrounds retain the oriented aspect and pixel order, and zero blur preserves
  the original decoded pixels. Versioned background/censored cache filenames avoid
  reusing earlier untransformed files.
- Byte and file image decoding share GIF/WebP animation handling. Checks verify
  oriented WebP frames and delays using the [documented container format](https://developers.google.com/speed/webp/docs/riff_container),
  GIF frame timing and straight-alpha SVG red/blue conversion.
- Twenty-one independent Chromium artwork fixtures cover sRGB, Display P3, Adobe
  RGB and grayscale ICC profiles in 8/16-bit PNG, JPEG and WebP, malformed
  profiles, and tagged/untagged CMYK/YCCK JPEG. Native file/byte images and prepared backgrounds match their sRGB
  pixels within two channel levels and preserve alpha exactly.
- Animated PNG artwork retains frame timing and subframe compositing, excluding
  a separate static poster from the animation. Prepared GIF/WebP/APNG backgrounds
  and censored artwork retain animation through composited animated PNG caches,
  including color profiles and orientation. Total decoded frame storage and
  dimensions are bounded; versioned cache filenames reject earlier static files.

- Remote snapshots publish the original separately resolved light/dark accents.
  Independent Mantine references cover 4,320 accent/shade combinations, including
  theme overrides and clamping. Client tests cover system and saved preferences,
  live updates, invalid colors and denied browser storage.
- Remote queue insertion checks cover 812 cases in both directions, both target
  edges and shuffled/ordinary queues, including self-drops and duplicate tracks.
  Invalid identities, removed targets and invalid edges leave the queue intact.
- Real Chrome touch input exercises holds, swipes, target-relative drags,
  automatic scrolling, cancellation and ordinary taps through the production
  WebSocket server and shared queue controller. A 38,000-entry queue keeps fewer
  than 20 DOM rows, preserves duplicate identities and reaches its final entry.
  Browser typing and scrolling exercise debounced searches, automatic paging,
  explicit empty results and preservation of the queue. Client tests also reject
  stale responses, duplicate page requests and empty-page fetch loops.

- Twenty original React/Mantine carousel CSS cases per macOS/Linux fixture cover
  actual wrapper tags, inline declaration precedence, inherited colors, headings,
  badges, geometry, artwork and hover selectors. Native tests apply and clear CSS
  live in both carousel styles at 100% and 150% zoom, checking painted artist
  glyphs, image filters/opacity/radii, metadata/link bounds, scoped play-button
  sizes/colors/gaps and navigation size/backdrop. The Taffy ancestor-cache
  regression and its 90 standalone unit tests pass on macOS. The macOS/Linux
  workspace runs passed the other 150 native tests; their former unscaled CSS
  padding assertion was corrected and passes separately on both platforms.
  Strict macOS/Linux Clippy, all 14 font/wrapping checks on both platforms, and
  all Windows Rust targets pass after the layout fix. Taffy's 90 standalone unit
  tests also pass on Linux Rust 1.91. Rebuilt macOS/Linux packages verify their
  archive structure, executable modes and 49 license notices; both installer CLI
  fixtures pass without touching user data.

- Sidebar filtering shares the existing ECMAScript engine with playback filters.
  Ninety-six cases execute the original sidebar's filter, ownership and saved
  ordering code, including lookaround, named/back references, legacy UTF-16 atoms,
  case folding and invalid expressions. Native form checks preserve invalid
  expressions and disable filtering while retaining width validation; the saved
  pattern and sidebar state survive reload.

- Sixty original CSS cases per platform compare nested font sizes, `em`/`rem`
  lengths, unitless versus computed inherited line heights, tracking, shadows,
  padding, borders, radii, relative weights, keywords and zero sizes. Invalid
  substituted font declarations reset to inheritance while invalid literal
  declarations preserve earlier valid values. Native computed-style checks cover
  both root sizes at 100% and 150% zoom. Rendered native text boxes pass on
  macOS/Linux, including zero-size text; the original references recheck on both
  platforms. All 31 native stylesheet tests and both live carousel CSS tests pass
  on both platforms.

- The collapsed sidebar keeps the original 80-pixel column and access to saved
  collections, owned/shared playlists, flat cover rows, collapse controls and
  playlist creation. Headless pointer and keyboard checks cover menu selection,
  regex filtering, shared visibility, owner boundaries, navigation, disabling
  the playlist list and persistence. Cover hover actions retain Next/Last queue
  placement, held shuffle and release suppression through the shared play path.
  Interaction checks and strict Clippy pass on macOS/Linux; all Windows Rust
  targets also pass. The full regression run covers 203 macOS/204 Linux core tests,
  155 native tests per platform, and one explicitly deferred offscreen GPU smoke
  test. The interrupted macOS wheel case passes on resumption. Rebuilt macOS/Linux
  archives verify their executable modes and 49 license notices, and both release
  installer CLI fixtures pass against temporary data.

- The original in-app menu remains available through the sidebar and expanded
  player, including on Windows/Linux. Sixty-four configurations execute the
  original preference conditions; 96 native comparisons cover English, French
  and Arabic captions and panel selection. Headless pointer checks verify command
  search focus after popup dismissal, navigation, private choices and persistent
  layout changes without closing the menu. Account controls retain management,
  logout, rescanning, expired-account sign-in and lazy library loading through
  the existing data layer. Thirty-two original library cases yield 48 native
  backend comparisons, preserving queue identities and saved selections. A local
  API fixture verifies loaded rows and expired-account recovery. macOS checks and
  strict Clippy pass. Linux also passes the complete menu checks and strict Clippy;
  all Windows Rust targets compile. Expanded-player menu activation passes on
  macOS. Full suites pass with 203 macOS/204 Linux core tests and 159 native
  tests per platform; one offscreen GPU smoke test remains explicitly deferred.
  Both refreshed release archives verify their executable modes and 49 license
  notices, and their installer CLI checks pass against temporary data.

- Normal CSS line height uses the selected font's ascent, absolute descender and
  line gap. The original renderer supplies 224 measurements per platform for
  regular/bold Inter/Poppins, seven sizes, normal/initial/unset/inherit and both
  zoom levels. macOS rounds metrics at device scale; Linux rounds at logical
  pixels. Double precision preserves exact half-pixel ties. Both native platform
  metrics and rendered text boxes match their references, all 32 stylesheet tests
  pass on macOS/Linux, strict Clippy passes on both, and all Windows Rust targets
  compile. Font-registration changes invalidate computed CSS caches. Windows
  line-box rendering remains unverified. Both live carousel CSS regressions pass
  on both platforms, and refreshed macOS/Linux archives and temporary installer
  fixtures verify this final CSS change.

- Both queue toolbars retain the original 300/380/480-pixel overflow boundaries,
  action order, server capability gates and busy states. The original source
  supplies 112 behavior cases and 128 real React/Mantine toolbar/search layouts
  per macOS/Linux fixture. Icons and the list-settings popup are substituted in
  the layout fixture; it verifies geometry and search behavior, not glyph pixels.
  Native headless comparisons cover both views, root sizes and zoom levels,
  including physical-pixel border rounding. Pointer/keyboard checks verify menu
  actions, duplicate-preserving shuffle/playlist creation, search focus, clearing,
  filter retention and Escape. Local API checks reject overlapping requests and
  stale playback/account restores while preserving queue identities. macOS/Linux
  checks and strict Clippy pass; all Windows Rust targets also check successfully.
  Refreshed macOS/Linux releases verify executable modes, 49 license notices
  and temporary installer fixtures for these queue changes.

- Expanded visualizer navigation executes 32 original handler cases; 74 original
  Motion samples cover delayed entry, closing, interrupted closes, restored modes,
  song-information fades, renewed timers and reopening during exit. Windowless
  references explicitly complete Motion controls after the final close sample.
  Native comparisons preserve the completed frame and transient return intent.
  macOS headless pointer checks cover expansion, validated settings dialogs,
  duplicate-identity song changes, fullscreen/Escape, return to player, compact
  preset navigation and restart without altering the queue or starting audio.
  Expanded-mode preferences now import and persist; the migration regression
  passes. Full suites pass with 203 macOS/204 Linux core tests and 167 native
  tests per platform; one offscreen GPU smoke test remains deferred. Strict macOS/Linux Clippy passes, and all Windows Rust targets check.
  The rebuilt macOS archive verifies signing, executable modes, 49 license notices
  and its temporary installer fixture. The refreshed Linux release archive and
  temporary installer fixture pass the same structure, license and preservation checks.

- The next sidebar-panel audit executes 384 original visibility cases, 60 sizing
  cases and 72 hover-control actions. Core comparisons pass, including saved
  height retention after reordering and combined-panel close semantics. Native sizing, combined-visual dimming and direct hover controls are connected.
  The shared resize container can shrink within its parent; a native pointer
  regression verifies that reordered controls remain above the playerbar, keep
  saved heights and close both combined panels. Cross-platform checks and the
  remaining panel visibility/overlay behavior are still in progress. Release archives predate this
  sidebar-panel phase.

- Sidebar panels retain the original default/saved fixed heights and auto-fit
  allocation. A pinned react-split-pane reference supplies 120 rendered geometries
  on macOS/Linux, including oversize saved layouts; native layouts agree within
  one pixel. Actual pointer checks reorder, restore and close a combined panel
  while keeping its controls above the playerbar. Combined backgrounds contain
  no interactive visualizer controls, and sidebar lyrics persist on the lyrics
  page. The native-only visualizer editor keeps its existing dedicated surface.
  The 22 queue regressions, four expanded-visualizer tests, shared geometry check
  and strict workspace checks pass on both platforms; all Windows Rust targets
  compile. These changes postdate the packaged release checks above.

- Lyric following uses the original instant first movement, cubic easing,
  350-850 ms duration and queued line-change cooldown. Forty-four trajectories
  and nine alignments execute the original helpers; manual scrolling pauses for
  two seconds, preserves its offset and resumes automatically. Line clicks resume
  following and seek to the original line timestamp. Scrollbar layout no longer
  shifts the line index. Track and variant replacement reset outstanding motion.
- Thirty-seven pinned React/Motion frames cover lyric loading, the three-second
  missing-message hold, its half-second fade, persistent sidebar messages, text
  entry and setting changes. Both platform references pass. Native paint checks
  include inherited layer opacity, centering, loading accessibility and independent
  sidebar/player timers. Player and sidebar settings open in observed dialogs;
  actual pointer/keyboard checks preserve navigation, edit only the chosen display
  and restore the previous settings target on close. Timed cues and wrapped-word
  highlights retain their settled geometry after the entry fade.

- Full post-panel/lyric suites pass: one capture-helper test, 204 core tests on
  macOS, 205 core tests on Linux, and 171 native tests on each platform. The
  existing offscreen projectM smoke remains ignored here; earlier GPU checks
  cover it separately. Strict workspace checks and all Windows Rust targets pass.
  The account-menu HTTP fixture explicitly clears inherited nonblocking mode on
  accepted sockets, avoiding a macOS request-arrival race.
- Refreshed macOS/Linux packages include these changes, pass archive/executable/
  49-license checks, and pass isolated installer checksum/replacement/cleanup/
  settings-preservation tests. The macOS bundle passes deep strict ad-hoc signing.
  SHA-256: macOS `6e806d0c71c0c5072e8a632e3db78a5c2510011e2c43977317c0cc21a0dd2965`;
  Linux `b992c6f5af839a7c72fd92da1fd7e4d9797b38bb566ba6e9b219bc84deffd205`.

- The original word renderer/animation engine and CSS supply 240 fresh highlight
  states and 24 pause frames per macOS/Linux fixture. Native state and macOS
  painted soft-edge columns agree, including reversed RTL gradients, foreground
  color/alpha, reused shaped twins and half-second pause fading. A portable ICU4X
  dictionary segmenter matches 116 original UTF-16 threshold, wrapping and RTL
  cases across the included scripts. Main cues split at the original boundaries;
  synchronized overlays retain their original cue grouping. Long wrapped and
  segmented cues preserve glyph layout while highlighting changes. Continuous
  cleanup/resume, paused-seek and overlay behavior add 52 original timeline
  frames per platform and pass native checks on both platforms. Windows Rust
  targets and strict workspace checks also pass. Ruby rendering and broader
  word presentation remain in audit. These changes postdate the packaged
  release hashes above.

- Lyric rows use the original fixed font sizes, semibold foreground text,
  line heights, percentage padding, viewport padding, inactive paint scaling
  and alignment-specific scale origins. Native geometry and accessible hit
  areas match 288 original layouts on both platforms. The original engine
  supplies 1,080 independent line-selection cases, including overlaps, lead
  times, paused playback, mixed word cues and zero timestamps.
- The original viewport edge mask supplies eight pixel captures per platform.
  Native compositing agrees for translucent layers, fractional geometry and
  scaled masks. The shared Metal/WGSL/HLSL filter path preserves the background
  visible through the fading text.
- Eighteen original sanitizer cases cover lyric formatting, entities, allowed
  links, ruby readings and rejected executable/local-file content. Native word
  twins reuse their rich text layouts; real-font checks verify readings and
  unchanged glyph geometry while highlighting moves. Japanese readings retain
  cue ends and share the word highlight. Background vocal and translation/
  pronunciation typography use the original relative font sizes and weights.
- Full suites before the final information/link changes pass: one capture-helper
  test, 207 macOS core tests, 208 Linux core tests and 177 native tests per
  platform, with the existing projectM smoke ignored. Both strict checks pass;
  all 20 Windows shader entry points compile and pass SPIR-V validation. The
  Windows Rust check initially rejected a pixel test that called a method absent
  on Windows; that test now uses the same platform guard as existing raster tests.
- Provider and matched-song information now appear inside both lyric surfaces.
  An additional 864 original layouts verify their spacing, font size, static
  dimming and effect on scroll limits. Gutter measurement resets when content or
  viewport size changes, avoiding narrow-layout feedback after removing rows.
  Scaled pointer tests verify allowed lyric links and reject unsafe URLs.
  These final additions pass focused macOS/Linux tests and strict validation;
  all Windows Rust targets compile. Release archives still predate this phase.
- Expanded live fixtures pass on Navidrome 0.64.2/0.54.5, Jellyfin 12.2.0/10.9.11
  and Gonic 0.22.0/0.16.4. Current servers and older Jellyfin return the two timed
  lyric lines; the older Navidrome/Gonic fixtures correctly return none. Both
  Navidrome fixtures preserve radio creation/editing/deletion. Supported modern
  Navidrome and both Jellyfin fixtures upload, retrieve and remove playlist
  artwork alongside the existing duplicate-playlist, queue and library checks.
- Refreshed macOS/Linux packages include this lyric phase, pass all 49 license,
  archive/executable and isolated installer checks, and preserve existing settings.
  The macOS bundle passes deep strict ad-hoc signing. SHA-256: macOS
  `cd4234dc38af9c5534b1f3abcd0fa44f8ca154dc563db134acf805dbe83ab1f3`;
  Linux `b9824e2f5fd88ea529e019ef99319f68cae275fcca33420b3bbd3092586f6ea1`.
  Pronunciation alignment work continues after these package snapshots.
- Forty-five cases execute the original UTF-16 pronunciation/ruby alignment
  helpers, including splits inside morphemes, whitespace, emoji and mismatched
  cue lengths. Native offline tokens retain source ranges and word ends; romaji
  overlays now use the same timed text-twin renderer. Actual dictionary checks
  split `音楽` between its two timed words without losing the readings. Both
  platform core suites pass (209 macOS / 210 Linux), focused native lyric/geometry/
  cue/wrapping checks pass, strict workspace checks pass, and all Windows Rust
  targets compile. Cue clicks seek their own raw timestamps and stop propagation
  to the enclosing line; invalid cue timestamps leave following paused. These
  changes postdate the package hashes immediately above. Broader real-dictionary
  pronunciation comparisons are now in progress.

- The original offline converter supplies 26 real IPADIC dictionary phrases,
  374 Hepburn spellings and character-range boundaries. Native tokens preserve
  UTF-16 ranges, contextual pronunciation, isolated fragment readings, original
  verb-token merging, macrons and romanization substitutions. The original
  conversion table/rules replace the earlier kana library and carry their MIT
  notice in native packages. Dictionary and reference checks pass on both
  macOS and Linux; Windows Rust targets compile.
- Lyric links retain Tab order, Enter activation, allowed external addresses and
  delegated line seeking. Twelve original browser keyboard cases and native
  accessibility/pointer checks cover wrapped labels, Arabic, inactive scaling
  and rejected executable addresses. Space does not activate anchors. The
  macOS source fixture waits for asynchronous popup delivery before comparing
  URLs. The lyric layout fixture preserves its explicit root-size override
  during the app's periodic theme refresh, removing a full-suite timing race.
- Ruby readings use the original half-size font, inherited weight/style and
  platform normal line metrics. Forty-eight original layouts per platform cover
  left/center/right alignment, narrow/wide wrapping and two text sizes. Native
  whole-block height comparisons allow 2.1 CSS pixels for font/layout differences;
  annotation overhang, complex inline wrapping and exact ruby baselines remain
  in the presentation audit. These changes postdate the latest package hashes.
- The full post-pronunciation/ruby macOS suite passes: one capture-helper test,
  211 core tests and 180 native tests, with the existing projectM smoke ignored.
  Fifteen focused lyric checks then cover keyboard reveal of offscreen anchors,
  manual-scroll preservation, rapid repeated activation before repaint and
  accessibility clicks. Reveal cancels active lyric scroll motion and starts
  the same two-second manual-follow pause; subsequent seeking resumes following.
  The headless frame helper runs callbacks before drawing, matching the native
  platform loop, so activation waits for the scrolled hit areas to update.
  Strict macOS checks pass. These focus additions postdate the full suite above.
- The final focus implementation passes the full Linux suite: one capture-helper
  test, 212 core tests and 181 native tests, with the existing projectM smoke
  ignored. Strict Linux checks and all Windows Rust targets pass. Original
  Japanese, keyboard-link and ruby references reproduce on both platforms.
- Refreshed macOS/Linux packages include pronunciation alignment, exact offline
  conversion and keyboard/accessibility lyric links. Both archives pass their
  executable and 50-license checks; isolated installers reject changed targets,
  replace the application, clean temporary files and preserve settings. The
  macOS bundle passes deep strict ad-hoc signing. SHA-256: macOS
  `d2e8d372deccf5c0133dc7d0d34cdc0d15ebf3274f357fa4b2de3629aa1f4455`;
  Linux `c6d367fbbc70b21d59f9914c43d786591d64a855dda086283fa5a22b94077750`.
  The next audit is bringing the original lyric export format, offset controls
  and selected-variant behavior across; full feature parity remains unfinished.
- Lyric export now opens the original format/offset controls with a selectable
  monospace preview and Copy/Export actions. Ninety-six cases execute the original
  formatter and normalizer, including raw text, structured/tuple lines, metadata,
  offsets, signed/fractional timestamps and empty arrays. Export uses the selected
  language rather than the primary cached raw string. Native mouse checks cover
  invalid offsets, live preview changes, clipboard contents, file-picker
  cancellation and atomic saving; the snapshot check preserves cached lyrics and
  the playback queue. The rich-text component now exposes its displayed content
  through accessibility. Focused macOS/Linux checks and Windows Rust compilation
  pass before that final shared accessibility addition. The full suites and
  refreshed packages are being checked after these changes.
- Full export/accessibility suites pass: one capture-helper test, 212 macOS core
  tests, 213 Linux core tests and 183 native tests per platform, with the existing
  projectM smoke ignored. Strict checks pass on both platforms and all Windows
  Rust targets compile. Refreshed packages pass archive, executable, 50-license
  and isolated installer checks; the macOS bundle passes deep strict ad-hoc
  signing. SHA-256: macOS
  `31df64e62e21b3db9c88861331c746153e50258776825739ab98ba631124e18d`;
  Linux `4067c86585534a41786cc939eaa0eecf5c60474c62b8b6cac0764794e1cc33f8`.
  Contextual localization work follows these package snapshots.
- Contextual labels now resolve before their ordinary key and retain context/
  plural/zero ordering and language fallback. Another 49,446 cases run the
  original i18next 25.10.10 across the existing catalogs, count choices and
  fallback languages. They pass alongside all previous nested/plural references.
  The full Linux core suite passes 214 tests, strict checks pass on both
  platforms and all Windows Rust targets compile. These changes postdate the
  export package hashes above.
- Formatted biography and release-note links now share the lyric focus controls.
  Headless HTML/Markdown checks cover Tab, Enter, Space, accessibility activation
  and offscreen reveal. The existing 12 alignment/width/scale cases also exercise
  formatted text, including wrapped labels and Arabic. Their pointer path uses
  aligned painted bounds, fixing centered/scaled links that opened the wrong URL.
  Full suites pass 213 macOS/214 Linux core tests and 184 native tests per
  platform, with the existing projectM smoke ignored. Strict checks pass and all
  Windows Rust targets compile. Linux independently reproduces the 49,446
  contextual localization cases. These changes postdate the packaged snapshots.
- The visualizer dialog and tooltip now use the existing fullscreen visualizer
  translation key. The native dialog check verifies its French accessible title.
  Raw LRC header offsets now follow the original display formatter rather than
  shifting line and word timestamps. Twenty-four original-runtime cases cover
  positive/negative offsets, header placement and two/three fractional digits;
  structured metadata and user display offsets retain their separate behavior.
  The translated dialog and timing checks pass on macOS/Linux; strict checks
  pass and all Windows Rust targets compile.
- Raw lyric display now preserves original plain-text stanza spacing, trailing
  lines, whitespace and metadata-looking text. It also recognizes NetEase's
  alternate millisecond timestamps and removes inline cue markers with the
  original punctuation spacing. The original formatter supplies 24 header
  offset cases, eight plain strings, 32 alternate inputs and 14 grammar/priority/
  order cases with 40 follow traces. Native checks render the blank/trailing rows
  with the configured gaps and preserve the playback queue. Whole-second LRC
  tags retain finite native timing instead of the source's absent-fraction NaN.
  Full core checks pass 217 macOS/218 Linux tests; 18 native lyric checks pass
  per platform. Strict checks pass on both platforms and all Windows Rust targets
  compile. Full native suites subsequently pass 184 tests per platform, with the
  existing projectM smoke ignored. Verified package snapshots have SHA-256:
  macOS `59ef4860e02a64330bee208231f49e505493b50ce4bc143fadc395b16664068e`;
  Linux `de9a15779ed38a23418aea4c72e583d6c6b3183a00d0e3eaf1379d7335cf9e25`.
  Both pass archive, executable, 50-license and isolated installer checks. The
  paragraph/link-color changes below postdate these package snapshots.
- Formatted lyric blocks now retain div breaks and collapsed one-em paragraph
  margins, including nested/empty paragraphs and whitespace between blocks.
  Empty text rows reserve their configured gap without adding a font line box.
  The original renderer supplies 96 paragraph layouts per platform in each of
  light/dark mode, alongside 48 existing ruby layouts. Native block heights stay
  within the existing 2.1 CSS-pixel font/layout bound. Painted links match original
  light/dark colors and retain the original absence of underlines. Core checks,
  all 18 focused native lyric checks and strict checks pass on both platforms;
  all Windows Rust targets compile. Exact ruby overhang/baselines and general
  presentation comparisons remain open.
- Full paragraph/link-color native suites pass 184 tests per platform, with the
  existing projectM smoke ignored. Verified packages have SHA-256:
  macOS `c0204f5b89d516bd3e2cea85f4e7c0c94fb61fac8c31968e547ce0cc098ed469`;
  Linux `a7d4d4d7a5bc205005cf2e95d71745ada0d15b780cf95504b51491fe6bd41fb1`.
  Archives, executable metadata, 50 licenses and isolated installers pass; the
  macOS bundle passes deep strict ad-hoc signing.
- Pronunciation-reading clicks now retain an enclosing lyric link and the
  parent line click. Twelve original rendered cases per platform verify the
  reading click and the browser accessibility name; native checks retain one
  link focus target and also exercise Enter. All 19 native lyric checks and
  strict checks pass on both platforms, and all Windows Rust targets compile.
  This small click fix postdates the package hashes immediately above.
- Karaoke pronunciation readings use the original 0.55-em size, medium weight
  and unit line height, including a bold enclosing word. Twenty-four layouts per
  platform render the actual original karaoke component; native tests compare
  whole-block geometry and rasterized reading ink against the original font
  metrics, allowing 1.5 CSS pixels for rasterization differences. Plain ruby keeps
  its separate half-size/inherited-weight/normal-line-height behavior. All 19
  native lyric checks and strict checks pass on macOS/Linux, and all Windows Rust
  targets compile. All five original reference modes reproduce on macOS. These
  changes postdate the package hashes above; full suites and refreshed packages
  are being verified.
- The full pronunciation/typography native suites pass 185 tests per platform,
  with the existing projectM smoke ignored. Verified packages have SHA-256:
  macOS `7a356dbe971e9b312ad064d1adde5a8de54928829501c2381fc0f91647de186a`;
  Linux `d42da38afbba3d31450deee4349bff617049ea4751f0e23aacea3db7fd83c5a4`.
  Archive/executable checks, 50 licenses and isolated installers pass; macOS
  deep strict ad-hoc signing passes. All five original reference modes reproduce
  independently on Linux as well as macOS. CJK justification work follows these
  package snapshots.
- CJK readings now distribute spare width across a wider base, with half a gap
  at each end. Forty-eight original cases per platform cover full/small hiragana,
  katakana and Han text, two sizes, two widths and three alignments. Native
  painted character separation matches within one CSS pixel. The comparator
  accounts for glyph batching order and uses the original main-text weight;
  existing plain/karaoke font and ink checks remain in the same matrix. Focused
  macOS/Linux comparisons and all 19 lyric checks pass, strict checks pass on both
  platforms and all Windows Rust targets compile. Longer annotations,
  mixed-script ruby and complex inline wrapping remain in the presentation audit.
- Long annotations can now borrow a bounded half-space from adjacent plain
  whitespace. The existing original layout matrix also compares neighboring
  single-character base positions at both text sizes and all three alignments;
  native glyph centers stay within 1.5 CSS pixels of the browser reference.
  All 19 lyric tests and strict checks pass on macOS/Linux; all Windows Rust
  targets compile. Thirty-six additional original layouts per platform verify
  overhang beside visible text at both ends and on either side independently.
  Native base centers match within 1.5 CSS pixels. Complex inline wrapping and
  mixed-script annotation baselines remain in the audit.

- The search page now preserves separate Tracks, Albums and Artists tabs, rather
  than merging and eagerly fetching every result type. Sidebar search opens the
  page, blank queries browse the selected type, typing debounces for 200 ms and
  Enter submits immediately. Tab changes replace the current history entry and
  retain the query, with the corresponding library layout. Native mouse/typing
  tests cover all tabs, canceled edits, back/forward restoration and an unchanged
  queue on macOS/Linux. Protocol tests check typed secondary filtering; all six
  real server fixtures pass blank/matching/no-match queries and one-item paging
  without lost or duplicate results. Full workspace checks pass on macOS/Linux:
  217/218 core tests and 186 native tests, plus one capture-helper check, with the
  existing projectM smoke ignored. Strict checks and all Windows Rust targets
  pass. Additional focused tests cover original plural captions in English,
  French and Arabic, Arabic tab direction and the restored folder Download
  action's save-dialog cancellation. The refreshed archives pass executable,
  50-license and isolated installer checks; macOS ad-hoc signing passes.
  SHA-256: macOS `b56a0c975284b2e093e4a4577d2a109f80c5d36143255b4500ce78b7a0519564`;
  Linux `44e3815f16154cf534e7dfafb2f5521bbb2f8ebaf7c2ae35e07d918070edd1c8`.
  Folder-tree and breadcrumb work follows these package snapshots.

- Library folders now browse Subsonic/Navidrome index roots and the selected
  Jellyfin library, including album directories. The lazy tree preserves paths,
  expansion and history, supports keyboard navigation, context menus and drag,
  and exposes tree/row accessibility states. Responsive breadcrumbs retain all
  ancestors through an overflow menu; 96 cases execute the original selection
  and truncation code. The main list always shares song-table columns and loads
  the current directory without song-page limits. Track double-click and folder
  play buttons use displayed songs without traversing nested directories.
  Shared table/grid selection matches 160 original Ctrl/Cmd/Shift cases.
  Headless tests cover narrow layouts, breadcrumbs, path history, selection,
  default-play preferences and queue preservation. The complete workspace passes
  218 macOS/219 Linux core tests and 190 native tests per platform, plus the
  capture-helper check, with the existing projectM smoke ignored. Strict checks
  pass on both platforms and all Windows Rust targets compile. All six current/
  older real server fixtures pass complete folder traversal with and without
  library selection. These changes postdate the latest package hashes above.
  Ordinary table-row interaction work follows this verified workspace snapshot.

- Ordinary table rows now select on a single click and activate on a double
  click. Original callbacks supply 224 playback cases for the bounded library
  window and full detail-list context. Headless mouse tests verify modifier
  selection, clicked-track positioning and independence from the saved play
  button preference. The full workspace passes 218 macOS/219 Linux core tests
  and 192 native tests per platform, plus the capture-helper check; the existing
  projectM smoke remains ignored. Strict checks pass on both platforms and all
  Windows Rust targets compile. Refreshed packages pass archive, executable,
  50-license and isolated installer checks; macOS deep strict ad-hoc signing
  passes. SHA-256: macOS
  `cda849da4d586c2ab7dae5293124ae1979f36e6458024611747e601f4f5c5823`;
  Linux `a1bcc2bd0e1ad3a9eb9b511ebaad417d320b00341c3d7553c95bf08cc51ee8dd`.
  Queue and expanded-album row corrections follow these package snapshots.

- Main/side queue text now selects on a single click, keeps ordered Ctrl/Cmd/
  Shift ranges, and plays the exact duplicate entry on double-click. Ranges
  follow the displayed filter, sort and page; removal also clears the selection
  anchor. Expanded album tracks use their own ordered song identities and loaded
  album context. Row Play controls queue only their clicked track, while grouped
  artwork preserves list playback and backend-independent Now/Next/Last rules.
  Headless tests exercise actual text clicks, duplicates, both queue surfaces,
  filters, pagination, cross-album selection, held controls and queue preservation.
  Original selection/playback callbacks supply the expected behavior.
- Jellyfin folder browsing now classifies by media type, including audiobook
  preferences and album directories without `IsFolder`. Unavailable audio rows
  are skipped without changing the server pagination offset. Four original
  controller cases cover empty/missing/null media sources and inconsistent folder
  flags; protocol checks also cover empty accepted pages, filtered pagination and
  repeated responses. The full workspace passes 219 macOS/220 Linux core tests,
  195 native tests per platform and the capture-helper check, with the existing
  projectM smoke ignored. Strict checks and all Windows Rust targets pass. All six
  current/older real server fixtures pass complete folder traversal and their
  existing integration checks. These changes postdate the latest package hashes;
  list keyboard work follows this verified workspace snapshot.

### Focused list keyboard and folder search verification

The completed table/queue/expanded-album selection snapshot passes 219 core tests
on macOS, 220 on Linux, 197 native tests on each, and the capture-helper test.
One native physical capture test remains intentionally ignored. Both platforms
pass strict workspace Clippy; all Windows Rust targets pass strict checks, with
native Windows C++ linking and runtime still unverified.

The original list-hotkey callback supplies 504 focus, selection-order, item-kind,
play-mode and key combinations. Native checks cover the 72 Select All cases and
actual mouse/keyboard selection, first-selected playback, saved play behavior,
text-input isolation, and filtered main/side queue deletion with duplicate songs.
These do not claim that all 504 cases have end-to-end native coverage. Twenty-four
original Jellyfin folder search/sort cases verify name/album/artist matching,
folder-first ordering and independent folder-name sorting.

Refreshed macOS and Linux archives pass executable/license checks and the isolated
updater's checksum rejection, replacement, cleanup and settings-preservation tests.
The macOS bundle also passes deep, strict ad-hoc signature verification.

- macOS ARM64 SHA-256: `5f2cb9519431c84ca52a31aee15aac84e8a139b44eefff605e58403ab49f8fb8`
- Linux ARM64 SHA-256: `74b80d5f7e0086715515084641cde5370d2cdcfb8f438ebb7ccc48c1bc763ddb`

Local-search and remaining navigation work follows this verified snapshot.


### Local search, current-track navigation and selection scope

Root library pages now display the shared local-search control. Forty-eight cases
execute the original SearchInput callbacks; the actual URL-filter hook confirms
its 300 ms debounce, history replacement and empty-query removal. Native input
checks exercise typing, cancellation, Enter, shortcut text replacement, Escape,
clear-button focus, filter restoration and the search/settings/queue focus rules.
The shared control's width/opacity transition frames remain in the visual audit.

The current-track shortcut preserves the current route and filter. Headless
keyboard tests center the loaded track in flat tables, album detail tables,
grouped song albums and expanded albums, including metadata/header geometry.
Unavailable tracks leave the viewport untouched. Twenty-four original table
callback cases cover absent/current/duplicate IDs and infinite-loader lookup
results; the native check also retains selection and playback state.

The list-key reference now executes the original ItemListState hook as well as
the hotkey callback. This exposed and corrected an earlier assumption: selected
items are returned in display order, regardless of click order. Main and side
queues now keep separate selection sets and range anchors. Native pointer tests
match four original independent-state snapshots, preserve duplicate track keys,
and verify separate drag payloads and deletion scopes. The shared deletion path
prunes removed keys from both selections while retaining unrelated selections.

This snapshot passes the full workspace suites: 219 core tests on macOS, 220 on
Linux, 199 native tests per platform and the capture-helper test. One physical
capture test remains ignored. Both platforms pass strict workspace Clippy, and
all Windows Rust targets pass strict checking. Windows native linking/runtime
remain unverified. Refreshed archives pass executable/license and isolated
installer checks; the macOS app passes deep, strict ad-hoc signature verification.

- macOS ARM64 SHA-256: `c612082535b3d8f033aba144211a850084808ad4ada0a30465b3ba16b94f690b`
- Linux ARM64 SHA-256: `0fcf4eeeba39b0b01a3e5e9e8ddffc9f7fd3c96e7625732d81cf9c07d619febc`

The next shared-search layout audit has 72 windowless original-browser cases on
each platform, including narrow viewports, larger fonts, clear/shortcut behavior
and filled-container controls. Native layout and transition comparisons follow
this verified functional snapshot.

### Shared search geometry and sidebar navigation audit

The shared search control matches 72 original windowless layouts per platform,
including narrow viewports, larger root fonts, filled containers, clear/escape
actions and shortcut focus. Native geometry runs at both 100% and 150% zoom.
The original renderer also supplies 110 normal/interrupted width and opacity
frames. Native checks compare the transition state and sampled painted frames;
pointer checks keep the overlapping clear button and underlying input reachable.
Tab/Enter retain the original fading button's keyboard behavior. These focused
checks pass on macOS/Linux, and both original references reproduce independently.

The sidebar audit corrects Now Playing to open the queue, restores the original
default visibility and 400-pixel width, and adds pointer resizing with the
original 260-400 bounds and collapse threshold. Seventy-eight original callback
sequences pass the Rust state check and native pointer checks at two zoom levels
in both reading directions. Artwork can open/close the expanded player, move
between the player bar and sidebar, and expose the current track's context menu.
Track titles open the queue; album labels open their detail page. These changes
are undergoing full workspace and platform regression checks. Packages still
refer to the previous verified snapshot above; full parity remains unfinished.

### Individual artist links and sidebar sections

Forty-six original JoinedArtists cases cover overlapping names, repeated and
unmatched artists, absent identities, case preservation and Unicode. Forty-four
original server-normalizer cases verify paired artist identities, legacy fields
and remixer inclusion for Navidrome, Jellyfin and Subsonic. The native player bar,
expanded player and artist columns support individual pointer, Enter and
accessibility navigation while retaining queue identity, at two zoom levels and
both reading directions. The artist-link snapshot passes 223 Linux core tests,
203 native tests and the capture-helper check, with one existing GPU smoke ignored.
A subsequent strict check found test-module placement, which has been corrected.

Sidebar section work adds independent library/collection disclosures, saved
collection order and rename/delete menus. The original component supplies the
initial sections, collection ordering and separate owned/shared playlist scopes.
A focused native pointer test verifies scrolling 75 playlists while the top
controls and artwork remain fixed. Separate owned/shared folder checks exercise
collapse persistence and navigation; a disposable HTTP server verifies scoped
folder moves and renames. These changes are undergoing combined workspace
regressions. They postdate the latest archive hashes above. Interactive checks
remain deferred, and full parity remains unfinished.

### Artist, sidebar and title-column regression snapshot

The combined artist/sidebar changes pass the full macOS and Linux workspaces:
222/223 core tests, 206 native tests on each platform, and the capture-helper test.
One physical GPU/capture smoke remains ignored. Strict workspace checks pass on
both platforms and all Windows Rust targets; Windows native linking and runtime
remain unverified. Collection highlighting follows live filters/search and marks
the corresponding library page.

The title-column audit executes 120 original title, title/artist and title/artwork
combinations, covering album versus track artists, grouped artwork, absent/empty/
Unicode subtitles and the queue's subtitle exclusion. Five original player-bar
cases retain subtitles and queue navigation. Twenty-four backend normalization
cases retain subtitle tags through queue persistence. These additions pass focused
native tests on both platforms, the complete 223/224 core suites, and strict
workspace checks. The full native regression above predates only these subtitle
additions. Native inline subtitles currently retain the title font size; the
original's smaller subtitle font and multiline title geometry remain in the
presentation audit.

All six pinned current/older Navidrome, Jellyfin and Gonic servers pass the
integration suite with generated subtitle metadata and paired artist identity
checks. This includes the previous authentication, complete folder traversal,
paged search, duplicate playlists, artwork, radio, lyrics and server-queue checks.
Archive refresh and remaining participant metadata work follow this snapshot.
Interactive checks remain deferred by request.

The artist/sidebar/title snapshot's refreshed archives pass executable, 50-license
and isolated installer checks. The macOS bundle also passes deep strict ad-hoc
signature verification. These packages precede the participant metadata changes.

- macOS ARM64 SHA-256: `93f9a47fcd46acdf81e5d2aa97387e60f5b5c42e5f30b5bc873f4ce7532694ae`
- Linux ARM64 SHA-256: `7146d0dd32e27e030481cbf31d6196c9c85dc6304aa74d77a18a4ce629bc392d`

### Additional participants and artist metadata links

Thirty original participant/composer cases preserve Navidrome and Subsonic roles,
subroles, missing identities and album-specific remixer behavior. The original
Navidrome normalizer groups raw subroles before assigning labels; absent and empty
subroles can replace one another, which the native normalizer now retains.
Composer columns expose individual links, and item information includes the full
participant list with independent accessibility identities. Seventeen original
card cases retain canonical artist names and bullet separators. Album-group artist
labels link individually, including when only related-artist names are available.

The complete core suites pass 225 tests on macOS and 226 on Linux. Focused native
checks cover composer, participant, card and grouped-album navigation at both zoom
levels and in both reading directions, retaining the music queue. Strict workspace
checks pass on both platforms and all Windows Rust targets. These changes follow
the latest verified archives above; genre-column navigation/badges and remaining
presentation work continue.

The participant snapshot also passes the full workspaces: 225/226 core tests,
206 native tests on each platform, and the capture-helper check, with one
physical GPU/capture smoke ignored. The macOS run completed in 340 seconds;
a sample traced its extra delay to wall-clock cursor blinking during slow test
frames. A platform-clock cursor correction and regression follow this snapshot.


The cursor now schedules its 500 ms blink and 300 ms focus pause through GPUI's
platform executor. The component regression advances just before and at each
boundary and verifies stopped/stale cycles stay inactive. Both component checks
and the affected visualizer settings test pass on macOS and Linux; the latter
completes in about 3.5 seconds on each platform. This change retains production
cursor timing while making headless time advancement deterministic.

### Genre identities, links and badges

Genre ID/name pairs now survive Navidrome, Subsonic and Jellyfin normalization
and saved queues. Twenty-one original normalizer cases cover missing identities,
numeric IDs, empty lists, repeated IDs and Subsonic's legacy name fallback.
The production API mapping and serialization are checked against those pairs.
Six original album-group cases retain the first name per ID in song order,
including comma-containing names and distinct IDs with the same display name.
Table, information-panel, detail-page and grouped-album links use those pairs;
card labels retain the original comma-separated text.

Badges retain the original first-four limit, seeded string-to-color palette,
Unicode word splitting, named-color mixing and luminance-based foreground.
Twenty-six original colors cover Unicode, punctuation and named-color mixtures.
Fourteen original column cases drive 112 native mouse/keyboard/accessibility
checks across table/detail, both zoom levels and LTR/RTL. Genre links leave the
queue intact. The full application test also checks information-panel and
album-group links alongside existing artist and composer links.

Ninety-six original Mantine layouts per platform cover wrapping table rows,
single-line detail rows, long names, root font sizes, zoom and RTL. Normal bounds
stay within one logical pixel. GPUI and Chromium distribute horizontal shrink
space differently when an individual badge exceeds the entire detail column;
those cases verify minimum badge widths, spacing, total packing and unchanged
vertical bounds. This is a known presentation difference. Source regeneration
is pinned and part of native CI. Interactive checks remain deferred.

### Participant ordering and server integration

The production JSON reader and typed participant map now retain insertion order.
The original Navidrome album helper assigns artist and remixer roles in server
field order; 48 original artist-normalization cases now include both orders.
Thirty-six participant/composer cases additionally verify displayed role order,
repeated-role grouping and persistence. The complete core suites pass 226 tests
on macOS and 227 on Linux with this representation.

All six disposable current/legacy server fixtures pass after the genre and
participant changes. They verify paired artist and genre identities, supported
track subtitles, library/search/queue/playlist behavior, artwork, streams and
mutations against Navidrome 0.64.2/0.54.5, Jellyfin 12.2.0/10.9.11 and Gonic
0.22.0/0.16.4. These fixtures use generated media and temporary local accounts.

The controllable cursor clock exposed a carousel test fixture that did not
request a redraw after replacing its home items when the theme was unchanged.
It now settles startup and explicitly notifies the view after seeding its data.
Its focused macOS test passes. The first full genre snapshot caught this fixture
failure and a Linux badge-density mismatch; it is not recorded as a passing run.

Genre geometry references now record the browser's display density and use real
browser zoom. Headless GPUI can simulate density changes, and badge borders use
the existing CSS device-pixel rounding helper. CSS font metrics and border caches
also follow display-density changes during normal rendering, without reloading
the theme. The regression exercises 1x/2x/1x transitions; physical monitor changes
remain part of the deferred interactive checks.


The corrected genre/participant snapshot passes the complete workspaces on both
platforms: 226 core tests on macOS, 227 on Linux, 207 native tests on each,
and the capture-helper test. The physical GPU/capture smoke remains ignored.
Formatting, strict workspace Clippy, both rich-text component regressions and
all Windows Rust targets pass. Original-source reference checks pass on both
platforms. Table width allocation and zoom parity work follows this snapshot.

The refreshed macOS and Linux archives pass executable, 53-license and isolated
installer checks. The macOS bundle passes deep strict ad-hoc signature checks.
These archives precede the table layout corrections below.

- macOS ARM64 SHA-256: `4f3cf400463f21b02b4b7fc3634ce81241b6747095f6a3f17063cf1e0188fe1a`
- Linux ARM64 SHA-256: `db18a237bd285b549d20ffb82ccb28c1712d77f479f419819260b36741e4cc4a`


### Table allocation, detail widths and zoom

The original table model supplies 160 width-allocation cases, including pinned
columns, proportional auto-fit, equal spare-space distribution, disabled columns,
zero widths, layout-fill space and rounding. Auto-size now follows those allocation
rules; the previous native text-measurement cache was removed. Display-density
variants preserve the original model's rounding at the measured viewport width.

The original detail table supplies 576 browser layout cases on each platform.
Detail cells retain their fixed numeric/action widths, proportional text widths,
padding contributions, empty-width fallback and narrow overflow. Table heights
are 40/66/88 CSS pixels; detail track heights are 32/40/48. Native geometry checks
cover both reading directions, both zoom levels, display density, and matching
headers/rows in the library, main queue, side queue and album detail.

Mouse checks retain column reorder and persisted widths. Resize uses the rendered
starting width and CSS-pixel deltas, previews without changing other auto-fit
columns, commits on release, and keeps the auto-fit preference. The source's
10-to-1000-pixel drag limits and disabled auto-size resize handles are retained.
The artist-link fixture now preserves its CSS viewport when changing zoom, so
every tested link remains visible. Complete workspaces pass: 227 core tests on
macOS, 228 on Linux, 208 native tests on each and the capture-helper test. The
physical capture smoke remains ignored. Formatting, strict workspace Clippy and
Windows Rust checks pass. Windowless original-source table checks pass on both
platforms.

Both rebuilt archives pass executable, 53-license and isolated installer checks;
the macOS bundle also passes deep strict ad-hoc signature verification.

- macOS ARM64 SHA-256: `6227e5092f11aabe61716028ad8c391e479cb596255b8b7eb2545c44177e8145`
- Linux ARM64 SHA-256: `6d3749435f146c6e0c35ecac58a848192b4d1a3b81e87d8be9d4d88744743da0`

### Title wrapping and playlist track drafts

The original title components and theme supply 144 browser layouts on each
platform. Native checks retain short-title height, compact/default/large line
limits, custom root font sizes, zoom, reading direction, artist line heights and
artwork widths. Detail rows remain single-line. Metadata cells retain the
original vertical padding. Hover play controls scale with zoom and use the
original base opacity. The 120 existing title/artist content cases and their
navigation/playback checks still pass. Mixed inline subtitle font sizes and
inline explicit-indicator wrapping remain presentation differences.

Playlist track editing now keeps a local draft until Save. The original row
callbacks and local reducer supply 168 single/selected/block/boundary cases.
Native mouse checks cover Up/Down, held movement, drops above/below rows,
duplicate-preserving removals, Cancel and Save As. Save compares the full original
server order before writing; conflicts reject replacement and failed writes keep
the draft for retry. Failed or cancelled loads cannot replace a later page.
The draft preserves queue state, list preferences and the separate expanded
player queue. Row boundary mapping also retains original positions in filtered
playlists and album groups. Complete workspace regressions pass: 227 core tests
on macOS, 228 on Linux, 210 native UI tests on each and the capture-helper test.
The physical capture smoke remains ignored. Formatting, strict Clippy and all
Windows Rust targets pass. Both refreshed packages pass archive/license and
isolated updater checks; the macOS ad hoc signature verifies.

- macOS SHA-256: `ef9d7051bea0266b2ed2b1dcfb95640ea62837dc7b4a3803a3a6c6de03118201`
- Linux SHA-256: `c01bcb9b8bad0621b0637e5d6d8d8338866ac8cbb23b7b1a2ce6307fdd4dd6cf`

### Metadata formatting and song normalization

The original table/detail components provide 806 cell cases, 624 absolute/partial
dates, 234 number formats and 2,535 relative dates across all 39 app languages.
Pinned windowless Chromium also supplies 180 local-time cases across six timezones,
including repeated hours, skipped hours, a half-hour shift and a skipped day.
Native formatting retains table/detail unit differences, zero count handling,
1024-byte boundaries, decimal rounding, day-length durations, original/release
year pairs, date precision, calendar/locale fallback and relative-time thresholds.
Headless native cells verify text in library, album tracks and both queue views.

Thirty original song normalizer cases verify all three backends' date fallback
fields, original/release year preservation, creation/last-played dates, bitrate,
bit depth, channels, sample rate, file size, disc defaults and sort names. The
fixes use Navidrome's actual sampleRate/playDate fields, Subsonic's created and
channelCount fields, and Jellyfin's first audio stream and media-source size.

Complete workspaces pass with 230 core tests on macOS, 231 on Linux, 211 native
tests on each and the capture-helper test. The physical capture smoke remains
ignored. Formatting, strict workspace Clippy and Windows Rust checks pass.
All six current/legacy live-server fixtures pass. Both refreshed archives pass
executable, 54-license and isolated installer checks; the macOS bundle passes
deep strict ad-hoc signature verification. Other metadata presentation surfaces
remain in the audit.

- macOS SHA-256: `c21fcc025007b2312c022378a0cb54a110f157ce6c5da7a74eef4d91def82ccd`
- Linux SHA-256: `2fda2d96b6f8113e25b07e6171ab3a817275194c08f4c8b6245237d9a2a506ef`

### Card, album and player metadata surfaces

The original components supply 429 duration cases, 234 cases each for cards,
album summaries, album groups and player fields, 468 song/album information
cases, and 936 player-field visibility cases across 39 languages. Native text
retains localized duration units and count plurals, release prefixes, track-year
ranges, size boundaries, first-track group metadata, translated release types,
player-specific units and zero-field visibility. Album primary release types
also follow the original backend normalizers; their version labels remain visible.
Headless GPUI checks verify text in all five surfaces in English, French and
Arabic without changing the queue. Complete workspaces pass with 231 core tests
on macOS, 232 on Linux, 212 native tests on each and the capture-helper test.
The Linux MPV IPC check timed out once, then passed in isolation and in the full
retry. Formatting, strict workspace Clippy, original-source checks and Windows
Rust checks pass. The archive hashes above precede these surface changes.


### Complete item information

The information view uses each original item type's field order and translated
labels, including compilation/favorite state, zero ratings and track numbers,
replay gain/peaks, updated dates, record labels, album version, artist biography,
MusicBrainz links, playlist ownership/visibility and smart-playlist state.
468 complete original component cases across 39 languages check song, album,
artist and playlist values and labels. Custom tags retain multivalue text and empty flags.
Artist, album-artist, album and genre text links navigate independently. Mapped
paths provide copy and system-open controls. Comments retain literal markup and
clickable URLs; eight original link cases check query strings, punctuation,
whitespace and attribute escaping. Native mouse/accessibility tests verify links,
clipboard values, gain/peak fields, tags and duplicate selected items while
preserving the playback queue. Complete workspaces pass with 233 core tests on
macOS, 234 on Linux, 213 native tests on each and the capture-helper test. Strict
Clippy and Windows Rust checks pass. The complete information references also
pass across all 39 languages. These checks precede the normalization fixes below.


Thirty-six additional original backend cases cover information fields for songs,
albums and artists. Ratings retain absent versus explicit zero state, including
live edits; Subsonic album/artist play counts stay absent. Last-played dates use
Navidrome playDate and its sentinel rule. Modified/created dates follow each
backend's actual fields and fallback order; unsupported album/artist sizes remain
absent. Complete suites pass with 234 core tests on macOS, 235 on Linux,
213 native tests on each and the capture-helper test. Strict workspace Clippy,
original-source checks and Windows Rust checks pass. Current and legacy
Navidrome, Jellyfin and Gonic integration runs also pass. The integration fixture
now waits for the initial scan before writing playlist and rating changes,
avoiding a Navidrome SQLite lock during startup.

Both packages pass archive, executable, 54-license and updater checks; the macOS
bundle also passes signature verification. These packages precede the subsequent
canonical explicit-status and inline-title changes.

- macOS SHA-256: `e763cb4e306078fa07ee98973cc44330202d32846f65442c42f1e9bf6175ce24`
- Linux SHA-256: `9c89dad3208c1f0a403dedab3c9ed3733ce1bffb5813a45e3aef6f710ce8fec6`

### Inline title fonts and wrapping

Title subtitles use the original 13-pixel font token and retain the distinct
subtitle line heights of plain, artist and artwork columns. Explicit/clean
Symbola badges share the title's inline text flow, so later rows use the full
column width. Per-run sizes reach CoreText, cosmic-text and DirectWrite, including
optical sizing, rasterization and font fallback. Per-row baselines and heights
also drive pointer hits and selection rectangles. Clamped text truncates the
last visible row with an ellipsis; hidden subtitles cannot enlarge that row.

216 windowless original component cases per platform cover three columns, three
row sizes, two root font sizes, two zoom levels, both directions, subtitles and
both badges. Native checks retain character advances within 0.13 CSS pixels,
exact wrapping and font sizes, and valid hits/selections. The complete table
checks now cover 360 cases per platform, with mixed-font heights bounded at one
native layout pixel. Both focused suites and strict workspace Clippy pass on
macOS and Linux. Original-source reference checks also pass on both platforms.
Canonical imported EXPLICIT/CLEAN values now drive the information view, badges
and artwork censoring alongside the existing raw backend values. Badge
accessibility labels retain nonzero bounds independently of their inline glyphs.
The complete native suites pass with 214 tests on each platform and the physical
capture smoke ignored. The two patched input component tests pass on both
platforms; strict workspace Clippy and all Windows Rust targets also pass.
The unchanged core suite has 234 passing tests on macOS and 235 on Linux, with
the capture-helper test passing on both. Windows linking and runtime rendering
remain unverified.

The refreshed archives pass executable, 54-license and isolated updater checks;
the macOS bundle's deep strict signature check passes. Both archives include the verified drag-preview scale, tag-editor localization,
mixed checkboxes and form-control shortcut exclusions. They precede the tag-save
validation and recovery changes below.

- macOS SHA-256: `6112340e3d80233c64680ae852c0789e3d17ff4eed36a69264269fd8372be40c`
- Linux SHA-256: `1f1605ac009aa5acab86fa059667d478a45ce2c369af5ab55b43b8693ea7c23f`

### Tag-editor language and mixed controls

Native tag controls use the original catalog keys for save, delete/undo,
single/multiple values, autocomplete sources, field configuration, selected-item
counts and rescanning. The shared input constructor preserves localized mixed
placeholders when value mode changes without translating metadata or tag names.
Rows wrap their controls to accommodate longer translations; selected-file and
rescan controls expose their state. A headless test clicks controls and checks
menus across all 39 languages, retaining literal metadata and the queue.

Mixed boolean fields render the original indeterminate state and expose it through
AccessKit. Mouse and keyboard checks cover mixed-to-checked and checked-to-unchecked
edits without writing audio files. Local playback/list shortcuts now exclude
focused checkboxes, radio fields and select controls, matching the original
form-input exclusion. The checkbox Space check also verifies that playback does
not change. Full native regressions pass with 216 tests and one deferred physical
capture test on each platform. Strict workspace Clippy, Windows Rust target
checks and refreshed package/installer checks pass. Windows linking/runtime
remain unverified.

### Tag-editor validation, field selection and save recovery

The original tag definitions, name resolver, add-field logic and empty-edit
validation generate 66 field definitions, 385 name variants, 776 add-field cases
and 1,234 blank-edit/removal cases. Native aliases use the TagLib wire names,
including MusicBrainz, ReplayGain, track/disc totals and AcoustID properties;
Electron field configurations use the same mapping. Typed controls match the
original definitions. Blank edited values fail before any file write, while
explicit removal remains available. New and mixed edits retain their dirty
state; undo restores mixed placeholders or removes a newly added field.

Known and configured fields are available through a filtered picker, and Enter
adds a typed name with duplicate/ASCII validation. Existing multivalue metadata
retains its value mode regardless of a single-value preference. Lyrics remain
multiline text and join stored entries with the original blank separator.
Only stored or newly added fields appear. Cancel discards pending edits and
returns to the prior page; Save and close returns after successful writes.
Artwork distinguishes mixed and absent covers with original translations.

A headless test edits disposable WAV metadata and artwork, forces one batch
member to fail, checks retained edits and per-file results, repairs the missing
file and retries through Save and close. Original PCM samples and unedited
metadata survive. Failed writes retain the draft; a server-refresh error remains
nonfatal. Complete native suites pass with 218 tests per platform plus one
deferred physical-capture test. Core suites pass with 235 tests on macOS and
236 on Linux; the capture helper passes on each. Strict workspace Clippy and
Windows Rust target checks pass. Windows linking/runtime remain unverified.

Archives pass executable, 55-license and isolated updater checks, and the macOS
bundle passes its deep strict signature check:

- macOS SHA-256: `8dded40d2f3cc78734483099749b84c580b5847952d1414c36643b820de44588`
- Linux SHA-256: `6abf013c461e65e7b0353c80992a71da5ba4b1556e4704a745feccef18defc5c`

A separate interoperability test executes the actual original metadata service
using pinned TagLib-WASM 1.5.1 on fifteen generated/DSD audio formats. Native
reads, metadata/artwork writes and removals match the original file contents,
with explicit preservation differences: the WASM API normalizes `03/12` to `3`
in WAV and drops TRACKTOTAL from MP3, MP4/AAC and ASF. Native writes retain the
literal WAV value and supported totals, and reject an unsupported ASF total
without replacing the file. Decoded PCM samples remain identical. The explicit
interoperability test and its strict Clippy check pass on macOS and Linux.

### Tag metadata normalization and server suggestions

The original service supplies 89 merged-metadata cases. Empty stored fields are
hidden, and lyric arrays join before mixed-value comparison. The original
controllers and suggestion hook supply 32 grouping cases, 161 source labels and
16 collation orderings across eight locales. Native suggestions retain separate
custom/server groups and case-sensitive choices while removing server entries
already represented by a custom value. Navidrome's excluded sources and
Jellyfin's Tags/Studios endpoints match the original. Catalogs use a per-session
one-hour cache that rejects revoked sessions and responds to Clear query cache
without blocking the UI on an active request.

Headless controls select Jellyfin sources and remove field configurations while
preserving pending metadata. The complete tag snapshot passes 238 core tests
on macOS, 239 on Linux and 219 native tests on both, with the capture helper
passing and physical capture deferred. Cache-expiry/invalidation tests and
strict workspace Clippy pass on both platforms. Original reference generators
reproduce on both. These changes postdate the archive hashes above.

### Navigation scroll restoration

Back/forward history now stores positions per visit, including separate visits
to the same route. Infinite lists reload their prior page count; paginated lists
restore their prior page. New visits and changed filters reset the position.
Pending restoration preserves the saved destination while content is short,
waits for asynchronous artist sections, and retries layout for at most 120
frames. Navigation, reloads and cache invalidation cancel obsolete restoration.

Native tests pass for repeated visits, forward/back navigation, asynchronous
paging, superseded requests, variable-height album rows and delayed artist
content. Complete workspace suites pass with 238 core tests on macOS, 239 on
Linux and 221 native tests on both. The capture helper passes; physical capture
is deferred. Strict workspace Clippy and all Windows Rust target checks pass.
Windows native linking and runtime remain unverified.

Both archives pass executable, 55-license and isolated updater checks; the macOS
bundle passes deep strict signature verification:

- macOS SHA-256: `4e10623fde8c9df24e998a6d3498b811dfe0b26bb8ec9c8e3707e6dc762e6eca`
- Linux SHA-256: `2da19666c0b9fff167db2e1f5b7ef91efbce03c238f327ab147e6f0ae7035c9c`

The next locale audit found a difference between ordinary Unicode lowercasing
and the original locale-specific catalog sorting, notably dotted/dotless I in
Turkish and Azerbaijani. ICU case mapping now preserves that behavior. Ten
additional original casing/sort fixtures cover English, Turkish, Azerbaijani,
Lithuanian and Greek. Core suites pass with 238 tests on macOS and 239 on
Linux; the four tag-editor control tests, strict workspace Clippy and all Windows
Rust target checks pass. Both refreshed archives pass executable, 55-license
and isolated updater checks; the macOS bundle passes deep strict signature
verification:

- macOS SHA-256: `c1b53256b6a98a5644a7c209cb033cd113ed9ad9b8c43d2afdb4f8eb9ec18c54`
- Linux SHA-256: `f58be7246d457f41472a7194b85baa67cf5993937d2d6b97d4e5d50d04000bc4`


### Numeric controls, sleep precision and custom transcoding

Auto DJ uses the original localized labels, descriptions, mode/strategy values
and selected states. Its count/timing fields now handle keyboard and mouse steps,
retain the original 1-50 and 1-5 limits, and persist valid edits immediately.
Long translated descriptions wrap within their columns. Searching the Auto DJ
section retains these controls in every catalog language. The tag configuration
Delete action now uses an existing catalog key in all 39 languages.

Other native numeric fields now handle step actions, including playback speed,
idle delay, autosave interval, volume controls, album artwork, sharing and sleep
time. Sleep timers accept hours, minutes and seconds, preserve an existing timer
when the custom value is invalid, and format hour-long countdowns as H:MM:SS.
End-of-album selection is disabled during track shuffle, matching the original.

Transcoding accepts custom unsigned bitrate/sample-rate limits and safe format
names instead of a small preset cycle. Empty limits remain absent in requests;
zero remains distinct from an absent bitrate. Existing native numeric bitrate
values still deserialize, and Electron imports retain optional/custom choices.
Jellyfin keeps its original MP3 fallback and lossy rate cap. Multiplying a bitrate
uses 64-bit arithmetic. A generator executes the original Subsonic and Jellyfin
controllers for 312 format/limit combinations and compares the native request
path and relevant parameters. Native controls validate all three fields together
before saving, preserving prior settings on an invalid edit.

Complete workspace suites pass with 240 core tests on macOS, 241 on Linux,
224 native tests on each and the capture helper. Physical capture remains
deferred. Strict workspace Clippy and all Windows Rust target checks pass.
Nine remaining playback/account switches now use the original translated labels
and selected states. The additional switch test passes in all 39 languages on
both platforms; strict Clippy and Windows Rust checks also pass with that change.
Both archives pass executable, 55-license and isolated updater checks. The macOS
bundle passes deep strict signature verification:

- macOS SHA-256: `1cbf5cf895058830ab3bc340407ec8e5e8e9e8938a187a66d63f53d807524cff`
- Linux SHA-256: `c0ce9104c2e6daafb6883074ef26144589bb68e26735701a24960b4499f35d7c`

These archives predate the Auto DJ collector correction below.

Live tests also pass against Navidrome 0.64.2/0.54.5, Jellyfin 12.2.0/10.9.11 and
Gonic 0.22.0/0.16.4. Each server returns decodable audio for both an omitted
bitrate and a custom 257 kbps request. The streams are decoded to FFmpeg's null
output without opening an audio device.


### Auto DJ collector and protocol comparison

The native collector now retains the original song/album query limits, skips
similarity requests when a music library is selected, mixes up to ten random
items before the genre pool in that case, and follows the original genre,
album-artist and random fallback order. It uses the first paired genre and
album artist instead of joined display text or an unrelated track artist.
Jellyfin receives genre IDs; album genre requests use the original backend
mapping. Navidrome artist requests retain their version-specific parameter.
Track similarity uses Subsonic's getSimilarSongs endpoint and excludes the seed
track even when duplicates are allowed. Album/artist radio endpoints retain
their separate behavior. Failed requests propagate without replacing them with
unrequested random choices.

A source generator executes both original collectors for 180 scenarios. The
protocol test checks request order, limits, filters and selected source counts
under both a server library selection and a native Auto DJ library override.
Random order is deliberately not compared. Duplicate rejection also applies
within one response pool, avoiding an original batch-filtering defect; this
preserves the user's duplicate preference. Core suites pass with 242 tests on
macOS and 243 on Linux. Expanded headless queue tests cover ordinary append,
queue replacement, a concurrent duplicate append and failed requests without
repeated retries. They pass on both platforms, as do strict workspace Clippy
and all Windows Rust target checks. Scoped song/album generation with both
strategies passes on all six disposable server versions listed above, with four
Auto DJ checks explicitly reported per server.

Both archives pass executable, 55-license and isolated updater checks; the macOS
bundle passes deep strict signature verification:

- macOS SHA-256: `58d92cb9a30e411ebc2c9f0a9e57465c52c807ebfa0f67ab480ba16b9a34c6e4`
- Linux SHA-256: `e659fd98abe8a1848391b1a216fd82135a43efeaac8bab5c9b04620a4be501f5`

These archives predate the direct player controls described below.


### Direct player controls

The player bar now exposes Auto DJ and sleep-timer popovers. Auto DJ shares the same validation, persistence and
controls as Settings, with only the active song/album strategy shown in the
popover. Each popover owns its input entities while synchronizing values with
Settings, so simultaneous views keep independent input focus and subscriptions
are released when the popover is removed.

Sleep presets, custom time and end modes use the same timer implementation.
Starting a timer closes its popover; canceling leaves it available for another
choice. The player button displays the timed countdown and selected state.
Custom time fields fit the compact panel and end-mode actions wrap. Both panels
have dialog roles and localized names, bounded height, outside-click handling
and Escape dismissal.

Two focused headless tests pass on macOS across English, French, Arabic and
Japanese, including numeric mouse/keyboard steps, invalid sleep durations,
settings persistence, independent focus, switching popovers, timer cancellation,
and preventing player-bar clicks from opening the expanded player. The check
for dismissal uses the live accessibility tree because GPUI's debug geometry map
retains historical selectors. A subsequent focus assertion found that closing
a popover left keyboard focus on its removed input. The component now restores
the prior focus only while focus still belongs to the popover. Both focused
tests and strict workspace Clippy pass with the correction. The broader suite
also exposed a track-radio mock using the old endpoint; its fixture now follows
the corrected original song-similarity protocol. Complete suites pass with 242
core tests on macOS, 243 on Linux and 227 native tests on each. Strict workspace
Clippy and all Windows Rust target checks pass. The archive hashes above predate
these controls.

The player Queue button and configurable shortcut now toggle the configured
side panel or queue drawer without altering navigation history. The Lyrics
button opens the expanded player and toggles its lyrics module. Player tabs
can be deselected, the default expanded player has no active module, and the
native-audio visualizer is available as a module beside the artwork. Tab names
use the original translations and the microphone icon is reproduced from the
locked original icon package. Silent headless checks cover pointer and keyboard
queue toggles, four locales, module visibility, persistence, queue preservation
and enabling/disabling the in-memory visualization buffer. Complete suites pass
with 242/243 core tests and 229 native tests on macOS/Linux, respectively; strict
Clippy and Windows Rust checks pass. These results predate the independent
Related-panel changes now being checked. Packages still need refreshing.

### Independent Related panel

Related tracks now use independent result, selection, scrolling and request state
with the shared metadata cells, column resizing/reordering, playback actions and
context menus. Opening or changing this module no longer navigates the library,
changes its history/selection or discards an unfinished playlist draft. Actual
page navigation dismisses the expanded player, matching the original location
effect. Artwork remains beside the Related table.

The original `songsQueries.similar` wrapper supplies a count of 50, and the shared
query defaults make results stale after ten seconds. The native panel preserves
server order, reuses fresh results and refetches stale results when reopened.
Requests omit unrelated library filters; Jellyfin retains its Similar/InstantMix
fallback. Blocking HTTP uses the existing blocking executor, while request
generations reject late results from another track. Errors have an explicit Retry
action and do not retry on every rendered frame.

Focused macOS protocol and headless tests pass. They exercise all three backend
protocols plus Jellyfin fallback, delayed old/new responses, cache reuse/expiry,
retry and empty results, selection and keyboard queue actions, favorite updates,
full-player columns and preservation of the browsing page and playlist draft.
The four-locale player-tab check also passes at the minimum 960-by-640 window
size, with player controls inside the window. The broader run found an artist-link
test that clicked old expanded-player geometry after navigation. It now asserts
the original dismissal behavior and reopens the player before the next link.
Complete suites now pass with 243 core tests on macOS, 244 on Linux and 230 native
tests on each, plus the audio-helper tests. Strict workspace Clippy and all
Windows Rust target checks pass. Windows native C++ linking/runtime and the
user-deferred interactive checks remain unverified.

The refreshed archives include all direct controls and the independent Related
panel. Both pass executable, archive, 55-license and isolated updater checks;
the macOS bundle passes deep strict signature verification:

- macOS SHA-256: `c018fb3d29d9ca558abdf3d9d195ad90cbf4a8f6d38640840e75c20be50418e3`
- Linux SHA-256: `49bda26ae95f41b53c74034ae2712eda02360f6d149c03c9a9823a11455895fe`

### Player configuration popover

The configuration action now opens the original playback workflow without
navigating away: engine and device selection, gapless/crossfade transitions,
four fade curves, 3-to-21-second duration, 0.01-step speed with BPM scaling and
double-click reset, pitch preservation/semitone/optional cent controls, and all
four sidebar module switches. Playing and non-native output guards follow the
original controls. Arabic reverses the label/control rows. The original sliders
icon is reproduced from the pinned icon package.

MPV device enumeration queries `audio-device-list` on an isolated idle instance,
retaining descriptions and distinct backend IDs, rejecting errors and bounding
the wait. The restart action stops playback and replaces the MPV process without
loading media. Native/MPV switches preserve the custom executable across saves
and retain each engine's device preference. Discovery runs off the UI executor
and ignores superseded results.

Headless pointer/accessibility checks cover four locales, minimum-window bounds,
duration release outside the slider, speed and pitch changes, sidebar toggles,
disabled controls, nested menus, dismissal, saved paths, and unchanged navigation
and queue contents. An IPC executable fixture checks enumeration, duplicate IDs,
unrelated events, failures, repeated engine restarts and absence of media loads.
Complete suites pass with 244/245 core tests on macOS/Linux, 231 native tests on
each and the audio-helper tests. Strict workspace Clippy and Windows Rust target
checks pass. The MPV fixture waits for consumed IPC commands before replacing an
instance, avoiding a scheduler-dependent assertion. The Linux accessibility
example also handles the slider's new committed event.

Refreshed archives pass executable, archive, 55-license and isolated updater
checks, and the macOS app passes deep strict signature verification:

- macOS SHA-256: `6f30e48f388e20f1c22d732960c3fb999a4689e879ca0a0e87c14c081edd34d7`
- Linux SHA-256: `65c2b93101d26d1e494ff36714ada88bf57e1eeced774afecb663f61ddeb37f6`

These archives include the configuration popover and predate the volume-button
device menu and wheel changes currently being checked.

### Volume wheel and output menu

The mute icon and volume slider share the original wheel-step behavior, including
both axes, clamping, clearing mute and ignoring zero deltas. The icon reflects
low/high/muted state and has the original volume tooltip. Right-click opens the
current engine's output list; native and MPV device IDs retain their descriptions.
Device changes are allowed during playback, while casting and Jukebox do not expose
local-device controls. Engine/generation checks reject obsolete asynchronous lists
and choices. The Outputs page uses the same query and selection paths.

The context menu now restores the prior focus when it still owns focus at dismissal.
A pointer test verifies the subsequent keyboard volume shortcut, device selection
while playing, asynchronous refresh, stale choices, persistence, unchanged queue
and navigation, and absence of media loads. The shared MPV fixture records consumed
IPC commands without accessing audio hardware. Complete volume-checkpoint suites
passed with 244/245 core tests on macOS/Linux, 232 native tests on each, strict
workspace Clippy and all Windows Rust target checks.

The volume-checkpoint packages passed executable, archive, 55-license and isolated
updater checks; the macOS bundle also passed deep strict signature verification:

- macOS SHA-256: `b6376766ad8365e2cfca9975dbd2133c3016387e93db088048dd1d9ad64ddd54`
- Linux SHA-256: `91c5e623ed56db9c4c18ed04e1a685d167ed65effc002804989f9e3fb913f0ba`

These hashes predate the following rating and engine-import corrections.

### Ratings and engine import

Shared five-star controls replace cycle buttons in library actions, table cells
and the player bar. Selecting the current value clears it; a 100 ms debounce
coalesces rapid choices. Keyboard focus/selection, separate labeled radio choices
and RTL order are supported. The original Mantine star geometry is reproduced
from the pinned package. Confirmed rating actions and shortcuts update matching
library, queue, related and metadata entries without reloading the browsing page.
Replies only change requested fields, preserving concurrent favorite changes.

Subsonic generic-error suppression is restricted to optional external metadata
lookups. Failed rating writes leave the confirmed rating unchanged and report the
server error, including numeric error zero. The source-derived fixture checks all
30 current/selected combinations and the original callback's debounce/propagation;
headless tests exercise those choices, both rating protocols, four languages,
minimum-window bounds, keyboard commands, shared updates, error handling and
navigation/queue preservation.

Electron playback.type now actually selects native audio or MPV while preserving
custom executable paths. Jukebox retains local-engine preferences and native
exports retain their exact saved paths. Migration tests cover repeated switching,
default MPV selection, unrelated imports and reviewable changes. Complete suites
pass with 245/246 core tests on macOS/Linux and 233 native tests on each, plus the
audio-helper tests. Strict workspace Clippy and Windows Rust target checks pass.
The subsequent mixed-server control check passes on both platforms: rating
availability follows the item's server rather than the active browsing account.
Rating-star assets also pass the native/double-size rendering check.

Refreshed packages pass archive, executable, 55-license and isolated updater
checks, with deep strict signature verification on macOS:

- macOS SHA-256: `055942fd47781629c60a505b597cce623302cab22ef5d3096f95f1013e401eb5`
- Linux SHA-256: `a64ed1cc1188ad431de2f9aa88a8b0260fdaefe103740a6eaf66c63b39a6ccf6`

These packages predate the row-hover and pending-save control changes now under
test. Inactive table favorites/ratings should appear only while hovering the row,
and table controls should reject duplicate clicks during a pending save.

### Table hover and pending writes

Inactive table favorites and ratings now follow explicit row hover state across
library, album tracks, main/side queues and Related tables. This matches the
original row-hover property and keeps the accessibility tree consistent with
what is visible. CSS-only group visibility left visible controls hidden from the
accessibility tree. Pointer interactions retain table selection and avoid row
navigation or dragging.

Favorite and rating request counts disable their respective table controls while
writes are pending. Blocking network work uses the existing blocking executor;
requests update only confirmed fields and clear pending state on both success
and failure. The rating entity notifies GPUI when its value, language or read-only
state changes so cached rendering and accessibility selection cannot remain stale.
The original player-bar controls retain their independent behavior.

The focused headless test passes with delayed rating/favorite replies, duplicate
clicks, failed writes, rendered selected/disabled states, hover enter/leave,
keyboard selection and preserved queue/history. Complete suites pass with
245/246 core tests on macOS/Linux and 233 native tests on each, plus the helper
tests. Strict workspace Clippy and all Windows Rust target checks pass.

The row-hover packages pass archive, executable, 55-license and isolated updater
checks; the macOS bundle also passes deep strict signature verification:

- macOS SHA-256: `120e6d8c82ed9925d4ee44e35309e00574a76254a3dfddf3495bea1a0a9a7ca3`
- Linux SHA-256: `2d16f54b8d215b2e30457545fc0286f8aa5ee417ac94ec5a8c34ae399716fb63`

These archives predate the following favorite and artist-list changes.

### Confirmed favorites, artist membership and remaining star controls

All six current/previous favorite shortcuts and remote favorite/rating commands
now update the same confirmed caches, including previous-track metadata. Late
previous-track replies cannot replace a newer track. Pending remote writes also
disable the matching table controls. Successful song changes refresh the current
artist's favorite/rated membership; failures and other accounts do not. Overview
refreshes preserve existing contents while loading and leave other sections alone.
Dedicated lists restore browsing position without adding navigation history.

The first mutation checkpoint passes complete macOS/Linux suites with 245/246
core tests and 234 native tests, strict workspace Clippy and Windows Rust checks.
A further comparison found that artist rated lists require three or more stars,
with original rating/favorite/play-count/album/track tie-breakers. Ninety-six songs
execute both original controllers to supply independent membership/order cases.
Jellyfin favorite lists retain play-count order. Overview/full favorite and rated
lists share saved sorting, including server-order preservation and reversal.
Artist overview and expanded album controls now use the shared five-star widget,
including pending-write guards, instead of cycling through numeric values.

Complete suites pass with 246/247 core tests on macOS/Linux, 234 native tests on
each and the helper tests. Strict workspace Clippy and all Windows Rust target
checks pass. The focused mutation regression also verifies the last-played cache
that supplies previous-track actions. Confirmed changes reach cached similar
artists as well. Source-derived membership/order cases reproduce on both hosts.

Refreshed packages pass archive, executable, 55-license and isolated updater
checks; the macOS bundle also passes deep strict signature verification:

- macOS SHA-256: `65133fc4bda4b8548031a9fa5c92c857351cf77e6fe16c05b31019abcef73261`
- Linux SHA-256: `1020d50063c6d3a24640e7601b1ff250c6e956466c56608b30e2d1462093b5ff`

These archives predate the artist overview table work below. Interactive checks
remain deferred.

### Artist overview tables

Artist overview song rows now use the shared
configurable song table, including independent selection/focus, column drag and
resize, virtual scrolling, metadata links, shared rating controls, context actions
and section Play/Next/Last controls. The original container supplies 24 independent
height cases across row sizes, header visibility and the five-row cap. Original
single-click selection and double-click playback replace direct row activation.
Twelve original playback cases distinguish first-selected-song keyboard commands
and row artwork controls from displayed-list double-click playback. Section
controls enqueue the full source list, with held shuffle and release suppression.
Confirmed membership updates retain scroll positions and prune removed selections.

The shared song sorter preserves the original secondary album/disc/track keys,
case-sensitive sort names/comments, first artist/genre keys, local calendar days
for added dates, missing metadata placement and stable complete ties. Seventy-two
songs execute the original sorter across 38 ascending/descending orders. Playlists
use the same implementation. Artist favorites expose the original 19 sort choices;
random order is retained across redraws and rebuilt when its inputs change.
Queue descending sorting also retains complete ties.

Rating controls use the original 100 ms ease hover scale, ignore input while
read-only, clear keyboard previews on focus departure and suppress list shortcuts
while focused. Complete workspace suites pass with 247/248 core tests on
macOS/Linux, 236 native tests on each and the helper tests. Strict workspace Clippy
passes on both platforms, and all Windows Rust targets pass strict checks. The
source-derived comparisons reproduce on both hosts.

The rebuilt macOS archive passes executable, 55-license, isolated updater and
deep strict signature checks. SHA-256:
`4d7aca2f96cc579d75e3ee0cf27b64f46653317a2627c737ae1797076604cf6c`.
The Linux archive passes executable, 55-license and isolated updater checks.
SHA-256: `57679a934bff8077c6e1dbfee7a7750eda0868bc56feb7bfec4e8bc32557dedd`.
Shared number-column hover playback controls and standalone image-column playback
overlays remain the next implementation work. Both archives predate that work.

### Shared row playback and sidebar playlists

The first visible row-index or track-number column now supplies delayed playback
controls, active song/album/artist/playlist indicators and duplicate-safe queue
markers. The popover preserves 150 ms opening and closing delays, pointer travel
between target and popup, held shuffle and release suppression. Escape, outside
clicks, scrolling, resizing and row removal dismiss it. Eight original windowless
Electron layouts and 80 fade frames reproduce across font sizes and zoom levels,
including placement at the top edge. The original hooks supply six column-priority
cases, 144 control states and 96 playlist-context cases. Queue entries retain their
playlist source through persistence and confirmation, while merged playlists and
ordinary data insertion follow the original context rules.

Standalone and combined-title artwork use one hover/hold control. Compact artwork
crops to the available cell; other row sizes retain square covers. Sixty-four
original playback callbacks check preferences, single-song actions, held releases
and duplicate queue identities. Conditional rendering also keeps hidden controls
out of the accessibility tree. Queue artwork retains a spoken Play label without
a tooltip. The first combined checkpoint passes full suites with 247/248 core
tests on macOS/Linux and 241 native tests on each, plus strict workspace Clippy.

Expanded and compact sidebar playlist rows now share Play/Next/Last controls.
Expanded rows include count, duration, ownership and public status; the source
playlist highlights the title or collapsed artwork. Collapsed artwork uses the
shared 32-pixel play overlay and the configured playback tooltip. Ninety-six
original component states check English/French metadata, hover visibility, active
colors and queue callbacks. Headless checks also cover pending-fetch disabling,
held-click suppression and navigation preservation. The final combined suites pass
247/248 core tests on macOS/Linux, 242 native tests on each, and one audio helper
test. Both strict workspace Clippy runs and the Windows Rust checks pass. The
workspace run skips the separate offscreen projectM and prepared-fixture TagLib
integration tests, which have their own dedicated commands.
Windows native C++ linking and runtime remain unverified.

Both refreshed archives pass executable, 55-license and installer/update checks.
The macOS application also passes deep strict ad-hoc signature verification.

- macOS SHA-256: `c98920b737386faa0c3dd5ad0a7a3fcb1e6a24ecb9dbf50de2928b10e85e38c9`
- Linux SHA-256: `b06b8ecd11722c49b48ee8b701c0166e6978ac49a9c881c2ec8351eb9f5481f9`

Sidebar and queue drag/drop edge placement is under implementation. The audit
also found that sidebar add drops must open a preselected, multi-playlist dialog
with persistent duplicate skipping and new-playlist creation; the current route
picker still performs immediate additions. That dialog and the remaining drag
and collapsed-artwork presentation differences are outstanding. Interactive
acceptance checks remain deferred. The archives above predate these changes.


### Queue placement and multi-playlist addition

Both queue views and sidebar playlists now accept drops above or below a row.
The drop callback receives the complete hitbox, including borders, so an exact
midpoint follows the original top-edge tie break. One hundred original callback
cases check block order and duplicate identities; headless pointer tests cover
all three surfaces at 100% and 150% zoom. Playlist track rows share the same edge
measurement.

Add-to-playlist actions now open a dialog without changing navigation or queue
selection. Sidebar drops preselect the destination; smart playlists reject add
drops. The dialog supports multiple existing and new playlists, search, keyboard
selection, held state during submission, cancellation, and a persistent duplicate
preference. Duplicate skipping matches the original: existing destination tracks
are excluded while repeated selected source tracks retain their multiplicity.
The legacy local-storage preference imports as either a boolean or its string
representation. Partial failures retain the failed destinations for retry and do
not recreate new playlists or append again to completed destinations.

Seventy-two original modal submissions match a local HTTP fixture across song,
album, artist, genre, folder and playlist sources. The combined checkpoint passes
250/251 core tests on macOS/Linux and 246 native tests on each, plus the audio
helper test. Strict workspace Clippy passes on both hosts, and all Windows Rust
targets pass checks. The two separately exercised projectM and TagLib integrations
remain excluded from the ordinary workspace run. Windows native linking and
runtime remain unverified. The packaged artifacts above predate this checkpoint.

### Sidebar drag feedback and source context

Filtered playlist reordering now saves only the visible owned or shared scope,
matching 20 original hook/callback cases, including clearing the filter afterward.
The other scope retains its saved order. Sidebar drags dim their source row;
non-playlist additions dim smart playlists and outline valid hovered destinations.
Collapsed artwork retains the 200 ms ease transition to 1.08 scale, including the
transformed accessible and clickable bounds. Sixteen original component states,
48 Motion opacity frames and 16 CSS scale frames reproduce on macOS and Linux.
Native pointer tests cover full and interrupted fades, smart sources, disabled
sorting, border placement, cancellation and clicks outside unscaled artwork bounds.

Dropped playlist tracks and single-playlist fetches now retain the source context.
Merged playlists and ordinary additions preserve the original context rules;
queue reorders retain their existing identities and playback position. Forty-eight
additional original context cases cover both row edges, repeated tracks, prior
context and current navigation. Cancelling a pending drop fetch leaves the queue
unchanged. The combined suites pass 250/251 core tests on macOS/Linux, 248
native tests on each, and the audio helper test. Both strict Clippy runs and the
Windows Rust checks pass. Both packages pass archive, executable, 55-license and
installer checks; macOS also passes deep strict ad-hoc signature verification.

- macOS SHA-256: `ae262e8d0f396378a4c6a387fb5f02960a76e1119b8ea2365da1043b86d599cc`
- Linux SHA-256: `f74ffff8a71c960ae83455b777d259a4120336a4e15c3e93f7a95a20e9cfe6bf`

Expanded-player responsive layout and transitions are the next implementation
audit. Remaining audits include other presentation, localization and RTL
differences. Physical acceptance remains deferred by the user.


### Expanded-player layout, artwork and panel transitions

The expanded view now fills the content area, with overlay controls and the
original landscape/portrait columns, maximum width, padding and artwork-size
constraints. Its columns match 16 original layouts, 48 transition frames and
64 interrupted reversal frames. A local canvas resolves artwork width after flex
shrinking, matching 108 original artwork/metadata layouts and intrinsic-image
frames. Original browser references reproduce on macOS and Linux, allowing at
most 0.02 CSS pixel for Chromium platform layout quantization.

Panel switches retain outgoing content for the original 400 ms exit before the
next panel enters. Artwork uses overlapping 400/800 ms fades and retains the
whole exiting group until every image finishes. Rapid changes and reversals match
240 original Motion frames. Queue and related masks use the original 5% bottom
fade or top/bottom fades according to the header setting. Artwork keeps its
intrinsic aspect ratio, rounded corners, shadow and 30-pixel explicit-image blur;
33 PNG/JPEG/WebP orientation cases also check the displayed aspect ratio.

The title uses the original weight/size, five-second pause, 50-pixel-per-second
scroll, 100-pixel gap and duplicate text. Thirty-nine original effect frames check
the pause and repeating motion. Resizing or replacing the title resets its clock.
Metadata follows the configured alignment, badge grouping and measured artwork
width. Player settings and table settings open over the player; headless pointer
checks cover edits, Escape dismissal, persisted settings, queue/navigation
preservation and restoration of an underlying layout draft. The expanded queue
uses the original complete table, ignoring imported library-style pagination and
card display choices.

The first combined run exposed two regressions: the application menu had become
inaccessible in the expanded view, and the related-panel test assumed instant
switching. The menu is retained in the overlay and the interaction test advances
the real transition clock. Both focused tests pass. The final combined suites pass on macOS (250 core,
254 native, one audio helper) and Linux (251 core, 254 native, one audio helper),
with the two documented GPU/TagLib fixture tests excluded from the ordinary suite.
Strict Clippy passes on both; Windows Rust checks pass. Both release archives pass
executable, 55-license and installer replacement/settings-preservation checks;
the macOS app also passes deep strict signature verification.

Checkpoint archives, before the subsequent background/overlay increment:
- macOS SHA-256: `e7c8c4c3806c98395aaa70186d514baff5314f1a1ed136e6dc7943213a913e48`.
- Linux SHA-256: `2b6c21ad469644377e5ab68429c5f719c938c182684bfa56dedc27cf51b5b816`.

Remaining audits include other presentation details, localization and RTL.
Physical acceptance remains deferred.


### Expanded-player backgrounds and overlay lifecycle

The player now stays over the library during its original entrance and exit:
100 ms entrance delay, 500 ms ease-out slide, retained outgoing content and
full-duration interrupted reversals. Restored expansion starts settled. Dominant
background colors use Motion's squared-channel interpolation and transition delay.
The actual original Motion variants supply 152 frames, reproduced on macOS and
Linux; native comparisons allow Chromium's color serialization and subpixel
rounding.

Backgrounds now use original card-resolution artwork, album art over the track
fallback, the music queue's image during radio mode, overlapping 400/800 ms image
fades, black shading at opacity/120, the independent theme header overlay, and
screen-space backdrop blur in rem. Playing radio disables dynamic layers; pausing
restores them. Pending artwork keeps the prior dominant color. The large artwork's
resolution and censoring remain independent of the background image.

Headless checks inspect painted shading and blur, exercise radio state, minimize,
reopen and navigation-triggered exits, and preserve queue/history/settings.
Keeping the library beneath the overlay exposed a shared scrollbar bug: bounds-only
capture handlers intercepted covered controls. Scrollbars now register hitboxes
and honor occlusion while preserving dragging outside their tracks. The real
localized player-tab test and focused covered/uncovered scrollbar tests pass.
Some existing interaction tests needed to advance the new entrance/exit clock.
The macOS workspace passes 250 core, 257 native and one audio-helper test,
with the two documented GPU/TagLib fixture tests excluded from the ordinary run.
Strict Clippy, archive/executable/55-license checks, installer preservation and
deep strict app signature verification pass. macOS archive SHA-256:
`7b807f7b4552df868ed32de7e59812f0c38ba7f938d06ab5a9f75877180332ac`.
The matching Linux suite passes 251 core, 257 native and one audio-helper test.
Strict Clippy, Windows Rust checks and Linux archive/executable/55-license and
installer preservation checks pass. Linux archive SHA-256:
`c995af6c5098ccb9f84f64e383d5660816d0dbcf2415775cb189a3fa9f08a5a3`.
Windows native C++ linking/runtime and physical acceptance remain unverified.


### Album and artist background layers

Detail backgrounds retain their cover-derived color when image mode is disabled.
Album colors use raw card artwork; artist colors use raw header artwork. Image
mode uses raw card artwork, independent of explicit thumbnail censoring, with
0.7 opacity and foreground blur measured in rem. The separate theme overlay keeps
0.9 opacity. Background dimensions track the rendered header: image height minus
64 CSS pixels, overlay height plus 64, and a 200-pixel minimum for color mode.
The layer sits behind its header rather than the global navigation chrome.

Forty original component/ResizeObserver layouts reproduce on macOS and Linux.
Eighty native layouts check size, opacity, blur and zoom; actual album, artist
and album-artist routes check source resolution, color retention, toggles and
queue preservation. Full workspaces and strict Clippy pass on macOS (250 core,
259 native, one audio helper) and Linux (251 core, 259 native, one audio helper),
with the two documented fixture/GPU tests excluded from the ordinary suites.
The broader header presentation remains under audit.


### Detail artwork viewer

Album, artist and playlist covers open a fullscreen artwork viewer by click,
Return or Space. The viewer requests the original image independently of saved
thumbnail/player resolutions, fits its aspect ratio, applies explicit-image
blur and shading, and closes by image click, close button or Escape. The native
interaction check verifies the actual HTTP request and queue/history preservation.
Original header handlers and BaseImage CSS supply the activation and image rules;
the shared Mantine modal now also supplies frame and close-button measurements.

Before the final modal-frame alignment, full suites passed on macOS (250 core,
260 native, one audio helper) and Linux (251 core, 260 native, one audio helper).
Both strict Clippy runs, Windows Rust checks, archive/executable/55-license checks
and installer preservation checks passed. Deep strict macOS signature verification
also passed. Checkpoint archives:
- macOS SHA-256: `3fc8bd4d2c4ccde617202515c331cf9bb6e6c9d3a17618d28c68efd2fd112c73`.
- Linux SHA-256: `2889f4c0fb51a38a78990049d9fc40d0e59995c1faa9f0b4c7dd8c1b0d23f90b`.

The modal-frame alignment uses a 300 ms opacity-only entrance and matches the
original image inset and close button. All 30 original interactions, including
16- and 20-pixel root fonts, pass on macOS and Linux. Physical acceptance remains
deferred; modal exit animation is still under comparison.

### Native artwork file drops

Artist, album-artist and playlist artwork accepts the first image from a native
file drop on Navidrome and Jellyfin. Picker and drop controls share the item's
server capability check. Dropped directories and non-image files are skipped;
invalid images and failed uploads retain the existing cover revision. Tag artwork
uses the same MIME selection helper. GPUI now assigns hitboxes to elements with
drag-over styles and redraws when an external drag exits, keeping the indicator
responsive without manual refreshes.

Headless platform events cover entry, movement, exit, cancellation and submission,
painted indicator geometry, six supported item/backend combinations, unsupported
targets, mixed files, multipart/base64 uploads, errors, queue and route/history
preservation. Full suites and strict Clippy pass on macOS
(250 core, 261 native, one audio helper) and Linux (251 core, 261 native, one audio
helper), with the two documented fixture/GPU tests excluded. Windows Rust checks
pass; native C++ linking and runtime remain unverified.

Refreshed checkpoint archives pass executable and 55-license checks, installer
checksum/replacement/data-preservation checks, and deep strict macOS signature
verification:

- macOS SHA-256: `37a00066e3103add8b0eee02d63582ac1a6f52ef159a68bc58922d6f15122f93`.
- Linux SHA-256: `914623188997e6909cc603db5e73ee89ed423fc82c55c47834e49cf8663b946c`.

These packages predate the detail-header playback work below. The responsive
header audit also now has 56 original runtime layouts on both platforms; native
layout comparison is pending.

### Detail-header playback

Playlist headers now retain the original collapse preference, including imports
from boolean and string local-storage exports. Collapsed and expanded playback
uses the displayed tracks in their current order, retaining duplicate playlist
entries and their playlist context. Expanded headers provide Play now, Next and
Last; holding each button shuffles that operation without a second click action.

Artist header playback fetches the complete discography independently of the
current song tab, text filter and selection. It applies saved album sorting and
release-group order/visibility, then keeps only tracks performed by the artist
or belonging to that album artist. Membership uses the original normalized
identities, including remixers and legacy server artist fields; explicitly empty
identities take precedence over obsolete saved fields. The default album order
is newest release first. Empty responses, partial failures and cancellation leave
the existing queue intact. Actions wrap within the minimum 960 by 640 window.
Full suites pass on macOS (250 core, 263 native, one audio helper) and Linux
(251 core, 263 native, one audio helper), with the two documented exclusions.
After the normalized-identity correction, the complete core suites pass with
251 tests on macOS and 252 on Linux, and the focused header checks pass on both.
Strict Clippy and Windows Rust checks also pass.

Checkpoint packages include the playback and identity changes and pass archive,
executable, 55-license and installer-preservation checks. The macOS bundle also
passes deep strict signature verification:

- macOS SHA-256: `50ed1fd1098f8a282d388b4a13a9cc96ad754d134bff50a23140656979b537ab`.
- Linux SHA-256: `e1bf91e3908568bc5855716dc79d91329fa29fa78a03a33b4963108b4bf92aaf`.

These packages predate the detail-page scrolling/layout work now in progress.
Windows native C++ linking and runtime remain unverified; the production build
uses MSVC and vcpkg on a Windows host. Interactive acceptance remains deferred.

The preceding collapse-only snapshot passed full suites and strict Clippy on
macOS (250 core, 262 native, one audio helper) and Linux (251 core, 262 native,
one audio helper), with the two documented exclusions. Windows Rust checks now
use a separate build directory because their intentionally disabled C++ build
steps must not replace production linker metadata. Windows checks and a subsequent
native Linux link/test pass with that isolation.

### Scrolling and responsive detail headers

Album, artist and playlist pages now scroll their header with their contents.
The artist overview uses its existing page scroll; other detail layouts reuse
the variable-height list, including grouped songs, release groups and expanded
albums. Back/Forward restoration and centering the playing track use that same
scroll position. Infinite loading no longer reads a previous uniform table's
scroll position. Table headings stay pinned after the page header scrolls away.

The original LibraryHeader supplies 56 layouts covering compact and ordinary
headers, narrow and wide containers, both responsive breakpoints, two root font
sizes and short/tall metadata. Native bounds match those layouts at 100% and
150% zoom on macOS and Linux, within physical-pixel rounding. Headers use the
actual content width, original artwork sizes, padding, title weight/size/wrapping,
metadata alignment, rounded artwork and image shadow. The type label navigates
to its library and supports keyboard activation. Playlist folder display names
and the stored collapse preference are retained.

Focused checks pass for header playback, scrolling to the current track,
stale-scroll pagination prevention and the original layouts. At the minimum
window size, playback checks scroll the controls into view before activation.
The album-selection fixture now reveals the first disc below the scrolling
header. Complete native reruns pass on macOS and Linux: 264 tests pass on each,
with the documented GPU smoke test excluded. Strict workspace Clippy and all
Windows Rust targets also pass.

Refreshed packages pass archive, executable, 55-license and installer-preservation
checks. The macOS bundle passes deep strict signature verification:

- macOS SHA-256: `cb0383ecf1053e0c74d4b355c256bebfd0b4d51a65f80262db85dc5ca9051d8d`.
- Linux SHA-256: `8ab3235098c48283bb135dd8636b5b739b4053af7f54c5f07642cabc9a92999c`.

These packages predate the playlist-description expansion/link work in progress.
Windows native C++ linking/runtime and interactive acceptance remain unverified.

### Playlist descriptions

Playlist headers now linkify the original description text with the shared
escaped-comment renderer. Long descriptions retain the original single-line
collapsed height, blank expansion-control spacing, 200 ms ease transition and
root-font/zoom behavior. Pointer activation and Enter/Space expand or collapse;
links retain their own keyboard/accessibility controls. Updated text starts
collapsed. The header playback fixture includes a multiline linked description
and still verifies duplicate entries, navigation preservation and all three
enqueue modes at the minimum window size.

All 16 focused playlist tests pass on macOS and Linux. Strict workspace Clippy
and Windows Rust target checks pass. The preceding complete suites covered
264 native tests on each platform; this change adds one description test.
Refreshed packages pass archive, executable and 55-license checks, checksum
rejection, replacement, cleanup and settings preservation. The macOS bundle also
passes deep strict signature verification. The playlist-description checkpoint
hashes are macOS `4943866b4224afe5911e75502cb263caf67d5625c0a09aa52086fbac2a3603f0`
and Linux `325046b515eba86feb373a41f4b3ebde8f943bfae5d3942cbbbcd362c1aef708`.
These packages predate the subsequent album/artist header-control changes.

### Album and artist header controls and metadata

Album headers now expose album radio, favorites and supported ratings. Artist
headers expose favorites and Navidrome ratings. Controls follow the item's
server, visibility preferences and outstanding mutations. Header ratings use
the original large size and horizontal padding. Pointer tests exercise actual
requests, disabled controls, server changes and queue/history preservation;
album radio starts its returned tracks independently of the general play-button
preference and retains the current detail route.

Album dates, track counts, duration, explicit status, size, play counts and
joined artist links now appear beside the artwork. Release type and version
appear above the title. Artist metadata retains optional counts and duration,
including explicit zero values. Jellyfin artist counts use `SongCount` rather
than `ChildCount`; Subsonic retains its original album-count default and omitted
song/duration fields. The original artist header supplies 195 localized cases,
verified by the pinned Electron runtime on macOS and Linux. Native rendered
metadata checks include English, French and Arabic.

Album comments now appear below the header with the original 75-pixel collapsed
limit, linked text, padding and arrow control. They share the playlist's 200 ms
transition, safe link renderer, keyboard activation and reset behavior. Headless
tests cover both description types, including expanded album height and keyboard
collapse. Full suites pass on macOS (252 core, 265 native, one audio helper) and
Linux (253 core, 265 native, one audio helper), with the documented TagLib fixture
and physical/GPU exclusions. Strict workspace Clippy passes on both platforms;
Windows Rust targets compile.

Packages pass archive, executable, 55-license and installer preservation checks;
the macOS bundle also passes deep strict signature verification. This checkpoint
predates the artwork-viewer closing fade:

- macOS SHA-256: `f6c21e5320324228db770b00e25aa6435a9f24027ad148720f4aedf549b81217`.
- Linux SHA-256: `79a6bb3c456fedb9b47da094cb52a8372578f9b903f1a9465c84f07bb4454dde`.

### Artwork-viewer closing fade

The locked original modal manager confirms a 300 ms CSS ease entrance and exit,
retained artwork during dismissal and restored focus. Fourteen Chromium opacity
frames reproduce identically on macOS and Linux. The windowless reference keeps
the hook's two animation-frame boundaries and waits for React's background removal
commit; it does not open a physical window.

Native fade-only dialogs now retain outgoing content until the exit completes.
They use the existing CSS timing implementation, block pointer events through
the closing view and remove only that view when its timer expires. Opening a
replacement viewer preserves its focus and lifetime. Existing dialogs retain
their previous transition configuration. The actual artwork viewer's headless
check compares painted opacity against all 14 frames, plus geometry, censoring,
dismissal, focus restoration and replacement. Complete native suites pass on macOS and Linux (265 passed, one documented GPU
exclusion on each). Strict workspace Clippy and Windows Rust target checks pass.
Both refreshed packages pass archive, executable, 55-license and installer
preservation checks; the macOS bundle passes deep strict signature verification.

- macOS SHA-256: `8a357e3d425e5883dced150e72811b0f06742a0e2f23565132d5f2cf5b7981af`.
- Linux SHA-256: `eef73d9886b38d065de32db1f0c3bd1f6b73a8c75ccf52ef561c25e5704f38e8`.

These packages predate the album tag and metadata-placement changes.

### Album content, tags and disc groups

Album metadata now appears above the tracks at smaller container widths and in a
300-pixel sticky column at the original large breakpoint. Text direction reverses
the columns, font size controls both breakpoints, and the metadata height retains
the original viewport limit. The column ends with its grid section and short
track lists reserve the metadata's height. The track list remains virtualized.
Resizing while the header is offscreen updates the layout from the page's measured
width. The original content CSS supplies 432 cases covering both text directions,
three metadata heights, two track heights, font sizes, breakpoints and scrolling;
all native cases pass at two zoom levels.

Release types, compilation, country/status, explicit status and mood now use the
original translations and grouping. Genres, tags, labels and grouping use separate
labeled pill groups. Supported label/grouping actions retain their backend filters;
unsupported actions render ordinary text pills. Album external links move into
metadata, and label copyright text appears below the track/metadata grid.
The original components supply 156 album tag/mood cases across 39 languages and
42 disc-grouping cases. Disc groups retain consecutive order and disappear for
single-disc, filtered and sorted views. Rendered metadata checks cover English,
French and Arabic. Both original references reproduce on macOS and Linux.

The actual page test covers resizing with the header offscreen, wheel scrolling
over metadata, disc selection, related-album paging, refreshed metadata and label
navigation while preserving the queue. Complete suites pass on macOS (252 core, 266 native and one audio helper) and
Linux (253 core, 266 native and one audio helper), with the documented fixture/GPU
exclusions. Strict Clippy passes on both platforms; Windows Rust targets compile.
The packaged checkpoint above predates these changes.

Sticky disc headings now retain the original row-size-dependent thresholds and
selection behavior. The original hook supplies 480 states, compared at two zoom
levels. Album search moves into the track controls; external providers use the
original embedded PNG/SVG logos, icon dimensions, button size and translated
labels. A native pointer fixture checks both discs at wide/narrow widths, search
placement, provider rendering/link activation and queue preservation. Complete
native suites pass on macOS and Linux (268 passed, one documented GPU exclusion
on each). Strict workspace Clippy passes on both platforms; Windows Rust targets
compile. Refreshed packages pass archive, executable, 55-license and installer
preservation checks. The macOS bundle passes deep strict signature verification.

- macOS SHA-256: `ea7441a6786184e10adfb8e0fa81c252ab89e80183502ee5e2ffa2486044a690`.
- Linux SHA-256: `ec24117959822b2463018f8ee0207b47b5c3708fc8c7eaf2218b1982c3815ae3`.

These packages predate the artist overview and biography changes in progress.
Interactive acceptance remains deferred as requested.

### Artist overview introduction and biography

Artist overviews now put discography, all-track and radio controls below the
scrolling header. Genre and external-link groups retain their original labels,
spacing, outlined genre buttons and provider logos. Metadata follows text
direction; biography width is capped at 1280 CSS pixels and its heading names
the artist with the original translation. Resolved empty biographies disappear.
The header no longer duplicates the overview's search, metadata or navigation.

Biography expansion reuses the album/playlist spoiler implementation, including
200 ms CSS ease timing, whole-content activation, keyboard controls and source
reset. The original shared Spoiler provides 56 transition frames plus short and
exact-limit cases; the original artist CSS and Mantine Grid supply 12 content
layouts. Native checks compare both at 100% and 150% zoom, including two root
font sizes and RTL. The references use standards-mode documents, as the original
application does. A separate actual-page mouse test checks translated headings,
empty biographies, provider painting and links, genre/discography/song navigation,
pending-radio disabling and queue preservation. Complete native suites pass on
macOS and Linux (271 passed and one documented GPU exclusion on each), as do
strict workspace Clippy and the Windows Rust compile check. Refreshed packages
pass archive, executable, 55-license and installer preservation checks; the macOS
bundle passes deep strict signature verification.

- macOS SHA-256: `88667994534e837def2fd7a1157eb7fc261ac35a17bab0354ef717bd52ba1da9`.
- Linux SHA-256: `d9fcdbdc3814fbc6dbd5f6b6623d8543d416f20108d1ea4101cf8c1060691e63`.

These packages predate the artist album-section work. Interactive acceptance
remains deferred as requested.

### Artist album sections

The overview now uses the original album sort choices, nullable numeric/date
ordering, stable ties and ID order. Discography loading, header playback and
complete-list playback share that client sorter. Search uses the existing Fuse
Bitap implementation with the original threshold, field weights and relevance
order; it searches title, release type, artists and genres after 300 ms. Random
ordering remains stable across redraws. Saved sort and grouping choices survive
switching between artist tabs.

Release groups have translated headings, counts, individual Now/Next/Last actions
with hold-to-shuffle, and responsive grids from two to eight columns. A group
initially displays 100 cards and View more reveals the rest. Group actions use
the complete filtered group, including other performers' tracks. Cards share
configured album metadata, selection, drag, context menus and expansion; action
selection stays scoped to the overview section.

Original references cover 26 sort orders across 72 albums, 69 fuzzy searches
across 98 albums, 18 grouping/preference configurations, 1,404 translated labels
and 56 grid layouts. Native geometry checks run at two zoom levels, including
RTL. The page fixture checks the 100-card boundary, View more, debounce timing,
sort direction, saved sort, range selection, dragging, expansion, all three queue
positions and preservation after failed fetching. Complete suites pass on macOS (255 core,
273 native) and Linux (256 core, 273 native), with the documented fixture/GPU
exclusions. Strict workspace Clippy passes on both platforms and Windows Rust
code compiles. Both refreshed packages pass archive, executable, 55-license and
installer preservation checks; the macOS bundle passes deep strict signature
verification.

- macOS SHA-256: `922e97a7862b7c9fe5f2ddc7a24e1165f9a92796e740b68ae4fbaeb0e121edd1`.
- Linux SHA-256: `2997b451e0a164670b1c5683c43d326a61553e328fe7f75f294754c3099be006`.

These packages predate the related-artist carousel changes. Interactive acceptance
remains deferred as requested.

The related-artist carousel and fuzzy searches on the remaining client-side
song views are the next confirmed parity gaps under review.

### Related-artist carousel and client-side fuzzy search

Related artists now use responsive pages, round artwork and the configured artist
card rows, current-artist exclusion, disabled page controls, Shift-wheel paging,
header swipes and sequential 300 ms exit/entry motion. The artist-information
request uses the original limit of ten. Original callbacks supply 70 navigation
scenarios and 65 motion frames, reproduced on macOS/Linux. Native pointer tests
check zoom, Arabic layout, navigation and preservation of the queue. Thirteen
artist tests and strict Clippy pass on both platforms for the carousel snapshot.

The shared Fuse-compatible matcher now covers the remaining original client-side
song views, playlists and radio. Search includes the original fields, typo
threshold and relevance order; explicit album/favorite sorts apply after search,
and playlist searches preserve relevance. Album tracks default to original order.
The sidebar queue uses the same filtered rows for display, selection, dragging
and current-track scrolling. Duplicate tracks retain separate queue/playlist
identities. Searches wait 200 ms in queues and 300 ms in artist/local list views.
Whole-list actions fetch and enqueue all filtered matches even from a partial
first page.

The original implementation supplies 560 additional field/relevance searches and
16 radio sort combinations. The native flow test verifies timing, sidebar row
positions, duplicate entries, filtered album/playlist paging and enqueue, and
radio matching against a local Subsonic fixture. Existing selection and toolbar
tests now use the original search fields and debounce timing. A complete suite
rerun, final static checks and refreshed packages are in progress. The archives
above still describe the earlier artist-album snapshot.

The next confirmed gap is the shared grid-carousel behavior in ordinary home
sections and album recommendations: those views still page fixed groups of
twenty and need responsive columns, gestures, transitions and original loading
behavior. Interactive acceptance remains deferred as requested.

The GPUI test refresh helper now flushes its redraw effect immediately. A focused
regression changes a view width without notification and verifies that the first
bounds read after refresh sees the new frame. The complete UI suites are being
rerun with this correction; older queued-refresh behavior could expose stale
geometry to a test.

The next carousel references reproduce 16 infinite-loading/resize scenarios and
56 CSS grid layouts on macOS/Linux. The original home playlist query also sorts
by most recently updated; the native home query currently sorts by name. These
references are prepared for the next implementation pass.


The completed search/carousel checkpoint passes 256 core tests on macOS and
257 on Linux, plus 278 native tests on each. Strict Clippy and all Windows Rust
target checks pass. The expanded original-song filter comparison adds 69 cases
and caught a nullable release-year mismatch: an explicitly unknown release year
now stays unknown instead of falling back to a legacy year. Year bounds exclude
unknown values, matching the original. The corrected refresh helper also passes
the full UI suites on both platforms.

The refreshed macOS archive passes package, updater and deep strict signature
checks, including all 55 license files:
`ef576fc1339d9e46a2d1fe9ce2dc2bbfe769286f135d96dafa649d6ffc82ea6d`.
The Linux archive passes the same package/license and updater checks:
`9cb018350cac1392d4e77fa008b6219f4234c2749d04fc064b8b1b46bad08e88`.
These builds predate the shared home/recommendation carousel changes below.

### Shared infinite grid carousels

Home albums, songs and playlists, album recommendations and related artists now
use one responsive carousel implementation. Native geometry matches 56 original
CSS grid layouts at both zoom levels and in both text directions. State checks
match 16 original loading, resize, refresh and duplicate scenarios. Shared
navigation retains the verified wheel/swipe cooldowns and sequential exit/entry
motion. Home playlists request UpdatedAt descending, as in the original.

Ordinary carousels retain duplicate results, keep their page during a refresh,
refetch previously loaded server pages, prefetch partial pages and show missing
card placeholders during next-page fetches. Home playlist cards use their own
metadata rows and disable drag and grid selection. Artist album selection is audited separately below. Home, artist and album pointer flows pass on
macOS; delayed-response/refresh integration and full regression checks are in
progress. Interactive checks remain deferred.


The delayed-response home fixture passes on macOS. It verifies partial-page
prefetch, visible placeholders, navigation during a pending fetch, atomic refresh
of all loaded pages, page retention, duplicate results, playlist ordering and
non-draggable playlist cards. It also confirms that song artwork is inert and a
carousel Play button explicitly replaces the queue even if the default library
play behavior is Add Last. The complete suites are running for this snapshot.

A further source comparison found card interactions to correct next: library
song card controls should use the list playback context, and the artist overview's
standalone album cards do not participate in library grid selection. The next
reference executes 24 original poster-card scenarios, including 48 click paths
and 144 play dispatches. The native changes now pass the original click/playback comparison;
artist-page regression and static checks are running.


The shared-carousel checkpoint passes the complete macOS suite (256 core, 281
native) and Linux suite (257 core, 281 native), with the documented exclusions.
Strict workspace Clippy passes on both platforms. All Windows Rust targets
compile; Windows native linking/runtime and interactive acceptance remain
unverified. The macOS package passes the archive, 55-license, updater and deep
strict signature checks:
`25bf1958e8a7de8c0442910af30f52093a996e309c6cbcbef6a800c08888b279`.
The Linux archive passes the same archive, license and updater checks:
`72c04f09c69095c2b073e5312ae2f9a901e0931e7c5f9fe79e9282a773ad3a05`.
Both archives predate the card-interaction changes below.

### Shared card interactions

Library song cards now route explicit Play, Next, Last and held shuffle actions
through the list's playback context. Standalone carousel and artist-album cards
play only their item, ignore library selection and preserve explicit queue
positions regardless of the global default play-button behavior. Library card
drags update selection when appropriate; standalone drags contain only their own
item. Artist album artwork navigates even when grid multiselect is enabled.

The reference executes 24 production card configurations, 48 click paths and
144 play dispatches. Native pointer checks cover navigation, selection, drag
selection, the real Play hit target and the 101-track surrounding list context.
Full regression checks for this checkpoint are pending. Card hover controls and
presentation remain under comparison with the original.


Card-interaction full runs reached 281 passing native tests on both platforms,
with one old playlist-card assertion requiring correction: the original card's
Play control explicitly uses Now, regardless of the default preference. The
updated test checks duplicate order and replacement. The first Linux core run
had a playback-event timeout; its isolated rerun and complete core rerun passed
(257 tests). Full native verification is pending the presentation work below.

### Card hover controls and compact metadata

The current implementation moves card controls onto the artwork, adds compact
metadata overlays, and uses the original circular Play/Next/Last, favorite,
rating, More and album-expansion controls. Pending favorite/rating requests
disable those controls. Metadata uses the original font sizes, spacing, first-row
weight and title links. Card selection no longer changes the content dimensions.

The production CSS/component references reproduce 48 layouts and 124 control
motion frames on macOS/Linux. The native motion regression passes all frames,
including interrupted poster movement and compact scaling. Native geometry and
pointer regressions are in progress. These changes are not yet packaged. The
library grid's overall responsive sizing, selected-outline offset, card badge
shadows and remaining home genre presentation still require comparison.

The card geometry regression now passes all 48 reference layouts. Pointer checks
cover pending mutations, More menus, title navigation, compact/poster switches,
playlist album playback, artist albums, home carousels and selection without
content resizing. Matching hover state to the card lifetime fixes disappearing
controls when changing size. Detail rows now fill their viewport, preventing
undersized artwork and overlapping controls. Full macOS/Linux runs each reached
283 passing native tests, with one preview test still assuming always-visible
expansion controls. That test now hovers before opening and passes on macOS;
the combined grid regression will rerun it on both platforms. Strict macOS
workspace Clippy passed at the card checkpoint.

### Library grid sizing and keyboard navigation

The responsive grid now uses measured content width, original breakpoints,
fixed-column preferences, compact/poster row heights and per-cell spacing.
Poster content keeps its original overflow and flex behavior with larger fonts.
The selected outline sits outside the card without changing its dimensions.
Arrow keys move by row or column, retain selection at boundaries, extend ordered
Shift ranges and reveal the selected row. These controls remain available when
mouse multiselect is disabled, matching the original.

The original callback supplies 405 sizing cases and 960 keyboard cases. Production
grid/card CSS supplies 242 rendered layouts covering both text directions, two
font sizes, all five gaps, compact/default/large cards and controls in constrained
artwork. All 242 layouts reproduce on macOS/Linux. Native sizing,
keyboard and all geometry checks pass on macOS. The full macOS run passes
256 core and 287 native tests with the documented exclusion. The Linux rerun
is in progress after a silent MPV timeout; the isolated retry passed. These changes are not
packaged. Card badge shadows, custom CSS aliases and remaining home presentation
still require comparison.

### Featured home genres

The home genre section now uses the original responsive tile counts, colors,
spacing, typography, hover Play controls and View more navigation. Its 59
production-component references reproduce on macOS/Linux and match native
geometry on macOS. Pointer/protocol checks verify fixed 100-song genre requests,
explicit queue replacement, preserved saved random-play options, held-click
suppression, disabled pending controls, cancellation and navigation. The shared
random form regression also passes after reusing its cancellable request path.
Full cross-platform native checks and packaging follow the remaining home layout
work. Interactive acceptance remains deferred.

The complete library-grid checkpoint now passes Linux too: 257 core and 287
native tests, with the documented exclusion, plus strict workspace Clippy.
The featured-genre checkpoint passes strict macOS Clippy.

### Home page spacing and section order

The home scroll content uses the original horizontal padding, desktop/web top
spacing, section gap and bottom space. The featured carousel has no extra
heading or Refresh button, and home customization remains in settings. Native
layout checks pass all 48 production-route references, covering font sizes,
window styles, single/multiple/disabled features and reordered sections with
disabled entries. The combined home checkpoint is undergoing complete macOS
and Linux regression runs; its archives are not yet refreshed.

The combined home runs passed 287 native tests on each platform, with one
featured-carousel layout regression caused by an extra wrapper. Removing that
wrapper preserves definite percentage widths inside narrow feature cards. The
focused full carousel comparison now passes on macOS, as do the 107 genre/home
layout cases. Complete reruns include the card styling changes below.

### Card badges, artwork shape and CSS selectors

Favorite badges retain the rotated-square geometry and blurred spread shadow;
rating badges retain their box and text shadows. Library artist cards use square
artwork while the related-artist carousel retains circular artwork. Original
card selectors now reach containers, images, metadata rows and rating badges,
including ancestor, selected, compact/poster and hover matching.

All 48 production card references, now including shadow styles, reproduce on
macOS/Linux. Native checks verify rendered badge paths, shadow layers, artwork
radii and live CSS sizing/typography, alongside the existing hover and pointer
regressions. Strict macOS Clippy passes. Complete suites, Windows Rust checks
and archive refresh are running. Control-specific and favorite-badge CSS aliases
still need comparison; grid smart scrolling and detail-grid edge spacing also
remain under audit.

The complete card/home checkpoint passes Linux with 257 core and 288 native
tests plus strict Clippy. The macOS full run passed 287 native tests and exposed
a fixture timing race: its second home navigation could click retained debug
bounds before the genre response completed. The fixture now waits for the
completed response and its focused regression passes. All Windows Rust targets
compile for this checkpoint. The refreshed macOS archive passes archive,
55-license, updater and strict signature checks:
`33a9dddd8ca81847c6144b8f69a164255a21aa2e9eb0c1d309dd2b4ec6827dd1`.
It predates the following grid scroll refinements.

### Grid smart scrolling and detail edges

The keyboard grid path now matches react-window 1.8.11 smart alignment: nearby
offscreen rows move only far enough to become visible, and distant rows center
with the original edge rules. All 440 original offset cases reproduce on
macOS/Linux. Native callback and keyboard regressions pass on macOS. Detail-grid
rows also include the original top/bottom section padding; the shared geometry
fixture is checking both ordinary and detail placement against all 242 original
layouts. Cross-platform suites and packaging for these refinements follow.

Both native placements now pass all 242 original grid layouts on macOS, and
the keyboard scroll regression passes. The complete suites are running for
this snapshot. The preceding Linux card/home archive passes archive,
55-license and updater checks:
`a739d067a176af67cfa7811ea3e17ba7a8ae8b8a7dd828a62943b7e13fd36975`.

The complete smart-scroll snapshot passes on both platforms: 256 core tests on
macOS, 257 on Linux, and 288 native tests per platform with the documented
exclusion. Strict workspace Clippy passes on both.

### Empty album recommendation sections

Recommendation rows now omit their spacing when the original carousel returns
no content. The three-rem stack margin remains even when every section is empty
or the album has no artist/genre sections. All 160 production Stack references
reproduce on macOS/Linux, covering both font sizes, two widths and combinations
of absent, empty, loading and populated sections. The native geometry regression
passes all cases on macOS. Complete reruns include the card selector work next.

### Card control and favorite-badge custom styles

Original selectors now reach Play/Next/Last, Favorite, Rating, More and Expand
controls, including their card/artwork ancestors. Custom control dimensions keep
the original centered anchors, plain-button content dimensions and the
positioning specificity of More/Expand. Favorite-badge geometry and shadows
follow custom size, position, color, opacity and box-shadow declarations.
Shared buttons apply custom normal/disabled paint after their default animated
opacity and disabled colors, so those defaults no longer replace CSS overrides.

All 48 custom-style references reproduce on macOS/Linux. Together with the 48
ordinary card references, native checks pass all 96 layouts on macOS and verify
actual painted colors/opacity, badge polygons/shadows, pending writes, menus,
selection, metadata navigation and expansion. Complete workspace runs, strict
Clippy and Windows Rust checks are running for this snapshot. Archives still
contain the preceding card/home checkpoint.

The complete control-selector snapshot passes 256 core and 289 native tests on
macOS, and 257 core and 289 native tests on Linux, with the documented exclusions.
Strict workspace Clippy passes on both. All Windows Rust targets also compile.

The next refinement resolves badge opacity through the original normal/hover
selector cascade before applying the 200 ms transition. A custom normal opacity
therefore retains the original fade, while a more-specific custom hover opacity
can choose its final value. The windowless reference now forces the real CSS
hover state and records intermediate opacity frames for both favorite and rating
badges. Native checks compare actual painted alpha through nested opacity groups.
The focused lower-specificity transition regression passes; the explicit hover
override cases and combined regressions are being checked before packaging.

Both normal and explicit hover-opacity cases now pass all 160 paired frames in
the 48 custom card layouts. The complete 96-layout card/pointer regression passes
on macOS. Updated references reproduce on Linux and the updated Windows Rust
targets compile. The final native suites and strict Clippy are running before
refreshing the macOS/Linux archives. The unchanged core retains the passing
256-test macOS and 257-test Linux checkpoint above.

The badge-transition checkpoint passes all 289 native tests on macOS/Linux,
with the documented offscreen projectM exclusion. Strict workspace Clippy passes
on both after refreshing Linux's cached GPUI test interface. Both archives pass
executable, 55-license and isolated updater checks; the macOS bundle also passes
strict ad-hoc signature verification.

- macOS SHA-256: `ad94ffc1f781abf03aef7506606ee60111b682656544247954942266154888dd`
- Linux SHA-256: `c11f6987a8642c9a0e8ddb8d77cb0ae87d112b1d72747221c5703904ef8db996`

### Jellyfin tracks without an album

The route audit found that Jellyfin's synthetic `dummy/<song-id>` album link was
missing from native normalization and navigation. The native fallback now loads
the song, retains its artwork/metadata and offers playback, favorites, genre and
artist navigation with the original missing-album notice. Synthetic album
playback resolves to that song without requesting nonexistent album children.
All 33 original song normalizer cases pass. Native pointer, request, retry and
navigation checks are in progress; the archives above predate this change.

The missing-album checkpoint passes 256 core tests on macOS, 257 on Linux and
290 native tests per platform, with the documented exclusions. Strict workspace
Clippy passes on both, and all Windows Rust targets compile. All 32 original
fallback-page query/action cases reproduce on macOS/Linux. Native clicks verify
opening the album link, appending the song, confirmed favorite propagation to
duplicate queue entries, genre navigation, Back/Forward and failed-load retry.

### Percentage letter spacing

The relative-font fixture now includes 22 percentage-spacing cases among its
82 original cascades per platform. Percentages remain relative to each
descendant's own font size, while inherited pixel/em tracking retains its computed
length. The native parser preserves that distinction and carries percentages
through text runs so widget font changes and inline font-size overrides can resolve
them at shaping time. Cascades, invalid variables, resets, zoom and actual input
reflow are being verified before the next full runs and archive refresh.

All 82 cascades reproduce on macOS/Linux. The macOS cascade comparison passes
at both zoom levels, and rendered checks preserve percentage tracking when child
widgets change font size. Multiline inputs match equivalent absolute tracking
after live size/percentage changes, including cursor placement and wrapping.
Tracked truncation also recognizes percentage values. Full native suites and
Windows Rust checks are running for the combined snapshot.


The combined missing-album and percentage-tracking checkpoint passes 256 core
tests on macOS, 257 on Linux and 291 native tests on each, with the documented
exclusions. Strict workspace Clippy passes on both; all Windows Rust targets
compile. Seven remote browser logic tests pass. The rebuilt macOS archive passes
executable, 55-license, isolated updater and strict ad-hoc signature checks:
`dd3fce3660a10ac63f574b1cbda0b4f02ebed11c36daf71985b84d0081d5f5f8`.
The matching Linux archive passes executable, 55-license and isolated updater
checks: `a83dad1f8b7d56217c93bc3fe06ec6013e5170364ad50e633c7c4fb3a79faa9a`.

### Restored-account validation and connection recovery

The startup audit found that saved sessions bypassed the original account check.
The native path now queries the backend user endpoint on restoration and account
selection, refreshes administrator permissions and retries transport failures
once after 500 ms. A failed connection displays the original localized notice
and Retry action, retaining credentials, saved accounts and duplicate queue
identities. A request generation prevents an old account check from replacing
the newly selected account's page. Twelve original hook cases and the original
connection-page action supply independent references. Focused and combined
regressions are in progress; the archives above predate this change.


The first combined account-check snapshot passes 257 core and 292 native tests
on macOS, with the documented exclusions, plus strict workspace Clippy. Windows
Rust targets compile. The follow-up preserves the original Retry/Back/Forward
history, keeps restored expanded-player/visualizer overlays from covering account
checks, and cancels old checks when importing settings. Focused pointer/history
checks pass; the final combined native rerun and package refresh follow.


The first combined Linux snapshot also passes all 258 core and 292 native tests
with the documented exclusions, and strict workspace Clippy. The final account
regression now also exercises an actual pending request during settings import,
account removal with a pending request, and the covered Retry target when an
expanded player was restored. The focused macOS test passes all of these cases.


The final account-check UI snapshot passes all 292 native tests on macOS and
Linux with the documented projectM exclusion. Strict workspace Clippy and
formatting pass; the updated Windows Rust targets compile. Core counts remain
257 on macOS and 258 on Linux from the complete run above. Packaging will include
the saved-password renewal refinement below.

### Jellyfin saved-password renewal

The original authentication reference now includes eight saved-password cases,
for 20 cases total. The native shared renewal path now handles Jellyfin expired
tokens, administrator-check authentication failures and artwork uploads. Password
authentication omits the expired token; retries use the renewed token and user
identity. Ordinary permission failures do not retry authentication. Temporary
renewal errors retain credentials, while rejected passwords and explicit logout
revoke the session. Focused protocol checks pass; the native flow, combined suites
and package refresh are in progress.


The combined renewal snapshot passes 258 core tests on macOS, 259 on Linux and
292 native tests on each, with the documented exclusions. Strict workspace
Clippy passes on both and all Windows Rust targets compile. All 20 original
hook cases reproduce on both platforms. Native account checks verify the
expired-token/password/new-token sequence without losing queue identities.

### Delayed credential restoration and imported accounts

Credential restoration now rechecks the current account identity before accepting
a delayed password-store result. Removed, replaced, signed-out and already active
sessions are not overwritten. Accepted sessions use current library/TLS options.
Settings imports apply the same normalized account binding and revoke discarded
clients, including imported signed-out accounts. The focused credential-binding
and native account-flow regressions pass, including late completion after account
removal and import. Combined checks and refreshed archives are in progress.


The delayed-restoration snapshot passes 258 core and 292 native tests on macOS,
259 core and 292 native tests on Linux, with the documented exclusions. macOS
strict Clippy and formatting pass, and Windows Rust targets compile. Linux Clippy
exposed stale dependency metadata after copying files with older timestamps;
the next snapshot refreshes copied source timestamps before rebuilding checks.

### Server form labels and checkbox controls

Eight original form combinations now supply label, description and visibility
references across all three backend types, public URL presence and Jellyfin Quick
Connect. Native input labels remain visible after typing, accessible input names
use the original translation keys, and original options expose checkbox state.
The public URL preference appears when an address is present. Descriptions and
right-to-left checkbox placement follow the selected language. The first focused
pointer/translation pass succeeds across English, French and Arabic; keyboard,
cancellation and combined platform checks are in progress. The archives above
still predate the restored-account and form changes.


The form/account checkpoint passes 258 core tests on macOS, 259 on Linux and
293 native tests on each, with the documented exclusions. All 20 original
account cases and eight form combinations reproduce. Native mouse/keyboard
checks cover English, French and Arabic, checked-state accessibility, preserved
credentials and cancellation of obsolete login attempts. Strict workspace Clippy
passes on both after refreshing Linux dependency metadata; formatting and all
Windows Rust checks pass. The seven remote browser tests pass.

A further import regression verifies that merging a changed or signed-out playing
account stops its old transport and preserves duplicate queue entries. Imports
that only rename an account or change library options retain playback. The focused
account and original native-import tests pass on both platforms; the final strict
checks and Windows Rust compilation include this refinement.

Both rebuilt archives pass executable, 55-license and isolated updater checks;
the macOS bundle also passes strict ad-hoc signature verification.

- macOS SHA-256: `d5489315dbe705fb9a7302da42f0e48adc6842b4173cb2f108bd41eaca14398b`
- Linux SHA-256: `4cf127b9d727091095f5fe0eff309b8b695f35401c3dbd03dd8b5ac2ef403318`

### Variable-font overrides

Explicit `font-variation-settings` now reach inherited text, line wrapping and
glyph rendering. The parser preserves case-sensitive four-byte tags, finite
fractional coordinates, duplicate last-value wins, comments/escapes, global
resets and invalid-variable inheritance. Canonical coordinates participate in
font, shaping and raster-cache identities. Unsupported axes leave static faces
unchanged; variable coordinates clamp to the font's range after face selection.

The pinned windowless Electron reference covers 28 declarations, 12 cascades,
528 Inter/Poppins measurements and 96 Greek/Cyrillic/Japanese/Hebrew fallback
measurements on both macOS and Linux. Native checks compare those widths at
ordinary and doubled zoom, verify changed glyph pixels and preserve input
wrapping/carets when settings change without editing the text.

Two private, subsetted variable companions preserve the original Japanese and
Hebrew fallback overrides. Ordinary text retains the existing five static CSS
faces and their mapped default weights. The companions remain outside the font
picker and share the original font licenses. The asset generator includes both
forms in its reproducibility record. CoreText instance names retain access to
their original shaping data; optical-size cache identity also survives a static
primary font with a variable fallback. DirectWrite resolves explicit fallback
instances before layout and applies axis overrides after weight/style selection.

Focused macOS/Linux comparisons pass, including fallback glyph pixels and width
clamping. Complete suites pass 258 core tests on macOS, 259 on Linux and 296 native
tests on each (one native projectM test remains in its dedicated runner). Strict
workspace Clippy passes on both; the final Windows Rust Clippy check also passes.
The existing 816 fallback, 126 italic, 2,080 feature and 360 synthesis comparisons
pass on both platforms. All twelve derived fallback assets reproduce byte for
byte. Archive refresh is in progress; the archives above predate this work.
Windows rendering and physical interaction remain unverified.

The rebuilt macOS archive passes executable, 55-license, isolated updater and
strict ad-hoc signature checks:
`64298b764eaa584210e9c7cf8272ff1b72cf97b8adbe2c1aabadf85d51018309`.
The matching Linux archive passes executable, 55-license and isolated updater
checks: `5bd62024c856927b791fdab38f4d33a697d99c7da336ce8e01329530225ecab0`.

### Discord settings labels and selections

The original settings component supplies four desktop/browser and enabled/disabled
configurations, including translated titles/descriptions, selection labels and
control updates. Native Discord controls now use those catalog keys, preserve
interpolated service names, expose checked state, and use the original readable
display/link choices. The selected option receives a checkmark. Checkboxes reverse
their layout for right-to-left languages. The application-ID and Last.fm fields
have explicit translated accessible names. English/French/Arabic pointer,
keyboard and persistence checks are in progress; the archives above predate this
settings change.

English/French/Arabic pointer, keyboard, accessible-name and persistence checks
pass. Complete native suites pass all 296 tests on each platform, with the
documented projectM exclusion. Strict workspace Clippy passes on macOS/Linux,
and strict Windows Rust Clippy passes. Archive refresh will include the remote
configuration changes below.

### Remote configuration and authentication

Port edits now retain the running listener and display the original restart
notice. Credential edits update the current listener and close existing,
authenticating and reply-waiting clients with the original 4002 code. The original
optional-authentication behavior is supported: either nonempty credential enables
authentication, and explicitly clearing both fields disables it. Missing keychain
credentials still fail restoration. Invalid authentication cannot receive the
playback snapshot. Field labels and descriptions use the original catalog keys;
the enabled state uses an accessible checkbox and port controls have bounded
numeric steps. Sixteen original authorization cases and sixteen handler changes
provide independent references. Live WebSocket, native form and migration checks
are in progress; the existing archives predate this change.

Transient remote disconnections now retry after the original two-second delay.
The original eighteen close-policy cases verify which failures retry; manual
connections supersede pending retries and obsolete sockets cannot start new
attempts. Credentials remain in memory. Eight browser logic tests pass. The real
macOS browser suite also passes credential rotation, optional authentication,
automatic reconnection, offline/restart behavior, 38,000-entry queue operations
and all three viewport layouts. The complete macOS suite passes 259 core and
297 native tests with the documented exclusions; strict Clippy passes. Linux
regressions and the package refresh are in progress. Windows Rust Clippy passes;
native Windows linking/runtime and physical interaction remain unverified.


The complete Linux suite also passes 260 core and 297 native tests, with the
same exclusions. Strict workspace Clippy and the real Chromium remote suite
pass. Both refreshed archives pass executable, 55-license and isolated updater
checks; the macOS bundle passes strict ad-hoc signature verification.

- macOS SHA-256: `567925150add8ebbacd336fed358a09b060170af8d9dfd9f8332cea4cedb19df`
- Linux SHA-256: `14a6e7bdcdf96de611ecca99aef6ce7a0904d3dd05d1dcaccfc5b37f6b03c094`

### General settings controls

Nineteen general, library and external-link switches now display original
translated titles/descriptions and accessible checked states. Their layout
reverses for right-to-left locales. Dependent external-link controls follow the
original visibility rules. Private mode uses the original translated actions.
Folder views have named menu choices; tree indentation accepts every integer
from zero to 64, with the original rounding and clamping. Folder inputs have
translated visible and accessible names. Changing folder view or separator
clears both owned and shared folder navigation. Forty-two original React
control cases supply callback, numeric-bound and visibility references.
Pointer, keyboard, persistence and English/French/Arabic checks are in progress;
the archives above predate this change.


The complete macOS suite passes 259 core and 299 native tests with the documented
exclusions. Strict workspace Clippy and formatting pass. The focused folder
and general-switch tests also pass; queue restoration off intentionally omits
the queue from disk while preserving the live queue.

Settings now opens the existing player configuration panel for transition and
crossfade controls. This removes duplicate cycling controls that bypassed the
original playback/engine restrictions. The panel retains separate focus and
popover state for the settings and playerbar entry points, with shared saved
values. English/French/Arabic checks cover both entry points and reject transition
or duration changes during playback, MPV output and Jukebox output. Current
scrobble-mode and play-button choices use original translated option labels;
the selected play-button menu choice has a checkmark. The expanded player
configuration regression passes on macOS; final cross-platform checks and
archive refresh are in progress.


The final shared-player/settings checkpoint passes 259 macOS and 260 Linux core
tests, plus 299 native tests on each platform and the documented exclusions.
Strict workspace Clippy and formatting pass; strict Windows Rust Clippy passes.
A further folder regression on macOS verifies that invalid tree-only drafts
cannot block saving the visible separator in navigation mode; the final Linux
suite includes that case. The refreshed macOS archive passes executable,
55-license, updater and strict ad-hoc signature checks:
`dc7cb9e4a8cb3fa080d5132fd85ece60919b09efb9c276cf416194caaa3e9eaa`.
The matching Linux archive passes executable, 55-license and updater checks:
`410b304d8ca0d51fc82c105b3424051e8726ca6fac30507cefd29021f59a9960`.

### List layout controls, search and reordering

The list-layout editor now uses original display, pagination, size, alignment,
pinning and column labels. General table switches, enabled columns/card rows,
automatic widths and fixed column counts expose checked states. Column widths
have translated accessible names that follow locale changes; row-order arrows
retain their glyphs and have translated action names. Left/right pin buttons
toggle independently, including clicking the active side to unpin. Drafts
still apply only when saved. English/French/Arabic pointer, keyboard and
persistence checks pass on macOS and Linux at the initial controls checkpoint.

Separate column and card-row searches use the original 300 ms debounce and
localized fuzzy matching. Matches receive an outline without hiding or
reordering rows. The original callbacks and pinned Fuse runtime supply 360
reference searches across English/French/Arabic, including whitespace, typos,
location-sensitive matches, Unicode and long labels. Native headless checks
verify painted outlines, unchanged row geometry, independent/cancelled timers,
locale changes, reordering during search and editor replacement.

The editor also has drag handles with top/bottom insertion markers, separate
column/card-row targets and protection against drops from replaced editors.
Reordering preserves each column's draft input and options. Nine original width
normalization cases and 32 original drag reorders provide independent expected
results. Width inputs have ten-pixel step controls and accept the original
zero-to-2000 range; invalid non-finite drafts still fail validation. The shared
data validator retains previously accepted widths up to 2400 for compatibility.
The complete macOS workspace passes 260 core and 301 native tests, with the
opt-in offscreen projectM test ignored. Linux passes 261 core and 301
native tests, with the same projectM test ignored. Strict Clippy passes on
both platforms, and the Windows Rust cross-check passes. Windows native C++
linking/runtime and physical checks remain unverified. The packages above
predate this section.

### Scrobbling settings controls

The native controls now retain the original mode dropdown, percentage slider,
mode-dependent threshold visibility, translated descriptions and checked
scrobbling/notification controls. The original component supplies six complete
configurations and their callback results. English/French/Arabic checks verify
visibility, checked states, slider bounds, mode choices and retained thresholds.
Duration changes save while editing; clearing the field retains its saved value.
Invalid/non-finite drafts and boundary steps preserve valid preferences.
Restoring settings also refreshes the percentage slider. Queue identities and
playback remain intact. The general toggle check covers all bundled languages.

The final workspace passes 260 core and 302 native tests on macOS, and 261 core
and 302 native tests on Linux. The opt-in projectM test remains ignored in these
runs. Strict Clippy passes on both platforms, and all Windows Rust targets pass
their strict cross-check. Windows native C++ linking/runtime remains unverified.
Updated packages pass archive, executable, all 55 license and updater checks;
the macOS bundle also passes strict ad-hoc signature verification. These packages
include the list-editor and scrobbling changes, before the window-settings audit:

- macOS SHA-256:
  `0911dc232f2eec49241f96ba92d2ab56a0f184f32754cafa783b5f12170dbe87`.
- Linux SHA-256:
  `5fd88615c838df3cbeedd71d0091d28db3a1c289e4b1eef2e41538fd9b1f9084`.

### Window settings dependencies

The original window component supplies 32 configurations covering tray-dependent
preferences, mutually exclusive sleep controls, title-bar choices and restart
boundaries. Disabling the tray clears and hides its dependent preferences.
Sleep controls retain their original mutual disabling. Custom title bars change
immediately; transitions to or from the native frame require a restart. Original
labels/descriptions and translated accessible window-button names are retained.
Headless checks pass for all combinations in English, and every tray/sleep
combination in French/Arabic. Real menu clicks, persisted values, duplicate queue
preservation and save-failure reporting pass on both macOS and Linux. The combined
checkpoint below includes these changes.

### Update preferences

The original update component supplies 16 configurations covering saved/default
channels, automatic updates and the environment override. The native page now
uses the original translated channel dropdown and checked automatic-update
control, with their descriptions. Nonempty `DISABLE_AUTO_UPDATES` hides these
preferences and preserves manual checks. Channel changes clear previous results;
an already-open menu cannot switch channels after a check or download starts.
Headless pointer and persistence checks pass in English/French/Arabic on macOS
and Linux, both normally and in a separate process with the override enabled.
Queue identities and playback remain unchanged. The combined checkpoint below
includes these changes.

### Dedicated global media keys

The native global-media setting previously enabled media-session integration
without registering the dedicated keys. It now uses the existing global shortcut
manager, independently of media-session integration. Explicit user shortcuts
take precedence; media registrations do not suppress ordinary local shortcuts
or their fallback after a failed global registration. macOS has no system-defined
Stop key; Linux maps the combined key to X11's AudioPlay. Preference changes
refresh registrations immediately and retain mutual exclusion, translated
labels and RTL layout. Linux's existing MPRIS integration remains available.
Twelve original configurations supply command and registration references.
Native mapping, custom-binding precedence and local-fallback checks pass on
macOS and Linux. Headless checkbox clicks in English/French/Arabic verify mutual exclusion,
saved preferences, media-session policy and retained Space shortcuts without
registering OS keys. Physical media-key and permission behavior remains deferred.

The combined window/update/media-key checkpoint passes 260 core and 304 native
tests on macOS, and 261 core and 304 native tests on Linux. The opt-in projectM
test remains ignored in these workspace runs. A Linux MPV fixture timed out once;
it passed individually, in a repeated core suite and in the final full run.
Playback waits now retain recent events and the failing caller for diagnosis.
Strict Clippy passes on both platforms after moving a test module below production
code. Formatting and the strict Windows Rust cross-check pass. Windows native
C++ linking/runtime remains unverified.

Updated packages pass archive, executable, 55-license and updater checks. The
macOS bundle also passes strict ad-hoc signature verification. These packages
include the window, update and dedicated-media-key changes, before the subsequent
playback-control audit:

- macOS SHA-256:
  `4334eabe07dde549967b086c1c7be66a568f0dba37e5ac6c5a8e0f65fad2376f`.
- Linux SHA-256:
  `6e668d53f69098e30fd8446610c58c05a7a13489d32cf59f35e6b9499f7420c0`.

### Playback-control preferences

Four original configurations supply control choices, numeric callbacks, limits
and switches. Control sizes now save independently on blur/Enter; the wheel step
uses the original 1-20 slider and saves on release. Blank width retains the
original zero value; blank radio/button fields retain their saved values. The
radio callback retains its original 100-track clamp despite the editor's 200
upper bound. English/French/Arabic callback, pointer, actual focus-loss,
persistence and queue-preservation checks pass on macOS and Linux. Numeric-step
and migration checks and strict Clippy also pass on both platforms.

The waveform audit restores the original width/gap/radius sliders, labels,
descriptions, alignment choices, conditional visibility and blank loading/skip
values. The drawing implementation retains separate stereo channels, continuous
paths for zero width/gap at center alignment, original gap defaults, normalization,
device-pixel rounding, clipped progress colors and cursor dimensions. A pinned
wavesurfer.js 7.12.8 reference supplies 252 geometry cases and loading-delay values.
The original reference reproduces on macOS and Linux. Native control checks
pass in English/French/Arabic. Drag checks verify that previews leave playback
unchanged and commit on release inside or outside the control using the decoded duration.
Continuous rendering also retains seeking at 150% zoom. Switching tracks cancels
and clears old data during the replacement's loading delay. Old merged-channel
caches use a different key.

The complete checkpoint passes 261 core and 306 native tests on macOS, and 262
core and 306 native tests on Linux. The opt-in projectM test remains ignored in
these runs. Strict Clippy passes on both platforms and the Windows Rust cross-check
passes; Windows native C++ linking/runtime remains unverified. The macOS package
passes archive, executable, all 56 license, updater and strict ad-hoc signature
checks, before the subsequent shared-slider audit. Its SHA-256 is
`850f5bc1047ea6c60117c98e780e56b1481569cab52a9eca0c5bade1f1f64f9b`.
The matching Linux package passes archive, executable, 56-license and updater
checks. Its SHA-256 is
`cb6735a1a0a717161f4ad10fd69d527f56388dd41a1b011c0064fcdb93f639b9`.
Both archives are available under `target/`.

### Shared slider keyboard and RTL behavior

The pinned original Mantine slider and drag hook supply 560 keyboard cases and
320 pointer cases. The native slider lacked keyboard focus/arrow/Home/End controls
and RTL pointer/layout behavior. Its drag-and-drop threshold also dropped small
pointer movements. All callback cases pass on macOS and Linux, along with focused
waveform/control integration tests and strict Clippy. Initial painting and resize
also exposed stale pixel positions; fill and thumbs now use relative layout.
Initial paint/resize comparisons pass on both platforms. Full runs exposed an
old test that assumed the physical right edge was the maximum in Arabic; the
fixture now targets the correct RTL edge and passes on both platforms. Strict
Clippy and the Windows Rust cross-check pass. Full workspace validation is being
repeated with the player-bar changes below. The verified packages above predate
these changes.

### Player-bar seeking, time display and scrobble status

The regular seek bar now previews without seeking while held, commits on release,
uses a seconds-based domain/step and preserves its preview during engine polls.
Release resolves when playback comes within half a second or a later poll arrives
more than one second after release. Track replacement cancels pending pointer
edits. The formatted thumb value appears during hover/drag. Clicking duration
switches total/remaining time and persists the choice; native exports and original
app-store imports retain it.

Four original hook scenarios and 40 original duration/toggle cases pass native
comparisons on macOS. A headless native test exercises actual LTR/RTL pointer and
keyboard events, engine-position messages, release, duplicate queue-entry
replacement, persistence and player-drawer click isolation.

The elapsed-time hover panel opens after 500 ms and displays the original
threshold conditions, capped progress, submitted state and translated reset/force
controls. Disabled/private states hide controls; missing songs and already
submitted tracks disable the relevant actions. Forty-five original panel cases
pass native headless checks in English, French and Arabic. Four original hook
timelines verify continuous listening, five-second sample limits, paused samples,
seek rebaselining, restart resets and manual state changes.

Eighteen original Jellyfin controller cases verify stopped submissions, pause and
resume flags, and bounded shuffled queue reporting. Successful submission updates
same-server duplicate queue entries and refreshes affected library views. A
follow-up unpause is conditional on the same queue entry still playing. Delayed
HTTP fixtures verify that counts change only after success and that paused or
replaced entries cannot resume the dashboard. Playback reports run outside the
UI executor and preserve request order.

The first complete macOS run passed 263 core and 314 native tests; Linux passed
264 core and 314 native tests, with the opt-in projectM test ignored on both.
Strict Clippy and Windows Rust cross-checks also passed. Those runs include the
shared slider changes above and precede the lifecycle refinements below.

### Playback report lifecycle and background refresh

Sixty original threshold cases exposed zero-second and unknown-duration behavior.
Native eligibility now follows those cases while rejecting nonfinite/negative
inputs and respecting private mode. Jellyfin audiobook metadata retains an
explicit resumable flag even when there is no saved checkpoint. Progress reports
follow ten seconds of playback position for resumable media; seeking rebaselines
that interval and reports at most once per second. Ordinary Jellyfin music does
not send periodic audiobook progress updates.

Four original lifecycle traces cover the two-second track-change delay, rapid
replacement, previous-track stop rules, explicit stop/resume and audiobook
progress. Native reports preserve their ordering. Stop/resume retains listening
progress for the same entry; restart and duplicate-entry replacement reset it.
The sleep-at-track-end path reports the departing track before advancing the
stopped queue. Jukebox restoration uses the same reporting state.

The original hook passes an already-millisecond duration through a seconds
conversion when submitting. Native reports use the actual track duration in
seconds, avoiding that 1000-fold position error. The controller request shape,
submission semantics and subsequent progress behavior are preserved.

Full lifecycle runs passed 264 core and 317 native tests on macOS, and 265 core
and 317 native tests on Linux (one opt-in projectM test ignored). Fourteen artist
and refresh regressions then passed on both platforms: album metadata refresh
retains page position, and artist section refresh retains selections and scroll
handles while discarding selections no longer present in the response. Strict
Clippy and the Windows Rust cross-check passed after the refresh fix.

Packages including the slider, seek and reporting changes pass archive,
executable, all 56 licenses and updater checks. The macOS bundle also passes
strict ad-hoc signature verification. They precede the presentation work below:

- macOS SHA-256: `40ec259b86ab1f7ff0182e399fd45bfa565258a9c532a3190ed4874774d6b437`
- Linux SHA-256: `7338ac050b2d04fd6fe17401e40bec05453731b097f832f51a7292e5e677386b`

Both archives are under `target/`. Interactive checks remain deferred.

### Player-bar presentation follow-up

Source comparison found remaining differences in the regular slider's fixed
elapsed/duration columns, 95-percent row width, sub-768px time-label visibility,
hover/focus thumb opacity, track color transition and formatted-value styling.
The scrobble condition bars also differed in RTL alignment and width transition.
Seventy-two windowless original layouts per platform and eighteen CSS timing
samples now cover the breakpoint, fixed columns, font size, zoom, track/thumb
geometry and motion. Native layout and painted thumb/color interpolation now pass all those cases on
macOS. The formatted label comparison uses the bundled Inter font, tabular digits
and zoom-adjusted optical size. Seeking and time-toggle interaction regressions,
RTL scrobble width animation and strict Clippy pass on both platforms. Full
workspace runs pass 264 core and 320 native tests on macOS, and 265 core and
320 native tests on Linux, with the opt-in projectM test ignored. Windows Rust
cross-checks and formatting pass. The package hashes above predate presentation
changes.

### Volume slider and album navigation follow-up

The original Previous/Next buttons pass the Alt modifier to album navigation.
Native pointer callbacks now route through the existing album shortcut actions;
empty music queues disable the main play button. The volume slider now uses the
same player-bar appearance and numeric hover label, honors its configured width
including zero, scales with root font size/zoom and hides at 480 CSS pixels.
Volume changes keep an immediate first update and a 100 ms trailing cadence;
external adjustments supersede pending pointer values.

Four original modifier cases, twelve volume visibility/width configurations,
three original Mantine throttle timelines and thirty-two browser volume layouts
provide comparison data. Native layout, actual pointer/modifier callbacks,
leading/trailing volume updates and external-adjustment preservation pass on
macOS. The full macOS native suite passes 321 tests (one opt-in projectM test
ignored), with strict workspace Clippy. The Linux native suite also passes
321 tests (one opt-in projectM test ignored), strict workspace Clippy and the
Windows Rust cross-check. Both packages pass archive, executable, all 56 licenses
and updater checks; macOS also passes strict ad-hoc signature verification.
These packages precede the theme correction and frame changes below:

- macOS SHA-256: `3705de404ad29fc0fc0b79ca9a9d046b4a88cac738718517fc83266da0292856`
- Linux SHA-256: `5dc2fb9c234f763f8fb8cc2809f86f43878ab2760caee896174d5bfddd868e32`

### Player-bar frame and theme follow-up

The browser references now load Feishin's actual Mantine theme. This corrected
the slider label's line height and corner radius, small elapsed-time font, and
the dragged thumb's two-layer shadow. The corrected slider comparisons and
seek interactions pass on macOS. Earlier presentation results above used
Mantine's fallback typography and should not be read as validation of those
specific app-theme properties.

The original frame/transport comparison covers 160 settled layouts per platform:
desktop versus compact at 768 CSS pixels, five widths, root-font size, zoom,
RTL, optional skip controls and radio mode. It executes the original responsive
frame, center controls, transport buttons and compact player composition.
Desktop left/right content is stubbed only to measure the three column slots;
compact motion is settled. This does not establish full metadata, right-control,
animated artwork or physical pointer parity. Native desktop/compact geometry, control order and icon sizes now match the 160
references on macOS. The frame uses the original 90-pixel height, equal desktop
columns and compact breakpoint after zoom. Radio mode omits seeking. Track title
context menus and explicit menu buttons remain available with sidebar artwork;
compact title/artist/album links select the player tab and retain the original
open/close behavior. Actual pointer/menu/navigation checks pass.

The first full run passed 264 core tests on macOS and 265 on Linux. It exposed
four native regressions: a nameless transport container, blocked empty-bar
clicks, a test expecting content-sized columns, and RTL auxiliary controls
extending off-screen. All four targeted regressions pass after fixes. Corrected full suites pass
322 native tests on both macOS and Linux, with one opt-in graphics test ignored.
Formatting, strict workspace Clippy on both platforms and the Windows Rust
cross-check pass. The updated macOS bundle passes archive, executable, all 56
licenses, updater and strict ad-hoc signature checks:

- macOS SHA-256: `33bb6a453666a528b3046945b25990ffc6da12c94eceb4e2ca8f4296781df37b`

The Linux package also passes archive, executable, all 56 licenses and updater checks.

- Linux SHA-256: `28066dae0ab34655db150aac03586620df94800b2bd097a34e5ccbf2be5cf37a`

### Network-control follow-up

Source comparison identified additional implementation work: direct player-bar
output selection, group-wide master volume/mute with Shift for individual control,
member mute/sliders, rapid member updates, passive disconnect and adoption of a
speaker's existing playback. These are not covered by the earlier network
protocol checks. Forty-eight original master-volume callbacks and thirty-two
member callbacks are captured in `network-volume-reference.json`; native wiring
and comparison tests are pending. A further 36 original connection cases cover
local/native/network engine choices, playing/paused/stopped speakers and failed
connection. Eight disconnect callbacks cover normal versus Shift, prior local engine and
saved seek position. These references are reproducible on macOS/Linux and registered in CI.
The broader right-control presentation also
remains under comparison.


### Network volume controls and output access

The player bar now exposes the output selector directly. Network volume changes
apply to the group unless Shift is held; keyboard changes remain coordinator-only.
The hover panel provides per-member sliders, wheel adjustments and mute buttons,
with Shift enabling individual controls. Stereo-pair detection uses the original
ChannelMapSet/HTSatChanMapSet distinction. Member requests coalesce while an earlier
request is running, preserve the final requested volume/mute, report failures and
reject replies for removed or replaced endpoints.

The Mac and Linux headless checks pass 48 original master callbacks, 32 member
callbacks and 96 original settled volume layouts per platform. Actual pointer,
wheel and Shift checks cover enabled/disabled member controls. Local HTTP fixtures
cover slow replies, final-value preservation, member removal and rejected writes.
The layout reference uses the original translated instruction text, theme and CSS.
Panel transition paint and the broader output-picker/right-control presentation
still need comparison. Connection adoption and passive disconnect remain open.
These changes are newer than the verified release packages above.


The complete macOS workspace run after the volume/output changes passes 264 core
and 325 native tests (one opt-in graphics test ignored). Original queue matching
now has a further 72 reproducible cases, including duplicate tracks, next-track
disambiguation, query IDs, pathname fallback and unavailable stream URLs. The
volume reference also records 48 CSS motion samples per platform; native motion
comparison is still pending.


### Speaker connection and disconnect follow-up

The volume/output snapshot passes complete workspace runs on both platforms:
264 core tests on macOS, 265 on Linux and 325 native tests on each, with the
opt-in graphics test ignored. Strict workspace Clippy and Windows Rust checks
also pass for that snapshot.

Connection now reads the speaker's current track, next URI and volume before
switching. Matching queued playback is adopted without load/play commands,
including paused playback and duplicate-track disambiguation. Failed or cancelled
connections preserve the current output. Normal disconnect reads the latest
position and restores local playback; Shift-disconnect leaves the speaker playing
and local playback paused. Restoring local volume occurs after detaching the
speaker. Live proxy streams retained by passive disconnect survive subsequent
local playback changes and are cancelled when the app exits.

Mac focused checks pass 72 original queue matches, all 36 original connection
choices and eight disconnect choices, including actual modifier clicks. The
native handoff fixture intentionally has no music-server session, so the normal
local-resume attempt is checked at that boundary without opening an audio output.
Core SOAP fixtures independently verify adoption without load/play, current/next
URI reads, latest-position retrieval and Stop only for normal disconnect. Proxy
checks verify retained live endpoints and shutdown cancellation using silent
HEAD requests. Complete regression runs for the handoff changes are in progress.


### Speaker motion and player-button follow-up

The complete handoff snapshot passed 267 core tests on macOS and 268 on Linux,
plus 327 native tests on each platform (one opt-in graphics test ignored).
Strict workspace Clippy and Windows Rust checks passed. Windows native linking
and runtime remain unverified.

A completed speaker disconnect now closes the output popup while preserving the
current route. Group volume panels retain their fade-out paint while pointers
pass through the complete hidden subtree. Forty-eight original CSS samples per
platform verify both directions of panel opacity/translation and Shift-controlled
member opacity. An actual pointer test verifies that an enabled child of a fading
panel cannot intercept clicks. The existing 96 settled volume layouts and live
output-selector checks pass on both platforms.

Player transport and volume action icons now use the original background color
mixes, 200 ms ease-in-out hover/focus fade, one-pixel pressed movement and 0.6
disabled opacity. The original theme disables focus rings; keyboard focus retains
its background highlight. Original component/PostCSS fixtures record 48 paint
states on each platform. Native pointer/focus tests compare all 42 reachable
states, excluding keyboard focus on disabled controls. The complete macOS native
suite passes 329 tests, with one opt-in graphics test ignored; strict checks pass.
The Linux suite also passes 329 native tests with the same ignored test.
Strict Linux workspace Clippy and Windows Rust cross-checks pass.

The macOS bundle, release archive, executable modes, 56 notices, strict ad-hoc
signature and installer replacement/checksum checks pass for this snapshot:
`target/feishin-native-macos-aarch64.zip`, SHA-256
`8e28fa2784322a02662d289a4d52f24f642cb4f231dbc8510c80f777ceab21a4`.

Right-side player-bar presentation is the next comparison. Its new original
component reference measures 96 arrangements across root font size, zoom,
direction, rating/favorite visibility and volume width. Native comparison and
implementation for these arrangements are not yet complete. Interactive checks
remain deferred at the user's request.


The Linux package for the speaker motion/button snapshot also passes archive,
executable, 56-notice and installer checks. The copied host archive verifies as
`target/feishin-native-linux-aarch64.tar.gz`, SHA-256
`2dfe884af1eacb68b5b49cc0b31a40fc4b94767ed6d2bb203890ed2c4875ef92`.
Both release archives predate the right-controls changes below.

### AT. Right-side player-bar composition

The right controls now use the original top rating/Auto-DJ row, middle action
row and bottom spacer. Controls follow the original cast, sleep, configuration,
lyrics, favorite, queue and volume order, with original dimensions, icon choices,
visibility and direction. Auto-DJ retains its compact uppercase label. Muted
local volume uses the original muted foreground. Player/visualizer expansion
remains available through the original player artwork/tabs and visualizer controls.

All 96 original right-control arrangements pass native macOS geometry checks.
The original fixtures reproduce on macOS and Linux. The first full application
run exposed tests clicking controls covered by the sleep popup or the entering
visualizer panel; the tests now dismiss the retained popup and wait for the panel
transition before using those controls. Full regression verification is ongoing.

The speaker picker still needs the original compact presentation and group-builder
workflow. An executable reference now captures 80 original group-builder selection
and confirmation cases, including first-selected coordinator, locked coordinator,
stereo-pair exclusions and minimum member count. Native comparison is pending.


The right-control snapshot now passes all 330 native tests on both macOS and
Linux (one opt-in graphics test ignored). Strict Clippy passes on both platforms,
as do the Windows Rust checks. The release packages above predate this snapshot.

### AU. Compact speaker picker and grouping

The original picker now has 88 reproducible content layouts per platform, and
its group-builder selection reference covers 80 cases. Both references are
registered in native CI. Native picker, group mutation and speaker-property
improvements are being implemented and have not yet completed verification.

The compact picker now reproduces all 88 original layouts on macOS and Linux,
including text geometry, group selection, connecting, searching, solo and grouped
states. Its selection rules pass all 80 original callback cases. Native protocol
checks cover first-selected coordinators, partial join failure, removal,
cancellation, nested properties, and coalesced property writes that reject stale
reads and replaced endpoints. Parent popups retain focus and remain open while
editing nested settings. The picker snapshot passes 335 macOS application tests
and strict Clippy. The first Linux full run passed 334 tests and hit the handoff
fixture's five-second wait limit; that test passes in isolation. Its wait now
allows twenty seconds under full-suite CPU contention and reports the call site.
A fresh full run is required after the settings presentation changes below.

The speaker-properties reference now measures 64 original layouts per platform,
covering loading/failure, both switch values, slider extremes, root font size,
zoom and RTL. Settings presentation is being matched to those references;
keyboard and actual SOAP setter checks are included. Final regression and release
package verification are still pending. Interactive checks remain deferred.

The compact controls now pass 200 layout/placement comparisons on each platform:
88 picker contents, 64 settings contents and 48 complete popups. The full-popup
reference caught the settings panel's rem-based minimum width, the original
arrow's additional 3.5-pixel gap, RTL side selection, and viewport-edge flipping.
The reference uses a fresh offscreen view for each root font size to avoid stale
Chromium font/zoom caches and asserts the viewport dimensions before recording.
Native tests verify the original 150 ms popup fade/translation, nested focus,
keyboard switch activation, actual six-field SOAP values, coalesced writes and
rediscovery at a replacement endpoint. Popup shadows, spinner rotation and the
Shift-disconnect hint transition are retained. Original popup CSS aliases reach
both speaker surfaces. Full regression checks are running; the release archives
listed above still predate these changes.


The macOS workspace run passes 267 core tests and 336 application tests, with
one opt-in graphics test ignored. Strict Clippy and formatting pass. A subsequent
small-window check verifies the original 80vh properties limit and scrolling to
and editing the final switch with enlarged text; all six picker/property tests
and strict Clippy pass again. Linux/Windows final checks remain in progress.

The current macOS package includes the compact picker, settings and right-side
player-bar changes. Archive contents, executable permissions, all 56 notices,
installer checksum rejection/replacement/cleanup and strict ad-hoc code signing
pass. `target/feishin-native-macos-aarch64.zip` SHA-256:
`2ba39d9d1c90b5004c968d617d5d494de08d82fbb5ab08b814dfda41c1dfaf83`.
Linux passes all 268 core tests and 336 application tests, including the final
small-window settings check. Strict Clippy passes on Linux and the Windows Rust
targets. The matching Linux archive passes executable, 56-notice and installer
checks: `target/feishin-native-linux-aarch64.tar.gz`, SHA-256
`38a624913188045108a218bfb704689f112535a097befd6ffd22eb84df806eec`.
Native Windows linking/runtime and the user's deferred interactive checks remain
unverified. Both archives contain the AU snapshot and predate subsequent changes.

### AV. Glassy theme selectors and generated surfaces

The CSS cascade now retains separate `::before`/`::after` targets and descendant
or direct-child `:has()` conditions on known aliases. The original Electron
runtime supplies 618 cascades covering positive/negative conditions, specificity,
selector lists, generated-box isolation and invalid nested relational selectors;
the same reference reproduces on macOS and Linux. Every selector in the built-in
Glassy stylesheet parses without warnings.

The native card shade, slider track background and expanded-player panel
background expose their original generated targets. Glassy hides the card shade,
colors the slider and rounds the panel as in the source. Custom shade colors and
opacity, inherited generated text color and variables, and window-bar presence
changes have native rendering checks. Focused macOS tests pass; full platform
regression checks are running. Arbitrary browser DOM generation is not implemented
by these native aliases. The final macOS run passes 268 core and 337 application
tests; Linux passes 269 core and 337 application tests. Strict Clippy passes on
both platforms and the Windows Rust targets. Release archives still contain AU.

### AW. Window-bar presentation audit

The original title-bar reference now measures 96 layouts across Windows/macOS
styles, short/clipped titles, root font size, zoom, RTL and narrow/wide windows.
It uses the actual wrapped Text, CSS and macOS raster assets. A fresh offscreen
view for each font/zoom combination prevents stale Chromium viewport zoom, and
every case asserts its viewport before recording. The native title bar still uses
generic controls in the AU packages. The current implementation matches all 96
layouts on macOS and Linux, with the original 30-pixel bar, 13-pixel default text,
narrow-window spacing, fixed Windows control group, centered/clipped macOS title,
original raster hover assets and original Remix SVG paths. Native pointer checks
cover Windows hover colors, all control focus targets and keyboard close in the
headless window. Titles without an artist retain the source's omitted app-name
suffix. The icon raster check accounts for the minimize glyph's original 14-by-2
pixel stroke. Full regression checks are running; release packages still contain AU.

The macOS full application run now passes 338 tests, with one opt-in graphics
test ignored, and strict Clippy and formatting pass. Linux final verification is
still running. An additional 126 original selector cases cover invalid generated
targets in negation and forgiving `:is()`/`:where()` lists. The parser now rejects
invalid `:not()` rules and discards only invalid branches in the forgiving lists;
the expanded 744-case reference and style regressions are being checked.

Linux also passes all 338 application tests, strict Clippy and the Windows Rust
checks. The final selector correction passes all 268 macOS core tests and all 38
native style regressions. The lyrics-page Favorite/Favorited labels now reuse
the original locale keys and expose a localized action name; the existing locale
regression passes. Final strict Clippy and formatting pass on macOS. Linux checks
for these last changes and refreshed release archives are in progress.

The refreshed macOS bundle and archive pass executable, 56-notice, installer
replacement/checksum/cleanup and strict ad-hoc signature checks. The archive
includes the theme selectors, generated surfaces, original title bars and final
locale correction: `target/feishin-native-macos-aarch64.zip`, SHA-256
`5fe29aac94928fb41baeab95282e163f8721b45f71958b3f6a97daba69f47c7f`.

The final Linux checks pass all 269 core tests, all 38 style regressions and the
locale regression, followed by strict workspace and Windows Rust Clippy. Both
original selector and title-bar references reproduce. The matching Linux release
archive is being built. Interactive checks remain deferred as requested.

The matching Linux archive passes executable, 56-notice and installer
replacement/checksum/cleanup checks: `target/feishin-native-linux-aarch64.tar.gz`,
SHA-256 `6c6f56e657b92ef90a636fba257dc836e8b3423db949e993fc0cedd654cc17d4`.

### AY. Shared modal theme surfaces

The shared dialog now exposes the original content/header/title/body/overlay and
close-button aliases. A scoped body render applies nested theme selectors to
controls without inheriting the library page's ancestor selectors or leaking
modal selectors back into the page. Multiline inputs expose the original textarea
alias. Custom dialog shadows remain effective during the existing animation.
The original wrapped modal supplies six Glassy/custom cascades across two root
font sizes, including hover/active states. Native rendering checks are in progress;
the release archives above still contain the preceding snapshot.

The modal check now passes on macOS. It also exposed incorrect nested-list
specificity: only the parent list shares its maximum specificity, while each
child branch retains its own. The parser now expands those branches separately,
including mixed explicit/implicit nesting and literal or escaped ampersands.
The expanded selector reference reproduces all 825 cases on macOS and Linux;
the complete regression checks are running. Standard modal frame geometry is
being compared separately from these theme checks.

AY passes 268 macOS / 269 Linux core tests and all 339 application tests on each
platform, with the one existing opt-in graphics test ignored. Strict workspace
Clippy passes on both platforms, and Windows Rust checks pass. Physical checks
remain deferred. The current release archives still contain AX.

### AZ. Standard modal frame comparison

The original wrapper supplies 96 frame layouts on each platform, covering narrow
and wide windows, short and overflowing bodies, title/close-button combinations,
two root font sizes, zoom and RTL. The native standard frame is being aligned to
the original centering, viewport bounds, padding, typography and fade transition.
The fullscreen artwork viewer keeps its explicit frame offset. Implementation
and regression checks for this section are in progress.

The standard frames now match the original 96 layout cases on macOS and Linux,
including native scrolling beneath fixed headers. CSS borders snap to device
pixels before layout, fixing Linux title/body widths at fractional zoom. Original
settings and export options add 128 frame references per platform. Pop transitions
use the original 300 ms easing, scale and translation on entrance and retained
exit. Transparent overlays accept outside clicks, cancellation vetoes are honored,
and focus returns after dismissal. The shared close control uses the original
Remix asset and a localized accessible name; titled application dialogs also
expose their names to assistive technology. Full regression checks for this
snapshot are still pending; release archives continue to contain AX.

The initial complete macOS run passes 268 core tests and 338 application tests,
with three failures caused by tests clicking through the newly retained dialog
exit. Those interaction steps are being corrected to wait for the original
transition. The modal/visualizer and small-window lyric-export checks pass on
both platforms, and all 224 frame references and both transition references
reproduce. Final complete regression and strict checks are still pending.

The corrected complete macOS run passes all 268 core tests and 341 application
tests, with one existing opt-in graphics test ignored (241.53 seconds for the
application suite). Strict checks and the corrected Linux run are in progress.

AZ now passes all 268 macOS / 269 Linux core tests and 341 application tests on
each platform, with the existing opt-in graphics test ignored. Strict workspace
Clippy passes on both platforms and Windows Rust checks pass. Native Windows
linking/runtime and physical acceptance remain unverified. The release archives
still contain AX.

### BA. Shared button variant comparison

A new reference executes the original wrapped Button and compiled stylesheet
for 600 combinations of variant, disabled state, hover/active/focus, font size,
light/dark mode and default/Glassy/custom themes. Another 32 cases cover compact
and regular sizes with and without a left icon. macOS and Linux produce the
same state styles. Fractional icon offsets differ by at most 1/128 CSS pixel
between the two source display scales; the comparison bounds that difference.
The Linux probe attaches its view to a hidden window on the isolated virtual
display, because an unattached Linux view reports no hover-capable pointer and
omits Mantine's pointer-dependent rules. The reference asserts pointer support.
Native buttons now expose their actual variant instead of always reporting
filled, and use the original root paint rules, dimensions, font sizes, weight,
icon spacing, corner radius, disabled opacity and filled focus outline. The
headless macOS comparison passes 572 rendered cases (all supported button
variants plus the 32 size cases). The source's unused custom secondary variant
has a reference but no separate native public button variant. Hover changes are
resolved before text shaping so inherited foreground updates reach glyphs.

The shared CSS parser now handles bounded, premultiplied sRGB color-mix, including
nested mixtures, implicit/explicit percentages, alpha normalization and Chromium's
legacy RGB alpha quantization. All 113 original mixing/invalid-input cases agree
on macOS/Linux. Core totals are 269 on macOS and 270 on Linux. Complete application
regression and strict checks for this batch are running.

Eighty original button transitions (560 sampled frames) also reproduce on both
platforms, including hover, leave, focus and disabling in both theme modes. These
show that modern mixed colors interpolate in Oklab, while legacy outline
backgrounds use sRGB. Native shared-button color transitions remain pending;
steady-state comparisons do not establish motion parity. Custom font/radius
Mantine defaults and further control layout comparisons remain in the audit.
The packaged releases still contain AX.

BA verification now passes strict workspace Clippy on macOS/Linux and Windows
Rust checks. Both complete application runs passed 341 tests with one ignored
and one failed test that clicked a table row beneath its sticky header. The
corrected interaction passes on both platforms. The selected-choice paint
regression and all 572 shared-button cases also pass on both after the final
fix. A new complete suite has not yet run after these corrections.

### BB. Command palette keyboard and inline actions

The pinned original cmdk library and original root keyboard handler supply
four navigation/pointer/inline-action sequences, reproduced on macOS and Linux. They cover
clamped boundaries, Home/End, Control-J/N/P/K, Alt group jumps, Meta first/last,
disabled and empty groups, pointer selection, Tab focus and Enter activation.
The synthetic key events establish handled-key behavior; native browser default
Shift-Tab traversal and frame timing are not measured by this reference.
Native stable result selection, shortcuts, pointer selection, action focus and
held shuffle are being corrected and tested. Broader palette presentation
comparison remains pending.

BB's native keyboard and pointer checks pass on macOS/Linux, covering input
shortcut precedence, composition Enter, clamped navigation, pointer selection,
Tab/Shift-Tab, key-down-only inline actions, held shuffle and release suppression.
Nested playlist dialogs retain the search and return focus to the initiating
control. General commands remain available while searching; subpages retain the
query and cached results. Loading commands retain selection without accepting
activation. Headers toggle their sections and separate result counts open the
filtered library. The complete updated macOS suite passes 343 application tests,
with one existing graphics test ignored, and strict workspace Clippy passes.

The Linux complete run passed 342 tests and timed out in a speaker handoff test.
Its second disconnect, intended as a barrier, could supersede the real command
before the playback actor processed it. The test now waits on the existing
completion callback instead. Corrected Linux regression passes all 343 application tests (one existing
graphics test ignored). Strict workspace Clippy and Windows Rust checks also pass. Core code has not changed since BA (269 macOS / 270 Linux tests).

### BC. Command palette presentation comparison

The original palette composition, wrapped controls and compiled styles supply
40 steady layouts per platform, spanning two font sizes, narrow/wide and
short/tall windows, empty/results/collapsed home pages, navigation and servers.
The reference uses inert network/playback fixtures and artwork placeholders;
it measures layout, typography and control visibility, not artwork rendering,
loading feedback, tooltip behavior or animation. The source reference reproduces on macOS and Linux. Native macOS comparisons
pass all 40 cases for dialog/input/list frames, section headings, result rows,
artwork slots, metadata, inline controls and footer bounds within one logical
pixel. The native palette now grows naturally and scrolls its whole dialog body;
keyboard navigation scrolls selected results into view. Original icon assets,
metadata spacing, command labels, album counts and compact breadcrumb/keycap
controls replace the earlier placeholder presentation. Rows and nested controls
preserve focus through paging and removal. The comprehensive palette keyboard/
pointer regression passes after these layout changes. The complete macOS suite passes 344 application tests with one existing graphics
test ignored, and strict workspace Clippy passes. Linux passed 343 tests and
caught a keycap font fallback mismatch; it now uses the platform monospace family.
The corrected Linux palette comparison passes all three tests, as do strict
workspace Clippy and Windows Rust checks. A new complete Linux suite has not run
after the keycap correction.
Release archives still contain AX.

### BD. Shared-button color transitions

The original 80 transitions now include 560 browser-rendered color samples,
which reproduce on macOS/Linux. The probe converts recorded colors in one
readback to avoid per-frame canvas readback delays in an unparented view.
The original 632 steady styles/sizes still reproduce after that harness change.
Native premultiplied sRGB/Oklab color transitions and per-variant opacity timing
are being checked against those frames. All 572 native steady button cases pass
on macOS with the new transitions settled. All 504 native frames for the nine supported variants pass on macOS, covering
hover, leave, focus and disabled state in both modes. Border comparisons select
the border paint pass separately from the translucent background pass. Linux also passes all 504 motion frames and 572 steady cases. Complete runs on
both platforms pass 344 application tests, ignore the existing opt-in graphics
test and expose one steady modal-color test that sampled before the transition
finished. That test now waits for the final color and passes on both platforms.
Strict workspace Clippy passes on both, as do Windows Rust checks. Custom CSS
transition/color-space overrides remain in this audit.
Release archives still contain AX.

### BE. Custom button theme dimensions

The original wrapped button supplies 288 layout references for normal/compact
sizes, optional icons, two root font sizes and nine theme configurations. These
cover font-size maps, radius maps, default radius names/numbers/units, theme
scale and font-family inheritance. macOS and Linux reproduce the references.
Native button defaults now use the original Mantine geometry declarations and
the existing CSS variable cascade before explicit builder styles. Original
Mantine rem values retain their scale calculation, and bounded CSS length
arithmetic preserves units and precedence. Both platforms pass all 288 native
cases. Core regression passes 270 tests on macOS and 271 on Linux. Linux passes
all 346 application tests with one existing graphics test ignored. macOS passes
345 application tests and times out in the speaker-disconnect fixture; its
isolated repeat passes and further diagnostics are in progress. Strict workspace
Clippy passes on both platforms and Windows Rust checks pass. The source layout
comparison permits the accumulated subpixel rounding of icon padding and spacing
(up to 1/32 pixel); native bounds remain within one logical pixel.
Release archives still contain AX.

### BF. Wrapped-button loading behavior

The original Button, Spinner and Icon composition supplies 512 settled loading
states, including disabled controls, normal/compact sizes, optional icons and
two root font sizes. Another 56 frames measure content/overlay opacity and the
loader's retained entrance/exit. The source uses the previously verified spinner
SVG geometry. Reference capture batches remounted controls below React's nested
update limit and freezes the initial CSS transitions before the delayed loader
transition, so unrelated frame scheduling does not advance the sampled fades.
Native contents now retain their width while hidden, loading controls block
activation and focus, and the original spinner uses a separate clipped layer.
Both original reference sets reproduce on macOS and Linux. Native comparisons
pass on both platforms, including original Inter typography, spinner geometry,
content/overlay opacity, retained exits, disabled activation and width preservation.
The complete runs each pass 344 application tests, ignore the existing graphics
test and fail four fixtures. Three fixture servers started their deadlines before
UI setup; Auto DJ also assumed an HTTP reply had already reached the UI. The
macOS speaker layout fixture allowed the live theme watcher to overwrite its
injected font size. Linux's fourth failure was an early playback-report deadline.
Those fixtures now start their request deadlines at the relevant action, and the
layout fixture retains its injected theme. The window-preferences test also stops
rebuilding the same theme for every control, and CI limits test concurrency to
four. Corrected focused regressions pass on macOS and Linux. A later Linux repeat also
reproduced the speaker handoff race: the adopted position could arrive after the
fixture manually assigned its fallback. The fixture now establishes that position
through the playback actor and waits for its acknowledgement. The corrected Linux
speaker suite passes all 13 tests. The subsequent macOS full run passes all 348
application tests, with the existing graphics test ignored. Linux's full run
also passes all 348 ordinary tests, with the same graphics test ignored.
Release archives still contain AX.

### BG. Custom button transition declarations

The original wrapped button supplies 42 custom hover/leave/reversal transitions with 756
sampled frames. Cases cover disabled transitions, property lists, shorthand and
longhand precedence, repeated timing lists, positive/negative delays, named and
cubic curves, four step modes, variables, invalid declarations and important
rules, invalid computed variables and state-dependent timing. Samples straddle
step boundaries to avoid treating floating-point equality at a discontinuity as
visible animation behavior. Batched frame reads avoid per-frame IPC scheduling.
The bounded native transition-list parser matches the original computed cascade.
Native button paint uses the cascaded timing and each endpoint's color format,
including shortened reversals across legacy RGB and modern mixed colors. macOS
and Linux pass all 756 custom frames and 504 existing default-theme frames.
Strict workspace checks pass on macOS/Linux, and all Windows Rust targets pass
strict cross-platform checks. Core tests pass on macOS (271) and Linux (272),
alongside the macOS capture-helper test; the opt-in tag integration test remains
ignored. The full macOS application suite passes all 348 ordinary tests, with
the existing graphics smoke test ignored. Linux's full application run also
passes all 348 ordinary tests, with the same ignored graphics test.

### BH. Scaled loading controls and lyric clearing

The loading reference adds 96 states covering custom Mantine scales of 0.8, 1.3
and 2 at both root font sizes. The native spinner offset and overlay inset now
use the theme's scaled rem unit. The fixture includes the original icon font-size
rules, and SVG raster dimensions allow half a physical pixel for native rounding.
Original uppercase action labels are retained in playlist editing/addition and
the lyric controls, including the original localized search/export/translation
labels.

The original Clear callback and automatic-provider gate supply eight executable
cases. Native Clear now discards a selected remote override and suppresses
automatic matching for the current song/server until the app or query cache is
reset. Server-provided lyrics and imported local text remain available; a later
manual selection still wins. Stale provider replies are rejected before writing
an override, so an earlier request cannot restore a cleared selection. A headless
mouse/HTTP/file check verifies the preserved queue, offsets, local text, later
manual selection and query-cache reset. That check and the playlist/lyric
regressions pass on macOS and Linux. Both platforms pass all eight focused
regressions; macOS also passes 26 lyric/cache checks. Strict checks pass on
macOS/Linux and for all Windows Rust targets. The loading source reproduces 608
settled states per platform, and both native loading tests pass. The macOS app
and archive now contain BH, with strict ad-hoc signature verification, 56 license
notices, archive checks and installer checksum/replacement safeguards verified.
Its archive SHA-256 is `c6452435135c5391ffba5dba0201418f764be8a01a4b670cf6ca59a741360a24`.
The Linux archive also contains BH, with 56 license notices, archive checks and
the installer checks verified. Its SHA-256 is
`f2acf02f9db95c3f3f2986113d5531df7b0a5df7b7fc830d77a60f536d3a36ac`.

### BI. Per-song lyric offsets and language selection

The original callbacks supply 60 states covering global defaults, per-language
offset precedence, manual timing adjustments, language changes and fresh local
lyric loads. Native song offsets now use milliseconds, with 50 ms buttons and
1 ms keyboard steps. The session retains each song/server's language and timing
choices independently of global preferences. Query-cache clearing resets them;
fresh lyric loads restore provider offsets while retaining the selected language
and fallback offset. The 60-state comparison passes on macOS. A native UI check
also covers typed values, keyboard steps and switching away from and back to a
song. It exposed a stale focused input after an asynchronous load; the field now
updates when valid text changes while retaining unfinished numeric input. The
corrected UI check passes. All 28 lyric/cache regressions and three existing
numeric-control checks pass on macOS, along with strict workspace checks.
Linux also passes all 31 focused lyric/cache/numeric-control checks. Strict Linux workspace checks and Windows cross-checks also pass. The macOS package now contains BI; signature,
archive, 56 license notices and installer checks pass. Its archive SHA-256 is
`78158a802238f5144bd6b1b7de30ff50fe64e33898c411f21a6c24fd4b3d1d9a`.
The Linux package also contains BI; archive, 56 license notices and installer
checks pass. Its SHA-256 is
`1795ffc9e8abf1138c83526cb64d22d2b38a3824e5e6398968b387afd43a85a3`.

The next confirmed difference is action placement: the native controls currently
appear on the dedicated player page, while the original also places a hover/focus
action strip inside expanded-player and sidebar lyric frames. The original lyric
search also opens a modal; the native search currently navigates to its own page.
These flows remain to be completed and compared.


### BJ. Lyric actions in player and sidebar frames

Both lyric frames now include the original two-row action strip. Controls retain
provider gating, language selection, quick pronunciation/translation toggles,
extra-layer switches, export, timing changes and Clear/Refresh. The strip overlays
the lyrics without reducing their viewport. Independent timing input entities
share only the song's offset, so simultaneous panes retain separate focus.
The original React component supplies 36 layouts and callback sets on each
platform plus 28 CSS opacity samples. Native checks match layout within one
logical pixel and opacity within 0.002, including wrapping at 220/360/640 pixels,
16/20-pixel root sizes, disabled controls, hover, focus and pointer exit. Mouse
checks cover language menus and layer switches from both panes. The original
number input is 36 pixels high at the default root size; the shared native
wrapper now retains that height and its 13-pixel text.

All 26 lyric regressions and three numeric-control regressions pass on macOS and
Linux. Strict workspace checks pass on both platforms, and Windows cross-checks
pass. Physical interaction remains deferred. The current release archives still
contain BI; the action-strip work is in the development source. The original
search-and-preview modal is the next functional gap being addressed.

### BK. Lyric search and preview modal

Search now opens over the current page with separate title/artist fields,
independent 500 ms debounces, ranked results, keyboard selection, preview,
confirmation and nested export. Preview reuses the timed-word and singer-cue
renderer, including the original fixed preview font size. Cue clicks seek without
replacing the installed lyrics. Confirmation stays bound to the song that opened
the dialog, even if playback moves on. Per-song request tokens prevent late
confirmations from restoring a cleared or superseded choice.

The original callbacks supply 12 confirmation/export cases and 13 debounce
frames. All 29 lyric regressions pass on macOS and Linux. The final preview
extension also passes all three search tests on macOS, including background
singer sizing and seeking. The full macOS application suite passes all 356
ordinary tests, with the existing graphics test ignored. Its 271 core tests and
capture-helper test pass; the opt-in tag fixture remains ignored. Linux's full
run has a ruby-spacing failure under investigation; its isolated repeat passes.
The Linux failure was a GPU readback deadline under concurrent software-Vulkan
pixel probes, rather than a ruby geometry assertion. The headless probe now
serializes its temporary GPU contexts, preserving the pixel comparisons and
deadline. The subsequent complete Linux rerun passes at checkpoint BL below.

The next settings comparison found different time units, numeric limits and
untranslated boolean controls. Four original forms supply 248 control changes,
including separate player/sidebar display settings and Arabic labels. Native
settings corrections and their comparisons are in progress.

### BL. Lyric settings controls and units

The native form now separates general, display and translation settings, using
the original translated labels/descriptions, switches and slider ranges. Timing
fields use milliseconds with 10 ms global-delay and 50 ms lead-time steps.
Player/sidebar values remain separate and switching surfaces synchronizes the
retained slider positions. Provider choices can be cleared; translation language
choices retain the original names. API keys remain in the system password store
with an explicit Save control.

Four original forms supply 248 control changes. The macOS native test passes
204 numeric/switch comparisons in English and Arabic, actual provider/language
menu clicks, keyboard number/slider steps and persisted values. All 30 macOS
lyric regressions and 14 migration checks pass, including large finite delay
imports, round trips and rejected nonfinite values. The manual search list now
sorts combined provider results by match score, while automatic matching keeps
its synchronized-lyric preference. Genius and NetEase results retain their
original unknown synchronization status; result accessible names include the
provider, identity, status and score. The three search regressions pass with
mixed synchronization/score ordering and stale-query rejection. Strict macOS
workspace checks pass. The BL macOS package includes the final search corrections;
strict ad-hoc signature verification, archive checks, 56 license notices and
installer safeguards pass. Its SHA-256 is
`cfb63f00cc20926e08ab4dadb28f57ed765645a270e96f409594ea48c1e3932f`.
The complete BL Linux snapshot passes 273 core tests, 357 application tests and
the capture-helper test, with no failures. The opt-in projectM graphics and
prepared-fixture TagLib tests remain excluded from this workspace run. The full
application run takes 887 seconds and includes the serialized GPU probe fix.
Strict Linux workspace checks and all Windows Rust targets pass. Windows native
C++ linking and runtime remain unverified. Package refresh is recorded below.

### BM. Manual provider identities and packaged startup

Manual lyric search now preserves the separately ranked provider batches before
the dialog's stable score ordering. Global automatic ranking could otherwise
discard a result when two providers returned the same ID, or change the order of
tied results. Automatic lookup still applies the original combined ranking and
synchronized preference. The HTTP fixture returns the same ID from NetEase and
LRCLIB, verifies both manual choices and confirms automatic selection. The
original dialog reference includes tied scores and shared IDs; the native test
selects each provider's result independently. All 19 core lyric checks and three
native search checks pass on macOS and Linux. Strict workspace checks pass on
both platforms, and all Windows Rust targets pass strict cross-checks.

The Linux package startup fixture uses private X11 and D-Bus sessions, software
Vulkan and a temporary data directory. It checks the actual packaged executable's
painted window and normal exit, and saves the virtual window and log as CI
artifacts. It does not use the physical desktop or saved account data.

The refreshed BM macOS app and archive pass strict ad-hoc signature verification,
archive/license checks and the installer safeguards. The archive SHA-256 is
`bde0c32b9949384decd24ed0007e90c15a5937aae1e496aa67802166ea2d47ca`.

### BN. Actual Linux window startup

The new packaged-executable check exposed an X11 native-handle panic in tray
initialization and black windows that headless component tests could not detect.
X11 windows now provide valid retained window/display handles. Their initial
background preference reaches native creation, so opaque application windows use
an opaque visual; alpha visuals retain a compatible Vulkan composite mode. The
software X11 driver advertised alpha visual support but presented black even for
a plain white clear. The same renderer painted correctly on Wayland.

The X11 event loop also drains events already buffered by synchronous XCB replies.
Without this, the first mapping event can remain queued and painting never starts.
A control build with the visual correction but without the queue drain still
failed the painted-window check. The combined fix passes actual startup at
1320x840 on X11 and 1440x1000 on Wayland, using private displays, temporary profiles
and normal Quit actions. Both runs enable Vulkan validation and reject graphics
errors, panics and blank windows. The Wayland check verifies protocol buffer
presentation and the rendered image. CI saves both images and logs.

The native AT-SPI controls fixture and retained Wayland window fixture also pass,
including four hide/show cycles and rapid restores. Strict Linux and macOS
workspace checks pass. These are isolated display checks; physical desktop,
audio and device acceptance remains deferred. The startup screenshots exposed
a separate sidebar heading that incorrectly consumed remaining vertical space;
that presentation correction is the next checkpoint. Release refresh follows it.

### BO. Sidebar headings, folder menus and release startup

Section headings retain the original 2.5rem minimum height without consuming
unused scroll space. Playlist headings share their horizontal row with the
create control. Native checks cover 16/20-pixel root fonts, short/tall windows,
collapse, navigation and the heading/create bounds. Folder toggles no longer
open their action popup on an ordinary click. Rename/Move actions are attached
to the folder's context menu; actual left/right clicks verify independent owned
and shared trees, navigation, persisted state and mutations. All 14 sidebar
checks pass on macOS and Linux across the focused runs. Strict checks pass on
both platforms and for all Windows Rust targets.

The refreshed packages pass archive, executable, 56-license and installer checks;
macOS also passes strict ad-hoc signature verification. Archive SHA-256 values:
macOS `9de92c31b79ca44f382e77824fbad646f2bc5a425330b637d8ebaf7eb9adb751`;
Linux `6755b2dde5101deb886ab3adc754446e35a4259fa00be941c5563d599d202766`.
The packaged Linux app paints and exits normally on X11 and Wayland with Vulkan
validation enabled. The first Wayland quit attempt exposed a fixture focus race:
focusing Weston's X11 container did not ensure client keyboard focus. The fixture
now selects Weston's class, clicks the empty top edge and verifies keyboard entry
before sending Quit. The repeat passes. Both archives are available in `target/`.

The original expanded sidebar action row is the next presentation comparison.
Its native controls still occupy separate rows in the BO package.

### BP. Sidebar action row and search focus

The expanded sidebar now uses the original 65-pixel action row, 7/12 search
allocation, button spacing and root-font scaling. The source component and shared
wrappers supply 36 layouts per platform across widths, root sizes, English,
French, Arabic and scan states. Native row/control/search-icon bounds match within
one logical pixel. RTL reverses the controls and the search icon/text placement.
The menu's 1.5-second spinner follows reported scanning status and remains usable.

Search explicitly retains its initiating focus for dialog dismissal. Native
checks exercise clicks, Enter/Space key press/release, focus restoration, history
navigation and opening the menu during a scan. The search control excludes
playback shortcuts while focused. All 15 sidebar, six application-menu, three
command-palette and two scan checks pass on macOS and Linux; the final focused
comparison includes the shortcut guard and scan-status distinction. Strict
workspace checks pass on both platforms and for all Windows Rust targets.
Windows linking/runtime and physical acceptance remain deferred.

The release archives still contain BO. The next comparison covers expanded and
collapsed navigation rows: the native collapsed column is missing the original
text labels, active icon variants and active-item marker.

### BQ. Navigation row labels, active icons and RTL markers

Expanded navigation retains the original 2.25rem rows, adjacent spacing, icon/text
alignment and selected foreground. Collapsed navigation has the original 25-pixel
Remix icons, translated multiline labels, selected fills and marker. The marker
follows the original overconstrained inset behavior in RTL, reaching the right
edge. The locked icon vendor now includes the original 24 Remix variants.

The original components supply 312 layouts per platform at 16/20-pixel root
sizes, in English, French and Arabic, with active and inactive rows. References
use the app's English fallback for untranslated labels. Native geometry matches
within one logical pixel; painted text/icon colors and accessibility labels and
selection are checked separately. All 16 sidebar checks and the original icon
rasterization check pass on macOS and Linux. Strict workspace checks pass on both
platforms and for every Windows Rust target.

The release archives still contain BO. The next correction covers the compact
menu and collections tiles, their history controls and their inclusion in the
scrolling column.

### BR. Compact menu, collections and scrolling history

The compact application menu and Collections controls retain their original
translated labels, 1.5rem icons, padding and root-font scaling. The menu uses the
original Lucide asset or the reported-scan spinner. Its popup continues to share
the same actions as the expanded sidebar and player. Collection entries use the
original album/song icon family. Optional Back/Forward controls retain the
original ActionIcon interaction styling and minimum widths; at larger root font
sizes the controls wrap into separate rows.

History, menu and navigation now share the compact scrolling column. The
expanded-only convenience footer no longer consumes compact scroll space.
Twenty-four original component layouts per platform cover 16/20-pixel root
sizes, English/French/Arabic, history visibility and reported scanning. The
reference isolates controls in a plain scrolling viewport, without the browser
scrollbar plugin. Native checks exercise history, opening/dismissing the menu
during scans, and the resulting scroll displacement. All 17 sidebar checks, six
application-menu checks and the icon rasterization check pass on macOS and Linux.
Strict checks pass on both platforms and for every Windows Rust target.

The macOS release archive now includes BP through BR. Strict ad-hoc signature,
archive/executable, 56-license and installer checks pass. Its SHA-256 is
`69a5c1a6e6e9ca98531095fb08dd28926eb9aa84303ff7722d258ef52c6c7c1f`.
The Linux archive also includes BP through BR and passes archive/executable,
56-license and installer checks. Its SHA-256 is
`2a5e9dedfebd72aef8a5acc55022718a02f7f1e1149c3a021fd8fcf0a9ff9348`.
The packaged app paints at 1320x840 on X11 and 1440x1000 on Wayland, exits through
normal Quit and reports no Vulkan validation failures. Both startup images were
inspected on the isolated displays and copied to `target/`.
Popup placement beside the compact sidebar and the expanded sidebar's outer
composition remain in the presentation comparison. Physical acceptance is still
deferred.

### BS. Compact popup placement and corrected history measurements

Compact application and collection menus open beside their tiles, preserve
start/end alignment, mirror the preferred side in RTL and flip/clamp at viewport
edges. Forty-eight actual Mantine menu layouts per platform check these rules.
Native compact-sidebar interactions also verify popup placement, keyboard
collection selection and opening the application menu while scanning.

The reference CSS loader now exposes both kebab-case and camelCase class names.
The previous compact-history fixture missed the original `size-xl` icon class;
BR's history dimensions were therefore understated. Corrected references and
native controls use 18/22.5-pixel icons and 48.8/60.5-pixel rows at 16/20-pixel
root sizes. Navigation, action-row, command-palette and popup-position references
are unchanged by this loader correction. The checked reference scripts retain
both class-name forms.

All 18 sidebar, six application-menu, four popover, two popup and one icon checks
pass on macOS and Linux. Strict workspace checks pass on both platforms and for
all Windows Rust targets. The refreshed macOS archive passes strict ad-hoc
signature, contents, 56-license and installer checks; SHA-256:
`27c0db48e4e3150955daa1ef29447492a8351d65233c563a0892703c37306730`.
The Linux archive passes contents, 56-license and installer checks; SHA-256:
`c4ef0cf616bfaa627b8b4d4adafb15efafa639a6209690762504a19fc22ab159`.
Both archives are copied to `target/`. The packaged Linux application paints and
exits normally on isolated X11 and Wayland displays in expanded and collapsed
modes, with Vulkan validation enabled. Collapsed startup preserves its private
profile setting; both collapsed images were inspected. CI now retains those
additional startup images and logs. Physical acceptance remains deferred.

Expanded sidebar composition remains the next presentation comparison.

### BT. Expanded sidebar composition and artwork controls

The expanded sidebar uses the original action-row/scroll/artwork arrangement,
root-font spacing and accordion heading alignment. Headings have the original
font size, weight, chevron size and RTL placement. Artwork occupies its square
outer frame with the original padding; its collapse control appears on hover
and hides the image without opening the full player. The extra sidebar footer
and scan row were removed. Server management, Logout, sidebar preferences and
scanning remain available through the existing menu/settings actions.

Forty-eight original component layouts per platform cover two widths, heights
and root font sizes, three languages and artwork visibility. The fixture uses
static accordion endpoints and a plain scrolling viewport to isolate geometry
from transition timing and the scrollbar plugin. Native bounds agree within one
logical pixel. Native checks cover section collapse, hover visibility and hiding
artwork without changing player expansion. The scan test now invokes the actual
account-menu action and includes its music-folder request.

All 19 sidebar checks pass on macOS and across the Linux runs. Both scan checks
and all six application-menu checks pass on each platform. Strict workspace
checks pass on macOS/Linux and for all Windows Rust targets. The macOS package
passes strict ad-hoc signature, archive/executable, 56-license and installer
checks; SHA-256:
`1bba0c866e19eb825e7d9c7791c440ad6e19d5cd1a8fae89c2239df5d8fc9e49`.
The Linux archive passes contents, 56-license and installer checks; SHA-256:
`41c85e2023d182695328510c7efc3dc6d90286dcf819edc386835e672d11f119`.
Both archives are copied to `target/`. The packaged Linux app paints and exits
normally on private X11/Wayland displays in expanded/collapsed modes, with Vulkan
validation enabled. The refreshed expanded images were inspected. Saved
collection row presentation and inline renaming are the next comparison;
accordion/artwork transitions and physical acceptance remain unverified.


### BU. Saved collection rows and inline rename

Saved collection rows retain the original icons, active colors, spacing,
truncation and hover controls. Rename opens the original-size form beside the
row and leaves the current page open. The form preserves translated labels,
RTL button alignment, trimmed names and the original 128 UTF-16-unit limit.
Cancel, Escape and outside clicks discard edits. Renaming updates current and
historical collection names; deletion preserves the queue. Storage validation
accepts original non-ASCII names without invalidating existing native names.

Ninety-six original component layouts per platform match native bounds within
one logical pixel. Both platforms pass the focused layout/interaction check,
two core collection checks, existing save/overwrite and sidebar-section checks,
four popover checks and menu edge fitting. Strict workspace checks pass on
macOS/Linux and for all Windows Rust targets. Packaged archives still contain
BT. Playlist folder header presentation is the next comparison; transitions and
physical acceptance remain in the audit.

### BV. Playlist folders, counts, expansion defaults and heading actions

Folder headers retain open/closed icons, descendant counts, truncation, RTL
ordering and the navigation view's larger icon frame. Nested tree branches use
the original connectors and indentation; the single-level view uses its separate
padding. The playlist heading now includes expand/collapse all and view-playlists
actions, and its Back control/current folder label replace the extra navigation
row. Expanded groups retain the original panel spacing.

New profiles start with folders closed. Older native profiles preserve their
existing open/closed behavior. Owned and shared expansion choices remain
independent, and native export/restore now retains both scopes, including shared
collapse state from older exports.

Two sets of 96 original layouts per platform cover headers and folder wrappers.
Native geometry matches within one logical pixel. Four folder tests, shared
navigation/mutation checks, sidebar-section checks, expanded composition and all
icon rasterization checks pass on macOS/Linux. The complete core libraries pass
274 tests on macOS and 275 on Linux. Strict workspace checks pass on both
platforms and for all Windows Rust targets. The three new icons are embedded
from the verified original icon package. Packaged archives still contain BT.
Temporary folder opening during drags is the next behavior check; transitions
and physical acceptance remain in the audit.

### BW. Temporary folder expansion during drag and drop

Dragging over closed folders opens their ancestor chain. Leaving a folder or
cancelling restores the prior expansion choices; a drop keeps the destination
chain open and persists it. Temporary state is bound to its server and ownership
scope. Folders already opened by the user retain their state.

The native pointer test covers nested targets, leaving, cancellation and keeping
a completed drop's destination open. It passes on macOS/Linux, together with the
existing drag feedback, add-to-playlist drop and queue/sidebar reorder checks.
Strict workspace checks pass on both platforms and for all Windows Rust targets.
Full app suites are in progress. Both refreshed release archives pass their
executable, 56-license and installer checks. The macOS bundle passes strict
ad-hoc signature verification. The Linux package paints and exits normally
in isolated X11 and nested Wayland checks, with expanded and collapsed sidebars;
all four startup images have been inspected.

BW archive SHA-256 values:

- macOS aarch64: `8cd76cb1714ce366a6ca3d26710aacb94c927f152ecbbb74fdf360448be13c95`
- Linux aarch64: `ef6d539318f35efaa39870e25b0675f3bfd573de47699508652aff25e0efe936`

### BX. Collection text insertion and deleted-bookmark navigation

Collection rename clips committed insertions to the original 128 UTF-16-unit
limit, preserving whole Unicode characters and selected-text replacement.
Provisional IME text remains intact until committed. Existing native names up
to 256 units remain editable, and programmatic values and undo retain their
full text. Fourteen original Electron cases cover ASCII, CJK, emoji, partial
capacity, selections, legacy names and composition. Native checks additionally
exercise paste, undo and redo with GPUI's in-memory test clipboard.

Deleting a collection now removes only the bookmark and its open rename form.
Current/history pages retain their filters, view preferences and pinned choices.
The sidebar mouse test deletes the bookmark, uses Back/Forward and verifies
edited filters and the queue position remain intact.

The original reference, both native collection tests and strict workspace checks
pass on macOS. Linux and Windows checks are pending. The BW archives predate BX.

### BY. Playlist folder collapse motion

Folder contents retain their layout while their height and opacity animate over
200 ms; chevrons rotate with the same ease-in-out timing. Interrupted reversals
restart the full duration from their current values, matching Motion. Fully
closed children remain excluded from painting, accessibility and pointer targets.

The pinned original runtime supplies 216 frames across two content heights,
initially open/closed states and complete/interrupted transitions. Native motion,
layout and clipped-click checks pass on macOS, together with folder layout,
hover expansion, shared-folder isolation and the three existing drag/drop
regressions. Strict macOS workspace checks pass. The full BW suites exposed an
outdated sign-out test that targeted a removed sidebar button; its replacement
now passes using the current account submenu's Logout action. Linux and Windows
checks are pending.

The refreshed macOS bundle passes strict ad-hoc signature verification, archive
and 56-license checks, and installer replacement/settings-preservation checks.
Its SHA-256 is
`c012a2c4aa112be39d3ec20fd05cb1c7408d2b2f6042f43c70b8c59e30cc86ad`.
The Linux archive remains BW.

### BZ. Expanded section transitions and inert content

Library, collection and owned/shared playlist sections retain the original
200 ms CSS ease curve, independently transitioning opacity and height, shortened
reversals and synchronized chevrons. Closing panels keep their layout while
immediately leaving pointer input, keyboard dispatch, tab order and accessibility.
The GPUI inert scope also covers deferred popovers and invalidates cached views
when their ancestor's inert state changes.

The actual Mantine Accordion supplies 216 reproducible reference frames. All
five motion/input tests pass on macOS. A broader sidebar run passes 25 tests and
finds an outdated heading assertion from before the Create control moved inside
the heading. Its replacement and strict/platform checks are in progress. The
reference scripts remain independent of physical windows and user input.

### CA. Sidebar artwork presence and initial fade

Artwork keeps its square layout space while entering and exiting over 300 ms,
with the original ease-in-out slide and opacity. Reversals restart from the
current value, and completed exits remove their space. The first mounted image
also retains the original 200 ms CSS fade; changing visibility interrupts that
fade using the original Motion value. Exit content remains interactive until it
is removed, as in the original component.

The actual SidebarImage supplies 432 reproducible frames at two root sizes and
widths, including initial-fade interruption, repeated reversals and completed
returns. Native layout, painted opacity, pointer targeting and presence checks
pass on macOS. The 48 original static sidebar references remain unchanged and
pass. Strict macOS workspace checks pass. Broader sidebar tests are in progress;
Linux and Windows verification and refreshed packages are pending.

### CB. Playlist section separator, action height and RTL spacing

The playlist section now has its original divider and root-font margins.
Headings containing actions grow to fit the action height plus vertical padding,
while plain headings keep their original minimum height. The shared/owned Back
controls and owned heading's right padding now retain their original RTL bounds.

Thirty-six original layouts and matching native checks pass on macOS, covering
widths, root font sizes, English/French/Arabic, expansion choices and folder
navigation. Strict workspace checks pass on macOS. The corrected heading test
checks that actions sit inside the heading. Static artwork checks wait for exit
completion before clicking a heading that a short sidebar's exiting image still
covers; the independent motion tests check input during that exit.

The collection reference now omits absent composition data before its JSON
comparison. This fixes a reference-check serialization mismatch without changing
any recorded input values. The original collection, folder, section, artwork,
static sidebar and heading references reproduce on macOS and Linux. Folder and
section layout snapshots are platform-specific because Chromium's fractional
layout rounding differs by up to 1/128 pixel; motion opacity and rotation match.
The corrected static artwork test passes, completing the focused macOS sidebar
checks across the runs.

The prior BW macOS and Linux suites each finished with 365 passing, one ignored
and two failing tests. Both failures were outdated sign-out/heading assertions and have passing
replacements. The refreshed full suite is running. Native Linux and Windows
checks are pending.

The macOS release includes BX through CB and passes strict ad-hoc signature,
archive/executable, 56-license and installer/settings-preservation checks.
Its SHA-256 is
`22f423eaf78fea6175a5da5762f0e180b838a57a2654f0ce804080b783d9ef03`.
The Linux archive remains BW.

### CC. Cancelling interactions when sections close

An additional regression check exposed a retained mouse press: pressing inside
a section, closing it, releasing and reopening could let a later release fire
the old click. Inert content now clears pending presses, pressed styling, hover
callbacks and visible/pending tooltips. It releases keyboard focus and excludes
key contexts and accessibility action listeners while closed.

The earlier Rust implementation fails the new pending-press check. With the fix,
all five motion/inert tests pass on macOS. They cover cached/uncached controls,
deferred content, text input, tab order, focus, cancellation and both waiting
and visible tooltips. Strict workspace checks pass on macOS/Linux and for every
Windows Rust target; Linux and Windows use Rust 1.91. Native Windows C++
linking/runtime remain unverified. All 27 sidebar checks and the 36-case playlist
heading check pass on macOS and Linux. A fresh full Linux app suite is running.

The refreshed macOS bundle passes strict ad-hoc signature, archive/executable,
56-license and installer/settings-preservation checks. Its SHA-256 is
`fa22ff971e1008c007ced227da291ba4f35b46fc78973fcf3ce246a02328eba4`.
The Linux release passes archive/executable, 56-license and installer checks.
It paints and exits normally on isolated X11 and nested Wayland displays with
expanded and collapsed sidebars. All four images have been inspected. Its SHA-256
is `e4dd5d44ee0dcfa98a005c18b3edfb39368184855b3367bc0ea872923212eaf1`.
The full CB macOS app suite finishes with 375 passing, zero failing and one
ignored test; the subsequent CC changes have the focused checks above. The full
CC Linux app suite finishes with 375 passing, zero failing and one ignored test.

### CD. Packaged library workflow and semantic activation

The new isolated packaged-app workflow exposed two defects. Accessibility
activation used synthetic mouse coordinates, so controls below the scroll
viewport could report success without running their action. Click listeners now
receive semantic keyboard activation directly, preserving explicit accessibility
overrides. A real-app regression verifies that the offscreen remember-credentials
control changes state; it fails before the fix.

Tracing bridges also submitted verbose records beyond the selected log level.
The shared logger now checks the live limit before formatting or writing records.
The packaged startup check rejects trace/debug output with default preferences.
All five focused accessibility checks and strict workspace checks pass on macOS.
The refreshed macOS bundle passes strict ad-hoc signature, archive/executable,
56-license and installer/settings-preservation checks. Its SHA-256 is
`0bba2b6b0d08e5043eb86d387e52d74ebca2cab44611871c251d43eb184b941c`.
All five focused checks also pass on Linux. Strict Linux and Windows Rust-target
checks pass with Rust 1.91. The Linux release passes archive, 56-license and
installer checks, plus the four expanded/collapsed X11/Wayland startup checks.
Its SHA-256 is
`7b8642bdb8141cb8d3bb48640b6c06d883a1f63dfc15d2e7241c140d60bc121b`.

The packaged library workflow passes on X11 and nested Wayland. It dismisses
release notes, disables credential retention, verifies bad-password rejection,
signs in and opens Home, Albums, Tracks and a playlist, then collapses its header
to expose the remaining virtualized rows. It uses private profiles and D-Bus,
dummy localhost credentials and a silent fixture. Both workflows and all four
startup checks verify normal exit and no trace/debug records at the default
log level. The fixture now supplies item details and ownership/count metadata.
The accessibility reader retries only nodes removed during a navigation change.
Inspection found follow-up issues with offscreen focus, detail type translations
and long status-message wrapping.

### CE. Focus visibility, translated detail types and status wrapping

Ordinary focused controls now use the same tracked scroll-container reveal path
as text links. Each keyboard/accessibility focus request reveals once, so later
manual scrolling is not continually reset. The real server-form check covers
offscreen activation, accessibility focus and Tab to the next offscreen button,
using physical bounds adjusted for the platform's display scale.

Detail type labels now use the original singular translation count and distinguish
album artists from artists and tracks from songs. Smart-playlist and album release
type overrides are retained. A rendered playlist check covers English, French and
Arabic; the packaged workflow checks the displayed English label. Long status
messages can shrink and wrap within their row so the dismiss control stays visible.
All five accessibility checks and the rendered English/French/Arabic header check
pass on macOS/Linux. Strict workspace checks pass on both platforms and for all
Windows Rust targets. The macOS bundle passes strict ad-hoc signature, archive,
56-license and installer checks; its SHA-256 is
`873f04dfe2cae2a5180af1a1c9dc1a46f8e815279a272f4d0c2831a5d1247c91`.
The Linux archive also passes package and installer checks; its SHA-256 is
`7753e81432e69a78ec35072af53d48b36c123af087e2e704f9f33169c1fab42b`.
The refreshed X11 library screenshots confirm the translated playlist heading
and wrapped status message with a visible dismiss control. A subsequent silent
playback workflow exposed the Stop behavior corrected below.

### CF. Packaged playback and Stop reset

The packaged library workflow now exercises actual MPV playback through the
production accessibility bridge. Its source is silent, output is explicitly null
and volume is zero; it verifies MPV's selected output in its private log. The
initial run passed play, pause, seek and next track, but exposed a Stop mismatch:
the original `mediaStop()` resets the timestamp, while the native app retained it.

Explicit Stop now resets the music position and cancels held seek previews.
Stopping radio preserves the music queue position. Account sign-out and imported
account replacement stop transport without changing the saved queue position.
The reporting check distinguishes preserved transport resumes from explicit Stop
and restart. Focused Stop, radio, sign-out and reporting checks pass on macOS and
Linux; the sleep-end check also passes on Linux.

The workflow additionally verifies bad-password rejection, sign-in, library
navigation, three-track playlist contents, disabled credential retention and
persisted queue identity. It checks zero progress after Stop, stable stopped
progress and restarting from the beginning. Isolated D-Bus activation now receives
the private display/runtime environment before accessibility services start.
Navigation retries cover both stale-object error forms without repeating actions.

Strict workspace checks pass on macOS/Linux and for all Windows Rust targets.
The refreshed macOS bundle passes strict ad-hoc signature, archive, 56-license
and installer/settings-preservation checks; its SHA-256 is
`3781a166a247cc1985f9da6b91c27fb59aee467b6e23c76bcc425ac60e0fcbc4`.
The Linux package passes archive, license and installer checks; its SHA-256 is
`55d993b9d3c2d5fb03da9d036f211ccc596aabe22c20fa54aa814b0732b27a1a`.
Both packaged playback/library workflows pass on X11 and nested Wayland. All
four expanded/collapsed startup checks pass. The full app runs exposed the link
focus regression below and were stopped so they could restart with its fix.
Physical interactive checks remain deferred.

### CG. One focus reveal for text links and ordinary controls

The complete app runs found that text links revealed twice: their existing focus
logic and CE's shared control logic both scrolled for the same request. The
original offscreen-link regression failed on macOS and Linux with two reveal
callbacks instead of one. Links now use the shared control focus path, removing
the duplicate state and handler. Their explicit keyboard/accessibility activation
still reveals a manually hidden link before using the ordinary click path.

All eleven focused link and accessibility checks pass on macOS, including ordinary
offscreen controls, Tab, semantic activation, link navigation, enclosing click
handlers and manual-scroll preservation. Twelve original keyboard/link reference
cases pass on macOS and Linux. Strict checks pass on macOS and Linux, along with
Windows Rust metadata checks. The Linux package passes archive, license, installer
and all four X11/Wayland expanded/collapsed startup checks; all four images were
inspected. Its SHA-256 is
`6cfb9b37b5978ee67c4fa5e11966e26b941a021482abd05bac94a3068402c6f9`.
The complete app suites pass on macOS and Linux: 377 passed, zero failed, one
ignored per platform. The ignored projectM test has separate integration coverage.

### CH. Localized Home preferences

Home settings use the original catalog labels for the featured section, its
single/multiple style, and all eight configurable section kinds. The carousel and
settings share the same section keys. The existing navigation/order/visibility
regression now checks the rendered English, French and Japanese labels as well
as persistence. It passes on macOS, with strict checks and package verification.
The macOS archive SHA-256 is
`ade46f5b07e33b8c672d70840826b110a2dcfa7233109a60e9940b2e5d58344a`.

### CI. Separate Navidrome password retention

The original eight-form reference exposed a skipped Save Password checkbox in
the native sign-in form. Removing that skip failed against the previous native
implementation. The restored option is off for new accounts and independent of
remembering the session token. Original account imports retain the choice, while
older native profiles preserve their previous password retention behavior.

Session persistence filters the password using a policy shared with background
request clones. Changing the option reports storage failures rather than falsely
acknowledging removal, and the disabled preference ignores a password in an older
stored record. Enabling it after restoring a token-only session asks for sign-in;
an existing in-memory password can be saved without replacing the session token.

The isolated credential-store integration test passes on macOS. It uses an
in-memory keyring in its own process, covering token retention, opt-in/out, old
request clones, failed writes/retry, and Jellyfin/Subsonic behavior. The rendered
form test passes for all eight original forms in English, French and Arabic,
including pointer and keyboard toggles, cancellation, and existing-account save
guards. The original 20 authentication cases and eight form references reproduce.
The full macOS core run passes 275 unit tests plus the isolated credential-store
integration test. Four related account UI checks pass, including restore/retry,
logout/restart, live TLS changes and Quick Connect. Strict macOS checks pass.
The refreshed macOS package passes signature, archive, 56-license and installer
checks; its SHA-256 is
`60429eadeb2dddbc84c0b9ad815c0204ca5377a11b5063c474423e93b8468599`.
Linux passes 276 core unit tests and the isolated credential-store integration
test. Its focused run also passes the form, Home label and four account UI checks.
Strict Linux and Windows Rust checks pass. A refreshed Linux package follows CJ.

### CJ. Logical and percentage CSS spacing

The original CSS uses inline/block padding and automatic inline margins. Native
custom styles previously ignored those declarations and rejected percentage
padding and `auto` in margin shorthands. The original-reference regression first
failed on `margin: auto`, where the previous native style retained one-pixel
margins. The adapter now accepts the physical and logical spacing shorthands and
longhands, preserves automatic and percentage lengths for native layout, and
maps inline start/end using the computed direction. The native style context also
exposes the selected language's `dir` attribute, with author CSS taking precedence.

The pinned Electron reference contains 424 cases covering physical/logical edges,
automatic centering, percentages, font-relative units, invalid declarations,
variables, resets, source order, `!important`, CSS direction and `dir` attributes.
An expanded negative check caught a malformed mixed value such as `initial 3px`
partially changing one edge; those declarations are now rejected as a whole.
The native regression checks both computed edges and actual outer/content bounds
at 100% and 150% zoom. Twenty-four surrounding CSS, layout and localization tests
pass on macOS and Linux. Strict macOS, Linux and Windows Rust checks pass for the
initial spacing change. The expanded malformed-value regression and strict checks
also pass on macOS and Linux, including Windows Rust checks. The macOS package
passes signature, archive, 56-license and installer checks; its SHA-256 is
`9aedefc69bc6a15198cd812f037fbcaf056eaf5332effa2c840922695ed8c420`.
The Linux archive passes archive, license and installer checks; its SHA-256 is
`7ee55d5a1517e863140191d201699d6c1f1c5dcc2ac2d40a18931ebd339801f3`.
Both final Linux display startup checks pass. The preceding spacing build also
passed both expanded/collapsed startup variants and the full silent library
workflow on X11 and Wayland, including the restored Save Password option. The
Home, stopped-playlist and collapsed-server screenshots were inspected.

### CK. Inherited physical and logical spacing

An added original-browser case failed because native `margin: inherit` retained
the child's previous margins. Spacing inheritance now resolves the parent's
computed style through the existing cache and copies its physical edges before
the child's zoom is applied. Logical declarations select physical edges using
the child's direction, matching the browser even when the parent has the opposite
direction. Parent font-relative sizes and percentage lengths remain intact.

The reference now has 488 cases. The complete computed-edge and rendered-bound
regression passes on macOS at both zoom levels, along with twelve surrounding CSS,
modal and live settings checks and strict checks. The pinned macOS/Linux browser
runtimes reproduce identical computed edge values; sixteen inherited-percentage
rectangles differ by exactly 1/128 pixel. The reference check permits that measured
geometry difference while keeping all computed values exact. The twelve native
checks also pass on Linux; strict macOS, Linux and Windows checks pass. Refreshed
macOS and Linux archives pass license, integrity and updater checks, and Linux
startup passes on X11 and Wayland. SHA-256: macOS
`3544487c7762a0fa9bec04d8aa0ec40030107d78a5799721ee29a7c2bb3332eb`,
Linux `468753fe344d333ee205bfa6049832c90c07492bdf3aca10f6484fa61c9fc644`.


### CL. Desktop media controls and metadata

The original Linux bridge exposes writable shuffle/repeat and richer track
metadata. Souvlaki's zbus backend omitted these properties; the packaged
regression failed on the previous native build with `Unknown property 'Shuffle'`.
The vendored extension now publishes and accepts those states, supplies unique
queue-entry track IDs, ignores stale or invalid SetPosition requests, emits Seeked
on committed seeks and avoids redundant metadata/status change signals.

The app publishes cached cover files without authentication URLs and honors the
existing explicit-artwork preference. Metadata includes separate artists, album
artists, genres, comments, dates, BPM, ratings, counts, disc/track numbers and
formatted lyrics. Empty queues clear old metadata. Updates also run while paused,
so downloaded artwork and changes made in the UI reach desktop controls. Windows
now clears previous metadata and decodes artwork file URLs before opening files.
The BPM field uses the standard `xesam:audioBPM` spelling rather than the original
bridge's `xesam:audioBpm` typo. Repeat and shuffle setters apply the requested value
idempotently; repeated requests do not toggle the state back.

The original bridge reference covers four metadata cases, nine rounded/clamped
volume cases, three repeat mappings and three seek conversions. Native metadata
and command regressions pass on macOS and Linux; strict macOS/Linux and Windows
Rust checks pass. Three surrounding radio/seek/scrobble regressions pass on macOS.
The refreshed macOS archive passes signing, 57-license, integrity and updater
checks (SHA-256
`80810f3f89a521a075300d66cd54d5834a2fd4f0b922515836b4d7cdf7d004aa`).
The Linux archive passes the same integrity, 57-license and updater checks
(SHA-256 `9bea6588105e6fd7ecf044a8f6c0a5a497c1d266ea8ffa32c2c4a0981af67075`).
Both packaged X11 and Wayland workflows pass actual D-Bus reads/writes for all
metadata fields, cached artwork, lyrics, shuffle/repeat idempotence and queue
identity, volume rounding/clamping, play/pause/stop, relative/absolute seeking,
invalid/stale seek rejection, Seeked notifications and empty-queue metadata.
The rich server fixture uses Navidrome's normalized date, playDate and rating
fields; initial runs exposed and corrected fixture-shape mistakes.


### CM. Configured media seeks and callback lifecycle

A new original-browser reference reproduced eighteen configured, default and
explicit seek offsets plus radio guards. The native command regression first
failed at 40 seconds where the configured forward step required 47. Undefined
media seek amounts now use the configured forward/backward intervals, with the
original five-second zero-value fallback. Twelve default/configured/explicit
command cases pass through the app on macOS and Linux, alongside the existing
metadata, volume and queue-state cases.

Windows now retains and removes the seek callback token as well as the button
token, and reattachment removes old callbacks first. macOS registers Stop,
removes its handlers on detach and reads the public positionTime property instead
of a private instance variable. Invalid durations are ignored. The Linux service
uses the original `org.mpris.MediaPlayer2.Feishin` identifier again.

Strict macOS/Linux checks and all Windows Rust targets pass. The refreshed
macOS archive passes signing, integrity, 57-license and updater checks (SHA-256
`86de8435f298f1cc33b9a64e4007b2d09c1e2401243e54035a4c441cbf6a275b`).
The refreshed Linux package (SHA-256
`e93a8b01327e9bfb203f2d09c0dec14c615dafa7c9cd6c32e115b20c05637217`)
passes archive/updater checks, the complete X11
library and desktop-bus workflow, and Wayland startup. Physical macOS/Windows
media controls remain deferred with the rest of the interactive checklist.

### CN. CSS translation, scale and transform origins

Native custom styles now support ordered translation and uniform scaling, pixel,
percentage and font-relative offsets, transform origins, resets and explicit
inheritance. The shared GPUI paint scope moves descendants, clipping, click targets
and accessibility bounds without moving surrounding layout. Images, SVGs, canvases
and lists use the same style path; a direct-artwork regression first demonstrated
that the previous Div-only implementation ignored image transforms.

The pinned original browser supplies 496 cases on both macOS and Linux. Checks at
100% and 150% zoom cover computed geometry, painted bounds, accessibility and clicks.
Expanded cases exposed and fixed uppercase origin keywords, reversed invalid
length/keyword origins, percentage scales and invalid percentage Z coordinates.
Malformed declarations preserve earlier valid values; numeric zero lengths retain
decimal/exponent forms. Zero and very small scales are supported. Empty transformed
hitboxes cannot capture a click at their origin, and zero-scale images, SVGs,
canvases and text produce no painted primitives.

Both native platforms pass the transform regressions and surrounding play-control,
foreground-filter, modal and spacing checks. The shared-element change also passed
page/keyboard, artist-table and popup regressions on both platforms. Strict macOS,
Linux and Windows Rust checks pass. Rotation, skew, negative/nonuniform scaling,
perspective and SVG filter effects remain implementation differences; they are not
covered by the uniform-transform claim.

The refreshed macOS archive passes signing, integrity, 57-license and updater
checks (SHA-256 `413d49abaec89fc02da05219c9c8132058299b986da15c304958fb1fda60e647`).
The Linux archive passes integrity, 57-license and updater checks (SHA-256
`47afaaf491670f8e4fe76fd08b70aa9630d69c842a136b7ce5a1ce5f9a44797b`), the complete
isolated X11 library/playback/MPRIS workflow, and Wayland startup. The stopped
playlist and Wayland startup captures were inspected.

### CO. Localized shortcut settings and accessible popup activation

Shortcut settings now reuse all 42 original action captions across 39 locales,
the original global-shortcut description and translated edit/clear/reset labels.
The shared settings search filters translated captions; its local-search shortcut
focuses the input. Global checkboxes expose checked/disabled state, clearing resets
both key and global scope, and moving focus out of recording cancels capture.
An independent reference executes the original caption map with its i18next
catalogs. Core checks validate all 1,638 captions; rendered checks exercise
English, French, Japanese and Arabic, editing, global scope, clearing, focus
cancellation and saved state.

The packaged workflow exposed a separate shared popup gap: the application Menu
button responded to mouse presses but exposed no accessibility activation.
A headless regression reproduced that failure. Popover triggers now expose
activation and expanded state, support Enter/Space and retain their existing
mouse-press path without toggling again on release. The menu regression checks
screen-reader activation, keyboard opening, mouse selection, navigation and
focus. Seven focused menu/popover/accessibility/shortcut checks pass on macOS
and Linux, alongside the four surrounding keyboard/language/settings checks.

Strict macOS/Linux and Windows Rust checks pass. The macOS archive passed
signing, integrity, 57-license and updater checks (SHA-256
`73a217c66c770530435968aeda1f43b9f3532c330951da86d8a671cc98ec6726`).
The Linux archive passed integrity, 57-license and updater checks (SHA-256
`d4b7eae4a36eb0c04b9f985b4a2e7ef9478ad35913e17bcbcf2918b58d645c93`).
The isolated packaged shortcut workflow passes and its capture was inspected.
The combined library workflow exposed a narrow Settings header with an unusable
search field when playback panels remain open; the following layout work addresses
that separately. The workflow distinguishes the Settings menu item from the
player button through AT-SPI's numeric role API.

### CP. Imported arrow and punctuation shortcut names

The original recorder saves arrow keys as `arrowdown`, `arrowleft`, `arrowright`
and `arrowup`, and punctuation as `equal` and `minus`. GPUI emits different names.
A rendered import/dispatch regression first failed on the original `mod+arrowdown`
binding. Import now translates these six names while retaining modifiers, action
and global scope. A reference executes the original keyboard-code converter.
Twelve primary-modifier and Alt combinations dispatch native actions and parse
for global registration on macOS and Linux without registering OS shortcuts.
The existing global-scope import check now uses the actual original arrow spelling.

All fourteen migration regressions and strict macOS/Linux/Windows Rust checks
pass. The macOS archive passed signing, integrity, 57-license and updater checks
(SHA-256 `cea9df32c414ec104db2045f3d7c3b0eddc820df286bb9efc9dba3bcb8f51e8e`).
Shifted punctuation, keyboard-layout independence and separate numeric-keypad
bindings remain in the keyboard parity audit. An additional regression reproduced
the shifted-Equal mismatch; the original stores a physical key plus Shift while
GPUI emits the resulting character.


### CQ. Settings search with narrow content

The combined packaged library workflow leaves queue, lyrics and visualizer panels
open, reducing Settings to about 280 pixels. The fixed toolbar previously squeezed
the search field until its text area and accessible text interface disappeared.
Settings and shortcut headers now wrap their controls and retain a 160-pixel minimum
search width. The existing search/reset/persistence regression also checks narrow
280- and 420-pixel windows. Focused settings and shortcut checks pass on macOS and
Linux; strict macOS/Linux and all Windows Rust checks pass.

The macOS archive passes signing, integrity, 57-license and updater checks
(SHA-256 `5317a5cea125728ddc523cb56a26d0149adbb23747fad1bbda16191dca0ebad5`).
The Linux archive passes integrity, 57-license and updater checks
(SHA-256 `151a8a635f4e7baf1950c62e040571ddf6948678b57641cbac278cf98e3130b1`).
Complete isolated X11 and Wayland workflows pass library sign-in, browsing,
playlist selection, silent MPV transport/restart, persisted queue, MPRIS and
shortcut search, recording, focus cancellation and clearing. Both final shortcut
captures were inspected with the playback panels still open.


### CR. Physical keyboard positions and original modifiers

New native recordings, default bindings and Electron imports now match physical
key positions. Platform adapters retain modifiers before shifted punctuation is
normalized, distinguish keypad keys from main keys and preserve positions across
keyboard layouts. Unknown hardware positions remain unidentified. Older native
bindings without the new flag retain their existing character matching, and menu
labels retain readable keys.

The original recorder supplies 102 supported key names. Tests cover all names
with Control and Control+Shift, all shifted ANSI punctuation, independent scan-code
positions for macOS/Windows/XKB, saved recording, layout changes, keypad isolation
and unsupported keys. Eighteen imported primary/Alt/Shift combinations dispatch
through the app. All four shortcut-setting checks pass on macOS and Linux, as do
all 276/277 core tests and strict macOS/Linux/Windows Rust checks. A packaged
regression first reproduced the old Shift+Equal recording as `ctrl-+`.

The macOS archive passes signing, integrity, 57-license and updater checks
(SHA-256 `eebc22292cc89f31d5978ea994b6389fc9dcba320b2551ffb5d52d229ffc6ce8`).
The Linux archive passes integrity, 57-license and updater checks (SHA-256
`b334d8e77f8ef29dd147f4078c062eab655e82927248b637def28479dae4c6b2`).
Actual isolated X11 and Wayland workflows pass shifted punctuation, distinct
numeric-keypad keys, saved recording and action dispatch. X11 also switches its
private display to a German layout and verifies physical Z independently from
its typed Y. Both captures were inspected. Complete interface suites each finished with 384
passes, one failure and one deferred test. macOS exposed an overlapping popup in
the compact French Settings flow. Linux's ruby-spacing check hit a software-Vulkan
pixel-read deadline while other graphics work was active; it passes when rerun
alone. Both failures are being addressed before final regression conclusions.


### CS. Mixed percentage and pixel transform calculations

Transform offsets and origins now retain affine percentages through CSS calc
arithmetic, including nested expressions, font-relative values and scalar
multiplication/division. Invalid dimensions, nonfinite values and percentage
products remain rejected. Absolute-only CSS properties retain their strict
length parsing.

The original browser reference expanded from 496 to 760 cases and reproduces
on macOS and Linux. Rendered transform checks pass on both platforms at two zoom
levels, including images, SVGs and canvases. macOS also passes the full 276 core
tests, password-retention integration, surrounding spacing/font tests and strict
checks. Linux's focused core/style checks pass; final broad checks and Linux
packaging follow the popup correction. The macOS archive passes signing,
integrity, 57-license and updater checks (SHA-256
`804b19fcf62df608076b1eee48b374ab3de0a34b0f020952fa6f6bcf887869b1`).


### CT. Nested popovers above scrolling Settings

The complete macOS run exposed a clipped Settings player popup. Its upper controls
could receive clicks through the scrolling page, moving focus outside the popup
and leaving it open after Escape. The next click then opened a device menu over
the French Crossfade button. A tighter dismissal assertion reproduced the earlier
focus failure.

GPUI now processes nested deferred overlays while preserving view identity,
focus ancestry, drawing order and cached subtree ranges. Player and output
configuration use that path, so their contents and nested menus escape page
clipping. Rendered checks pass on macOS and Linux for compact English/French/Arabic
Settings, nested menu dismissal, player settings persistence, output selection
and speaker property editing. Cached and uncached nested controls also retain
correct inert behavior, input handling and accessibility. Strict macOS/Linux and
all Windows Rust checks pass. Complete interface suites pass on macOS (911.80 s)
and Linux (995.26 s), each with 385 passes, zero failures and one excluded
OpenGL smoke test. Both earlier failures pass in these complete runs.


### CU. Calculated layout sizes and the Glassy player

The original Glassy theme uses `height: calc(100% - 180px)` for its expanded
player content. Native custom CSS previously dropped mixed pixel/percentage
calculations and omitted that responsive-container alias. Independent browser
regressions reproduced a 120-versus-190-pixel width and an 836-versus-720-pixel
Glassy player height.

GPUI now retains affine lengths until the containing size is available. Its Taffy
adapter resolves the same terms for containers and measured leaves, clamps
nonnegative properties and preserves signed margins/offsets. CSS zoom scales
absolute terms; flex shorthands retain nested calc expressions. The expanded
player exposes the original responsive-container selector.

All 564 browser spacing/sizing cases pass on macOS at two zoom levels, covering
min/max sizes, flex sizing, physical/logical edges and negative results. Sixteen
Glassy layouts also pass alongside the original layout/motion cases. Surrounding
font, generated-surface and native CSS checks and strict macOS checks pass. All
44 native style tests pass. The broad run caught a `0%` versus `0px` flex-basis
distinction; percentage units now remain intact even when their value is zero.
Linux also passes all 44 style tests (174.06 s), all six expanded-player layout
and motion checks, the independent 564-case browser spacing comparison and the
16 Glassy layout references. Strict Linux and all Windows Rust checks pass.

The refreshed macOS archive passes signing, integrity, 57-license and updater
checks (SHA-256 `5e293a0e2365017e036f9e2798ab856345befbc529fe4962793a08e25c6d1c6b`).
The Taffy library's 90 unit tests also pass with calculated lengths enabled.

### CV. Duplicate shortcut recording and dispatch

The original recorder accepts duplicate shortcuts, outlines them in red and runs
each matching local action. Native validation previously rejected duplicates and
blocked unrelated edits when imported settings contained them. A rendered
regression also reproduced the second local action being shadowed.

Duplicate keys now remain editable and display the original red dashed warning.
The existing GPUI key matcher supplies all active actions, retaining physical-key
matching, input exclusions and component precedence. Duplicate local actions run
once each; menu commands retain their individual behavior. Global shortcuts
retain one registration per key. An independent check executes the original
recorder, warning calculation and pinned Mantine hook, including all four input
exclusions. All ten shortcut checks and strict macOS checks pass, including
duplicate import, recording, unrelated edits, persistence, input exclusion,
focused-list dispatch and individual menu actions. Linux passes the same ten
checks, the original recorder/hook reference and strict Linux/Windows Rust checks.
The complete macOS interface suite passes with 386 passes, zero failures and one
excluded offscreen OpenGL smoke test (917.28 s). This snapshot includes CU/CV;
later artwork, viewport and scoping changes have separate checks below.

### CW. Expanded-player artwork theme styling

The full-player image bypassed the original image selectors, so Glassy retained
five-pixel corners instead of eighteen. A regression against the production
image scene reproduced the mismatch. Original module and public image aliases
now reach the image, including its explicit-content class. Custom filters replace
the default shadow; custom opacity applies after filtering. Production image
scene checks pass on macOS and Linux for original and Glassy corners, custom
opacity and filter replacement, alongside all six layout/motion checks. Strict
macOS/Linux and Windows Rust checks pass.

The macOS archive passes signing, integrity, 57-license and updater checks
(SHA-256 `5459698f7efda3d64dacd3020b66c242d3ea7fc6153827d06690802bf8bc0d91`).
The Linux archive passes integrity, 57-license and updater checks (SHA-256
`b1c174e2530aefe37af72d2860ce1bd5fc27862b2594fa0c9a45cb360b94bc57`).
Its isolated X11 library run passes login, browsing, null-output playback, desktop
media controls and shortcut checks, including duplicate dispatch. The new player
settings check selected a panel's identically named Configure button; its locator
now selects the Settings action, and the packaged workflow is being rerun.

### CX. Viewport lengths and actual expanded-player placement

The full application exposed a 990-versus-1080-pixel Glassy player height that
the isolated layout probe could not catch. The expanded container now receives
its original selector, and the shared CSS parser resolves viewport-length tokens
after variable substitution. The parser preserves quoted strings, URLs, other
units and percentage terms, and shares its token walker with font-relative
lengths. Native viewport values follow content size and zoom.

Expanded-player controls expose their original container and group selectors,
including Glassy's 120-pixel bottom offset and circular group buttons. The actual
image/control scene passes, including live resizing. All 632 browser spacing and
sizing cases pass in the 44-test macOS style suite, alongside all eight expanded
player checks, all 276 core unit tests and strict workspace checks. Linux passes
all 277 core unit tests, all 44 style tests, the actual full-player scene and all
eight expanded-player checks. The independent 632-case browser reference and
strict Linux/Windows Rust checks also pass.

### CY. Expanded-player CSS stays inside its subtree

The original player selector belongs to its container. Native rendering also
attached it to the application body while expanded, leaking inherited colors and
descendant rules to unrelated controls. A regression reproduced a red application
body from a rule targeting only the expanded player.

The player now shares the existing dialog subtree scoping mechanism. Its ancestor
context is present during construction, layout, prepaint and paint, including lazy
controls and canvas artwork, and is restored before siblings render. Repeated
references to the same named ancestor no longer apply inherited font sizes twice.
Modal portals retain their body-root ancestry. macOS passes the original leak
regression, actual expanded-player artwork/control scenes, all five modal-related
checks and strict workspace checks. Linux passes the same scoping check and all
five modal checks, with strict Linux and Windows Rust checks.

### CZ. Original expanded-player header controls

The original header contains three 36-pixel controls, eight-pixel padding and
eight-pixel gaps at the default font size. Native rendering added a fourth menu
button and used different button dimensions, moving Minimize from x=8 to x=70.
The original Group, ActionIcon, settings button and shared-player components now
supply sixteen browser cases covering two font sizes, two zoom levels, RTL and
Glassy styling. Coordinates are relative to the player container.

The header uses the original three actions, dimensions, icon sizes and translated
tooltips. Header and tab controls share the existing button construction and
original aliases; Glassy circles reach both groups. All sixteen rendered native
header cases pass on macOS and Linux, including physical-pixel rounding of circular
corners. macOS passes the complete interface suite with 387 passes, zero failures
and one excluded offscreen OpenGL smoke test (942.33 s). This snapshot includes
CX/CY/CZ and precedes the Settings navigation changes below. Linux passes the same
387 tests with zero failures and the same excluded smoke test (1023.20 s).

The macOS archive passes signing, archive/executable integrity, 57-license and
updater checks (SHA-256
`eec3e63c3da69bf7b7c2dd5d19a7ea4ece5f7d1530ef72246dacb4a5f548d1c1`).
The corresponding Linux archive passes integrity, 57-license and updater checks
(SHA-256 `5fb06a895312cd6a6392c77f35a12aac83b87e099d9eb6580603da2b48a99d54`).
That packaged Linux snapshot also passes the full isolated library workflow on
X11 and Wayland: login, navigation, silent playback, persisted queue, MPRIS,
physical keyboard shortcuts and nested player configuration. The X11 fixture now
allows clicking the same screen position twice without waiting for nonexistent
pointer movement.

### DA. Settings categories and persisted navigation

Whole-page references now run the original web build against the silent disposable
library in an unparented Electron view. They cover Home, albums, tracks, a playlist
and Settings without external accounts or native windows. Priming the hidden view
before capture lets its actual entry animation finish; all five pages report full
opacity and no browser console errors.

The comparison exposed the native Settings page's flat list. It now has the
original General, Playback, Hotkeys, Window and Advanced tabs, with the original
section order, localized labels, active-category search and retained search text.
The selected tab is saved and imported from Electron preferences. Keyboard arrows
respect text direction, Home/End select the endpoints and accessibility exposes
the selected tab. The original app independently confirms scoped search, retained
queries and persisted tab selection. Core preference/import checks and the native
category, keyboard and accessibility regression pass on macOS. All 277 core tests
pass. The 33 affected settings/control regressions pass after updating two tests
to select Advanced for Restart and Playback for equalizer input; both cache
regressions also pass. Tabs wrap at narrow widths, as in Mantine, and strict macOS
checks pass. Linux passes all 278 core tests and all 42 combined Settings/search
regressions, followed by strict Linux and Windows Rust checks.

### DB. Compact Settings search lifecycle

The original Settings search starts at 36 pixels and expands to 200 pixels on
activation. Native Settings used a permanently expanded field. Settings now
shares the existing compact list-search control, including its transition,
clear button, focus handling and Escape behavior. Queries survive category
changes and blur; an empty field collapses after blur, and the local search
shortcut opens and focuses it. Category clicks explicitly move keyboard focus.

The original full-app reference independently checks widths, focus, retained
queries and Escape on macOS and Linux. The native regression first reproduced
the old 688-pixel field, then passed with the corrected behavior. All nine
affected search, category, reset and shared-control checks pass on macOS
(39.69 s), followed by strict workspace checks (19.33 s). Mock-window activation
and two transition phases ensure blur is delivered before measuring the closed
control. Linux passes all 42 combined Settings/search checks (184.52 s), followed
by strict Linux and Windows Rust checks. Windows native linking/runtime remains
outside these cross-compilation checks.

### DC. Original default list layouts and album credits

The full-app album comparison exposed missing album-artist credits. Native list
defaults used generic `name/artists/year` rows for all grids, while the original
album grid uses `name/albumArtists/releaseYear`. The native defaults now load all
twelve original list configurations from the existing independently generated
preference fixture. This also restores original table columns, widths, borders,
grid spacing and the default playlist table view.
Tracks also retain the original paginated default; explicit infinite-scroll
tests select that mode before testing multi-page history restoration.

Artist/genre sublists and the full-player queue inherit the corresponding original
album, song and full-screen defaults. Saved shared layouts are inherited when no
explicit per-page override exists; explicit saved rows retain priority. A core
regression reproduced the wrong album rows before the fix and now checks every
original configuration. All 278 macOS and 279 Linux core tests pass. The rendered
album-card regression passes on both platforms, verifying accessible artist credits,
navigation, unchanged queue and a saved override hiding credits. Strict macOS,
Linux and Windows Rust checks pass. Complete interface and package workflows are
in progress; the wider run identified tests still targeting formerly visible
plain-title cells after the original combined-title default was restored.

The macOS package containing DA/DB/DC passes signing, archive/executable and
57-license verification, plus updater rejection, replacement and preservation
checks (SHA-256
`9c91c22119066c9cd172b880cd2bd5862b9ee0cf5ed01ab32709d0b5a613f61d`).
The Linux archive passes integrity, 57-license and updater checks (SHA-256
`0a63b9409ec16d1b7e73148c2786e7cdc78ffaede9a5951408c6385a6f84296f`).
Its isolated X11 workflow reaches browsing, silent playback, MPRIS and all five
Settings tabs. The search fixture still expected the old always-visible input;
it now opens compact search with the local-search shortcut before typing. The
album capture also exposed a painted-text truncation issue that accessibility
alone did not catch. Painted artist credits are under investigation below.

### DD. Text measurement keeps the final available width

The packaged album credits exposed a shared text-layout cache problem. An
intrinsic measurement could truncate a no-wrap label to an ellipsis, then the
final measurement reused that result despite receiving a wider space. GPUI now
requires both wrapping and truncation widths to match before reusing measured
text. This applies to ordinary metadata and linked artist text.

The album-card regression now uses the platform font system and waits for page
entry before checking painted glyphs. It reproduced one ellipsis glyph with the
old cache, then passed with the full credit, navigation, preserved queue and saved
row overrides (macOS, 7.94 s). Cross-platform rendering, broader text regressions
and refreshed packages are in progress.
All eleven affected default-layout, credit, navigation, playback and pinned-filter
checks pass together on macOS (89.80 s), followed by strict workspace checks
(15.60 s). The earlier full DC run completed with 381 passes and nine failures
from assumptions about old default columns, hidden inline actions, queue headers
and pagination. Those nine checks are included in the corrected eleven-test run.
The full DD/DE macOS run completed with 391 passes, zero failures and one excluded offscreen
OpenGL smoke test (938.71 s).
The corresponding older Linux DC suite completed with the same 381 passes,
nine stale-fixture failures and one excluded smoke test (1092.81 s). The latest
Linux painted-credit, navigation-restoration and pinned-search checks all pass
(three checks, 23.99 s).

### DE. Search choices in pinned filter panels

Pinned filter panels displayed their controls, but choice loading returned early
unless the full filter page was active. Opening or searching a pinned panel now
uses the existing debounced request path, with the same stale-result checks.
Closed panels continue to reject new searches. The scheduling regression
reproduced the missing initial request before the one-condition fix, then passed
for opening, search changes and unpinning on macOS. Protocol fixtures remain
independent of this navigation regression.

The refreshed macOS package includes DD/DE and passes signing, archive/executable
integrity, 57-license verification and updater checks (SHA-256
`6e1f5985947f0128889c67b7c469d29939b0718170de795866cf2351e8ed6c62`).

### DF. Translated scrobble accessibility label

The elapsed scrobble control referenced a nonexistent `player.scrobble` key,
exposing that key to accessibility. It now uses the existing translated
`setting.scrobble` label. The original-state, locale and manual-control regression
passes on macOS (7.69 s). Strict Linux and Windows Rust checks pass; full Linux
interface and package workflows include this label correction.
The Linux archive passes integrity, 57-license and updater checks (SHA-256
`06247deec374e0935f23e07fb61d651e143f9468bb31326d3b855b5655a6eab1`).
Its complete X11 and Wayland library, silent-playback, MPRIS, shortcut and Settings
workflows pass. The actual packaged album capture now paints full artist credits.
This snapshot predates DG.

### DG. Original filter visibility and live editing

The independent whole-app reference now exercises the original album filters. It
confirms the initially open 300-pixel panel, immediately applied favorite choices,
Reset and persisted unpinning. The original album and track defaults are retained
in a native page-sidebar preference map, with explicit values imported from app
preferences and native exports. The core import/default regression passes on macOS.

Native filter changes now save as they are made, with a 300-ms delay for year input
and validation before replacing active filters. Closing the full filter page
retains those changes; pinned panels refresh the current list without an Apply
step. The panel moves to the leading side at the original width and hides below
the original container breakpoint without changing its saved visibility. Pending
year edits cancel on navigation, and unpinning cancels choice requests. Rendered
interaction, cross-platform and refreshed package checks are in progress.
All 279 macOS core tests pass (8.75 s). The native pinned-filter regression passes
real mouse interaction, immediate changes, year debounce, Reset, width/placement,
narrow-window visibility and persistence (8.55 s). Thirteen surrounding filter,
list, collection and style checks pass. Full interface validation is running.
Strict macOS workspace checks pass (27.59 s). The original reference also checks
both sides of the 36em filter-panel breakpoint at 16- and 20-pixel font sizes.
The native check follows those font sizes, keeps Favorites and artist overviews
separate from list-sidebars, and preserves explicit per-page choices. Editing
filters from a later result page clears that return position, so closing filters
starts with the first matching page. The three filter/navigation checks pass
together (24.37 s), followed by strict workspace checks (17.02 s). Formatting and
Python fixture checks pass. A refreshed macOS package is building.
The updated macOS archive passes signing, executable/archive integrity,
57-license verification and updater checks (SHA-256
`67b20b8a7bf33489dbe2cc5473b198e6f869b5bda03ab7f5deca479988a09d9f`).

### DH. Bounded software-GPU pixel probes

The full DF Linux suite completed with 390 passes and one pixel-read timeout in
ruby lyric spacing (1143.76 s). The unchanged pixel check passed four isolated
runs. Software Vulkan shares CPU time with parallel tests and compilation, so
test-only pixel readback now allows 60 seconds; the production frame deadline
stays at 10 seconds. The updated probe and three filter/navigation checks pass
together on Linux (26.22 s), as do all 280 core tests and strict Linux and Windows
Rust checks.

### DI. Stable search and lyric fixtures with default filters

The full DG macOS suite exposed two fixture problems (389 passes, two failures,
938.54 s). The lyric fixture's accepted socket could inherit nonblocking mode and
return before the request arrived; it now explicitly reads in blocking mode.
The search fixture counted the newly enabled filter-choice fetch as a song search;
its local debounce assertions now count song requests separately. Global search
uses GPUI's executor timer with the same 200-ms production delay, allowing tests
to verify the boundary directly. All five affected lyric, search, filter and
navigation checks pass together on macOS (27.13 s), followed by strict checks
(11.81 s).

The Linux whole-app Electron reference failed while copying large rendered pages
with its 64-MiB shared-memory allocation. Using Chromium's disk-backed memory on
Linux lets all five original pages, filter interactions, responsive breakpoints
and Settings checks complete. Native rendering does not use this switch.
Refreshed full suites and packaged workflows are in progress.
The DI Linux lyric/search/filter/navigation/pixel checks all pass (six checks,
33.72 s), followed by strict Linux and Windows Rust checks. The DI macOS archive
passes signing, executable/archive integrity, 57-license verification and updater
checks (SHA-256
`4d7b5039b29ad38844bf1bcb9c24200c819d4577defcddda9735c1424c023634`).

### DJ. Removing custom filters applies immediately

Clearing an entire custom-tag filter removed the displayed choice but skipped the
live commit path. The shared removal handler now saves and applies the remaining
filters. A mouse regression reproduced the stale active filter before the fix,
then passed on macOS (14.55 s) and Linux (15.88 s), preserving the favorite filter
and checking the saved view. Strict macOS workspace checks pass (11.79 s).
Strict DJ Linux and Windows Rust checks pass. The refreshed DJ macOS archive
passes signing, integrity, 57-license and updater checks (SHA-256
`01cb6e3e24ca1f4d1a58006ed43c1b2d9798cfd49a633b5f4023ecce2a053ff2`).

### DK. Complete filter choice paging and accurate reference requests

Filter choices previously stopped at the first 100 entries. Artists, genres,
libraries and custom tag values now expose further pages through Load more.
Changing the search or choice category starts at the first page and preserves
selected filters. Pending requests retain the existing generation checks; failed
additional pages preserve already loaded choices for retry, and empty results
stop paging even when a server reports an incorrect total. Navidrome tag requests
retain library scope and send the requested offset; Jellyfin pages its cached
catalog. The core tag paging regression passes on macOS, as does the native
artist/tag paging, accessible activation, selection and search regression
(12.19 s). The expanded cached-catalog and surrounding checks are recorded below.
Filter year inputs now have distinct translated From year and To year names;
search, tags, selection mode and empty/loading labels reuse the original catalogs.

The disposable reference server also incorrectly treated `_end=0` as an empty
page. Navidrome's [REST controller](https://github.com/deluan/rest/blob/master/controller.go)
computes a nonnegative maximum from the range, and its
[repository](https://github.com/navidrome/navidrome/blob/master/persistence/sql_base_repository.go)
only applies a positive limit. The fixture now follows those rules, including
negative offsets. Four HTTP checks pass. All five original pages and the original
filter/Settings interactions pass on macOS with an added assertion that artist
choices actually appear. This corrects reference evidence rather than changing
native server behavior.
The expanded native paging regression also passes with Jellyfin's cached tag
catalog. All three affected paging, pinning and filter/history checks pass
together on macOS (15.27 s), followed by strict workspace checks (14.69 s).
Formatting and Python fixture syntax checks pass.
All 280 macOS core tests pass (16.82 s). The final filter controls also disable
old choices while a new search/category is loading, preventing a value from the
previous category being selected. All three affected native checks pass again
(17.22 s), followed by strict workspace checks (14.88 s).
The refreshed DK macOS archive passes signing, executable/archive integrity,
57-license verification and updater checks (SHA-256
`8715ead9600b05dad84bc43bf2ac7e8f8d1747320343a6c9aa0b367fcc667077`).

### DL. Packaged keyboard fixture waits for focus

The X11 library workflow could type before its asynchronous accessibility focus
request had reached the field. It now waits for the focused state, using the same
check as the existing accessibility fixture, and waits for unmasked text to be
reported before continuing. Passwords retain their masked accessibility value.
The subsequent run reaches sign-in, browsing, live filters, pinning and silent
playback. A missing artwork file in the separate temporary harness caused its
media metadata check to fail; the complete fixture has been restored and fresh
X11/Wayland workflows continued as recorded below. No authentication or playback code was changed
for these fixture failures.
The full DI macOS interface suite completed with 391 passes, zero failures and
one explicitly excluded projectM smoke test (974.15 s). That snapshot predates
DJ/DK; the later affected checks are recorded separately above.

### DM. Pinned filters retain the original grid width

The actual packaged album image exposed another layout difference: the native
page's outer horizontal padding left only two columns beside the pinned panel,
while the original shows three at a 1320-pixel window width. Pinned content now
uses the full available width while the page heading keeps its own padding.
A native regression reproduced two columns before the change, then passed with
three. The independent whole-app original reference now checks its actual
`--columns` value as well. All four affected filter, paging and navigation checks
pass on macOS (23.89 s), followed by strict workspace checks (13.71 s). All five
original pages and the filter/Settings reference interactions also pass.
The complete fixture passes packaged MPRIS metadata/artwork, transport, seeking,
volume, shuffle/repeat and stale-position checks. The subsequent Settings pointer
check failed after shortcut-driven zoom. A focused run passes its nested menus
and Escape behavior with stable bounds. The full workflow later exposed that
stable bounds alone are insufficient when those bounds are clipped; DN records
the corrected fixture setup.

The DM macOS archive passes signing, integrity, 57-license verification and
updater checks (SHA-256
`30e7af4203fcd48597d00989bcb83e5c6013d914867853efdcc62d46a3fca94d`).
The full DJ Linux interface suite completed with 391 passes, zero failures and
one excluded projectM smoke test (1161.70 s), including the longer pixel-read
allowance and the custom-filter removal fix. The final DK/DM changes have separate
Linux validation recorded below. The Settings pointer fixture passes after explicitly
waiting for stable bounds, without the extra diagnostic capture.
Final Linux validation passes all 281 core tests (12.21 s) and all four affected
paging, pinning, filtering and navigation checks (26.32 s), followed by strict
Linux (15.71 s) and Windows Rust target checks (12.98 s). All five original Linux
pages, populated filter choices, actual three-column grid, responsive breakpoints
and Settings interactions pass. The Linux archive and complete packaged X11/Wayland workflow results are
recorded in DN below.


### DN. Genre server filters and isolated shortcut layout

Genre pages used local filtering despite the original genre album/song views
using their full server-side list filters. The native routes now retain the
search, sort, year, favorite, library and custom tag filters in both paged
browsing and complete queue fetches. A shared query scope preserves the current
genre, matching the original route override, and validates the server and target
kind. Server-filtered results are not filtered a second time using incomplete
list metadata. The filter editor exposes the appropriate album/song controls;
Save collection is available for both targets and retains the genre and filters.
The original implementation is in `genre-detail-content.tsx`, using
`ListFilters` and `AlbumListView`/`SongListView` with `overrideQuery.genreIds`.

The new protocol regression checks Navidrome and Jellyfin album/song parameters
for both paged lists and full fetches, including false favorites, custom tags,
year, search, descending order and offsets. Existing classic Subsonic genre
membership checks still pass. All 281 macOS core tests pass (6.94 s). A new native
regression exercises both genre targets, actual list/queue requests, saved
collections and silent queue insertion with missing optional metadata. It and
four surrounding filters, paging, saved-collection and navigation checks pass
(25.08 s). Cross-platform checks and refreshed archives follow.

The DM Linux archive passes executable, 57-license and updater verification
(SHA-256 `a9255a8467c96441d3c3087f2539004eedc4b43f15a6664711d138fa57d82951`).
Its actual album image confirms three columns, artist credits and the populated
pinned filter panel. Its full X11 workflow passes browsing, silent playback,
MPRIS and physical/duplicate shortcuts, then fails the Settings pointer check.
The failure image shows a clipped button after shortcut testing increased zoom
to 160% while the full workflow retained side panels. The focused shortcut-only
fixture did not have those panels. The harness now records Zoom out, verifies
its inverse 0.1 steps and restores the starting zoom before the next pointer
scenario. It retains real X11 pointer clicks and stable-bounds checks.
The completed X11/Wayland results with this isolated setup are recorded below.

The complete DM macOS interface suite passes with 392 tests, zero failures and
one excluded projectM smoke check (944.41 s). The DN genre change adds one test
and has the targeted validation above. Strict DN macOS workspace checks pass
(14.53 s), as do formatting and whitespace checks. The corrected full X11
workflow passes through nested player configuration, persisted transitions,
Escape, silent playback restart and normal exit. Wayland validation follows.

Both complete Linux workflows now pass with the corrected zoom setup on the DM
package: X11 paints at 1320x840 and Wayland at 1440x1000. Both exercise rejected
login and recovery, library navigation, persisted filters, silent MPV transport
and restart, MPRIS, shortcut recording and dispatch, nested player menus,
transition persistence and Escape, then exit normally with isolated profiles.
No physical desktop, audible output or capture permission is used.
The DM Linux archive and actual X11/Wayland album captures are copied to host
`target/`. The first DN macOS archive passes signing, 57-license/integrity and
updater checks (`126d2e85deb58082b5c6a8410a29b149084983261d4a8b9a55e02c37ff4cf9ba`).
The final genre control also hides genre selection on genre pages, as the
original does; its fixed genre scope continues to apply to all requests.

Final genre controls pass on macOS (6.68 s), including rendered artist/library
choices and absence of the overridden genre choice, followed by strict workspace
checks (12.76 s). Linux passes all 282 core tests (5.54 s), all five affected
interface tests (26.38 s), strict Linux checks (12.76 s) and all Windows Rust
target checks (12.17 s). Windows native C++ linking and runtime remain unverified.
The final DN macOS bundle and archive pass signing, integrity, all 57 bundled
licenses and updater validation (SHA-256
`d51be3c303aecdfdb442d02c1ba0e87eedcb5445d8661ab994cfb99d5e54b841`).
The final DN Linux archive passes executable, 57-license and updater checks
(SHA-256 `c43cc9b00c7c9164bea90a7ec1a3254e09add58ee5873b935e0f53b9cf1f9a06`).
The final packaged executable paints and exits normally on isolated X11
(1320x840) and Wayland (1440x1000), with no panic, Vulkan validation error or
Wayland protocol error. This final package includes the genre changes; the
complete library/media/shortcut workflows on DM and final DN affected-route
regressions are recorded separately above. Both final archives are available in
host `target/`, and final formatting, Python syntax and whitespace checks pass.

### DO. Featured backgrounds, clipped shadows and normalized release years

The complete Home comparison found that single featured cards painted their
color gradient over the blurred artwork. The original places its background
overlay at z-index -1 and blurred artwork at z-index 0. The native card now paints
that gradient first and exposes the original background-overlay CSS class.
A new independent Electron reference uses both original CSS modules and the
pinned Mantine color generator, with two uniform artwork colors in single and
multiple layouts. Native Metal/Vulkan samples exercise the actual featured card.

The same check exposed a shared rendering error with empty titles. A clipped
text-shadow scope could lose its closing marker when its content mask was
empty. Its opening marker had already cleared the working surface, so the
background disappeared. Scene insertion now preserves scope markers even when
fully clipped, allowing the renderer to restore the parent and balance its
snapshot stack. The card regression checks each reference with both an ordinary
and empty title. Before correction the empty-title background was transparent;
with a title, the single-card layer comparison also failed independently.

Album normalization now always assigns the normalized release year, including
zero when unavailable. Previously invalid/missing Navidrome release dates could
retain a generic raw year, and invalid complete Subsonic dates could retain an
older year. The original 24-case backend-normalizer reference now checks the
displayed year as well as the date columns. A conflicting raw Navidrome year
reproduced the failure before the fix.

The original-app fixture now includes Navidrome genre tags, album release dates
and playlist duration. Both original macOS and Linux five-page checks pass,
including a populated Home genre, displayed release year and absence of NaN
text. All 281 macOS and 282 Linux core unit tests pass after normalization,
along with the password-retention integration check. The optional TagLib
interoperability check remains excluded. Rendering and package validation follow.

DO rendering checks pass on both platforms: all eight actual-card scenes agree
with their original pixel samples within three channel levels, including the
empty-title regressions (macOS 6.47 s, Linux 9.67 s). Strict macOS, Linux and
Windows Rust checks pass. The DO macOS archive passes ad hoc signing, integrity,
57-license and updater checks (SHA-256
`ec393a73c6bccc7531a244427fdfa3ed903c462eac0382927e7b42bf5dc95c86`).
Complete DO interface suites are running from the compiled DO binaries. The
following DP work is separate from that validation snapshot.

### DP. Initial sidebar playlists and complete Home composition

The full-page comparison found that sidebar playlists stayed empty on initial
Home until another library route was opened. The shared loading path returned
for Home and artist overview before loading playlist options. Playlist loading
now precedes those route branches. The existing real-request carousel regression
now enters through that path and checks the rendered sidebar rows. It reproduced
zero entries before correction and passes with both entries after correction
(macOS 8.30 s). The packaged workflow now requires the sidebar playlist before
leaving Home and dismisses the isolated password-store warning before its image.

The original complete Home frame measures [444, 80, 832, 248] for the featured
card in a 1320x840 web-style window. A new native whole-page check reproduced
[468, 159, 784, 248]: the generic outer toolbar and padding consumed an extra
79 vertical pixels and 48 horizontal pixels. Home now uses its own original
spacing without that permanent toolbar. Its whole-page frame regression is
being verified; the original fixture also checks sidebar and playerbar geometry.

The whole-page Home frame now passes on macOS, including sidebar, playerbar and
featured-card geometry. The original page supplies four scroll states at 0, 200,
201 and 0 pixels. Home uses the shared page scroll handle and shows its 65-pixel
heading only beyond the original threshold. Native checks match heading geometry,
font metrics, actual wheel activation and scroll reset when returning to Home.
The heading retains the original CSS aliases and shared subheader background.

The broad DO runs found an older detail fixture supplying only `year: 2024` while
expecting an album release year of 2024. The original Navidrome normalizer instead
requires a release/date/minYear field. The fixture now supplies minYear/maxYear;
the full metadata/navigation/mouse/stale-result regression passes with the
corrected fixture. The independent invalid/missing-year regression remains in
place. Home scrolling and the corrected detail test pass together (15.35 s).
The surrounding Home suite and final strict checks are running.

The complete DO macOS suite finished with 393 passes, the one metadata-fixture
failure described above, and one excluded projectM smoke check (963.16 s). The
corrected fixture passes in the DP run. All 15 DP Home/metadata tests pass
(171.84 s), including the full carousel sweep, followed by strict workspace
checks (12.96 s). Panel tests now use the original playerbar queue control when
Home has no permanent toolbar. Navigation and package checks follow.

The complete DO Linux suite also finished with 393 passes, the same corrected
metadata-fixture failure and one excluded smoke check (1082.30 s). Another 12
DP macOS navigation, artist, panel and sidebar checks pass (78.19 s). The first
DP Linux test link was killed by memory exhaustion while a separate release
link ran. A sequential retry is in progress.

The original five-page comparison passes on macOS. Chromium on Linux measures
the Home title 1/64 CSS pixel wider than macOS. The reference comparison permits
that one layout-unit difference only for title width; visibility, threshold,
frame, position, height and font properties remain exact.

DP release archives pass executable and 57-license checks. Updater tests verify
checksum rejection, replacement, cleanup and settings preservation on both
platforms. SHA-256:

- macOS aarch64: `6f1ae81a704ab5482f3965fe99c8b6e5c3ca35203d0babdd4c73176b177b2082`
- Linux aarch64: `60580a7a16e02b51ea21d44a28453647ab97ffe74cbda1bbdb30e3765a979e06`

The DP package passes complete private X11 (1320x840) and Wayland (1440x1000)
workflows: rejected and accepted login, initial Home sidebar playlists, library,
pinned filter persistence, playlists, silent MPV transport/seek/restart, MPRIS,
shortcut recording/duplicates/focus exclusions, zoom restoration and nested
player settings. Both exit normally. A retained Home screenshot exposed a
capture race after message dismissal; the harness now waits for the accessibility
node to disappear before taking that image. This capture refinement is in DQ.

### DQ. Standard album and track library headers

The full-page comparison exposed generic outer navigation and action rows on
album/track libraries. Their original header spans the pinned filter panel and
list: a 65-pixel title/play/count/search row, followed by a 45-pixel control bar.
The new shared header restores that placement, count badge, compact sort/filter/
refresh/layout controls and the original play menu's Now/Next/Last choices.
Existing queue fetching, filters, search and list configuration remain connected.
Save collection moves into the filter panel. Original icon shapes are retained
from the pinned react-icons package.

The original album layout button cycles cards, details and table; the previous
Rust toggle skipped details. Albums now retain the three-state cycle, while
tracks retain their two-state cycle. A whole-app headless check is being verified
against measured original header geometry, with actual clicks, hold-to-shuffle,
menu dismissal and layout persistence.

The initial DQ whole-app header regression passes on macOS (7.28 s). Header,
title, count, Play and all six toolbar controls agree within one CSS pixel.
Opening Play preserves the queue; all three choices and their held shuffle
variants dispatch the expected list request, close after selection and support
Escape. The original play-choice positions also agree within one pixel. The
menu now retains its view entity across redraws, preserving press timers.
Visibility assertions use the live accessibility tree: GPUI's debug bounds map
can retain coordinates from earlier frames and is not a presence test.

The collection regression now clicks Save collection from both pinned filters
and the separate editor. It preserves artist/favorite filters, search, sort and
direction. Those checks pass, as do the original five-page browser measurements.
A broader library run and final header search/badge refinements are in progress.

The DP Linux retry passes all 27 affected interface checks (486.34 s), strict
workspace checks (10.84 s), all Windows Rust targets (14.87 s) and the complete
original five-page reference. DQ's 12 surrounding macOS library checks pass
(144.03 s), followed by strict workspace checks (14.25 s). The final header
refinement restores the original nested CSS scopes and right-edge search
placement, and matches the count badge's 11-pixel type, tracking and radius.
The loading count reuses the existing original-icon spinner.

The final header regression passes (6.79 s), followed by strict workspace
checks (11.33 s); original full-page/header references pass on macOS and Linux.
The complete DQ interface snapshots exposed older tests that focused the removed
global search on Home/libraries, expected play choices before opening the menu,
or treated the restored sidebar playlist request as an account-check request.
Those fixtures retain their original behavior assertions and use the rendered
controls and explicit playlist responses.

A separate navigation regression reproduced a production bug: removing a focused
search field left the window without a dispatch path for shortcuts. The shared
focus-lost subscription restores the app focus when the focused element leaves
the tree. The new test fails before correction and passes for Home, Albums and
Tracks after correction (4.05 s). All 52 surrounding navigation/menu/shortcut
checks pass (193.36 s); the selected/duplicate track held-play regression passes
through the new menu (18.24 s). Strict workspace checks pass (13.89 s).

The updated DQ macOS package passes ad hoc signing, archive/executable validation,
all 57-license checks and updater checksum/replacement/cleanup/settings tests.
Its SHA-256 is
`663585f2a8d94b82a2075a516231be5f1c00cb51ee9d213eb0fc99bdc94ce4b4`.
The complete earlier macOS snapshot finished with 390 passes, six stale-fixture
failures and one excluded projectM smoke test (935.31 s). All six failures pass
after the fixture/control updates; the final shortcut input check passes in
5.64 s. The complete earlier Linux snapshot finished with the same 390 passes,
six fixture failures and one exclusion (1868.96 s). The corrected Linux follow-up
also includes the DR and DS changes.

### DR. Complete playlist header composition

The original five-page harness now measures both expanded and collapsed playlist
headers, artwork, title, play controls, count/duration badges and filter controls.
The original expanded header is 224 pixels tall at a 920-pixel content width;
the collapsed header is 65 pixels, followed by the shared 45-pixel filter bar.
The native page now uses this composition, including the original Tracks/Albums,
sort, filter, refresh, More, edit, collapse and layout controls. Artwork viewing,
upload/drop, playlist replacement and editing retain their existing handlers.
Expanded Play/Next/Last controls and the collapsed play menu preserve duplicate
entries, displayed order, held shuffle and queue insertion behavior.

The original reference also measures 800, 960, 1600 and 2000-pixel windows. The
native whole-page test matches the artwork, metadata, title, search, play buttons
and wrapping filter controls within one CSS pixel at every width. It checks
collapse persistence, the three collapsed play choices and expanded queue/menu
actions. The final macOS responsive check passes in 17.93 s. All 34 surrounding
macOS playlist, artwork, library header and navigation regressions pass (175.11 s),
as do strict workspace checks. Linux validation follows.

The DR macOS package passes ad hoc signing, archive/executable verification, all
57-license checks and updater checksum rejection, replacement, cleanup and settings
preservation. Its SHA-256 is
`c412e9b4aa122530b3f3b2316cc66a006e5ee450adfeb8a119391fbfa4d9b0f6`.

### DS. Shared table headings

The original shared column map supplies 42 reference cases. Native main, album
detail, artist, related-track and side-queue headers now share its translated
uppercase headings and clock, disc, index, favorite and other icon choices.
Configuration labels remain descriptive. All 42 mappings match the original.
Native column geometry, zoom, resizing and reorder checks pass (three checks,
9.86 s), as do the album, artist and related-track regressions (three checks,
13.54 s). Strict macOS workspace checks pass after moving a now test-only import.

The DS macOS package passes signing, archive/executable validation, all 57-license
checks and updater replacement, cleanup and settings preservation. Its SHA-256 is
`d3be5844e42fbe93a2d72b24c2bca104a0c9246514c87eb1528a0b58bf0f30bd`.

All 83 affected DQ/DR/DS Linux interface checks pass (451.00 s), including the
six corrected fixtures, navigation focus, playlist actions, responsive headers,
column layout and resizing. Strict workspace checks and all Windows Rust targets
also pass. The original five-page Linux comparison permits one-eighth of a CSS
pixel for accumulated platform font advances; native geometry checks retain the
one-pixel bound.

### DT. Complete album and artist headers

Album and artist pages now use the original full-width header composition,
artwork actions, Play/Next/Last group, album radio, ratings, favorites and More.
Information remains in the More menu. Shared play controls retain the original
semibold labels and light/dark backgrounds. Original whole-page measurements
cover 800, 960, 1320, 1600 and 2000-pixel windows. The initial native responsive
and dark-background check passes on macOS (27.73 s). The final test also checks
the original black background in the light theme, waiting for the shared color
transition to finish (38.07 s).

The 17-check macOS/Linux follow-ups each exposed the initial light-theme fixture
and one real artist-card regression, with the other 15 checks passing. The page
width measurement had compared against its fallback estimate: when the estimate
matched the rendered width, it never initialized the saved width and album cards
collapsed to one pixel. The measurement now compares against the saved width.
The artist regression asserts usable card widths and preserves group expansion,
search, sorting, drag, album expansion, duplicate-safe playback and navigation.
It and the final album/artist header regression pass together on macOS.

### DU. Complete Settings page header and tabs

Settings now uses its original full-width heading, search/reset group, five
desktop tabs and section spacing. The extra generic library toolbar is removed.
The original browser header flexes with its page height; the native header uses
the same flex sizing and can grow when narrow controls wrap, keeping its actions
above the tabs. Original whole-page geometry supplies the heading, buttons,
first three shared tabs and section heading. The wide-page and narrow French
search/reset checks pass. All five surrounding macOS Settings, localized search,
keyboard-tab, saved-selection and shortcut checks pass (51.36 s), followed by
strict workspace checks (14.19 s). The original seven-page comparison passes on
Linux, including the new Settings geometry.

The DU macOS package passes ad hoc signing, archive/executable validation,
57-license checks and updater checksum rejection, replacement, cleanup and
settings preservation. Its SHA-256 is
`e528e8d786809afa1244396709d81c1c6b7cf212f09e723a9c86f019e7c85a49`.
The Linux package and private-display workflows are in progress. The workflow
now closes its queue panels before capturing Settings, matching the original
default layout; separate narrow-layout checks retain panel-width coverage.

All seven affected DU Linux checks pass (80.98 s), followed by strict workspace
checks (17.22 s) and all Windows Rust targets (21.78 s).

### DV. Accessible card-title navigation

The original album, album-artist, genre and playlist card names are navigation
links. The native titles had pointer handlers but lacked the corresponding link
semantics and keyboard activation. They now expose named, focusable links with
Enter activation and visible focus. The regression fails before the change and
passes after it, including assistive activation, enabled grid multiselection and
queue preservation. The artist-album and grid keyboard regressions also pass
(three macOS checks, 13.31 s). The final focus-styled title check passes (7.85 s).
The packaged workflow can additionally exercise and capture album/artist
navigation with `FEISHIN_CHECK_DETAIL_LINKS=1`; CI enables this on X11 and Wayland.

The DV macOS package passes ad hoc signing, archive/executable validation, all
57-license checks and updater checksum/replacement/cleanup/settings checks. Its
SHA-256 is `4da8d99b5a7f57d28a2358c9e46592be91d4b9252518d3f6c6e96355b3a8f671`.
Strict workspace checks pass (13.44 s). The DU Linux package passes archive and
updater validation with SHA-256
`5da7efc5019eeb457597393dc4bd6c02d5e962a4658ed729f1fe7cdc89db582e`.
Its isolated workflow exposed a test-harness race while reading a dismissed
notification. Tree traversal now skips removed objects and retains live siblings;
a deterministic disappearing-node check passes. The DV Linux retry includes the
new album/artist navigation checks. Both affected Linux UI checks pass (12.06 s),
followed by strict workspace checks (14.47 s).

### DW. Inline theme settings

The original component supplies 32 theme names and eight combinations of system,
accent and primary-shade preferences. Their labels, descriptions, visibility,
disabled states and callbacks are captured by `theme-settings-reference.cjs`.
The native Settings page is being updated to expose these controls inline,
including searchable grouped choices, color swatches, an editable accent value,
primary shade and custom CSS. The superseded separate theme page is removed.
Validation is in progress; this checkpoint does not establish full theme/UI parity.

The DV Linux archive passes executable/license validation and all updater checks;
its SHA-256 is `132976cc983c45abff7f41926595f2ee225134df1c854594945c2e6d34361b98`.
The complete X11 packaged workflow passes, including album/artist link activation,
silent playback, MPRIS, filters, Settings, physical and duplicate shortcuts and
player configuration. The Wayland run is in progress. Captures still show an
oversized album-track toolbar and rectangular detail play buttons, so the prior
header geometry tests alone do not establish full detail presentation parity.

DW's first interaction run passes all eight original visibility/disabled-state
combinations and saves manual, dark and light theme selections. It exposes a
color-picker keyboard-focus failure. The picker now associates focus with its
named button; the regression is being rerun before declaring it corrected.

Both DV Linux packaged workflows pass, including the full Wayland run. The
validated Linux archive and captures have been copied back to `target/`.
DW's keyboard test reaches the palette after the focus correction and then
reproduces an existing shared color-conversion bug: floating-point channels were
truncated, so the green swatch read `#1CB953` instead of `#1DB954`. Hex conversion
now rounds channels, with all five original accent swatches covered. The revised
interaction check also targets the theme search field specifically and uses
Enter to confirm a filtered choice. Original theme layout captures now include
font size and line height; native row and caption geometry checks are in progress.

The final original theme-row reference passes on macOS and Linux. Chromium's
un-sized accent input differs by platform (196 pixels on macOS, 250 on Linux),
so each platform has its own measured reference. The native row and caption
geometry now pass within one pixel on macOS, and custom filenames match the
original labels, including punctuation and non-ASCII boundary behavior. Invalid
custom files are excluded from choices and exposed in the Settings error row.
The interaction test caught Enter being consumed by the text input; confirmation
now uses its existing PressEnter event. Final keyboard and live-file checks are
in progress before packaging this checkpoint.

The final DW macOS theme batch passes all four tests (77.32 s): eight original
visibility/disabled-state combinations, searchable choices, automatic search
focus, Enter and arrow-key selection, system theme preferences, graphical accent
selection, primary shade, durable settings and live discovery/error reporting of
custom files. The original swatch bytes and custom filename labels also pass.
All eight surrounding macOS Settings/search/reset, localized shortcuts, theme
pipeline, custom CSS and visualizer-gradient regressions pass (36.68 s), followed
by strict workspace checks (18.34 s). macOS packaging and the matching Linux
validation/package/display pipeline are in progress.

The DW macOS package passes signing, archive/executable and 57-license checks,
plus updater checksum rejection, replacement, cleanup and settings preservation.
Its SHA-256 is `8c2c83c1f282c2476d4c3bd9cb960e6df13c70c04271cc24656cdadf9441cb19`.
All 12 affected Linux tests pass (88.84 s), strict workspace checks pass (22.73 s),
and all Windows Rust targets check (24.10 s). Windows native linking and runtime
remain unverified. Linux release packaging and the new inline-theme workflow are
in progress.

### DX. Album toolbar and detail button paint

The complete original album page supplies an independent 36-pixel toolbar
reference with search, sorting and list settings, plus a 16-pixel play-button
corner radius. Native toolbar and painted-corner corrections are in progress.

The initial DX painted-corner probe found 3-pixel native corners. The independently
captured original uses 16 pixels. Both album/artist headers now match, including
all five responsive header frames, dark/light paint and the 36-pixel album toolbar.
The metadata caption had inherited a five-pixel taller line height; it now uses
the original 13/16 text metrics. The extra 12-pixel gap between table header and
first track is removed. Three macOS header/playlist/sticky-disc checks pass
(44.34 s); the row adjacency is asserted. The local filter now uses the original
localized Search placeholder.

DW's Linux archive passes archive/license and updater checks with SHA-256
`452f113ba0b4f6633b7350ba3b21125cb042162ab4370f913b744afeafec7058`.
Its initial X11 workflow failed at accent input focus. The harness had assumed
use_theme_accent_color defaults to true, but it defaults to false. It enabled the
theme accent (disabling the custom input), then observed the old false value before
the asynchronous save. It now explicitly verifies both switch transitions before
editing. An attempted parent-observer correction created a notification loop and
was removed, along with the unnecessary inline-render change. The shortcut-only
harness also handles default zoom before the fixture saves all defaults.

The strengthened theme check focuses and edits the RGB field and tests an actual
primary-shade drag while the parent refreshes. The drag check fails before the
fix (saved 6 instead of dragged 3): rendering re-applied the unchanged saved value.
ThemeFields now synchronizes the slider only when the saved setting changes,
retaining the in-progress pointer value and saving on release. Verification is
in progress. The preceding inline-render trial passed three macOS layout,
painted-search-border and RGB-entry checks (84.34 s); that unnecessary production
render change has been removed. Three independent metadata, artist-intro and
fuzzy-search regressions also passed before the observer-loop test was terminated.

The final DX macOS batch passes all three theme/header checks (92.16 s),
including the new RGB focus/Enter and mid-drag refresh regressions. Strict
workspace checks pass (11.86 s), and the complete seven-page original comparison
passes with the new toolbar/radius references. The corrected packaged Linux
harness passes theme selection, both system/accent transitions, RGB entry,
primary shade and persistence. The album workflow correctly changes sorting; its
new persistence assertion has been corrected to the model's `Name`/`Id` enum
serialization. Final packages and the complete Linux workflows are in progress.

The DX macOS package passes ad hoc signing, archive/executable and all 57-license
checks, plus updater checksum rejection, replacement, cleanup and settings
preservation. Its SHA-256 is
`73a0d70b31853b6d1b1a1890691373ea993d400a07448078edf9ca5e4fd3c167`.
The focused Linux theme/shortcut workflow passes through player configuration
and normal exit. The final Linux source snapshot is
`5ea4931cd188c704c3e13fdb6ddf93ee590c80e08d693733a0282b718be0ff53`;
its wider regression, package and complete X11/Wayland runs are in progress.

### DX Linux regression and package checkpoint

The final DX snapshot passes all 18 affected Linux GPUI tests in 95.95 seconds,
strict workspace checks in 16.52 seconds, and Windows Rust target checking in
12.13 seconds. Native Windows C++ linking and runtime remain unverified.
The Linux release builds in 1 minute 27 seconds. Archive, executable, all 57
license files and updater rejection/replacement/cleanup/settings preservation
checks pass. Its SHA-256 is
`5f3f2b32da5be63150d1601f2427a6c4bc7f3bcee95d3d203113c8102e0b57c2`.
The original seven-page Electron comparison passes on Linux. Complete packaged
X11 and Wayland workflows are running with the theme and album-toolbar checks.

### DY custom-theme invalid-color recovery

The original loader drops invalid entries while retaining valid and inherited
colors, keeps the theme selectable, and shows nonfatal warnings separately from
invalid-file errors. The native loader previously rejected non-string color
entries and let invalid strings replace inherited colors. It now sanitizes
custom colors before merging, preserves the original rejected-key order across
inheritance, and uses the original warning text in searchable Settings rows.
Correcting a theme file clears its warning on reload or the existing file watch.

The six expressions from the original locked validate-color 2.2.4 dependency
are generated into a native asset and run with Rust's bounded regex engine.
The general CSS/input parser remains broader, as in the original. The generator
also executes the original sanitizing and inheritance functions for 44 color
cases and inherited valid/invalid overrides, and retains the MIT license.
All four affected core appearance tests pass on macOS. Interface, reload and
packaged workflow validation is in progress; DY is not yet packaged.

The complete DX packaged X11 workflow passes: sign-in rejection/recovery,
filters, album search/clear and both sort directions, list-layout saving,
artist navigation, playlist playback, silent MPV transport, desktop media
controls, all Settings tabs, inline theme selection/system/accent/shade controls,
shortcut recording and physical-key distinctions, duplicate bindings, and player
configuration/Escape. The Wayland counterpart is in progress. The validated
Linux archive was copied back to the host and its SHA-256 verified unchanged.

DY's initial macOS interface batch passes all five theme/CSS tests in 92.06
seconds. A subsequent warning-removal assertion incorrectly used GPUI's retained
debug-bounds map to detect absence. The assertion now inspects the current
accessibility tree; final search/reload verification is running.

DY's final macOS theme interaction/search/reload checks pass (2 tests, 97.76
seconds), followed by strict workspace checks (18.04 seconds). The bundle is
being rebuilt with the new validator license.

DX's Wayland workflow reached the last player-configuration steps but exceeded
the 240-second outer allowance after adding album and theme coverage. All
preceding checks passed; this is not recorded as a complete Wayland pass.
The outer library allowance is now 360 seconds, with the existing per-action
and state deadlines unchanged. A fresh Wayland run is in progress.

The DY macOS release builds in 1 minute 44 seconds and passes bundle signing,
archive/executable/all 58 license checks and updater rejection, replacement,
cleanup and settings-preservation checks. Its SHA-256 is
`f96b6833e664167a7d5e2d81b35d1f81d2d10901c5b0ffb833bc397cda79aa20`.
The DY source is frozen separately for Linux validation before the continuous
accent-color picker work begins.

The extended DX Wayland run passes completely and exits normally at 1440x1000.
Both X11 and Wayland now cover the complete library, album controls, themes,
shortcuts, silent playback, media controls and player configuration workflows.
DY's frozen source archive SHA-256 is
`7812d4428550aa2ab3bd5fd42a1baab1c46209a978d883be40de877fb2c47640`.
Its Linux verification uses that archive while subsequent source work continues
on the host; source is never synchronized during a running container build/test.

### DZ continuous accent picker (in progress)

The original theme setting saves color changes on drag release. The continuous
native picker now has a saturation/brightness area, hue strip and the five
original swatches; drag previews update the visible color without saving early.
The original Mantine callbacks supply 112 pointer/keyboard cases and 275 RGB
conversion cases. Independent windowless Electron frames cover slider, thumb and
swatch geometry at 16px and 24px root font sizes. Native behavior/layout tests
are running. This source has not yet passed final validation or been packaged.

DY's frozen Linux source passes all four core appearance tests, six affected
GPUI tests (99.01 seconds), strict workspace checks (29.28 seconds), and Windows
Rust target checks (23.33 seconds). Release/archive and updater checks pass;
packaged X11/Wayland theme-warning and settings workflows are running.

DZ's isolated picker check passes 112 original callback cases, 275 exact RGB
conversions, and both geometry frames. Its updated accessibility/escape check
passes in 0.34 seconds. The full Settings interaction test, layout test and exact
accent-swatch test also pass in the surrounding 95.88-second run; that batch's
only failure was an added accessibility assertion which assumed the preceding
case ended at white instead of gray. The isolated accessibility test now sets its
own white starting color and passes. Root font sizing in the probe uses the
actual GPUI theme, matching Root's normal rem-size behavior. Native conversion
retains the original floating-point operation order at the 170-degree rounding
boundary. Strict checks and a new macOS package are in progress.

The DY Linux release builds in 2 minutes 22 seconds. Its executable/archive/all
58 licenses and updater checks pass, with SHA-256
`868f236163a87df24e2ce9af0dbc69a55ed6206f8af97f2b0e3502ed4b4d95a5`.
The archive and completed DX display captures are copied back to the host.
DZ's strict macOS workspace check passes in 12.67 seconds; packaging is running.

DY's packaged X11 and Wayland theme-warning/settings/shortcut workflows both
pass and exit normally. Their captures are copied back to the host. DZ's first
macOS release and archive checks also pass, but a subsequent gradient audit
identified an interpolation mismatch in the saturation field. The picker pixel
regression and corrected package are in progress; the earlier DZ archive is
interim, not the final validated source.

DZ's real GPU saturation-field comparison reproduced a 36-level color error
before the fix. The white overlay now ends at transparent white, matching
Chromium's premultiplied gradient result under GPUI's straight-RGBA blending.
Eighteen interior samples at two root font sizes pass within three levels; the
independent browser fixtures differ by at most one level between Mac and Linux.
The combined picker pixel/geometry/interaction test passes on macOS in 0.47
seconds. Strict macOS workspace checks pass in 11.56 seconds, and the release
build finishes in 1 minute 32 seconds. Signing, archive, executable, all 58
licenses and updater checks pass. The final DZ macOS archive SHA-256 is
`651d4c16b28fde92111616dee15c35b688b6bbf0c74221febb21ec066dfe2ad2`.

The frozen DZ source archive SHA-256 is
`947dcd56c2e9d6ac50782e09c40e436ab09105ff6851de251f9082fc06d8368c`.
Linux passes all five theme-settings tests in 104.31 seconds, strict workspace
checks in 13.34 seconds, and Windows Rust checks in 28.50 seconds. Native Windows
C++ linking/runtime remain unverified. The Linux release/package/display checks
continue against this frozen source while EA work proceeds on the host.

### EA inline custom CSS editing (in progress)

The custom CSS controls now retain the Settings page, with Open Folder, Edit,
Save, Close, an expanding multiline editor and a separate preview. Drafts are
retained across Close/reopen; preview updates on blur, and Save atomically writes
the file and settings before applying the theme. The previous standalone CSS
route is removed. Native button activation explicitly finishes a focused edit
so pointer/accessibility Save cannot write the previous draft. Existing CSS
parser warnings remain available below the preview.

A windowless original-component reference covers twelve editor states including
external updates and disable/re-enable. The native editor retains source
formatting instead of Chromium CSSOM serialization; declaration comparisons test
the common CSS behavior. Broader browser sanitizer equivalence is not established.
The existing CSS mouse-save/file-reload/import/disable/cascade regression passes
in 14.79 seconds. New state and packaged workflow validation is in progress.

EA's original-state comparison passes in 0.72 seconds, including focused Save,
preview/draft separation and external-change preservation. The existing CSS
mouse/file/cascade regression and real-app form/navigation/accessibility test
both pass in 19.21 seconds. Strict macOS workspace checks pass in 15.21 seconds.
The first probe's blur assertion ran with an inactive simulated window; activating
that headless window enables actual focus/blur notifications and the check passes.
EA's refreshed macOS package is building.

DZ's Linux executable/archive/all 58 license and updater checks pass. Its archive
SHA-256 is `1a5334218da01831498fb06b9da3b2e02c7d85ddbb438885dd911f578114ba12`,
verified after copying it back to the host. The first packaged X11 spectrum run
reaches the pointer preview but produces an off-center color from the reported
center coordinates. This run is not recorded as a pass. The retry waits for stable
reported bounds and captures the control before dragging to distinguish a fixture
race from an accessibility/pointer mapping defect. Production DZ sources and the
validated package remain frozen; only the isolated workflow harness is updated.

EA's macOS release builds in 59.57 seconds and passes signing, archive/executable,
all 58 license and updater checks. Its archive SHA-256 is
`d6107ca44b230fb1f8eec757f0e57ee39a44d0c59fa62ed6506da91ade6b735a`.
The preview is selectable and escapes CSS text before the native HTML text view.

### EB X11 accessibility coordinate correction (in progress)

Waiting for stable bounds reproduces DZ's pointer failure. AT-SPI reports the
correct window-relative saturation rectangle (1056, 414, 186, 110), but its screen
origin adds a client shadow inset even though GPUI node coordinates already
include that area. It also uses stored window coordinates instead of translating
the drawable's actual root position. On the isolated X server this shifts clicks
eight pixels right and twelve down, producing `rgb(45, 99, 99)` instead of the
center's `rgb(64, 128, 128)`.

The adapter now reuses X11 TranslateCoordinates to obtain the real drawable
origin and passes drawable bounds directly to AccessKit. The packaged center
check remains unchanged in its expected color. The frozen EA/EB source archive
SHA-256 is `35a9c395cb93d08df9207ea30ef8f044a95aad6d068a14e9e3b4fe6445383011`.
Linux checks are running against that snapshot, followed by full packaged X11
and Wayland library workflows including inline CSS and the continuous picker.
The revised native CI workflow enables these checks alongside the existing
album controls and detail navigation checks.


EB passes the six Linux theme tests (100.05 s), two CSS/navigation/accessibility
tests (19.50 s), strict workspace Clippy (19.49 s), Windows Rust checks (20.41 s),
and optimized release build (2 min 48 s). Archive/executable/58-license and updater
checks pass. The Linux archive is copied back and its SHA-256 is verified:
`077e1af0c6d312f2e16d6b94be4497edd2917d7628f82250eb1cbee3845d5fd3`.

The packaged X11 center click now previews the expected `rgb(64, 128, 128)`.
The subsequent unsaved-preview assertion fails: the previously focused RGB input
receives Blur after the picker changes its displayed value, and the existing
Blur handler persists the preview before mouse release. This is a distinct
application focus-ordering defect, not an X11 position failure. The packaged
workflow remains failed until that persistence defect is fixed and retested.

### EC focus-safe accent preview and CSS help (in progress)

The original CSS enable row's information icon and localized help text are
restored. Its Lucide geometry is extracted from the already installed locked
react-icons 5.6.0 source and recorded in the reproducible icon map. The affected
inline CSS state test passes on macOS (0.77 s).

The full inline theme test now activates its simulated headless window and retains
focus in the RGB input before starting the saturation drag, reproducing the
packaged accessibility sequence. Validation and the focus-ordering correction
are in progress. Interactive checks remain deferred.

An independent original-runtime artwork resolution reference now records 17
states, including immediate persistence, one-pixel keyboard changes, blur clamps,
empty fields and editor remounting. It also confirms that the original temporarily
stores out-of-range values and permits fractional sizes; the native resolution
model currently validates whole pixels in the 0-2000 range. The inline artwork
settings conversion has not yet been implemented.

EC's active-window regression fails before the correction (72.00 s): the saved
accent is the green saturation midpoint instead of the prior selected green.
The RGB input commit handler now skips commits while the shared picker reports
an active spectrum drag. All six affected macOS theme tests pass (112.13 s),
including the original callback/layout/pixel fixtures and the focused-input
regression. The frozen EC Linux update archive SHA-256 is
`7fca139f930dc333f98029f02651f9ba63c6593d5559da9b4565089136e60465`.

The EB Wayland retry passes the inline CSS workflow through disabling and
preserving the externally changed file. Its first attempt incorrectly used a
local-search shortcut while a form field retained focus; the original shortcut
policy ignores forms, so the fixture now activates the visible Search button.
The second attempt exposes an empty-string test-helper mistake: selecting text
and typing an empty string does not delete it. The shared helper now presses
Backspace for empty replacements. These attempts are not recorded as complete
workflow passes. Full EC package runs are in progress against the frozen source.

### ED inline artwork settings (in progress)

Artwork aspect ratio, placeholder priority and image resolution now render on the
General Settings page. The existing Theme row renderer is shared with these
controls. Resolution Edit/Close stays inline, with the original table/card/
sidebar/header/fullscreen order, localized labels, pixel suffix and one-pixel
keyboard steps. Valid edits persist per field immediately, refresh affected
artwork and preserve unrelated drafts. Reopening resets empty drafts to the
saved values; closing the editor blurs its input so bounds clamp before unmount.
The obsolete separate image settings route and Save action are removed.

The 17-state original reference executes the actual shared numeric input and
original resolution component in unparented Electron 43.7.0/Mantine 9.3.1.
The native regression checks its first 13 states plus Close/reopen. Native
settings retain their existing integer 0-2000 pixel validation: invalid drafts
keep their last valid typed value until blur clamps them. The original stores
out-of-range values temporarily and permits fractional resolutions. Those
numeric-model differences remain explicit; this does not claim identical behavior
for the reference's fractional states. macOS inline checks pass (5.68 s), and the
existing artwork/cache regression passes (3.46 s). Fresh surrounding checks,
strict validation and macOS packaging are in progress. The frozen ED source
archive SHA-256 is
`ac526ea0d462a5d4683e03e8f7789e7e5ec78a0d8d8447eefc5ba12637ea841d`.

The packaged workflow now exercises immediate artwork saves, keyboard increments,
empty drafts, reopening and blur clamps. Native CI enables it on both isolated
Linux displays. This workflow has not yet run against ED.

### EE button focus audit (not yet implemented)

The original shared Button's actual pointer handling focuses an enabled button
and clears input focus on a disabled button, as measured in an unparented original
renderer. Native buttons currently suppress pointer focus globally. This can
leave edited fields focused after clicking another control and delay their
Blur-driven saves. The original focus fixture is recorded; the shared native
button correction and its keyboard focus-visible behavior are still being audited.
No native button implementation change has been made at this checkpoint.

ED's final macOS batch passes two artwork tests (5.69 s), seven surrounding
settings/CSS/accessibility checks (61.17 s), strict workspace Clippy (18.53 s),
and the optimized release build (2 min 20 s). Signing, archive/executable,
58-license and updater checks pass. The macOS archive SHA-256 is
`67a0c48e1433e62c9eac4d29ef716190bed92c74960fd7061e60b4d5073302bd`.

EE's original focus reference now enables Chromium's simulated focus through
DevTools, without creating a parent window. Four cases verify pointer activation,
disabled activation, Tab and Enter. An enabled pointer click emits Blur, Focus,
then Click with no focus-visible outline; a disabled click emits Blur; Tab focuses
the button with focus-visible, and Enter then activates it. The native regression
is being run before implementing the shared correction.

EC's Linux pipeline completed successfully: six theme tests (115.89 s), strict
workspace Clippy (11.86 s), Windows Rust checks (16.90 s), optimized release
(2 min 02 s), archive/executable/58-license validation and updater checks. Both
full packaged X11 and Wayland workflows pass, including the real saturation
pointer/accessible slider preview and release behavior, inline CSS draft/save/
reload/disable, library playback, MPRIS, shortcuts and player configuration.
The copied Linux archive SHA-256 is
`496e990d5a5609d7808fb7b11405515c4864cee340980d4af0d2e68ae55f6499`.
Virtual-display startup captures were inspected; no physical desktop was used.

EE now allows ordinary pointer focus for wrapped application buttons, clears
input focus for disabled wrapped buttons, and separates focus-visible indication
from focus itself. The native regression fails before the pointer fix (0.04 s)
and passes afterward (0.02 s), comparing five actual original-browser states:
enabled and disabled pointer clicks, Tab, Enter and Space. Direct Blur/Focus/Click
ordering matches, and the deferred text-input Blur notification commits before
activation. An intermediate Enter failure was a fixture omission of key release;
GPUI's existing keyboard activation was retained. The original reference check
is included in native CI. Surrounding dialog and settings checks are in progress.

The frozen EE Linux source delta SHA-256 is
`26de78dd6837b42201c378329649bb36104b8e0a1b0bc93dc15593f3c26c49c9`.
It is applied after the frozen ED delta. Its Linux original-button and artwork
reference checks pass; native checks are still running against that frozen source.

### EF fractional artwork resolution (in progress)

The resolution model now retains finite floating-point values, including the
original editor's temporary negative/out-of-range values. Blur clamps to 0-2000;
empty drafts preserve the saved value, and fractional values survive persistence,
imports, reopening and artwork requests. Existing whole-number cache keys remain
unchanged. The native editor regression now compares all 17 original states
without substituted expectations. The packaged workflow adds fractional saves,
negative drafts and clamping. That new native/package verification is in progress.

Jellyfin artwork requests also retain the original renderer's `width` parameter
and quality 96. Other backends retain `size`; zero omits resizing, and finite
fractions and draft values pass through as the original controllers specify.
Six affected macOS core tests pass (0.25 s), including all three backend request
fixtures, imports, cache preservation and remote artwork. NaN/infinity are rejected
before a request. These changes supersede ED's integer-only limitation.

EE's six macOS theme tests pass (110.81 s). The 16-test surrounding batch reports
13 passes and three failures (215.98 s). The CSS test did not activate its simulated
window, suppressing focus-loss notifications; the timer test typed into the old
input after activating a button that now correctly takes focus. Both fixtures now
model active-window focus explicitly. The third failure is a real regression:
a disabled control clears focus and the app's fallback moves it outside the open
popover, making Escape ineffective. Open popovers now use the existing per-window
focus-lost subscription to restore their container focus. Repair checks are running.
EF's two macOS image tests pass (6.45 s), including all 17 original editor states.

EF's full macOS core suite passes: 282 tests (40.39 s). Strict workspace Clippy
passes (19.96 s) before the final popover deferral refinement. The active-window
CSS and timer repairs pass. The first popover fix passes the playerbar cases but
still fails the Settings entry's Escape check; deferring restoration until after
the application's focus fallback and preserving descendant focus corrects it.
The full player configuration test then passes (25.87 s), including nested menus,
disabled controls, all tested locales and both entry points. Final affected
artwork/cache/popover checks and fresh strict validation are running.

The final popover implementation also passes the four-locale timer workflow
(19.09 s), and fresh strict macOS workspace Clippy passes (10.97 s).

All eight final macOS artwork/cache/button/menu/speaker regressions pass (106.75 s).
The final EF source delta SHA-256 is
`b38a243215d4db3d59f05440abfa2739b3fd7ef3919334c22b903f2c974af095`.
It applies after ED and EE. A complete 410-case macOS interface run is now running
because pointer focus is shared across the app; the opt-in projectM smoke test
retains its existing exclusion. The optimized macOS package is rebuilding.

EF's macOS optimized bundle, signing, archive/executable/58-license checks and
updater checks pass (release build 2 min 37 s). Archive SHA-256:
`7126fc975908e01c24d45ddbf10d1d64ed707914c4775c0d8597944663629255`.
Linux passes all 283 core tests (43.03 s). Both complete EF interface suites
continue against the frozen EF implementation; the Linux pipeline will proceed
to strict checks, packaging and full X11/Wayland workflows only if they pass.
The earlier frozen EE Linux batch finished with 21 passes and the same three
focus failures subsequently corrected in EF (357.76 s).

### EG path mapping preview and missing-file recovery (in progress)

The original General path editor saves each field after 500 ms, previews a cached
random song, refreshes that sample on demand, and synchronizes external preference
changes. Its missing-file tag editor uses immediate saves and the failing song's
path, then offers Reload. The previous native UI had only explicit Save fields
and no preview/recovery workflow. A new shared native section restores these
behaviors. A ten-state reference executes the original React component and actual
Mantine debounce hook in unparented Electron. It records delayed and immediate
updates, original prefix concatenation, refresh visibility and external updates.
Both macOS native tests pass (9.96 s), including all ten original reference
states and actual temporary-file recovery. Reload recomputes paths, reads the
existing title, and preserves file bytes and duplicate queue entries. The
recovery controls appear only when every target is missing, as in the original.
Preview updates use a content-keyed text view to avoid its additional 200 ms
rich-text debounce. Removed controls are checked against the current
accessibility tree, since debug bounds retain prior-frame entries.

Strict macOS workspace Clippy passes (18.15 s). The original ten-state reference
recheck and workflow YAML check pass. Seven neighboring settings, tag and import
tests are running. The packaged Linux workflow now includes path editing,
mapped preview, refresh and resetting preferences; that check has not run yet.
EG has not yet been packaged. EF packages do not contain EG.

All seven neighboring macOS settings/tag/import regressions pass (95.48 s).
The frozen EG source delta SHA-256 is
`ab2b37be1195b9c984f40173b8098678b9c925f434a2c997ecb05b13f9c1509c`.
It applies after EF. The optimized macOS bundle, seal, archive/executable,
58-license and updater checks pass (release build 1 min 02 s). Archive SHA-256:
`e000d6c5c0a4cf9643b5f62d43a8b9506cbb27672481970725db6c0aff015fb5`.
The Linux checkout remains frozen at EF while its complete suite runs.

### EH modal focus recovery (in progress)

Both complete EF interface suites expose an additional shared-button regression:
lyrics export disables its focused Save button while the native file picker is
open. The application's focus fallback then moves outside the modal, so Escape
does not dismiss it after canceling the picker. The isolated macOS case fails
before the fix (4.53 s). Root now restores the top dialog/sheet container after
the application's fallback, preserving focused descendants and nested popovers.
Both lyrics export regressions pass afterward (4.99 s). Three surrounding modal
geometry, nested dismissal and scoped-control regressions pass (17.25 s).
Final strict checks, remaining full-suite results and Linux repair checks are
pending. EG packages predate this correction.

EH's final strict macOS workspace Clippy passes (14.17 s), together with the
player configuration and sleep-timer popover regressions (45.40 s). The frozen
EH delta SHA-256 is
`65a5436ee302791529bf0794a81df0105a45d1db0a6333e6b1c77c0fc1f44b88`.
It applies after EG. The full EF suites continue against their earlier source.

### EI settings import file drops

The import page now exposes the original drop zone, file chooser, localized
empty/error states and file icon. A shared bounded reader serves drops and the
native picker, rejects multiple files, reports read/JSON errors and uses the
existing migration preview before Apply. Cancellation and Escape invalidate
pending reads. Preferences, duplicate queue entries and stored credentials remain
unchanged until the existing backup-and-apply transaction succeeds.

Nine unparented Electron reference states execute the actual original drop-zone
component, including file drops, native-input change events, multiple files,
invalid JSON, ignored non-file drops and read failures. The native regression
compares drop states, cancellation, pending-read invalidation, Apply, persistence
and backup creation. It and the existing native-data import regression pass
(9.86 s). The original-icon render check passes (0.07 s); strict macOS workspace
Clippy passes (17.80 s), and the original reference recheck and workflow YAML
validation pass. Native picker interaction remains in deferred interactive
acceptance; GPUI's headless platform does not implement that dialog.

### EJ album metadata reload regression

The complete EF macOS suite exposed a test still clicking the album header's
removed refresh button. The original-matching album header retains Reload in the
native menu. The test now invokes that command and keeps its request counts,
metadata, disc selection, paging, stale-response, local-edit and queue assertions.
It reproduces the old failure in isolation (6.28 s) and passes after correction,
together with lyrics export and the new import drop test (23.90 s). Linux repair
checks and updated packages for EH through EJ remain pending.

The combined frozen EI/EJ delta SHA-256 is
`184d574ddbd4c9676766c005361ba96bc5d3e8a30eb505b7920208b073ed3c56`.
It applies after EG and EH.

### EK compressor controls and numeric ranges

The compressor now has its original six sliders, localized labels/descriptions,
units, disabled visibility, reset, draft/blur/Enter behavior and numeric ranges.
Makeup gain accepts 0-30 dB, knee accepts 1-10 in half steps, and the editor's
attack minimum is 0.1 ms. Slider previews persist without reconfiguring audio
until release. Empty drafts retain the stored value. Reset preserves enablement.
Eighteen original component states, using its actual numeric wrapper and hooks
in unparented Electron, match the native regression (6.58 s).

Imports accept the original upper bounds. Native DSP and the shared MPV/speaker
filter builder retain the full 30 dB makeup gain; four affected core checks pass
(0.22 s), including real FFmpeg HTTP transcoding with makeup 30 and knee 10.
The filter reference matches the original builder except for the existing safe
FFmpeg knee cap of 8: the installed engine explicitly rejects larger values, so
the editor's saved value is preserved while engine requests stay supported.
Legacy native attack values down to 0.01 ms remain importable.

Four neighboring native settings, import and full-page regressions pass (89.68 s).
Strict macOS workspace Clippy passes (18.90 s); reference rechecks and Python/YAML
validation pass. Packaged X11/Wayland workflows now include compressor sliders,
typed values, clamping, reset and disabling; they have not run against EK yet.
An additional 24-state original equalizer reference identifies remaining missing
band/preamp number controls and independent preset-name behavior for EL.

The frozen EK delta SHA-256 is
`d4831ba065da868a3f028f5d554b579507ff1654aaa4fbddf460d800109cf01d`.
It applies after EI/EJ.

### EL equalizer number entry and preset editing

The equalizer now exposes its preamp slider and all 12 direct band inputs, with
fractional values, blur/Enter clamping and empty-draft preservation. Band pointer
previews stay local until release, matching the original hook. Reset and preset
selection synchronize values and preserve enablement. Disabled sections hide
their controls. Equalizer and compressor preset names are independent, trim on
save, clear after saving and support Enter. Preset menus retain built-in/custom
groups and scoped deletion. Preset-name search remains an outstanding control
difference; this checkpoint does not claim exact dropdown equivalence.

A 24-state reference runs the original component, actual numeric wrapper and
pointer hook in unparented Electron. The native comparison covers numeric drafts,
fractional entry, clamping, actual pointer preview/release, menu selection, separate
drafts, saving, resetting, reloading/deleting presets and preserved duplicate queue
entries. It exposed collapsed number-input height, corrected with explicit field
space; all 12 inputs now have visible accessible bounds. Menu tests resolve
computed accessible labels rather than expecting labels on the container itself.
This check and the broad native page regression pass (38.57 s). The older page
regression now enables the section and expects original preamp clamping/Enter
blur. Final strict, repair and package checks are in progress.

The complete frozen EF macOS interface run finishes with 407 passes, two failures
and one existing opt-in exclusion (3098.47 s). The only failures are the lyrics
modal focus and stale album-refresh test, already reproduced and repaired in EH
and EJ. No additional failure was found. Linux remains frozen at EF until its
complete suite returns.

EL's final seven repair regressions pass (36.98 s), including both failures from
the complete EF run. Nineteen migration/audio/FFmpeg checks pass (0.36 s), and
strict macOS workspace Clippy passes (16.62 s). The frozen EL source delta is
`deea7e7c5cf50cdd17f327db24cec544a36fbba3ba01f8e2e1d1f4106f0b5cff`;
it applies after EK. The optimized macOS package is rebuilding. Thirteen new
original preset-search states record case-insensitive/trimmed filtering, keyboard
choice and dismissal for the next control correction.

EL packaging completed: the macOS bundle and archive pass signing/seal, executable,
58-license and updater checks. The archive SHA-256 is
`c52be0b1b1b025fbb7801e73640fefbe49f86f95c29399a39bf91f9133e56e61`.
The later EM search changes are not included in this package.

## EM: searchable audio presets

Equalizer and compressor presets reuse the native Select and grouped list. Search
matches names without case sensitivity and ignores surrounding whitespace. The
original component supplies 17 states for filtering, empty matches, arrow/Enter
selection, retained queries after selection/Escape and reset after disabling.
Its first synthetic click does not open the original popup; native opening is
asserted directly instead of copying that reference-harness timing artifact.
Named combo-box/list-box/option nodes expose expanded and selected state. Preset
save/delete and locale/import refreshes reapply the active query. Existing custom
preset and 24-state equalizer checks cover dynamic option replacement.

Initial macOS checks exposed unnamed options; explicit delegate item labels fix
the accessibility actions and names. The 13-state search predecessor and 24-state
equalizer test pass (15.48 s). Final 17-state, neighboring-page, strict and package
checks are pending. Linux's complete frozen EF run finished with 407 interface
passes, two failures already corrected by EH/EJ, one opt-in projectM exclusion
(3473.93 s), and 283 core passes. The new deltas have not yet been applied there.

EM final macOS interface batch: four tests pass (50.39 s), covering all 17 preset
search, 24 equalizer and 18 compressor reference states plus broad pages. Original
Electron checks reproduce all three fixtures. Strict checks and packages are running.

EM strict macOS workspace checks pass (16.60 s). The frozen source delta is
`62e5b03e2fbc890a5fc3b6cd91835ad72db23fcd70d4759b92ee6689c078f00a`.
The first Linux original-EQ check exposed a reference-harness timing difference:
unparented Linux WebContentsView instances do not supply display animation frames.
The harness now schedules frame callbacks on a 16 ms timer, as the existing motion
references do, without modifying the original component or fixture values. All
18 compressor, 24 equalizer and 17 search states pass on both systems; Linux's
10 path and nine import-drop states pass as well. The harness delta is tracked
separately from the frozen EM Rust source.

EM macOS release completed in 1m 33s. Bundle signing, archive/executable,
58-license and updater checks pass. Archive SHA-256:
`26ebe65aae4c436c8a0e65d1585c8b264366563b3d9de69cd9ee57b9c4c20a51`.
Linux's updated complete core suite passes 284 tests (42.66 s); affected interface,
strict, Windows Rust and package checks are still running.

## EN: custom preset name compatibility

The original equalizer/compressor saves any nonempty trimmed preset name. Native
save and import added a 128-character restriction, preventing long existing names
from round-tripping. Five additional original states cover 160-character Japanese
EQ names and 159-character compressor names, saving/clearing each draft and deleting
each preset. The native comparison failed on the first save before the fix (9.30 s).
The shared save path and import validator now retain these names; empty-name and
audio-parameter validation remain, along with the existing 16 MB import bound.
The native comparison also previews the exported state after both long-name saves.
Final native, migration and strict checks are running.

EN macOS validation passes: three native interface/import checks (21.87 s), all
15 migration tests (0.26 s), and strict workspace checks (13.09 s). Its 29-state
original fixture includes the new long-name cases. Linux remains on frozen EM
while its ongoing checks and packaging finish.

EN macOS package completed in 1m 41s and passes bundle signing, archive/executable,
58-license and updater checks. Archive SHA-256:
`ef873fcccb9fc2e499d34c930360349f7a15443d5af33625537c1fac5dc5ec39`.
Frozen EN source SHA-256:
`0f498b6a3b686b7736377835b512729909edd6fb58ea580ef48807b4a7bba4e2`.
The original checks reproduce all 18 compressor, 29 equalizer and 17 preset-search
states. Linux EM passes all 12 affected interface tests (126.22 s), including both
full-suite failures corrected by EH/EJ, plus strict workspace checks (36.02 s).
Windows Rust and packaged-window checks remain in progress. A 3,291-file native
source/asset comparison finds only the four expected EN files and one vendor
patch-note difference, to synchronize after the current Linux work finishes.

Linux EM Windows Rust checks pass (31.23 s); this checks all Rust targets against
Windows APIs but does not link the native Windows C++ libraries or run Windows.
Linux release compilation completes in 3m 39s. Archive/executable, 58-license and
updater checks pass; archive SHA-256:
`54bbee2335574d77f205363f93a6f1306e141638332a223ffdaaebdc32baa4db`.
The full packaged X11/Wayland workflows have started with path, compressor and
equalizer coverage enabled. EN remains queued for synchronization after them.

The first EM packaged X11 run passes library/album controls, playback, MPRIS and
theme/accent checks, then stops in the test driver while entering `-1` for the
image-resolution boundary case. The driver passed it as an xdotool option. Its
shared typing helper now uses an explicit end-of-options separator; Python syntax
and a private-Xvfb negative-text invocation pass. This was not a native app failure.
EN and the typing fix are now synchronized to Linux. All 3,291 native source and
asset files match macOS byte-for-byte, including the previously stale vendor note.
Final EN Linux checks and full packaged X11/Wayland workflows are running.

EN Linux validation passes all 15 migration tests (0.26 s), four affected native
checks (41.48 s, including the neighboring sleep-timer regression), and strict
workspace checks (15.73 s). The original 29-state equalizer fixture reproduces
on Linux. Windows Rust checks and the final package rebuild follow in sequence.

EN Windows Rust checks pass (15.35 s), and Linux release compilation finishes in
2m 14s. The final EN Linux archive passes executable/archive, 58-license and updater
checks, with SHA-256
`7716b0e1ff1ebb21881d11a68f37888464b80f11d9b29f0363dd3459eae44763`.
Full X11/Wayland packaged workflows are running with the corrected typing helper.
Workspace formatting, Python syntax and native CI YAML checks also pass.

The next EN X11 run passes fractional/negative image editing and full path preview,
refresh and reset checks. It then exposes an overly exact test comparison for
compressor attack: native f32 `0.1` is persisted through serde_json::Value as
`0.10000000149011612` (and default knee `2.83` as `2.8299999237060547`).
The packaged test now permits only f32 rounding (`rel_tol=1e-7`, `abs_tol=1e-8`)
for float expectations and includes actual values in timeout diagnostics. A
standalone float round-trip confirms both cases and rejects an incorrect value.
The app source and verified EN packages are unchanged; both complete packaged
workflows are rerunning with this assertion correction.

EN final X11 and Wayland packaged workflows both pass. They exercise rejected
login/sign-in, home/library/details, album sorting/layout, playlist groups, silent
MPV transport, queue persistence, MPRIS, themes/accent/CSS, image/path/compressor/
equalizer preferences, preset save/load/delete, shortcut recording/dispatch and
player configuration. Both apps exit normally using isolated profiles. The
verified Linux EN archive is copied to the host; its SHA-256 remains
`7716b0e1ff1ebb21881d11a68f37888464b80f11d9b29f0363dd3459eae44763`.

A suspected missing player bar in the captures was a mistaken visual reading.
The saved images show the bar and its controls, consistent with the accessibility
and layout evidence. No production change was made for that observation.

## Checkpoint EO - wrapped preset selector styling

The new preset selectors still used GPUI control defaults and bypassed the
original Select wrapper's CSS aliases. An original-component reference now
records 12 paint/geometry states covering two root font sizes, focus, disabled,
open/hovered options and custom CSS. Native input/dropdown/option style wiring
and the affected interface checks are in progress.

EO's 12 original selector states now pass native paint checks on macOS (2.02 s).
They compare input background/opacity/border/radius, option height, dropdown
colors and hover colors, including custom rules and both root font sizes. The
reference uses the actual original Select wrapper and compiled stylesheet.
Broader preset/page regressions and strict checks follow.

EO macOS validation passes four preset/style/page checks (48.07 s), the separate
18-state compressor check (5.91 s), and strict workspace checks (11.55 s).
Frozen EO source SHA-256:
`1f27b7c26c0ab817d18d8749bd276e061847df3b5b66c3ec8a991d4e5e9ecc91`.
The original selector reference also reproduces on Linux; native checks follow.

## Checkpoint EP - CSS image fitting

The built-in context-preview stylesheet exposed an unsupported `object-fit`
property warning even when the user's CSS only changed a border radius. CSS
image-fit declarations were not reaching GPUI images. A new original-browser
reference covers 32 image scenes: large/small intrinsic sizes, all five fit modes,
rounded boxes, inheritance/reset keywords, variables and invalid fallback. The
native regression fails before the fix on the unsupported property. CSS-to-image
style support and windowless GPU comparisons are in progress.

EP's 32 macOS GPU comparisons pass (0.46 s). The first crop comparison caught
linear sampling of a pixel outside a small image's cropped source; the shared
source-region clamp corrects Metal, Vulkan and Direct3D. Nine affected native
checks pass (23.86 s), including all context/drag regressions and the existing
188-scene foreground-filter reference. Strict macOS checks pass (19.62 s).
Context and drag artwork now receive their original image CSS aliases, retain
Cover when the general aspect-ratio preference is enabled, and honor a custom
Contain override. These actual image-path checks pass for both preview types.

Linux's original 32-scene reference differs from macOS only at two rounded-edge
samples in four scenes, by at most nine channel levels. Platform-specific
references preserve those original differences; flat pixels and fit results are
identical. EO Linux checks and Windows Rust checks are complete; EP checks follow.

Frozen EP source SHA-256:
`9684e14ad4d18beb018ad01215e79aac42f76f28bb7a1195a9bc46b5fdcc1b53`.
Final macOS strict checks pass (11.27 s); packaging is running. Linux reproduces
its 32 original browser scenes and validates all 20 Windows HLSL entry points.
The first Linux native pixel check matches macOS's rounded-edge smoothing and
therefore differs by nine levels from two original Linux fractional edge samples.
A narrowly bounded corner-edge tolerance is being added; other pixels retain
three-level comparisons. The pending adjustment affects the regression test only.

### EP final image checks and macOS package

The precise rounded-corner edge allowance passes the image regression on macOS
(0.40 s) and Linux (3.01 s). macOS strict checks pass (11.89 s). Linux's nine
affected image/CSS/context/drag checks pass (36.33 s), strict checks pass
(25.82 s), and Windows Rust checks pass (29.36 s). Windows native linking/runtime
remain unverified. Frozen test-edge delta SHA-256:
`eb66e67b73ed8c3bf26df8042c3afa3e14a64040cce8a6b8576a2318bd4f788a`.

The EP macOS release build, ad-hoc bundle seal, archive (58 license entries),
and updater checks pass. Archive SHA-256:
`c8216e548d6c3e617b87a66356ed2f91ebaca428a9ccf8e6fbada8830c6ad400`.
This is a local ad-hoc signed package; distribution signing/notarization has
not been performed. Linux's matching EP build and packaged workflows continue.

### EQ smart-playlist editor parity work

The audit found omitted date-picker operator variants, a truncated 21-item sort
list and unlocalized rule/operator controls. The original source supplies 90
date mappings, 133 sort choices and operator labels in all 39 bundled locales.
The native core passes those mappings and retains open-ended date ranges, typed
relative-date ranges and API operator names. Existing nested-rule and custom
field regressions, plus the full sort-list comparison, pass on macOS (three
tests, 6.29 s). Searchable field/operator/sort controls and direct/calendar date
entry are implemented; their new end-to-end interaction check is in progress.

EP Linux release packaging completes in 2m 58s. Its archive, executable,
58 license entries and updater checks pass. The archive copied back to the
host also passes validation. SHA-256:
`5216914241ea6e42e62d6bbaea21638dbaf22190771e67e2cef17b68c02a9ca3`.
The complete X11 and Wayland workflows are still running against frozen EP.

EQ's five affected macOS interface checks pass (19.16 s). Search selects retain
all 133 original sort choices, grouped custom/standard fields and translated
operators. Calendar entry supports pointer and arrow/Enter selection, direct ISO
date editing and clearing; date edits validate without overwriting the previous
rule, while untouched imported values remain intact. Twelve cases executing the
original component verify separate count/percentage drafts, clearing, zero,
fractional values and counts beyond the former native-only million-item cap.
Blank newly added sort rows remain editable and do not enter saved queries.
Whole-number limits retain the original integer JSON representation. Strict
checks and the matching Linux checks are pending.

EQ macOS strict workspace checks pass (16.74 s). The source is frozen for packaging and subsequent Linux validation.

Frozen EQ source SHA-256:
`5e9dd89632aba5ca16980b508845a53d1735ffc13329332e0c93b15195114f9c`.
The macOS release rebuild completes (2m 26s), followed by bundle, archive,
58-license and updater validation. Archive SHA-256:
`ea490e736fdf2de9ed5a009822e2a384f5487fb791bed8e6ecd22b0256a628e5`.

EP's complete X11 workflow passes. The matching Wayland workflow exceeds the
outer six-minute budget after finishing settings, playback and shortcut checks,
while entering the final player configuration. Its owned application/display
processes have exited; no unrelated processes were touched. The harness budget
for the expanded library workflow is now ten minutes. Frozen harness delta:
`6f80d37ff9f724a53e373fc36fe30aa55b9fb311f2b6a482036c905b1180ff09`.
Linux received EQ and this harness delta before the Wayland rerun and subsequent
EQ core/interface/strict/Windows Rust checks; no source is changed during that run.

### ER selected labels and live editor choices

A new pointer/keyboard regression reproduces a retained selected field disappearing
after a canceled no-result search. The stored rule remains intact, but the shared
Select displayed its title by the filtered/highlighted index. The repair retains
the confirmed item independently and exposes its accessible value; data refresh
resolves confirmed/highlighted choices by value. Empty group headings are omitted.
Six original controlled-Select states cover the initial value, no-result search,
Escape, blur, relabeling and reordered options. Live editor choices refresh on
language/custom-field changes while retaining invalid in-progress text drafts.
Repair and neighboring preset tests are in progress.


ER's initial canceled-label assertion mistakenly inspected the TextRun role;
static component labels use Label. After correcting the assertion and allowing
blur/redraw to settle, the previous filtered-index title implementation fails
at the intended missing-label assertion (9.41 s). Restoring the confirmed-item
repair passes all seven affected macOS checks (29.71 s). These checks also retain
invalid numeric drafts during live Arabic/operator and custom-field relabeling.
Seven additional cases execute the original sort parser and save fallback. They
cover old/new ordering, blanks and mixed signs; empty sorting saves +dateAdded,
and comma-separated syntax takes precedence over the legacy order flag. The
last editable sort row cannot be removed, matching the original controls.

EP's complete packaged Wayland rerun passes, including all nine opt-in settings
and detail workflows. EQ Linux's affected core checks (3) and interface checks
(5, 18.44 s) pass; strict checks pass (26.52 s).

ER macOS strict checks pass (19.40 s). EQ Windows Rust checks pass (24.36 s);
native Windows linking/runtime remain unverified.


Frozen ER source SHA-256:
`0759e5f7427ec79b32ee84938887daba5f279ddc47f4041cf5024c49841b2a8f`.
The first Linux build reused an older core artifact after tar restored a source
mtime preceding the previous build, causing a missing PartialEq diagnostic despite
the correct source. The frozen source files received current mtimes and the same
pipeline is rebuilding; no content was changed during validation.

### ES complete calendar grids and level-aware navigation

The upstream calendar helper always returned five weeks, hiding month-end dates
when a sixth week was required. It now generates the actual number of complete
weeks, preserves separate month boundaries, and supports the original Monday
week start. The editor opens with the original twelve-month chooser; year/month
keyboard navigation changes its current level without prematurely committing a
day. The shared selection path handles pointer/keyboard confirmation and disabled
choices. Four original wrapped DateInput cases verify four/five/six-week grids,
leap day and month-end selection. Native validation is in progress.


ER's rebuilt Linux suite passes all seven affected checks (30.45 s), strict checks
(29.85 s), and Windows Rust checks (26.97 s). Both original references reproduce.
ES's two affected macOS checks pass (12.42 s), including calendar grid geometry,
month/year navigation by pointer and keyboard, and deferred date confirmation.
Strict macOS checks pass (12.12 s). The source is frozen for Linux validation.


Frozen ES source SHA-256:
`c22c932349cf7dce0465139f78d18690f509b6811081320b093996bc3bddeb91`.
Linux reproduces the four original calendars and passes both affected interface
checks (14.46 s), strict checks (13.96 s), and Windows Rust checks (17.75 s).

### ET smart-rule actions and editable drafts

The remaining editor comparison found missing Reset/Clear actions, automatic
selection of a field/operator for new rules, and validation that prevented
removing a rule with unfinished text. New rules now start blank; new groups use
the original Any default. Incomplete rules stay editable and are omitted from
saved queries. Structural changes retain unaffected input entities and drafts;
save/JSON conversion still validates complete rules and limits. Reset restores
the original rules for the editing session; Clear creates the original empty
rule, and both preserve sort choices, limit drafts and unknown metadata.

Two original callbacks and query converters supply Reset/Clear expectations.
The comparison also covers exclusion of the current playlist from playlist-rule
choices. Whole-number values now keep the original integer JSON representation,
including relative-date ranges. The three affected core tests pass (0.06 s).
Native interface validation and packaged calendar/Reset/Clear checks follow.


ET's three affected macOS core tests pass (0.06 s). All six affected interface
checks pass (20.85 s), including original Reset/Clear expectations, unfinished
rule removal, separate limit drafts and current-playlist exclusion. Static menu
labels are resolved through the accessibility consumer's derived names rather
than assuming explicit names on their parent nodes. Strict checks pass (14.19 s).
The source is frozen for macOS packaging and matching Linux validation/packaging.


Frozen ET source SHA-256:
`729b5e74618f77f27414bfbd1beb612f5cc0b5c9faf12539306946fd72c180db`.
The macOS optimized release build finishes in 2m 18s. Bundle seal, archive,
58-license inventory and updater replacement/settings-preservation checks pass.
Archive SHA-256:
`85754167838364716b637ce5971d5645eb38840e5b5cd910afe143b4732cdad7`.
This is a local ad-hoc signed bundle, without distribution signing/notarization.
Linux ET passes its three affected core checks (0.07 s) and all six interface
checks (22.67 s). Strict checks and the release/package workflows continue.

The packaged smart-editor harness uses the exact translated action name,
Create Playlist. Its test-only label delta has SHA-256:
`20443b244296d87b18d08e74a6e4b9944715af709b4ebc181c71daee9299c402`.
The delta was applied after the first Linux validation process exited, before
the complete packaged rerun. Compiling or executing source was not replaced
during either process.


ET Linux strict checks pass (16.89 s), followed by Windows Rust checks (16.85 s).
The optimized Linux build completes in 2m 52s. Its archive, executable,
58-license inventory and updater checks pass, including validation of the copy
returned to the host. Archive SHA-256:
`4b727ba1daa9f8344032edcff402c77d9524eea487ddda1e76def01e02079ca8`.
The complete packaged X11 and Wayland workflows pass on this same archive.

ET's final packaged rerun exits successfully on both isolated displays with all
ten optional checks enabled. In addition to sign-in, navigation and silent MPV
transport, it covers album controls, desktop media controls, themes and accent
editing, custom CSS, image settings, path mappings, compressor/equalizer editing,
shortcut recording/dispatch, nested player settings, persistence and normal quit.
The new smart-editor path selects May 31 from a six-week calendar, round-trips
through JSON, resets the editing session's rules and clears them while retaining
sort and limit settings. Both displays pass the same assertions. Captured calendar
and cleared-query windows are saved for visual inspection.

The combined run log is `target/native-et-packaged-validation.log`; screenshots
and application logs use `target/native-startup-et2-library*` and
`target/native-startup-et2-wayland-library*`. X11 paints at 1320 by 840 and Wayland
at 1440 by 1000. The host copies of both release archives retain the hashes above.
Workflow YAML, Python checks and whitespace validation also pass. This evidence
does not replace the user's deferred interactive acceptance or a native Windows
runtime check.

### EU readable calendar date fields

ET's captured single-date field gave its text only the space left over after the
calendar button, clipping the year even at the default text size. Calendar-backed
inputs now reserve the original 170-pixel input width, expressed relative to text
size, while the adjacent calendar trigger uses its own width. Other rule inputs
keep their existing sizing and rule rows continue wrapping.

Frozen EU source SHA-256:
`2028fda5aec7311d1c2c5e31a7a5a75aa2a02f8c354aa10087d4f97c7acfecc8`.
Both affected interface checks pass on macOS (14.58 s) and Linux (16.90 s).
Strict checks pass on macOS (11.85 s) and Linux (11.30 s), followed by Windows
Rust checks (14.57 s). Native Windows linking/runtime remain unverified.
The macOS optimized release rebuild completes in 1m 05s; bundle, archive,
58-license inventory and updater checks pass. Its archive supersedes ET with
SHA-256 `40b3383112f65bb0e2eff1fde690f3731695680b05777d141909333e81226409`.

The Linux optimized release rebuild completes in 1m 22s. Archive, executable,
58-license inventory and updater checks pass, including the returned host copy.
Its archive supersedes ET with SHA-256
`f872fa624d1d768519c70b1e45ca406678ad1805e239677b53adf6e07d30a7bd`.
The complete library workflow with smart-editor checks enabled passes again on
both X11 and Wayland, covering month-end dates, Reset/Clear, JSON round trips,
silent playback, media controls, shortcut editing/dispatch, player configuration,
persistence and normal shutdown. The preceding ET runs cover all ten optional
settings/detail workflows; EU changes only date-input layout.

Visual inspection of both EU calendar captures confirms that `2021-05-15` is
fully visible alongside the calendar trigger. Both release archive checksums
match their validated host copies, and the checked source matches the frozen EU
delta. Final build/test logs are `target/native-eu-macos-validation.log` and
`target/native-eu-linux-validation.log`; packaged screenshots and application
logs use `target/native-startup-eu-library*` and
`target/native-startup-eu-wayland-library*`. The deferred acceptance checklist and
native Windows runtime validation remain outstanding.


### EV standalone Rust workspace

At the user's request, the legacy application source and its Electron, React,
TypeScript, Vite and PNPM build configuration are removed. The old web container,
packaging jobs and source-executing comparison generators are retired. The native
CI workflow, documentation and editor configuration now describe the Cargo
workspace. Existing settings import and the native browser remote remain intact.

The native crates own all 60 assets formerly sourced from the legacy application:
39 translation catalogs, 33 bundled theme definitions, compatibility variables,
stylesheets, titlebar controls and service icons. SHA-256 checks confirm that all
60 asset files retain their exact bytes. Settings-search keys are stored directly
in Rust. The core build script no longer parses TypeScript or CSS. Recorded
regression fixtures and their license/provenance information are retained;
historical generator commands above refer to source available in Git history.

Frozen EV source delta SHA-256:
`1b728221497142dba7f4d1a5106bf6befdb69b5e9d3cb3b6d527d8451239a941`.
A subsequent test-only correction replaces a retired generator command in the
optional metadata test's ignore message with its supported
`FEISHIN_TAGLIB_REFERENCE` fixture input. No application behavior changes.

Validation:

- macOS: all 284 core unit tests and the credential integration test pass;
  all 14 affected GPUI checks pass (154.08 s); strict checks pass (54.12 s).
- Linux: all 285 core unit tests and the credential integration test pass;
  all 14 affected GPUI checks pass (191.42 s); strict checks pass (1m 09s).
- Both core runs leave the optional archived metadata-service interoperability
  test ignored. The native remote suite passes all eight tests.
- All Windows Rust targets pass strict cross-checks (30.13 s). Native Windows
  C++ linking and runtime remain unverified.
- The macOS optimized release rebuild completes in 2m 19s. Its bundle seal,
  archive, executable modes, all 58 license files and updater checks pass.
  The EV archive supersedes EU with SHA-256
  `1e84d281ed7876170cde0901b818eaf0c6a683dd2abbd8ee67ccfd33f88b0d82`.

The Linux optimized release rebuild completes in 2m 50s. Its archive,
executable, all 58 license files and updater checks pass, including the returned
host copy. The EV archive supersedes EU with SHA-256
`e922d873bb6609b6b2fae6112431454a09c982f4a79ecf19cd6e3b0a4df5ab4a`.

Both complete packaged library workflows pass on isolated X11 and Wayland
displays, with all ten optional detail, theme, settings and smart-editor checks
enabled. They cover sign-in, library navigation, silent playback, desktop media
controls, persistence and normal shutdown. Captured Home and theme-settings
screens render their migrated artwork, icons and controls correctly on both
display systems. All 61 screenshots and application logs are retained locally.

The final optional metadata-test note compiles on Linux (2.65 s) and reports the
archived-fixture requirement correctly; it remains ignored without that input.
Formatting, whitespace, Python syntax, workflow YAML, editor JSON and active
local-document links pass validation. Every retired path in the deletion
manifest is absent, and all 60 migrated asset hashes remain unchanged.

Evidence is saved under `target/native-ev-*-validation.log`,
`target/native-ev-linux-test-note.log`, `target/native-ev-removed-paths.json`,
`target/native-ev-asset-hashes.json`, `target/native-startup-ev-library*` and
`target/native-startup-ev-wayland-library*`. The user's interactive acceptance
and native Windows linking/runtime remain outstanding. Removal of the old
implementation does not establish full feature parity.
