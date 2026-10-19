# Feishin native component patch

Source: the unmodified gpui-component 0.5.1 crates.io source package, except for
changes in Cargo.toml, src/lib.rs, src/button/button.rs, src/input/input.rs,
src/input/number_input.rs, src/menu/mod.rs, src/menu/context_menu.rs, src/menu/popup_menu.rs,
src/menu/menu_item.rs, src/text/style.rs, src/text/node.rs,
src/text/text_view.rs, src/text/inline.rs, src/styled.rs and the two highlighter import lists:

- An optional StateStyle callback resolves native CSS after normal component
  styles and before painting hover, active, focus, disabled and checked states.
- PopupMenuStyles supplies live application callbacks for popup containers,
  items, dividers and labels. Items expose checked, keyboard-selected and
  disabled states and retain compound hover/active selectors.
- Popup menus can append asynchronous rows while retaining their focus, selection
  and active CSS callbacks. The application library selector uses weak menu
  entities and rejects results from a changed account or connection.
- Context and dropdown menus identify their own CSS classes. Context menus can
  retain a selected-item preview below the scrollable actions, with bounds and
  the scrollbar scoped to the appropriate container.
- Deprecated std::usize imports are removed; primitive associated constants
  retain the same values on current Linux Rust.
- Text inputs and number inputs forward the same callback to their native input.
- Button hover text uses the variant foreground rather than a hardcoded red.
- Buttons accept an optional delayed expanding-circle indicator and a 500 ms
  primary-button hold callback. Their keyed
  gesture state survives redraws, suppresses the release click and its propagation
  to parent navigation controls, and cancels on
  pointer exit, disabling, loading or removal. Ordinary mouse, keyboard and
  accessible activation retain the normal click callback. Its indicator waits
  100 ms, expands over 300 ms and clears when the hold fires. Circular play
  controls opt into 200 ms ease-in-out scaling and opacity, with CSS reversal
  shortening. These defaults follow the original React hook's inline props;
  stylesheet-only timing would be different. The native tests execute those hooks
  and compare complete/interrupted frames plus disabled/loading GPU samples.
- The existing scalar transition is shared as `animation::CssTransition`, with
  configurable duration and CSS cubic-bezier timing. Buttons can opt into custom
  hover/press transforms while retaining explicit opacity. Original play defaults
  are unchanged; carousel arrows use 200 ms ease and a 0.95 pressed scale. Native
  tests compare actual pointer painting and accessibility geometry with original
  transitions and interrupted reversals.
- Spinner supports explicit cycle duration/easing; circular play controls use
  the original one-second linear cycle and an independent loading glyph size.
  Loading replaces their custom play children. Other buttons retain their
  existing loading composition and spinner defaults.
- Rich text has an optional safe_urls_only style for remote release notes.
  It removes executable and local-file links/images, including HTML, image
  links and Markdown references, while preserving their visible text.

The package version and dependency requirements are unchanged. Its Apache-2.0
license is included here and in native binary packages. The host application's
headless painted-color tests exercise actual input focus, button hover/press,
checked/disabled colors and popup keyboard/mouse states. Callbacks are optional
for existing component users.
## Native inspector dependency

The `inspector` feature uses `wry?/devtools` so enabling GPUI's native inspector
does not activate the optional embedded browser. Applications that separately
enable `webview` still receive its developer tools.

## Accessibility

Button, checkbox, switch, input and slider components expose native roles, names,
values, selected/toggled/disabled states and focus. Buttons prefer their descriptive
tooltip for their accessible name. Masked password inputs omit their secret value.
Password visibility buttons toggle on mouse, keyboard and accessible clicks, expose
a named action and eye/eye-off icons, and respect the disabled input state.
Masked input copy/cut actions preserve the clipboard and text, and their context
menu items are disabled. Revealed inputs retain copy/cut and undo behavior,
matching Chromium password controls.
Inputs accept SetValue through their existing validated input path; slider SetValue,
Increment and Decrement enforce finite, stepped, bounded values and emit the same
change event as mouse interaction. Slider has an optional accessible label builder.
These changes additionally touch `src/checkbox.rs`, `src/switch.rs`,
`src/input/state.rs` and `src/slider.rs`.

Inputs additionally support independent accessible names and text-selection actions.
Accessible edits retain normal undo/redo history. Number input step buttons have
accessible names; the application supplies translated actions and their field name.
Input labels have an optional render-time transform. Native form placeholders and
accessible names follow the current language while retaining text, selection, focus
and undo history. User-defined tag field names retain their literal spelling.
Popup menus expose Menu/MenuItem/separator roles and selected, checked and disabled
states through `src/menu/menu_item.rs` and `src/menu/popup_menu.rs`.

## Text decoration defaults

Explicit underline/strikethrough style literals in the input, diagnostic
highlighter and rich-text components select GPUI's solid pattern default. This
keeps their existing rendering while allowing application CSS to supply dotted,
dashed, double and wavy line patterns.

## Directional text input

