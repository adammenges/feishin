// todo("windows"): remove
#![cfg_attr(windows, allow(dead_code))]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    AtlasTextureId, AtlasTile, Background, Bounds, ContentMask, Corners, DevicePixels, Edges, Hsla,
    Pixels, Point, Radians, ScaledPixels, Size, bounds_tree::BoundsTree, point,
};
use std::{
    fmt::Debug,
    iter::Peekable,
    ops::{Add, Range, Sub},
    slice,
};

#[allow(non_camel_case_types, unused)]
pub(crate) type PathVertex_ScaledPixels = PathVertex<ScaledPixels>;

pub(crate) type DrawOrder = u32;

#[derive(Default)]
pub(crate) struct Scene {
    pub(crate) paint_operations: Vec<PaintOperation>,
    primitive_bounds: BoundsTree<ScaledPixels>,
    layer_stack: Vec<DrawOrder>,
    pub(crate) shadows: Vec<Shadow>,
    pub(crate) quads: Vec<Quad>,
    pub(crate) paths: Vec<Path<ScaledPixels>>,
    pub(crate) underlines: Vec<Underline>,
    pub(crate) monochrome_sprites: Vec<MonochromeSprite>,
    pub(crate) polychrome_sprites: Vec<PolychromeSprite>,
    pub(crate) surfaces: Vec<PaintSurface>,
    pub(crate) backdrop_blurs: Vec<BackdropBlur>,
    backdrops_fenced: bool,
}

impl Scene {
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn remap_atlas_tile_for_test(&mut self, old: &AtlasTile, new: &AtlasTile) {
        for sprite in &mut self.monochrome_sprites {
            if &sprite.tile == old {
                sprite.tile = new.clone()
            }
        }
        for sprite in &mut self.polychrome_sprites {
            if &sprite.tile == old {
                sprite.tile = new.clone()
            }
        }
        for operation in &mut self.paint_operations {
            let tile = match operation {
                PaintOperation::Primitive(Primitive::MonochromeSprite(sprite)) => &mut sprite.tile,
                PaintOperation::Primitive(Primitive::PolychromeSprite(sprite)) => &mut sprite.tile,
                _ => continue,
            };
            if tile == old {
                *tile = new.clone()
            }
        }
    }

    pub fn clear(&mut self) {
        self.paint_operations.clear();
        self.primitive_bounds.clear();
        self.layer_stack.clear();
        self.paths.clear();
        self.shadows.clear();
        self.quads.clear();
        self.underlines.clear();
        self.monochrome_sprites.clear();
        self.polychrome_sprites.clear();
        self.surfaces.clear();
        self.backdrop_blurs.clear();
        self.backdrops_fenced = false;
    }

    pub fn len(&self) -> usize {
        self.paint_operations.len()
    }

    pub fn push_layer(&mut self, bounds: Bounds<ScaledPixels>) {
        let order = self.primitive_bounds.insert(bounds);
        self.layer_stack.push(order);
        self.paint_operations
            .push(PaintOperation::StartLayer(bounds));
    }

    pub fn pop_layer(&mut self) {
        self.layer_stack.pop();
        self.paint_operations.push(PaintOperation::EndLayer);
    }

    pub fn insert_primitive(&mut self, primitive: impl Into<Primitive>) {
        let mut primitive = primitive.into();
        let bounds = match &primitive {
            Primitive::MonochromeSprite(sprite) => sprite.transformed_bounds(),
            _ => *primitive.bounds(),
        };
        let clipped_bounds = bounds.intersect(&primitive.content_mask().bounds);

        // Clipped scope endings still restore their parent surface and balance
        // the renderer's snapshot stack, even when they composite no pixels.
        if clipped_bounds.is_empty()
            && !matches!(&primitive, Primitive::BackdropBlur(blur) if blur.opacity_scope.is_some())
        {
            return;
        }

        let order = self
            .layer_stack
            .last()
            .copied()
            .unwrap_or_else(|| self.primitive_bounds.insert(clipped_bounds));
        match &mut primitive {
            Primitive::Shadow(shadow) => {
                shadow.order = order;
                self.shadows.push(shadow.clone());
            }
            Primitive::Quad(quad) => {
                quad.order = order;
                self.quads.push(quad.clone());
            }
            Primitive::Path(path) => {
                path.order = order;
                path.id = PathId(self.paths.len());
                self.paths.push(path.clone());
            }
            Primitive::Underline(underline) => {
                underline.order = order;
                self.underlines.push(underline.clone());
            }
            Primitive::MonochromeSprite(sprite) => {
                sprite.order = order;
                self.monochrome_sprites.push(sprite.clone());
            }
            Primitive::PolychromeSprite(sprite) => {
                sprite.order = order;
                self.polychrome_sprites.push(sprite.clone());
            }
            Primitive::Surface(surface) => {
                surface.order = order;
                self.surfaces.push(surface.clone());
            }
            Primitive::BackdropBlur(blur) => {
                blur.order = order;
                self.backdrop_blurs.push(blur.clone());
            }
        }
        self.paint_operations
            .push(PaintOperation::Primitive(primitive));
    }

    pub fn replay(&mut self, range: Range<usize>, prev_scene: &Scene) {
        for operation in &prev_scene.paint_operations[range] {
            match operation {
                PaintOperation::Primitive(primitive) => self.insert_primitive(primitive.clone()),
                PaintOperation::StartLayer(bounds) => self.push_layer(*bounds),
                PaintOperation::EndLayer => self.pop_layer(),
            }
        }
    }

