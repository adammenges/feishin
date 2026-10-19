# Feishin native Taffy patch

Source: the complete Taffy 0.9.0 crates.io package, MIT, archive SHA-256
`a13e5d13f79d558b5d353a98072ca8ca0e99da429467804de959aa8c83c9a004`.
The package records revision `599b1b7d7410d97e13bafc4c01fbe6d7a9cf060a`.
Its unchanged license is copied from that revision into `LICENSE.md` and native
binary packages.

`src/tree/cache.rs` and `src/tree/taffy_tree.rs` invalidate ancestor final-layout
entries when a descendant needs a new final layout. A cached ancestor result does
not replay its descendants' stored layouts. Intrinsic block measurements inside
flex layout can otherwise overwrite text-node bounds and leave the final centered
text outside its clipping box. Size measurements and unchanged final layouts
continue to use the existing cache.

A library regression checks wide root layout, narrow descendant layout, and root
restoration. The application regression exercises live carousel CSS, image
filters, hover text, clearing CSS, and both carousel styles at two zoom levels.

The high-level layout API accepts a calc resolver for caller-owned opaque handles.
Container algorithms and measured/unmeasured leaves use the same callback. Existing
callers retain the previous API and default behavior. GPUI supplies bounded affine
pixel/percentage terms through aligned integer handles, without dereferencing
pointers. Native browser comparisons cover dimensions, min/max sizes, flex basis,
physical/logical margins and padding, negative-result clamping and zoom.