Input layout, painting, pointer hits, selections and cursor positions share the
same row alignment and logical grapheme cells. A clicked bidi boundary retains
its visual affinity. Left/Right and Shift arrows follow the original Chromium
input's logical grapheme movement relative to its paragraph direction, including
programmatically placed cursors inside combining sequences. Backspace retains
the original separate combining-mark deletion. The isolated input-editing.cjs
probe records 112 original keyboard cases on macOS and Linux without native
windows; the application checks these through actual GPUI input events.


Multiline input uses GPUI's paragraph-context shaping and browser-compatible
BreakWord boundaries for both wrapping and drawing. Soft-wrap affinity retains
the clicked row at a shared source boundary. Up/Down and Shift selection preserve
the pixel-rounded caret coordinate; Home/End select the visible physical row.
Bidi editing resolves Chromium 150's physical-run boundary rules separately from
pointer insertion affinity. Wrapped RTL spaces can hang outside the paragraph,
while unwrapped inputs retain their scrolling alignment. Original Electron
references cover 1,200 multiline actions per platform, including forced newlines,
Arabic ligatures, mixed-direction text and combining characters. Headless tests
exercise every selectable wrapped cell edge and the corresponding IME candidate
position; nonempty candidate bounds use the actual physical selection intervals.
These changes also touch src/input/movement.rs, text_wrapper.rs and element.rs.
Text revisions invalidate stale preferred cursor coordinates after committed
or IME insertion; paint refreshes the coordinate from the current layout.
An additional 240 independent original insertion/navigation cases cover this
path on macOS/Linux. Long multiline placeholders use the same wrapping as text,
with empty carets at the paragraph edge. Masked Unicode candidate ranges and
point hits map display cells back to source UTF-16 offsets.

StyledExt::refine_style preserves a widget's existing background color when a
runtime image fill comes from a background-image longhand. Explicit colors and
background shorthands retain their own cascade behavior. Native gradient paint
checks cover this path alongside corners, opacity, resizing and cache eviction.

`src/animation.rs` exposes the transition destination for current CSS pseudo-state
matching. Hover selectors cease matching immediately on pointer leave, while
existing transition progress continues driving the visual fade.

Resizable panel groups keep zero minimum width/height so a percentage-sized group
can shrink inside a flex column with a header and playerbar. A native pointer
regression reorders lyrics beneath the queue, keeps its controls above the
playerbar, restores the order and closes a combined lyrics/visualizer panel.

Resizable groups also expose an opt-in auto-fit allocation for the original
sidebar: explicit heights remain fixed, remaining panels share available space
and divider allocation remains reserved. The ordinary proportional-resize policy
stays the default. Native geometry checks compare 120 original rendered layouts,
including saved sizes larger than the available space.

Inline rich text exposes its displayed string as an accessible label. This makes
export previews, biographies and other formatted text available to assistive
technology without adding another painted copy. The export fixture verifies
that its accessible preview follows the edited offset and matches copied/saved
text. Rich-text updates retain their existing 200 ms platform debounce.
Inline links share GPUI's focus/accessibility controls with lyric links. Their
ordinary pointer handling uses aligned, wrapped UTF-8 range boxes and the inverse
paint transform; left clicks preserve selection suppression and URL filtering.
Keyboard focus reveals offscreen links, Enter follows the pointer path, and
Space preserves ordinary anchor behavior. HTML and Markdown checks cover
accessibility activation and scrolling; the shared alignment matrix also covers
wrapped labels, Arabic and scaled text.

Disclosure buttons expose their expanded state through `aria_expanded`, retaining the button's existing focus and keyboard handling.

Input cursor blink/pause timers use GPUI's platform scheduler. The normal 500 ms
blink and 300 ms pause are unchanged; headless tests can advance the same clock
without wall-clock callbacks keeping a slow render loop busy. The component
regression checks both deadlines and invalidation after stopping.


Popovers remember the prior focus when opening and restore it when they close
while focus still belongs to their content. This keeps application shortcuts
available after dismissing a numeric editor or starting a sleep timer, without
replacing focus that has already moved outside. Native player-popover tests
exercise mouse/keyboard editing, dismissal and focus restoration in four locales.

Popovers retain the optional `defer_content(false)` mode. The native player and
output popovers now use deferred rendering, including their nested dropdowns,
through GPUI's nested overlay support. They escape scrolling content clips and
retain focus ancestry. Player configuration tests cover compact Settings, nested
menus, choosing values and dismissing the correct popup across four locales.