    pub fn finish(&mut self) {
        if !self.backdrop_blurs.is_empty() && !self.backdrops_fenced {
            self.fence_backdrops();
            self.backdrops_fenced = true;
        }
        self.backdrop_blurs.sort_by_key(|blur| blur.order);
        self.shadows.sort_by_key(|shadow| shadow.order);
        self.quads.sort_by_key(|quad| quad.order);
        self.paths.sort_by_key(|path| path.order);
        self.underlines.sort_by_key(|underline| underline.order);
        self.monochrome_sprites
            .sort_by_key(|sprite| (sprite.order, sprite.tile.tile_id));
        self.polychrome_sprites
            .sort_by_key(|sprite| (sprite.order, sprite.tile.tile_id));
        self.surfaces.sort_by_key(|surface| surface.order);
    }

    // Backdrops read the scene painted before them. Keep normal batching within
    // each segment, but prevent later primitives in a shared layer from moving
    // ahead of the read, including when paint operations are replayed.
    fn fence_backdrops(&mut self) {
        let stride = self
            .paint_operations
            .iter()
            .filter_map(|operation| {
                if let PaintOperation::Primitive(primitive) = operation {
                    Some(primitive.order())
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(0)
            .checked_add(2)
            .expect("Too many scene draw orders");
        let mut base = 0u32;
        let mut indices = [0; 8];
        for operation in &self.paint_operations {
            let PaintOperation::Primitive(primitive) = operation else {
                continue;
            };
            let order = if matches!(primitive, Primitive::BackdropBlur(_)) {
                base = base
                    .checked_add(stride)
                    .expect("Too many backdrop draw orders");
                let order = base;
                base = base
                    .checked_add(stride)
                    .expect("Too many backdrop draw orders");
                order
            } else {
                base.checked_add(primitive.order())
                    .expect("Too many scene draw orders")
            };
            let index = match primitive {
                Primitive::Shadow(_) => {
                    self.shadows[indices[0]].order = order;
                    0
                }
                Primitive::Quad(_) => {
                    self.quads[indices[1]].order = order;
                    1
                }
                Primitive::Path(_) => {
                    self.paths[indices[2]].order = order;
                    2
                }
                Primitive::Underline(_) => {
                    self.underlines[indices[3]].order = order;
                    3
                }
                Primitive::MonochromeSprite(_) => {
                    self.monochrome_sprites[indices[4]].order = order;
                    4
                }
                Primitive::PolychromeSprite(_) => {
                    self.polychrome_sprites[indices[5]].order = order;
                    5
                }
                Primitive::Surface(_) => {
                    self.surfaces[indices[6]].order = order;
                    6
                }
                Primitive::BackdropBlur(_) => {
                    self.backdrop_blurs[indices[7]].order = order;
                    7
                }
            };
            indices[index] += 1;
        }
    }

    #[cfg_attr(
        all(
            any(target_os = "linux", target_os = "freebsd"),
            not(any(feature = "x11", feature = "wayland"))
        ),
        allow(dead_code)
    )]
    pub(crate) fn batches(&self) -> impl Iterator<Item = PrimitiveBatch<'_>> {
        BatchIterator {
            shadows: &self.shadows,
            shadows_start: 0,
            shadows_iter: self.shadows.iter().peekable(),
            quads: &self.quads,
            quads_start: 0,
            quads_iter: self.quads.iter().peekable(),
            paths: &self.paths,
            paths_start: 0,
            paths_iter: self.paths.iter().peekable(),
            underlines: &self.underlines,
            underlines_start: 0,
            underlines_iter: self.underlines.iter().peekable(),
            monochrome_sprites: &self.monochrome_sprites,
            monochrome_sprites_start: 0,
            monochrome_sprites_iter: self.monochrome_sprites.iter().peekable(),
            polychrome_sprites: &self.polychrome_sprites,
            polychrome_sprites_start: 0,
            polychrome_sprites_iter: self.polychrome_sprites.iter().peekable(),
            surfaces: &self.surfaces,
            surfaces_start: 0,
            surfaces_iter: self.surfaces.iter().peekable(),
            backdrop_blurs: &self.backdrop_blurs,
            backdrop_blurs_start: 0,
            backdrop_blurs_iter: self.backdrop_blurs.iter().peekable(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Default)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
pub(crate) enum PrimitiveKind {
    Shadow,
    #[default]
    Quad,
    Path,
    Underline,
    MonochromeSprite,
    PolychromeSprite,
    Surface,
    BackdropBlur,
}

pub(crate) enum PaintOperation {
    Primitive(Primitive),
    StartLayer(Bounds<ScaledPixels>),
    EndLayer,
}

#[derive(Clone)]
pub(crate) enum Primitive {
    Shadow(Shadow),
    Quad(Quad),
    Path(Path<ScaledPixels>),
    Underline(Underline),
    MonochromeSprite(MonochromeSprite),
    PolychromeSprite(PolychromeSprite),
    Surface(PaintSurface),
    BackdropBlur(BackdropBlur),
}

impl Primitive {
    pub fn bounds(&self) -> &Bounds<ScaledPixels> {
        match self {
            Primitive::Shadow(shadow) => &shadow.bounds,
            Primitive::Quad(quad) => &quad.bounds,
            Primitive::Path(path) => &path.bounds,
            Primitive::Underline(underline) => &underline.bounds,
            Primitive::MonochromeSprite(sprite) => &sprite.bounds,
            Primitive::PolychromeSprite(sprite) => &sprite.bounds,
            Primitive::Surface(surface) => &surface.bounds,
            Primitive::BackdropBlur(blur) => &blur.bounds,
        }
    }

    fn order(&self) -> DrawOrder {
        match self {
            Primitive::Shadow(value) => value.order,
            Primitive::Quad(value) => value.order,
            Primitive::Path(value) => value.order,
            Primitive::Underline(value) => value.order,
            Primitive::MonochromeSprite(value) => value.order,
            Primitive::PolychromeSprite(value) => value.order,
            Primitive::Surface(value) => value.order,
            Primitive::BackdropBlur(value) => value.order,
        }
    }

    pub fn content_mask(&self) -> &ContentMask<ScaledPixels> {
        match self {
            Primitive::Shadow(shadow) => &shadow.content_mask,
            Primitive::Quad(quad) => &quad.content_mask,
            Primitive::Path(path) => &path.content_mask,
            Primitive::Underline(underline) => &underline.content_mask,
            Primitive::MonochromeSprite(sprite) => &sprite.content_mask,
            Primitive::PolychromeSprite(sprite) => &sprite.content_mask,
            Primitive::Surface(surface) => &surface.content_mask,
            Primitive::BackdropBlur(blur) => &blur.content_mask,
        }
    }
}

#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
struct BatchIterator<'a> {
    shadows: &'a [Shadow],
    shadows_start: usize,
    shadows_iter: Peekable<slice::Iter<'a, Shadow>>,
    quads: &'a [Quad],
    quads_start: usize,
    quads_iter: Peekable<slice::Iter<'a, Quad>>,
    paths: &'a [Path<ScaledPixels>],
    paths_start: usize,
    paths_iter: Peekable<slice::Iter<'a, Path<ScaledPixels>>>,
    underlines: &'a [Underline],
    underlines_start: usize,
    underlines_iter: Peekable<slice::Iter<'a, Underline>>,
    monochrome_sprites: &'a [MonochromeSprite],
    monochrome_sprites_start: usize,
    monochrome_sprites_iter: Peekable<slice::Iter<'a, MonochromeSprite>>,
    polychrome_sprites: &'a [PolychromeSprite],
    polychrome_sprites_start: usize,
    polychrome_sprites_iter: Peekable<slice::Iter<'a, PolychromeSprite>>,
    surfaces: &'a [PaintSurface],
    surfaces_start: usize,
    surfaces_iter: Peekable<slice::Iter<'a, PaintSurface>>,
    backdrop_blurs: &'a [BackdropBlur],
    backdrop_blurs_start: usize,
    backdrop_blurs_iter: Peekable<slice::Iter<'a, BackdropBlur>>,
}

impl<'a> Iterator for BatchIterator<'a> {
    type Item = PrimitiveBatch<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut orders_and_kinds = [
            (
                self.shadows_iter.peek().map(|s| s.order),
                PrimitiveKind::Shadow,
            ),
            (self.quads_iter.peek().map(|q| q.order), PrimitiveKind::Quad),
            (self.paths_iter.peek().map(|q| q.order), PrimitiveKind::Path),
            (
                self.underlines_iter.peek().map(|u| u.order),
                PrimitiveKind::Underline,
            ),
            (
                self.monochrome_sprites_iter.peek().map(|s| s.order),
                PrimitiveKind::MonochromeSprite,
            ),
            (
                self.polychrome_sprites_iter.peek().map(|s| s.order),
                PrimitiveKind::PolychromeSprite,
            ),
            (
                self.surfaces_iter.peek().map(|s| s.order),
                PrimitiveKind::Surface,
            ),
            (
                self.backdrop_blurs_iter.peek().map(|blur| blur.order),
                PrimitiveKind::BackdropBlur,
            ),
        ];
        orders_and_kinds.sort_by_key(|(order, kind)| (order.unwrap_or(u32::MAX), *kind));

        let first = orders_and_kinds[0];
        let second = orders_and_kinds[1];
        let (batch_kind, max_order_and_kind) = if first.0.is_some() {
            (first.1, (second.0.unwrap_or(u32::MAX), second.1))
        } else {
            return None;
        };

        match batch_kind {
            PrimitiveKind::Shadow => {
                let shadows_start = self.shadows_start;
                let mut shadows_end = shadows_start + 1;
                self.shadows_iter.next();
                while self
                    .shadows_iter
                    .next_if(|shadow| (shadow.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    shadows_end += 1;
                }
                self.shadows_start = shadows_end;
                Some(PrimitiveBatch::Shadows(
                    &self.shadows[shadows_start..shadows_end],
                ))
            }
            PrimitiveKind::Quad => {
                let quads_start = self.quads_start;
                let mut quads_end = quads_start + 1;
                self.quads_iter.next();
                while self
                    .quads_iter
                    .next_if(|quad| (quad.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    quads_end += 1;
                }
                self.quads_start = quads_end;
                Some(PrimitiveBatch::Quads(&self.quads[quads_start..quads_end]))
            }
            PrimitiveKind::Path => {
                let paths_start = self.paths_start;
                let mut paths_end = paths_start + 1;
                self.paths_iter.next();
                while self
                    .paths_iter
                    .next_if(|path| (path.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    paths_end += 1;
                }
                self.paths_start = paths_end;
                Some(PrimitiveBatch::Paths(&self.paths[paths_start..paths_end]))
            }
            PrimitiveKind::Underline => {
                let underlines_start = self.underlines_start;
                let mut underlines_end = underlines_start + 1;
                self.underlines_iter.next();
                while self
                    .underlines_iter
                    .next_if(|underline| (underline.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    underlines_end += 1;
                }
                self.underlines_start = underlines_end;
                Some(PrimitiveBatch::Underlines(
                    &self.underlines[underlines_start..underlines_end],
                ))
            }
            PrimitiveKind::MonochromeSprite => {
                let texture_id = self.monochrome_sprites_iter.peek().unwrap().tile.texture_id;
                let sprites_start = self.monochrome_sprites_start;
                let mut sprites_end = sprites_start + 1;
                self.monochrome_sprites_iter.next();
                while self
                    .monochrome_sprites_iter
                    .next_if(|sprite| {
                        (sprite.order, batch_kind) < max_order_and_kind
                            && sprite.tile.texture_id == texture_id
                    })
                    .is_some()
                {
                    sprites_end += 1;
                }
                self.monochrome_sprites_start = sprites_end;
                Some(PrimitiveBatch::MonochromeSprites {
                    texture_id,
                    sprites: &self.monochrome_sprites[sprites_start..sprites_end],
                })
            }
            PrimitiveKind::PolychromeSprite => {
                let texture_id = self.polychrome_sprites_iter.peek().unwrap().tile.texture_id;
                let sprites_start = self.polychrome_sprites_start;
                let mut sprites_end = self.polychrome_sprites_start + 1;
                self.polychrome_sprites_iter.next();
                while self
                    .polychrome_sprites_iter
                    .next_if(|sprite| {
                        (sprite.order, batch_kind) < max_order_and_kind
                            && sprite.tile.texture_id == texture_id
                    })
                    .is_some()
                {
                    sprites_end += 1;
                }
                self.polychrome_sprites_start = sprites_end;
                Some(PrimitiveBatch::PolychromeSprites {
                    texture_id,
                    sprites: &self.polychrome_sprites[sprites_start..sprites_end],
                })
            }
            PrimitiveKind::BackdropBlur => {
                let start = self.backdrop_blurs_start;
                self.backdrop_blurs_iter.next();
                self.backdrop_blurs_start += 1;
                Some(PrimitiveBatch::BackdropBlurs(
                    &self.backdrop_blurs[start..start + 1],
                ))
            }
            PrimitiveKind::Surface => {
                let surfaces_start = self.surfaces_start;
                let mut surfaces_end = surfaces_start + 1;
                self.surfaces_iter.next();
                while self
                    .surfaces_iter
                    .next_if(|surface| (surface.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    surfaces_end += 1;
                }
                self.surfaces_start = surfaces_end;
                Some(PrimitiveBatch::Surfaces(
                    &self.surfaces[surfaces_start..surfaces_end],
                ))
            }
        }
    }
}

#[derive(Debug)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
pub(crate) enum PrimitiveBatch<'a> {
    Shadows(&'a [Shadow]),
    Quads(&'a [Quad]),
    Paths(&'a [Path<ScaledPixels>]),
    Underlines(&'a [Underline]),
    MonochromeSprites {
        texture_id: AtlasTextureId,
        sprites: &'a [MonochromeSprite],
    },
    PolychromeSprites {
        texture_id: AtlasTextureId,
        sprites: &'a [PolychromeSprite],
    },
    Surfaces(&'a [PaintSurface]),
    BackdropBlurs(&'a [BackdropBlur]),
}

/// Ordered CSS backdrop operations. Blur lengths are in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum BackdropFilterOperation {
    /// Gaussian standard deviation in logical pixels.
    Blur(f32),
    /// Nonnegative sRGB brightness multiplier.
    Brightness(f32),
    /// Nonnegative sRGB contrast multiplier around mid-gray.
    Contrast(f32),
    /// Nonnegative saturation multiplier.
    Saturate(f32),
    /// Grayscale blend, from zero to one.
    Grayscale(f32),
    /// Sepia blend, from zero to one.
    Sepia(f32),
    /// Hue rotation in radians.
    HueRotate(f32),
    /// Color inversion blend, from zero to one.
    Invert(f32),
    /// Premultiplied opacity multiplier, from zero to one.
    Opacity(f32),
    /// Replace RGB with a solid color while retaining source alpha coverage.
    MaskColor([f32; 4]),
    /// Fade a layer at its vertical edges, with logical coordinates and a fractional ramp.
    VerticalAlphaMask {
        /// Top of the layer in logical pixels.
        top: f32,
        /// Height of the layer in logical pixels.
        height: f32,
        /// Fraction of the height occupied by each edge ramp.
        fade: f32,
    },
    /// Alpha-mask shadow, composited below its input before the next function.
    DropShadow {
        /// Horizontal and vertical offsets in logical pixels.
        offset: [f32; 2],
        /// Gaussian standard deviation in logical pixels.
        sigma: f32,
        /// None uses the element's computed text color.
        color: Option<[f32; 4]>,
    },
}

impl BackdropFilterOperation {
    pub(crate) fn amount(self) -> f32 {
        match self {
            Self::Blur(n)
            | Self::Brightness(n)
            | Self::Contrast(n)
            | Self::Saturate(n)
            | Self::Grayscale(n)
            | Self::Sepia(n)
            | Self::HueRotate(n)
            | Self::Invert(n)
            | Self::Opacity(n) => n,
            Self::DropShadow { sigma, .. } => sigma,
            Self::MaskColor(_) | Self::VerticalAlphaMask { .. } => 0.,
        }
    }

    pub(crate) fn is_valid(self) -> bool {
        if let Self::VerticalAlphaMask { top, height, fade } = self {
            return top.is_finite()
                && height.is_finite()
                && height > 0.
                && fade.is_finite()
                && fade > 0.
                && fade <= 0.5;
        }
        if let Self::MaskColor(color) = self {
            return color
                .into_iter()
                .all(|n| n.is_finite() && (0. ..=1.).contains(&n));
        }
        if let Self::DropShadow {
            offset,
            sigma,
            color,
        } = self
        {
            sigma.is_finite()
                && (0. ..=10000.).contains(&sigma)
                && offset
                    .into_iter()
                    .all(|n| n.is_finite() && n.abs() <= 10000.)
                && color.is_none_or(|color| {
                    color
                        .into_iter()
                        .all(|n| n.is_finite() && (0. ..=1.).contains(&n))
                })
        } else {
            self.amount().is_finite()
        }
    }

    pub(crate) fn scaled(self, scale: f32, current_color: Hsla) -> Self {
        match self {
            Self::VerticalAlphaMask { top, height, fade } => Self::VerticalAlphaMask {
                top: top * scale,
                height: height * scale,
                fade,
            },
            Self::Blur(sigma) => Self::Blur(sigma * scale),
            Self::DropShadow {
                offset,
                sigma,
                color,
            } => {
                let rgba: crate::Rgba = current_color.into();
                Self::DropShadow {
                    offset: offset.map(|n| n * scale),
                    sigma: sigma * scale,
                    color: Some(color.unwrap_or([rgba.r, rgba.g, rgba.b, rgba.a])),
                }
            }
            other => other,
        }
    }

    pub(crate) fn blur_sigma(self) -> Option<f32> {
        match self {
            Self::Blur(sigma) | Self::DropShadow { sigma, .. } => Some(sigma),
            _ => None,
        }
    }

    fn color_parameters(self) -> [f32; 4] {
        let code = match self {
            Self::Blur(_)
            | Self::DropShadow { .. }
            | Self::MaskColor(_)
            | Self::VerticalAlphaMask { .. } => 0.,
            Self::Brightness(_) => 1.,
            Self::Contrast(_) => 2.,
            Self::Saturate(_) => 3.,
            Self::Grayscale(_) => 4.,
            Self::Sepia(_) => 5.,
            Self::HueRotate(_) => 6.,
            Self::Invert(_) => 7.,
            Self::Opacity(_) => 8.,
        };
        [0., 0., self.amount(), code]
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum BackdropTextureId {
    Snapshot,
    Level(u32, usize),
}

pub(crate) type BackdropParameters = [f32; 264];
pub(crate) const SHADOW_SOURCE_LEVEL: u32 = 1 << 22;

pub(crate) struct BackdropPass {
    pub source: BackdropTextureId,
    pub target: BackdropTextureId,
    pub original: Option<BackdropTextureId>,
    pub parameters: BackdropParameters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpacityScope {
    Begin,
    Refresh,
    End,
    FilterBegin,
    FilterEnd,
}

impl OpacityScope {
    pub(crate) fn is_begin(self) -> bool {
        matches!(self, Self::Begin | Self::FilterBegin)
    }

    pub(crate) fn is_end(self) -> bool {
        matches!(self, Self::End | Self::FilterEnd)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BackdropBlur {
    pub order: DrawOrder,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub corner_radii: Corners<ScaledPixels>,
    pub sigma: ScaledPixels,
    pub opacity: f32,
    pub background_alpha: f32,
    pub saturation: f32,
    pub operations: Vec<BackdropFilterOperation>,
    // Opacity scopes use the same snapshot/composite fences as backdrop reads.
    pub opacity_scope: Option<OpacityScope>,
}

impl BackdropBlur {
    // One shared pass plan keeps Metal, Vulkan and Direct3D filter order identical.
    // Level zero contains two full-size work textures, never a downsampled input.
    pub(crate) fn passes(
        &self,
        viewport: Size<DevicePixels>,
    ) -> (Vec<BackdropPass>, BackdropTextureId) {
        use BackdropTextureId::{Level, Snapshot};
        let mut passes = vec![];
        let mut source = Snapshot;
        let mut operations = self.operations.clone();
        if operations.is_empty() {
            operations.push(BackdropFilterOperation::Blur(self.sigma.0));
        }
        let mut operations = operations.into_iter().peekable();
        let mut work_index = 0;
        while let Some(operation) = operations.next() {
            let target = Level(0, work_index % 2);
            if let BackdropFilterOperation::VerticalAlphaMask { top, height, fade } = operation {
                let mut parameters = [0.; 264];
                parameters[3] = -4.;
                parameters[8..12].copy_from_slice(&[
                    top / viewport.height.0 as f32,
                    height / viewport.height.0 as f32,
                    fade,
                    0.,
                ]);
                passes.push(BackdropPass {
                    source,
                    target,
                    original: None,
                    parameters,
                });
            } else if let BackdropFilterOperation::MaskColor(color) = operation {
                let mut parameters = [0.; 264];
                parameters[3] = -3.;
                parameters[8..12].copy_from_slice(&color);
                passes.push(BackdropPass {
                    source,
                    target,
                    original: None,
                    parameters,
                });
            } else if let Some(sigma) = operation.blur_sigma() {
                let shadow =
                    if let BackdropFilterOperation::DropShadow { offset, color, .. } = operation {
                        let original = Level(SHADOW_SOURCE_LEVEL, 0);
                        passes.push(BackdropPass {
                            source,
                            target: original,
                            original: None,
                            parameters: [0.; 264],
                        });
                        // Translate before convolution so the blur can reach pixels
                        // above a positive offset instead of cutting a hard edge.
                        let shifted = Level(SHADOW_SOURCE_LEVEL, 1);
                        let mut parameters = [0.; 264];
                        parameters[0] = offset[0] / viewport.width.0 as f32;
                        parameters[1] = offset[1] / viewport.height.0 as f32;
                        parameters[3] = -2.;
                        passes.push(BackdropPass {
                            source: original,
                            target: shifted,
                            original: None,
                            parameters,
                        });
                        source = shifted;
                        Some((offset, color.unwrap_or([0., 0., 0., 1.]), original))
                    } else {
                        None
                    };
                let mut blur = self.clone();
                blur.sigma = ScaledPixels(sigma);
                if shadow.is_some() {
                    blur.opacity_scope = Some(OpacityScope::FilterEnd);
                }
                let step = blur.downsample();
                let mut level = 2;
                while level <= step {
                    let reduced = Level(level, 1);
                    passes.push(BackdropPass {
                        source,
                        target: reduced,
                        original: None,
                        parameters: [0.; 264],
                    });
                    source = reduced;
                    level *= 2;
                }
                passes.push(BackdropPass {
                    source,
                    target: Level(step, 0),
                    original: None,
                    parameters: blur.parameters(true, viewport),
                });
                passes.push(BackdropPass {
                    source: Level(step, 0),
                    target: Level(step, 1),
                    original: None,
                    parameters: blur.parameters(false, viewport),
                });
                source = Level(step, 1);
                if self.operations.is_empty() {
                    continue;
                }
                let mut parameters = [0.; 264];
                let original = shadow.map(|(_, color, original)| {
                    parameters[3] = -1.;
                    parameters[8..12].copy_from_slice(&color);
                    original
                });
                passes.push(BackdropPass {
                    source,
                    target,
                    original,
                    parameters,
                });
            } else {
                // Adjacent color functions retain float precision and clamp in order;
                // quantizing after each function differs from Chromium's filter graph.
                let mut parameters = [0.; 264];
                parameters[8..12].copy_from_slice(&operation.color_parameters());
                let mut count = 1;
                while operations.peek().is_some_and(|operation| {
                    operation.blur_sigma().is_none()
                        && !matches!(
                            operation,
                            BackdropFilterOperation::MaskColor(_)
                                | BackdropFilterOperation::VerticalAlphaMask { .. }
                        )
                }) {
                    let operation = operations.next().unwrap();
                    parameters[8 + count * 4..12 + count * 4]
                        .copy_from_slice(&operation.color_parameters());
                    count += 1;
                }
                parameters[3] = count as f32;
                passes.push(BackdropPass {
                    source,
                    target,
                    original: None,
                    parameters,
                });
            }
            source = target;
            work_index += 1;
        }
        (passes, source)
    }

    pub(crate) fn parameters(
        &self,
        horizontal: bool,
        viewport: Size<DevicePixels>,
    ) -> BackdropParameters {
        let width = viewport.width.0 as f32;
        let height = viewport.height.0 as f32;
        let step = self.downsample() as f32;
        let mut parameters = [0.; 264];
        parameters[..8].copy_from_slice(&[
            if horizontal { step / width } else { 0. },
            if horizontal { 0. } else { step / height },
            self.sigma.0 / step,
            0.,
            self.bounds.origin.x.0 / width,
            self.bounds.origin.y.0 / height,
            self.bounds.size.width.0 / width,
            self.bounds.size.height.0 / height,
        ]);
        if self.opacity_scope == Some(OpacityScope::FilterEnd) {
            // Foreground samples outside the source are transparent. Backdrops
            // retain their reflected border sampling.
            parameters[7] = -1.;
        }
        parameters
    }

    // Bound the separable kernel after averaging each 2x2 downsampling stage.
    pub(crate) fn downsample(&self) -> u32 {
        let mut scale = 1;
        while self.sigma.0 / scale as f32 > 16. && scale < (1 << 20) {
            scale *= 2;
        }
        scale
    }

    pub(crate) fn sprite(&self, viewport: Size<DevicePixels>) -> PolychromeSprite {
        PolychromeSprite {
            order: self.order,
            backdrop_background_alpha: self.background_alpha,
            grayscale: false,
            opacity: self.opacity,
            bounds: self.bounds,
            content_mask: self.content_mask.clone(),
            corner_radii: self.corner_radii,
            tile: AtlasTile {
                texture_id: AtlasTextureId {
                    index: 0,
                    kind: crate::AtlasTextureKind::Polychrome,
                },
                tile_id: crate::TileId(0),
                padding: 0,
                bounds: Bounds::new(Point::default(), viewport),
            },
            source_uv: [0., 0., 1., 1.],
            backdrop_params: [
                self.saturation,
                f32::from(self.opacity_scope == Some(OpacityScope::End)),
            ],
        }
    }
}

impl From<BackdropBlur> for Primitive {
    fn from(blur: BackdropBlur) -> Self {
        Self::BackdropBlur(blur)
    }
}

#[derive(Default, Debug, Clone)]
#[repr(C)]
pub(crate) struct Quad {
    pub order: DrawOrder,
    pub border_style: BorderStyle,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub background: Background,
    pub border_color: Hsla,
    pub corner_radii: Corners<ScaledPixels>,
    pub border_widths: Edges<ScaledPixels>,
}

impl From<Quad> for Primitive {
    fn from(quad: Quad) -> Self {
        Primitive::Quad(quad)
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub(crate) struct Underline {
    pub order: DrawOrder,
    pub pad: u32, // align to 8 bytes
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub color: Hsla,
    pub thickness: ScaledPixels,
    pub wavy: u32,
}

impl From<Underline> for Primitive {
    fn from(underline: Underline) -> Self {
        Primitive::Underline(underline)
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub(crate) struct Shadow {
    pub order: DrawOrder,
    pub blur_radius: ScaledPixels,
    pub bounds: Bounds<ScaledPixels>,
    pub corner_radii: Corners<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub color: Hsla,
}

impl From<Shadow> for Primitive {
    fn from(shadow: Shadow) -> Self {
        Primitive::Shadow(shadow)
    }
}

/// The style of a border.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[repr(C)]
pub enum BorderStyle {
    /// A solid border.
    #[default]
    Solid = 0,
    /// A dashed border.
    Dashed = 1,
}

/// A data type representing a 2 dimensional transformation that can be applied to an element.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct TransformationMatrix {
    /// 2x2 matrix containing rotation and scale,
    /// stored row-major
    pub rotation_scale: [[f32; 2]; 2],
    /// translation vector
    pub translation: [f32; 2],
}

impl Eq for TransformationMatrix {}

impl TransformationMatrix {
    /// The unit matrix, has no effect.
    pub fn unit() -> Self {
        Self {
            rotation_scale: [[1.0, 0.0], [0.0, 1.0]],
            translation: [0.0, 0.0],
        }
    }

    /// Move the origin by a given point
    pub fn translate(mut self, point: Point<ScaledPixels>) -> Self {
        self.compose(Self {
            rotation_scale: [[1.0, 0.0], [0.0, 1.0]],
            translation: [point.x.0, point.y.0],
        })
    }

    /// Clockwise rotation in radians around the origin
    pub fn rotate(self, angle: Radians) -> Self {
        self.compose(Self {
            rotation_scale: [
                [angle.0.cos(), -angle.0.sin()],
                [angle.0.sin(), angle.0.cos()],
            ],
            translation: [0.0, 0.0],
        })
    }

    /// Scale around the origin
    pub fn scale(self, size: Size<f32>) -> Self {
        self.compose(Self {
            rotation_scale: [[size.width, 0.0], [0.0, size.height]],
            translation: [0.0, 0.0],
        })
    }

    /// Perform matrix multiplication with another transformation
    /// to produce a new transformation that is the result of
    /// applying both transformations: first, `other`, then `self`.
    #[inline]
    pub fn compose(self, other: TransformationMatrix) -> TransformationMatrix {
        if other == Self::unit() {
            return self;
        }
        // Perform matrix multiplication
        TransformationMatrix {
            rotation_scale: [
                [
                    self.rotation_scale[0][0] * other.rotation_scale[0][0]
                        + self.rotation_scale[0][1] * other.rotation_scale[1][0],
                    self.rotation_scale[0][0] * other.rotation_scale[0][1]
                        + self.rotation_scale[0][1] * other.rotation_scale[1][1],
                ],
                [
                    self.rotation_scale[1][0] * other.rotation_scale[0][0]
                        + self.rotation_scale[1][1] * other.rotation_scale[1][0],
                    self.rotation_scale[1][0] * other.rotation_scale[0][1]
                        + self.rotation_scale[1][1] * other.rotation_scale[1][1],
                ],
            ],
            translation: [
                self.translation[0]
                    + self.rotation_scale[0][0] * other.translation[0]
                    + self.rotation_scale[0][1] * other.translation[1],
                self.translation[1]
                    + self.rotation_scale[1][0] * other.translation[0]
                    + self.rotation_scale[1][1] * other.translation[1],
            ],
        }
    }

    /// Apply transformation to a point, mainly useful for debugging
    pub fn apply(&self, point: Point<Pixels>) -> Point<Pixels> {
        let input = [point.x.0, point.y.0];
        let mut output = self.translation;
        for (i, output_cell) in output.iter_mut().enumerate() {
            for (k, input_cell) in input.iter().enumerate() {
                *output_cell += self.rotation_scale[i][k] * *input_cell;
            }
        }
        Point::new(output[0].into(), output[1].into())
    }
}

impl Default for TransformationMatrix {
    fn default() -> Self {
        Self::unit()
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
pub(crate) struct MonochromeSprite {
    pub order: DrawOrder,
    pub pad: u32, // align to 8 bytes
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub color: Hsla,
    pub tile: AtlasTile,
    pub transformation: TransformationMatrix,
}

impl MonochromeSprite {
    pub(crate) fn transformed_bounds(&self) -> Bounds<ScaledPixels> {
        if self.transformation == TransformationMatrix::unit() {
            return self.bounds;
        }
        let corners = [
            self.bounds.origin,
            self.bounds.top_right(),
            self.bounds.bottom_left(),
            self.bounds.bottom_right(),
        ];
        let transform = |point: Point<ScaledPixels>| {
            self.transformation
                .apply(point.map(|value| Pixels(value.0)))
                .map(|value| ScaledPixels(value.0))
        };
        let mut minimum = transform(corners[0]);
        let mut maximum = minimum;
        for corner in &corners[1..] {
            let point = transform(*corner);
            minimum = minimum.min(&point);
            maximum = maximum.max(&point);
        }
        Bounds::from_corners(minimum, maximum)
    }
}

impl From<MonochromeSprite> for Primitive {
    fn from(sprite: MonochromeSprite) -> Self {
        Primitive::MonochromeSprite(sprite)
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
pub(crate) struct PolychromeSprite {
    pub order: DrawOrder,
    // Uses the existing alignment slot; regular image sprites leave this zero.
    pub backdrop_background_alpha: f32,
    pub grayscale: bool,
    pub opacity: f32,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub corner_radii: Corners<ScaledPixels>,
    pub tile: AtlasTile,
    // Saturation and explicit padding preserve the GPU's eight-byte struct alignment.
    pub backdrop_params: [f32; 2],
    /// Normalized source origin and size for cover-cropped image sampling.
    pub source_uv: [f32; 4],
}

// Metal, WGSL and HLSL use scalar arrays for these source coordinates.
const _: () = {
    assert!(std::mem::offset_of!(PolychromeSprite, source_uv) == 104);
    assert!(std::mem::size_of::<PolychromeSprite>() == 120);
};

impl From<PolychromeSprite> for Primitive {
    fn from(sprite: PolychromeSprite) -> Self {
        Primitive::PolychromeSprite(sprite)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PaintSurface {
    pub order: DrawOrder,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    #[cfg(target_os = "macos")]
    pub image_buffer: core_video::pixel_buffer::CVPixelBuffer,
}

impl From<PaintSurface> for Primitive {
    fn from(surface: PaintSurface) -> Self {
        Primitive::Surface(surface)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PathId(pub(crate) usize);

/// A line made up of a series of vertices and control points.
#[derive(Clone, Debug)]
pub struct Path<P: Clone + Debug + Default + PartialEq> {
    pub(crate) id: PathId,
    pub(crate) order: DrawOrder,
    pub(crate) bounds: Bounds<P>,
    pub(crate) content_mask: ContentMask<P>,
    pub(crate) vertices: Vec<PathVertex<P>>,
    pub(crate) color: Background,
    start: Point<P>,
    current: Point<P>,
    contour_count: usize,
}

impl Path<Pixels> {
    /// Create a new path with the given starting point.
    pub fn new(start: Point<Pixels>) -> Self {
        Self {
            id: PathId(0),
            order: DrawOrder::default(),
            vertices: Vec::new(),
            start,
            current: start,
            bounds: Bounds {
                origin: start,
                size: Default::default(),
            },
            content_mask: Default::default(),
            color: Default::default(),
            contour_count: 0,
        }
    }

    /// Scale this path by the given factor.
    pub fn scale(&self, factor: f32) -> Path<ScaledPixels> {
        Path {
            id: self.id,
            order: self.order,
            bounds: self.bounds.scale(factor),
            content_mask: self.content_mask.scale(factor),
            vertices: self
                .vertices
                .iter()
                .map(|vertex| vertex.scale(factor))
                .collect(),
            start: self.start.map(|start| start.scale(factor)),
            current: self.current.scale(factor),
            contour_count: self.contour_count,
            color: self.color,
        }
    }

    /// Move the start, current point to the given point.
    pub fn move_to(&mut self, to: Point<Pixels>) {
        self.contour_count += 1;
        self.start = to;
        self.current = to;
    }

    /// Draw a straight line from the current point to the given point.
    pub fn line_to(&mut self, to: Point<Pixels>) {
        self.contour_count += 1;
        if self.contour_count > 1 {
            self.push_triangle(
                (self.start, self.current, to),
                (point(0., 1.), point(0., 1.), point(0., 1.)),
            );
        }
        self.current = to;
    }

    /// Draw a curve from the current point to the given point, using the given control point.
    pub fn curve_to(&mut self, to: Point<Pixels>, ctrl: Point<Pixels>) {
        self.contour_count += 1;
        if self.contour_count > 1 {
            self.push_triangle(
                (self.start, self.current, to),
                (point(0., 1.), point(0., 1.), point(0., 1.)),
            );
        }

        self.push_triangle(
            (self.current, ctrl, to),
            (point(0., 0.), point(0.5, 0.), point(1., 1.)),
        );
        self.current = to;
    }

    /// Push a triangle to the Path.
    pub fn push_triangle(
        &mut self,
        xy: (Point<Pixels>, Point<Pixels>, Point<Pixels>),
        st: (Point<f32>, Point<f32>, Point<f32>),
    ) {
        self.bounds = self
            .bounds
            .union(&Bounds {
                origin: xy.0,
                size: Default::default(),
            })
            .union(&Bounds {
                origin: xy.1,
                size: Default::default(),
            })
            .union(&Bounds {
                origin: xy.2,
                size: Default::default(),
            });

        self.vertices.push(PathVertex {
            xy_position: xy.0,
            st_position: st.0,
            content_mask: Default::default(),
        });
        self.vertices.push(PathVertex {
            xy_position: xy.1,
            st_position: st.1,
            content_mask: Default::default(),
        });
        self.vertices.push(PathVertex {
            xy_position: xy.2,
            st_position: st.2,
            content_mask: Default::default(),
        });
    }
}

impl<T> Path<T>
where
    T: Clone + Debug + Default + PartialEq + PartialOrd + Add<T, Output = T> + Sub<Output = T>,
{
    #[allow(unused)]
    pub(crate) fn clipped_bounds(&self) -> Bounds<T> {
        self.bounds.intersect(&self.content_mask.bounds)
    }
}

impl From<Path<ScaledPixels>> for Primitive {
    fn from(path: Path<ScaledPixels>) -> Self {
        Primitive::Path(path)
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
pub(crate) struct PathVertex<P: Clone + Debug + Default + PartialEq> {
    pub(crate) xy_position: Point<P>,
    pub(crate) st_position: Point<f32>,
    pub(crate) content_mask: ContentMask<P>,
}

impl PathVertex<Pixels> {
    pub fn scale(&self, factor: f32) -> PathVertex<ScaledPixels> {
        PathVertex {
            xy_position: self.xy_position.scale(factor),
            st_position: self.st_position,
            content_mask: self.content_mask.scale(factor),
        }
    }
}