Sliders emit `ChangeEnd` on pointer release, including outside release, and on
completed accessibility adjustments. Continuous consumers retain `Change`.
Sliders accept keyboard focus, arrows and Home/End, with a visible focus ring.
The RTL option mirrors horizontal pointer geometry, track fill, thumb placement
and Left/Right keys while preserving vertical movement. Native pointer listeners
retain small movements and outside drags without a drag-and-drop threshold.
Stepped pointer and accessibility values use the configured minimum as their
origin. Disabled controls suppress adjustment and cancel pending pointer edits.
The application slider wrapper applies its current locale to every slider.
Original Mantine callbacks supply keyboard and pointer reference fixtures.
Relative fill and thumb positions paint correctly on the first frame and resize.
Dynamic domains preserve subscriptions; cancelling a pointer edit prevents an old
track's release from seeking its replacement. Optional formatted thumb labels
appear on hover and while dragging. The player-bar variant preserves the original
compact padding, rem-based track/thumb dimensions, extended fill, RTL thumb
offset, formatted label typography/shadow and drag scale/shadows. A shared CSS
transition drives its 200 ms hover/focus thumb opacity and foreground-to-primary
fill color; keyboard focus also reveals the control.
Crossfade duration uses the committed event while playback speed remains live;
the native tests exercise both paths and disabled controls.

Context menus now retain the previous focus across opening/replacement and
restore it on dismissal while focus still belongs to the menu. The native
volume-device test verifies that keyboard shortcuts work after choosing a device.

Popover triggers expose expanded state and handle keyboard/accessibility clicks
on the trigger itself. Mouse opening remains on the existing press path, so a
release does not toggle the popup twice. Button and sidebar dropdown triggers
implement the stateful interactivity contract using their existing element IDs.
The application-menu regression exercises accessibility activation, keyboard
opening, mouse selection, navigation and focus restoration; the playback and
speaker popover checks cover the shared behavior.

The optional continuous ColorPicker uses the original Mantine hue/saturation
gestures, RGB rounding, slider/thumb geometry and five-swatch row. Preview events
remain distinct from release commits, Escape cancels drags, and accessibility
value/step actions share the same conversion. Its white-to-transparent saturation
layer retains white RGB at zero alpha for GPUI's straight-alpha interpolation.
Independent original callbacks, browser layouts and GPU pixel samples are tested
in the native theme-settings suite. The legacy discrete palette remains available.

The spectrum state exposes `is_dragging` so consumers that mirror its preview in
an editable field can keep a queued focus-loss event from committing that preview.
The native theme regression retains RGB input focus before the pointer gesture;
it failed by saving the saturation midpoint before release and passes with the
guard. The normal typed-input Enter/Blur and final pointer commit remain enabled.

Wrapped application buttons allow GPUI's normal pointer focus, committing the
previous input's Blur notification before Click. Disabled wrapped buttons clear
input focus without activating. Their focus-visible CSS and built-in outline
follow the window's keyboard modality. Internal unwrapped widget buttons retain
their existing pointer-focus policy. Five original-browser states verify event
order, focus, disabled behavior and keyboard activation.

Open popovers restore their container focus when focus is cleared, so a disabled
child click keeps Escape/Tab available. This uses the existing per-window focus-
lost subscription, defers until the application fallback has run, and preserves
descendant focus. Ordinary focus moves and popup dismissal proceed normally.

Root applies the same deferred focus recovery to the top active dialog or sheet
when its focused control disappears or becomes disabled. This preserves Escape
after canceling a lyrics export save dialog, while retaining descendant focus
and normal nested-dialog restoration.
The player configuration regression covers disabled controls followed by Escape.

Searchable Select uses named combo-box/list-box/option accessibility nodes and
tracks focus on its actual trigger. Lists can leave results unselected until the
first arrow key; Select keeps the query after selection or Escape. Replacing
items reapplies the current query, so saved/deleted presets and locale changes
update the open list without discarding the search. Reset clears transient state
when an enabled-only control is removed. Native audio-preset tests exercise the
original browser's filtering, case/whitespace handling, keyboard selection,
cancellation, re-enabling and custom preset save/load/delete.

Select accepts the same application state-style callback used by other wrapped
controls, resolving input, dropdown and option aliases at render time. Focus,
disabled, hover and keyboard selection retain their interaction behavior while
custom CSS overrides paint and geometry. The trailing control is positioned
independently of input padding. Twelve original wrapped Select states verify
native paint at two font sizes, including focused and disabled inputs, hovered
options and custom colors/radii; audio preset behavior is checked separately.

Calendar day, month and year choices expose button roles and disabled state.
Day buttons carry the full ISO date as their accessible name, allowing the
native smart-playlist calendar to support screen-reader activation and precise
pointer tests. Calendar navigation follows its active day/month/year level and emits a selection
only when a day is confirmed. The application retains direct date typing.


Select retains its confirmed item separately from transient filtered/highlighted
indices, so a canceled no-result search cannot erase the displayed label. Item
refresh resolves confirmed and highlighted values against the new choices before
reapplying the query. This preserves keyboard search while labels/options change.
Selected values are exposed to accessibility; empty group headings are omitted.
The application's original-component comparisons cover cancellation, live
relabeling/reordering and neighboring audio preset behavior.


Calendar grids use the actual number of weeks (four through six), keep separate
month boundaries and accept a configured week start. The native rule editor uses
Monday and opens at the month chooser, matching the original date input. Keyboard
navigation retains an independent cursor, handles all three levels, and applies
the same disabled/range-selection path as pointer activation. Month-level arrows
move between years. Original four/five/six-week and leap/month-end fixtures are
compared against the actual native calendar grid and pointer selection.
