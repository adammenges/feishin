use crate::{
    Bounds, DevicePixels, Font, FontFallbacks, FontFeatures, FontId, FontMetrics, FontRun,
    FontStyle, FontVariations, FontWeight, GlyphId, LineLayout, Pixels, PlatformTextSystem, Point,
    RenderGlyphParams, Result, SUBPIXEL_VARIANTS_X, ShapedGlyph, ShapedRun, SharedString, Size,
    TextCell, point, px, size, swap_rgba_pa_to_bgra,
};
use cocoa::appkit::CGFloat;
use collections::{HashMap, HashSet};
use core_foundation::{
    attributed_string::CFMutableAttributedString,
    base::{CFRange, CFType, CFTypeRef, TCFType},
    dictionary::{CFDictionary, CFDictionaryRef},
    number::CFNumber,
    string::CFString,
};
use core_graphics::{
    base::{CGGlyph, kCGImageAlphaNoneSkipLast, kCGImageAlphaPremultipliedLast},
    color_space::CGColorSpace,
    context::{CGContext, CGPathDrawingMode, CGTextDrawingMode},
    display::CGPoint,
    geometry::CGAffineTransform,
};
use core_text::{
    font::{CTFont, CTFontRef},
    font_descriptor::{
        kCTFontDefaultOrientation, kCTFontSlantTrait, kCTFontSymbolicTrait, kCTFontWeightTrait,
        kCTFontWidthTrait,
    },
    line::CTLine,
    run::CTRunRef,
    string_attributes::{kCTFontAttributeName, kCTParagraphStyleAttributeName},
};
use font_kit::{
    font::Font as FontKitFont,
    handle::Handle,
    metrics::Metrics,
    properties::{Style as FontkitStyle, Weight as FontkitWeight},
    source::SystemSource,
    sources::mem::MemSource,
};
use parking_lot::{RwLock, RwLockUpgradableReadGuard};
use pathfinder_geometry::{
    rect::{RectF, RectI},
    vector::{Vector2F, Vector2I},
};
use smallvec::SmallVec;
use std::{borrow::Cow, char, convert::TryFrom, sync::Arc};
use unicode_segmentation::UnicodeSegmentation;

use super::open_type::{
    apply_features_and_fallbacks, apply_synthetic_italic, apply_variable_optical_size,
    apply_variable_weight, apply_variations, needs_synthetic_bold,
};

// CoreText's Rust bindings omit paragraph styles and secondary insertion positions.
#[repr(C)]
struct ParagraphStyleSetting {
    spec: u32,
    value_size: usize,
    value: *const std::ffi::c_void,
}

unsafe extern "C" {
    fn CTParagraphStyleCreate(settings: *const ParagraphStyleSetting, count: usize) -> CFTypeRef;
    fn CTRunGetAdvances(
        run: CTRunRef,
        range: CFRange,
        advances: *mut core_graphics::geometry::CGSize,
    );
    fn CTRunGetStatus(run: CTRunRef) -> u32;
    fn CTRunGetStringRange(run: CTRunRef) -> CFRange;
    fn CTFontCopyVariation(font: CTFontRef) -> CFDictionaryRef;
}

struct MemoryShaper {
    bytes: Arc<Vec<u8>>,
    data: harfrust::ShaperData,
}

#[derive(Clone, PartialEq, Eq)]
struct NativeFont(CTFont);
impl std::hash::Hash for NativeFont {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        unsafe { core_foundation::base::CFHash(self.0.as_CFTypeRef()) }.hash(state);
    }
}

#[allow(non_upper_case_globals)]
const kCGImageAlphaOnly: u32 = 7;

pub(crate) struct MacTextSystem(RwLock<MacTextSystemState>);

#[derive(Clone, PartialEq, Eq, Hash)]
struct FontKey {
    font_family: SharedString,
    font_features: FontFeatures,
    font_variations: FontVariations,
    font_fallbacks: Option<FontFallbacks>,
    font_weight: FontWeight,
}

struct MacTextSystemState {
    memory_source: MemSource,
    system_source: SystemSource,
    fonts: Vec<FontKitFont>,
    font_selections: HashMap<Font, FontId>,
    font_ids_by_native_font: HashMap<(NativeFont, bool, FontVariations, Option<Pixels>), FontId>,
    font_ids_by_font_key: HashMap<FontKey, SmallVec<[FontId; 4]>>,
    postscript_names_by_font_id: HashMap<FontId, String>,
    memory_shapers: HashMap<String, MemoryShaper>,
    font_features: HashMap<FontId, FontFeatures>,
    optical_sizes: HashMap<FontId, Pixels>,
    variations: HashMap<FontId, FontVariations>,
    bold_requested: HashSet<FontId>,
    synthetic_bold: HashSet<FontId>,
    /// UTF-16 indices of ZWNJS
    zwnjs_scratch_space: Vec<usize>,
}

impl MacTextSystem {
    pub(crate) fn new() -> Self {
        Self(RwLock::new(MacTextSystemState {
            memory_source: MemSource::empty(),
            system_source: SystemSource::new(),
            fonts: Vec::new(),
            font_selections: HashMap::default(),
            font_ids_by_native_font: HashMap::default(),
            font_ids_by_font_key: HashMap::default(),
            postscript_names_by_font_id: HashMap::default(),
            memory_shapers: HashMap::default(),
            font_features: HashMap::default(),
            optical_sizes: HashMap::default(),
            variations: HashMap::default(),
            bold_requested: HashSet::default(),
            synthetic_bold: HashSet::default(),
            zwnjs_scratch_space: Vec::new(),
        }))
    }
}

impl Default for MacTextSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformTextSystem for MacTextSystem {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        self.0.write().add_fonts(fonts)
    }

    fn all_font_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        let collection = core_text::font_collection::create_for_all_families();
        let Some(descriptors) = collection.get_descriptors() else {
            return names;
        };
        for descriptor in descriptors.into_iter() {
            names.extend(lenient_font_attributes::family_name(&descriptor));
        }
        if let Ok(fonts_in_memory) = self.0.read().memory_source.all_families() {
            names.extend(fonts_in_memory);
        }
        names
    }

    fn font_id(&self, font: &Font) -> Result<FontId> {
        let lock = self.0.upgradable_read();
        if let Some(font_id) = lock.font_selections.get(font) {
            Ok(*font_id)
        } else {
            let mut lock = RwLockUpgradableReadGuard::upgrade(lock);
            let font_key = FontKey {
                font_family: font.family.clone(),
                font_features: font.features.clone(),
                font_variations: font.variations.clone(),
                font_fallbacks: font.fallbacks.clone(),
                font_weight: font.weight,
            };
            let candidates = if let Some(font_ids) = lock.font_ids_by_font_key.get(&font_key) {
                font_ids.as_slice()
            } else {
                let font_ids = lock.load_family(
                    &font.family,
                    &font.features,
                    &font.variations,
                    font.fallbacks.as_ref(),
                    font.weight,
                )?;
                lock.font_ids_by_font_key.insert(font_key.clone(), font_ids);
                lock.font_ids_by_font_key[&font_key].as_ref()
            };

            let candidate_properties = candidates
                .iter()
                .map(|font_id| lock.fonts[font_id.0].properties())
                .collect::<SmallVec<[_; 4]>>();

            let ix = font_kit::matching::find_best_match(
                &candidate_properties,
                &font_kit::properties::Properties {
                    style: font.style.into(),
                    weight: font.weight.into(),
                    stretch: Default::default(),
                },
            )?;

            let mut font_id = candidates[ix];
            let mut selected = lock.fonts[font_id.0].clone();
            let weight = if let Some((family, weight)) = font
                .variations
                .fallback_companion(&selected.native_font().postscript_name())
            {
                font_id = lock.load_family(
                    family,
                    &font.features,
                    &font.variations,
                    font.fallbacks.as_ref(),
                    font.weight,
                )?[0];
                selected = lock.fonts[font_id.0].clone();
                weight
            } else {
                font.weight.0
            };
            let weighted = apply_variable_weight(&mut selected, weight)?;
            let optical = if let Some(size) = font.optical_size {
                apply_variable_optical_size(&mut selected, f32::from(size))?
            } else {
                false
            };
            apply_variations(&mut selected, &font.variations)?;
            let bold = needs_synthetic_bold(&selected, font.weight.0);
            let italic = apply_synthetic_italic(&mut selected, font.style);
            if weighted
                || optical
                || italic
                || bold
                || font.optical_size.is_some()
                || font.variations.iter().next().is_some()
            {
                let features = lock.font_features[&font_id].clone();
                font_id = FontId(lock.fonts.len());
                lock.font_ids_by_native_font.insert(
                    (
                        NativeFont(selected.native_font()),
                        bold,
                        font.variations.clone(),
                        font.variations.get("opsz").map(px).or(font.optical_size),
                    ),
                    font_id,
                );
                lock.postscript_names_by_font_id
                    .insert(font_id, selected.native_font().postscript_name());
                lock.fonts.push(selected);
                lock.font_features.insert(font_id, features);
                if bold {
                    lock.synthetic_bold.insert(font_id);
                }
            }
            lock.variations.insert(font_id, font.variations.clone());
            if let Some(size) = font.variations.get("opsz").map(px).or(font.optical_size) {
                lock.optical_sizes.insert(font_id, size);
            }
            if font.weight.0 >= 600. {
                lock.bold_requested.insert(font_id);
            }
            lock.font_selections.insert(font.clone(), font_id);
            Ok(font_id)
        }
    }

    fn font_metrics(&self, font_id: FontId) -> FontMetrics {
        self.0.read().fonts[font_id.0].metrics().into()
    }

    fn typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Bounds<f32>> {
        Ok(self.0.read().fonts[font_id.0]
            .typographic_bounds(glyph_id.0)?
            .into())
    }

    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        self.0.read().advance(font_id, glyph_id)
    }

    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId> {
        self.0.read().glyph_for_char(font_id, ch)
    }

    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        self.0.read().raster_bounds(params)
    }

    fn rasterize_glyph(
        &self,
        glyph_id: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        self.0.read().rasterize_glyph(glyph_id, raster_bounds)
    }

    fn layout_line(&self, text: &str, font_size: Pixels, font_runs: &[FontRun]) -> LineLayout {
        self.0.write().layout_line(text, font_size, font_runs)
    }
}

impl MacTextSystemState {
    fn add_fonts(&mut self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        let fonts = fonts
            .into_iter()
            .map(|bytes| {
                let bytes = Arc::new(bytes.into_owned());
                let handle = Handle::from_memory(bytes.clone(), 0);
                let font = handle.load()?;
                if let (Some(name), Ok(reference)) =
                    (font.postscript_name(), harfrust::FontRef::new(&bytes))
                {
                    self.memory_shapers.insert(
                        name,
                        MemoryShaper {
                            data: harfrust::ShaperData::new(&reference),
                            bytes,
                        },
                    );
                }
                Ok(handle)
            })
            .collect::<Result<Vec<_>>>()?;
        let names = fonts
            .iter()
            .filter_map(|handle| handle.load().ok()?.postscript_name())
            .collect::<collections::HashSet<_>>();
        let previous = self
            .memory_source
            .all_fonts()?
            .into_iter()
            .filter(|handle| {
                handle
                    .load()
                    .ok()
                    .and_then(|font| font.postscript_name())
                    .is_none_or(|name| !names.contains(&name))
            });
        self.memory_source = MemSource::from_fonts(fonts.into_iter().chain(previous))?;
        self.font_selections.clear();
        self.font_ids_by_font_key.clear();
        Ok(())
    }

    fn load_family(
        &mut self,
        name: &str,
        features: &FontFeatures,
        variations: &FontVariations,
        fallbacks: Option<&FontFallbacks>,
        weight: FontWeight,
    ) -> Result<SmallVec<[FontId; 4]>> {
        let name = crate::text_system::font_name_with_fallbacks(name, ".AppleSystemUIFont");

        let mut font_ids = SmallVec::new();
        let family = self
            .memory_source
            .select_family_by_name(name)
            .or_else(|_| self.system_source.select_family_by_name(name))?;
        let fallback_descriptors = fallbacks.map(|fallbacks| {
            fallbacks
                .fallback_list()
                .iter()
                .filter_map(|name| {
                    let family = self
                        .memory_source
                        .select_family_by_name(name)
                        .or_else(|_| self.system_source.select_family_by_name(name))
                        .ok()?;
                    let fonts = family
                        .fonts()
                        .iter()
                        .filter_map(|font| font.load().ok())
                        .collect::<Vec<_>>();
                    let properties = fonts
                        .iter()
                        .map(FontKitFont::properties)
                        .collect::<Vec<_>>();
                    let index = font_kit::matching::find_best_match(
                        &properties,
                        &font_kit::properties::Properties {
                            weight: weight.into(),
                            ..Default::default()
                        },
                    )
                    .ok()?;
                    let mut font = fonts[index].clone();
                    let weight = if let Some((family, weight)) =
                        variations.fallback_companion(&font.native_font().postscript_name())
                    {
                        font = self
                            .memory_source
                            .select_family_by_name(family)
                            .ok()?
                            .fonts()
                            .first()?
                            .load()
                            .ok()?;
                        weight
                    } else {
                        weight.0
                    };
                    if let Err(error) = apply_variable_weight(&mut font, weight) {
                        log::warn!("Cannot select fallback font weight: {error}");
                    }
                    if let Err(error) = apply_variations(&mut font, variations) {
                        log::warn!("Cannot apply fallback font variations: {error}");
                    }
                    if let Err(error) = apply_features_and_fallbacks(&mut font, features, None) {
                        log::warn!("Cannot apply fallback font features: {error}");
                    }
                    Some(font.native_font().copy_descriptor())
                })
                .collect::<Vec<_>>()
        });
        for font in family.fonts() {
            let mut font = font.load()?;

            apply_features_and_fallbacks(&mut font, features, fallback_descriptors.as_deref())?;
            // This block contains a precautionary fix to guard against loading fonts
            // that might cause panics due to `.unwrap()`s up the chain.
            {
                // We use the 'm' character for text measurements in various spots
                // (e.g., the editor). However, at time of writing some of those usages
                // will panic if the font has no 'm' glyph.
                //
                // Therefore, we check up front that the font has the necessary glyph.
                let has_m_glyph = font.glyph_for_char('m').is_some();

                // HACK: The 'Segoe Fluent Icons' font does not have an 'm' glyph,
                // but we need to be able to load it for rendering Windows icons in
                // the Storybook (on macOS).
                let is_segoe_fluent_icons = font.full_name() == "Segoe Fluent Icons";

                if !has_m_glyph && !is_segoe_fluent_icons {
                    // I spent far too long trying to track down why a font missing the 'm'
                    // character wasn't loading. This log statement will hopefully save
                    // someone else from suffering the same fate.
                    log::warn!(
                        "font '{}' has no 'm' character and was not loaded",
                        font.full_name()
                    );
                    continue;
                }
            }

            // We've seen a number of panics in production caused by calling font.properties()
            // which unwraps a downcast to CFNumber. This is an attempt to avoid the panic,
            // and to try and identify the incalcitrant font.
            let traits = font.native_font().all_traits();
            if unsafe {
                !(traits
                    .get(kCTFontSymbolicTrait)
                    .downcast::<CFNumber>()
                    .is_some()
                    && traits
                        .get(kCTFontWidthTrait)
                        .downcast::<CFNumber>()
                        .is_some()
                    && traits
                        .get(kCTFontWeightTrait)
                        .downcast::<CFNumber>()
                        .is_some()
                    && traits
                        .get(kCTFontSlantTrait)
                        .downcast::<CFNumber>()
                        .is_some())
            } {
                log::error!(
                    "Failed to read traits for font {:?}",
                    font.postscript_name().unwrap()
                );
                continue;
            }

            let font_id = FontId(self.fonts.len());
            font_ids.push(font_id);
            let postscript_name = font.postscript_name().unwrap();
            self.font_ids_by_native_font.insert(
                (
                    NativeFont(font.native_font()),
                    false,
                    FontVariations::default(),
                    None,
                ),
                font_id,
            );
            self.postscript_names_by_font_id
                .insert(font_id, postscript_name);
            self.fonts.push(font);
            self.font_features.insert(font_id, features.clone());
        }
        Ok(font_ids)
    }

    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        Ok(self.fonts[font_id.0].advance(glyph_id.0)?.into())
    }

    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId> {
        self.fonts[font_id.0].glyph_for_char(ch).map(GlyphId)
    }

    fn id_for_native_font(
        &mut self,
        requested_font: CTFont,
        bold_requested: bool,
        variations: FontVariations,
        optical_size: Option<Pixels>,
    ) -> FontId {
        let postscript_name = requested_font.postscript_name();
        let mut font = unsafe { FontKitFont::from_native_font(&requested_font) };
        let optical_size = variations.get("opsz").map(px).or(optical_size);
        if let Some(size) = optical_size
            && let Err(error) = apply_variable_optical_size(&mut font, size.0)
        {
            log::warn!("Cannot apply resolved fallback optical size: {error}");
        }
        if let Err(error) = apply_variations(&mut font, &variations) {
            log::warn!("Cannot apply resolved fallback font variations: {error}");
        }
        let bold = bold_requested && needs_synthetic_bold(&font, 600.);
        let key = (
            NativeFont(font.native_font()),
            bold,
            variations.clone(),
            optical_size,
        );
        if let Some(font_id) = self.font_ids_by_native_font.get(&key) {
            *font_id
        } else {
            let font_id = FontId(self.fonts.len());
            self.font_ids_by_native_font.insert(key, font_id);
            self.postscript_names_by_font_id
                .insert(font_id, postscript_name);
            self.fonts.push(font);
            if bold {
                self.synthetic_bold.insert(font_id);
            }
            if let Some(size) = optical_size {
                self.optical_sizes.insert(font_id, size);
            }
            self.variations.insert(font_id, variations);
            // Native fallback IDs are also used to measure isolated punctuation.
            self.font_features.insert(font_id, FontFeatures::default());
            font_id
        }
    }

    fn is_emoji(&self, font_id: FontId) -> bool {
        self.postscript_names_by_font_id
            .get(&font_id)
            .is_some_and(|postscript_name| {
                postscript_name == "AppleColorEmoji" || postscript_name == ".AppleColorEmojiUI"
            })
    }

    fn shape_memory(
        &mut self,
        font_id: FontId,
        text: &str,
        rtl: bool,
        font_size: Pixels,
        features: &FontFeatures,
    ) -> Option<harfrust::GlyphBuffer> {
        let native = self.fonts[font_id.0].native_font();
        let name = native.postscript_name();
        if !self.memory_shapers.contains_key(&name) {
            // CoreText gives optical/variation instances generated PostScript
            // names. Their original memory face still owns the shaping tables.
            let font = self
                .memory_source
                .select_best_match(
                    &[font_kit::family_name::FamilyName::Title(
                        native.family_name(),
                    )],
                    &self.fonts[font_id.0].properties(),
                )
                .or_else(|_| self.system_source.select_by_postscript_name(&name))
                .ok()?
                .load()
                .ok()?;
            let bytes = font.copy_font_data()?;
            let reference = harfrust::FontRef::new(&bytes).ok()?;
            self.memory_shapers.insert(
                name.clone(),
                MemoryShaper {
                    data: harfrust::ShaperData::new(&reference),
                    bytes,
                },
            );
        }
        let memory = self.memory_shapers.get(&name)?;
        let font = harfrust::FontRef::new(&memory.bytes).ok()?;
        let mut variations = Vec::new();
        unsafe {
            let values = CTFontCopyVariation(native.as_concrete_TypeRef());
            if !values.is_null() {
                let values = CFDictionary::<CFNumber, CFNumber>::wrap_under_create_rule(values);
                let (tags, values) = values.get_keys_and_values();
                for (tag, value) in tags.into_iter().zip(values) {
                    let tag = CFNumber::wrap_under_get_rule(tag as _);
                    let value = CFNumber::wrap_under_get_rule(value as _);
                    if let (Some(tag), Some(value)) = (tag.to_i64(), value.to_f64()) {
                        variations.push(harfrust::Variation {
                            tag: harfrust::Tag::from_be_bytes((tag as u32).to_be_bytes()),
                            value: value as f32,
                        });
                    }
                }
            }
        }
        variations.push(harfrust::Variation {
            tag: harfrust::Tag::new(b"opsz"),
            value: self
                .optical_sizes
                .get(&font_id)
                .copied()
                .unwrap_or(font_size)
                .0,
        });
        let instance = harfrust::ShaperInstance::from_variations(&font, variations);
        let shaper = memory.data.shaper(&font).instance(Some(&instance)).build();
        let mut buffer = harfrust::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.set_direction(if rtl {
            harfrust::Direction::RightToLeft
        } else {
            harfrust::Direction::LeftToRight
        });
        buffer.guess_segment_properties();
        let features = features
            .tag_value_list()
            .iter()
            .filter_map(|(tag, value)| {
                let tag: &[u8; 4] = tag.as_bytes().try_into().ok()?;
                Some(harfrust::Feature::new(
                    harfrust::Tag::new(tag),
                    *value,
                    0..usize::MAX,
                ))
            })
            .collect::<Vec<_>>();
        Some(shaper.shape(buffer, &features))
    }

    fn raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        // Optical axes and native hinting depend on the actual drawing size.
        // Scaling a base-size bounding rectangle can clip the final glyph.
        // CoreText bitmap glyphs must be requested at device size; scaling the
        // context selects a different emoji strike and changes its ink bounds.
        let font = self.fonts[params.font_id.0]
            .native_font()
            .clone_with_font_size(
                (params.font_size.0
                    * if params.is_emoji {
                        params.scale_factor
                    } else {
                        1.
                    }) as CGFloat,
            );
        let rect = font.get_bounding_rects_for_glyphs(
            kCTFontDefaultOrientation,
            &[params.glyph_id.0 as CGGlyph],
        );
        let scale = if params.is_emoji {
            1.
        } else {
            params.scale_factor
        };
        let mut bounds: Bounds<DevicePixels> = RectF::new(
            Vector2F::new(
                rect.origin.x as f32 * scale,
                -(rect.origin.y + rect.size.height) as f32 * scale,
            ),
            Vector2F::new(
                rect.size.width as f32 * scale,
                rect.size.height as f32 * scale,
            ),
        )
        .round_out()
        .to_i32()
        .into();
        if !params.is_emoji && bounds.size.width.0 > 0 && bounds.size.height.0 > 0 {
            // CoreGraphics font smoothing can cover the adjacent pixels.
            bounds.origin.x -= DevicePixels(1);
            bounds.origin.y -= DevicePixels(1);
            bounds.size.width += DevicePixels(2);
            bounds.size.height += DevicePixels(2);
        }
        if self.synthetic_bold.contains(&params.font_id)
            && !params.is_emoji
            && bounds.size.width.0 > 0
            && bounds.size.height.0 > 0
        {
            let outset = DevicePixels(
                // The stroke uses a miter join with a limit of four radii.
                (synthetic_bold_stroke(params.font_size.0) * params.scale_factor * 2.).ceil()
                    as i32,
            );
            bounds.origin.x -= outset;
            bounds.origin.y -= outset;
            bounds.size.width += DevicePixels(outset.0 * 2);
            bounds.size.height += DevicePixels(outset.0 * 2);
        }
        Ok(bounds)
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        glyph_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        if glyph_bounds.size.width.0 == 0 || glyph_bounds.size.height.0 == 0 {
            anyhow::bail!("glyph bounds are empty");
        } else {
            // Add an extra pixel when the subpixel variant isn't zero to make room for anti-aliasing.
            let mut bitmap_size = glyph_bounds.size;
            if params.subpixel_variant.x > 0 {
                bitmap_size.width += DevicePixels(1);
            }
            if params.subpixel_variant.y > 0 {
                bitmap_size.height += DevicePixels(1);
            }
            let bitmap_size = bitmap_size;
            let synthetic_bold = self.synthetic_bold.contains(&params.font_id) && !params.is_emoji;
            let raster_width = bitmap_size.width.0 as usize * if synthetic_bold { 4 } else { 1 };

            let mut bytes;
            let cx;
            if params.is_emoji {
                bytes = vec![0; bitmap_size.width.0 as usize * 4 * bitmap_size.height.0 as usize];
                cx = CGContext::create_bitmap_context(
                    Some(bytes.as_mut_ptr() as *mut _),
                    bitmap_size.width.0 as usize,
                    bitmap_size.height.0 as usize,
                    8,
                    bitmap_size.width.0 as usize * 4,
                    &CGColorSpace::create_device_rgb(),
                    kCGImageAlphaPremultipliedLast,
                );
            } else if synthetic_bold {
                bytes = vec![0; raster_width * bitmap_size.height.0 as usize];
                cx = CGContext::create_bitmap_context(
                    Some(bytes.as_mut_ptr() as *mut _),
                    raster_width,
                    bitmap_size.height.0 as usize,
                    8,
                    raster_width,
                    &CGColorSpace::create_device_gray(),
                    kCGImageAlphaOnly,
                );
            } else {
                // CoreGraphics only applies its native font smoothing to color
                // surfaces. Match Skia's black-on-white mask extraction.
                bytes = vec![255; raster_width * bitmap_size.height.0 as usize * 4];
                cx = CGContext::create_bitmap_context(
                    Some(bytes.as_mut_ptr() as *mut _),
                    raster_width,
                    bitmap_size.height.0 as usize,
                    8,
                    raster_width * 4,
                    &CGColorSpace::create_device_rgb(),
                    kCGImageAlphaNoneSkipLast,
                );
            }

            // Move the origin to bottom left and account for scaling, this
            // makes drawing text consistent with the font-kit's raster_bounds.
            if synthetic_bold {
                cx.scale(4., 1.);
            }
            cx.translate(
                -glyph_bounds.origin.x.0 as CGFloat,
                (glyph_bounds.origin.y.0 + glyph_bounds.size.height.0) as CGFloat,
            );
            let drawing_scale = if params.is_emoji {
                1.
            } else {
                params.scale_factor
            };
            cx.scale(drawing_scale as CGFloat, drawing_scale as CGFloat);

            let subpixel_shift = params
                .subpixel_variant
                .map(|v| v as f32 / SUBPIXEL_VARIANTS_X as f32);
            cx.set_text_drawing_mode(CGTextDrawingMode::CGTextFill);
            cx.set_gray_fill_color(0.0, 1.0);
            if synthetic_bold {
                cx.set_text_drawing_mode(CGTextDrawingMode::CGTextFillStroke);
                cx.set_line_width(synthetic_bold_stroke(params.font_size.0) as CGFloat);
                cx.set_rgb_stroke_color(0., 0., 0., 1.);
                cx.set_miter_limit(4.);
            }
            cx.set_allows_antialiasing(true);
            cx.set_should_antialias(true);
            cx.set_allows_font_smoothing(true);
            cx.set_should_smooth_fonts(true);
            cx.set_allows_font_subpixel_positioning(true);
            cx.set_should_subpixel_position_fonts(true);
            cx.set_allows_font_subpixel_quantization(false);
            cx.set_should_subpixel_quantize_fonts(false);
            let font = self.fonts[params.font_id.0]
                .native_font()
                .clone_with_font_size(
                    (f32::from(params.font_size)
                        * if params.is_emoji {
                            params.scale_factor
                        } else {
                            1.
                        }) as CGFloat,
                );
            if synthetic_bold {
                let matrix = CGAffineTransform::new(1., 0., 0., 1., 0., 0.);
                let path = font
                    .create_path_for_glyph(params.glyph_id.0 as CGGlyph, &matrix)
                    .map_err(|()| anyhow::anyhow!("Cannot extract synthetic bold glyph outline"))?;
                cx.translate(
                    (subpixel_shift.x / params.scale_factor) as CGFloat,
                    (subpixel_shift.y / params.scale_factor) as CGFloat,
                );
                cx.add_path(&path);
                cx.draw_path(CGPathDrawingMode::CGPathFillStroke);
            } else {
                font.draw_glyphs(
                    &[params.glyph_id.0 as CGGlyph],
                    &[CGPoint::new(
                        (subpixel_shift.x / params.scale_factor) as CGFloat,
                        (subpixel_shift.y / params.scale_factor) as CGFloat,
                    )],
                    cx,
                );
            }

            if params.is_emoji {
                // Convert from RGBA with premultiplied alpha to BGRA with straight alpha.
                for pixel in bytes.chunks_exact_mut(4) {
                    swap_rgba_pa_to_bgra(pixel);
                }
            }

            if synthetic_bold {
                bytes = synthetic_bold_mask(&bytes, raster_width, bitmap_size.height.0 as usize);
            } else if !params.is_emoji {
                bytes = bytes
                    .chunks_exact(4)
                    .map(|pixel| {
                        let coverage = 54 * u32::from(255 - pixel[0])
                            + 183 * u32::from(255 - pixel[1])
                            + 19 * u32::from(255 - pixel[2]);
                        (coverage / 256) as u8
                    })
                    .collect();
            }
            Ok((bitmap_size, bytes))
        }
    }

    fn layout_line(&mut self, text: &str, font_size: Pixels, font_runs: &[FontRun]) -> LineLayout {
        const ZWNJ: char = '\u{200C}';
        const ZWNJ_STR: &str = "\u{200C}";
        const ZWNJ_SIZE_16: usize = ZWNJ.len_utf16();

        self.zwnjs_scratch_space.clear();
        // Construct the attributed string, converting UTF8 ranges to UTF16 ranges.
        let mut string = CFMutableAttributedString::new();
        let mut max_ascent = 0.0f32;
        let mut max_descent = 0.0f32;

        {
            let mut ix_converter = StringIndexConverter::new(&text);
            let mut last_font_run = None;
            for run in font_runs {
                let font_size = run.font_size;
                let text = &text[ix_converter.utf8_ix..][..run.len];
                // if the fonts are the same, we need to disconnect the text with a ZWNJ
                // to prevent core text from forming ligatures between them
                let needs_zwnj = last_font_run.replace(run.font_id) == Some(run.font_id);

                let utf16_start = string.char_len(); // insert at end of string
                ix_converter.advance_to_utf8_ix(ix_converter.utf8_ix + run.len);

                // note: replace_str may silently ignore codepoints it dislikes (e.g., BOM at start of string)
                string.replace_str(&CFString::new(text), CFRange::init(utf16_start, 0));
                if needs_zwnj {
                    let zwnjs_pos = string.char_len();
                    self.zwnjs_scratch_space.push(zwnjs_pos as usize);
                    string.replace_str(
                        &CFString::from_static_string(ZWNJ_STR),
                        CFRange::init(zwnjs_pos, 0),
                    );
                }
                let utf16_end = string.char_len();

                let cf_range = CFRange::init(utf16_start, utf16_end - utf16_start);
                let font = &self.fonts[run.font_id.0];

                let font_metrics = font.metrics();
                let font_scale = font_size.0 / font_metrics.units_per_em as f32;
                max_ascent = max_ascent.max(font_metrics.ascent * font_scale);
                max_descent = max_descent.max(-font_metrics.descent * font_scale);

                unsafe {
                    string.set_attribute(
                        cf_range,
                        kCTFontAttributeName,
                        &font.native_font().clone_with_font_size(font_size.into()),
                    );
                }
            }
        }
        if let Some(rtl) = font_runs.first().and_then(|run| run.text_direction.rtl()) {
            let direction = i8::from(rtl);
            let setting = ParagraphStyleSetting {
                spec: 13, // kCTParagraphStyleSpecifierBaseWritingDirection
                value_size: std::mem::size_of_val(&direction),
                value: &direction as *const _ as _,
            };
            unsafe {
                let style = CFType::wrap_under_create_rule(CTParagraphStyleCreate(&setting, 1));
                string.set_attribute(
                    CFRange::init(0, string.char_len()),
                    kCTParagraphStyleAttributeName,
                    &style,
                );
            }
        }
        // Retrieve the glyphs from the shaped line, converting UTF16 offsets to UTF8 offsets.
        let line = CTLine::new_with_attributed_string(string.as_concrete_TypeRef());
        let glyph_runs = line.glyph_runs();
        let mut runs = <Vec<ShapedRun>>::with_capacity(glyph_runs.len() as usize);
        let mut ix_converter = StringIndexConverter::new(text);
        let graphemes = text
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .chain(std::iter::once(text.len()))
            .collect::<Vec<_>>();
        let mut clusters: HashMap<usize, (Pixels, Pixels, bool)> = HashMap::default();
        let use_cells = text
            .graphemes(true)
            .any(|grapheme| grapheme.chars().count() > 1)
            || glyph_runs
                .iter()
                .any(|run| unsafe { CTRunGetStatus(run.as_concrete_TypeRef()) & 1 != 0 });
        let mut pen_x = px(0.);
        let mut native_x = px(0.);
        for run in glyph_runs.into_iter() {
            let native_start = native_x;
            native_x += px(run.get_typographic_bounds().width as f32);
            let run_shift = pen_x - native_start;
            let attributes = run.attributes().unwrap();
            let font = unsafe {
                attributes
                    .get(kCTFontAttributeName)
                    .downcast::<CTFont>()
                    .unwrap()
            };
            let rtl = unsafe { CTRunGetStatus(run.as_concrete_TypeRef()) & 1 != 0 };
            let source = unsafe { CTRunGetStringRange(run.as_concrete_TypeRef()) };
            let to_original = |index: usize| {
                index
                    - self
                        .zwnjs_scratch_space
                        .partition_point(|inserted| *inserted < index)
            };
            let mut source_converter = StringIndexConverter::new(text);
            source_converter.advance_to_utf16_ix(to_original(source.location as usize));
            let start = source_converter.utf8_ix;
            source_converter
                .advance_to_utf16_ix(to_original((source.location + source.length) as usize));
            let end = source_converter.utf8_ix;
            let fragment = &text[start..end];
            let mut source_end = 0;
            let requested = font_runs.iter().find(|run| {
                source_end += run.len;
                start < source_end
            });
            let bold_requested =
                requested.is_some_and(|run| self.bold_requested.contains(&run.font_id));
            let requested_style = requested.and_then(|run| {
                self.font_selections
                    .iter()
                    .find_map(|(font, id)| (*id == run.font_id).then(|| font.clone()))
            });
            let font = requested_style.as_ref().map_or(font.clone(), |style| {
                let family = font.family_name();
                if family == style.family.as_ref()
                    || self.memory_source.select_family_by_name(&family).is_ok()
                {
                    return font.clone();
                }
                // Browser system fallbacks match the requested CSS weight in the
                // selected family, independently of the primary face's default cascade.
                let weight = style.weight.0;
                let ct_weight = if weight <= 50. || weight >= 950. {
                    0.
                } else {
                    [-0.8, -0.6, -0.4, 0., 0.23, 0.3, 0.4, 0.56, 0.62]
                        [((weight - 50.) / 100.) as usize]
                };
                unsafe {
                    let traits = CFDictionary::from_CFType_pairs(&[(
                        CFString::wrap_under_get_rule(kCTFontWeightTrait),
                        CFNumber::from(ct_weight).as_CFType(),
                    )]);
                    let attributes = CFDictionary::from_CFType_pairs(&[
                        (
                            CFString::wrap_under_get_rule(
                                core_text::font_descriptor::kCTFontFamilyNameAttribute,
                            ),
                            CFString::new(&family).as_CFType(),
                        ),
                        (
                            CFString::wrap_under_get_rule(
                                core_text::font_descriptor::kCTFontTraitsAttribute,
                            ),
                            traits.as_CFType(),
                        ),
                    ]);
                    core_text::font::new_from_descriptor(
                        &core_text::font_descriptor::new_from_attributes(&attributes),
                        font.pt_size(),
                    )
                }
            });
            let font_size = px(font.pt_size() as f32);
            let variations = requested
                .and_then(|run| self.variations.get(&run.font_id))
                .cloned()
                .unwrap_or_default();
            let optical_size = requested
                .and_then(|run| self.optical_sizes.get(&run.font_id))
                .copied();
            let font_id = self.id_for_native_font(font, bold_requested, variations, optical_size);

            // CoreText assigns kerning to caret halves and can replace an entire
            // decomposed grapheme. Browser shaping keeps its actual OpenType cluster.
            if rtl
                || self.is_emoji(font_id)
                || optical_size.is_some()
                || self
                    .variations
                    .get(&font_id)
                    .is_some_and(|values| values.iter().next().is_some())
                || fragment
                    .graphemes(true)
                    .any(|grapheme| grapheme.chars().count() > 1)
            {
                if let Some(requested) = requested {
                    let features = self.font_features[&requested.font_id].clone();
                    let primary = self
                        .shape_memory(requested.font_id, fragment, rtl, font_size, &features)
                        .filter(|shaped| {
                            shaped.glyph_infos().iter().all(|glyph| glyph.glyph_id != 0)
                        });
                    let (selected_id, shaped) = if let Some(primary) = primary {
                        (requested.font_id, Some(primary))
                    } else {
                        (
                            font_id,
                            self.shape_memory(font_id, fragment, rtl, font_size, &features),
                        )
                    };
                    if let Some(shaped) = shaped {
                        let scale =
                            font_size.0 / self.fonts[selected_id.0].metrics().units_per_em as f32;
                        let glyphs = shaped
                            .glyph_infos()
                            .iter()
                            .zip(shaped.glyph_positions())
                            .map(|(glyph, position)| {
                                let index = start + glyph.cluster as usize;
                                let next_x = pen_x + px(position.x_advance as f32 * scale);
                                clusters
                                    .entry(index)
                                    .and_modify(|cell| {
                                        // Marks may have compensating advances; the cluster's
                                        // insertion edges follow its total advance, not its extrema.
                                        cell.1 = next_x;
                                    })
                                    .or_insert((pen_x, next_x, rtl));
                                let result = ShapedGlyph {
                                    position_is_advance: true,
                                    id: GlyphId(glyph.glyph_id),
                                    index,
                                    position: point(pen_x, px(0.)),
                                    paint_offset: point(
                                        px(position.x_offset as f32 * scale),
                                        px(-position.y_offset as f32 * scale),
                                    ),
                                    is_emoji: self.is_emoji(selected_id),
                                };
                                pen_x = next_x;
                                result
                            })
                            .collect();
                        runs.push(ShapedRun {
                            font_size,
                            font_id: selected_id,
                            glyphs,
                        });
                        continue;
                    }
                }
            }

            let mut glyphs = match runs.last_mut() {
                Some(run) if run.font_id == font_id && run.font_size == font_size => {
                    &mut run.glyphs
                }
                _ => {
                    runs.push(ShapedRun {
                        font_size,
                        font_id,
                        glyphs: Vec::with_capacity(run.glyph_count().try_into().unwrap_or(0)),
                    });
                    &mut runs.last_mut().unwrap().glyphs
                }
            };
            let mut advances =
                vec![core_graphics::geometry::CGSize::new(0., 0.); run.glyph_count() as usize];
            unsafe {
                CTRunGetAdvances(
                    run.as_concrete_TypeRef(),
                    CFRange::init(0, 0),
                    advances.as_mut_ptr(),
                );
            }
            for (((&glyph_id, position), &glyph_utf16_ix), advance) in run
                .glyphs()
                .iter()
                .zip(run.positions().iter())
                .zip(run.string_indices().iter())
                .zip(advances)
            {
                let mut glyph_utf16_ix = usize::try_from(glyph_utf16_ix).unwrap();
                let r = self
                    .zwnjs_scratch_space
                    .binary_search_by(|&it| it.cmp(&glyph_utf16_ix));
                match r {
                    // this glyph is a ZWNJ, skip it
                    Ok(_) => continue,
                    // adjust the index to account for the ZWNJs we've inserted
                    Err(idx) => glyph_utf16_ix -= idx * ZWNJ_SIZE_16,
                }
                if ix_converter.utf16_ix > glyph_utf16_ix {
                    // We cannot reuse current index converter, as it can only seek forward. Restart the search.
                    ix_converter = StringIndexConverter::new(text);
                }
                ix_converter.advance_to_utf16_ix(glyph_utf16_ix);
                let start = graphemes[graphemes
                    .partition_point(|start| *start <= ix_converter.utf8_ix)
                    .saturating_sub(1)];
                let next_x = pen_x + px(advance.width as f32);
                clusters
                    .entry(start)
                    .and_modify(|cell| {
                        cell.0 = cell.0.min(pen_x);
                        cell.1 = cell.1.max(next_x);
                    })
                    .or_insert((pen_x, next_x, rtl));
                glyphs.push(ShapedGlyph {
                    position_is_advance: false,
                    paint_offset: Default::default(),
                    id: GlyphId(glyph_id as u32),
                    position: point(px(position.x as f32) + run_shift, px(position.y as f32)),
                    index: ix_converter.utf8_ix,
                    is_emoji: self.is_emoji(font_id),
                });
                pen_x = next_x;
            }
            pen_x = native_x + run_shift;
        }
        let mut starts = clusters.keys().copied().collect::<Vec<_>>();
        starts.sort_unstable();
        let formatting = |fragment: &str| {
            fragment.chars().all(|character| {
                matches!(
                    unicode_bidi::bidi_class(character),
                    unicode_bidi::BidiClass::LRE
                        | unicode_bidi::BidiClass::RLE
                        | unicode_bidi::BidiClass::LRO
                        | unicode_bidi::BidiClass::RLO
                        | unicode_bidi::BidiClass::PDF
                        | unicode_bidi::BidiClass::LRI
                        | unicode_bidi::BidiClass::RLI
                        | unicode_bidi::BidiClass::FSI
                        | unicode_bidi::BidiClass::PDI
                        | unicode_bidi::BidiClass::BN
                )
            })
        };
        let mut cells = starts
            .iter()
            .enumerate()
            .flat_map(|(ix, start)| {
                let (left, right, rtl) = clusters[start];
                let end = starts.get(ix + 1).copied().unwrap_or(text.len());
                let indices = graphemes
                    .iter()
                    .copied()
                    .filter(|index| *start <= *index && *index <= end)
                    .collect::<Vec<_>>();
                let count = indices
                    .windows(2)
                    .filter(|range| !formatting(&text[range[0]..range[1]]))
                    .count()
                    .max(1);
                let mut offset = 0;
                indices
                    .windows(2)
                    .map(|range| {
                        let hidden = formatting(&text[range[0]..range[1]]);
                        let step = (right - left) / count as f32;
                        let leading = if rtl {
                            right - step * offset as f32
                        } else {
                            left + step * offset as f32
                        };
                        if !hidden {
                            offset += 1;
                        }
                        let trailing = if rtl {
                            right - step * offset as f32
                        } else {
                            left + step * offset as f32
                        };
                        TextCell {
                            range: range[0]..range[1],
                            start_x: leading,
                            end_x: trailing,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        if cells
            .iter()
            .any(|cell| formatting(&text[cell.range.clone()]))
        {
            let level = font_runs
                .first()
                .and_then(|run| run.text_direction.rtl())
                .map(|rtl| {
                    if rtl {
                        unicode_bidi::Level::rtl()
                    } else {
                        unicode_bidi::Level::ltr()
                    }
                });
            let bidi = unicode_bidi::ParagraphBidiInfo::new(text, level);
            let (_, visual_runs) = bidi.visual_runs(0..text.len());
            let mut origin = px(0.);
            for range in visual_runs {
                let visible = cells
                    .iter()
                    .filter(|cell| {
                        range.contains(&cell.range.start) && !formatting(&text[cell.range.clone()])
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                for cell in &mut cells {
                    if range.contains(&cell.range.start) && formatting(&text[cell.range.clone()]) {
                        let x = visible
                            .iter()
                            .find(|next| next.range.start > cell.range.start)
                            .map(|next| next.start_x)
                            .or_else(|| {
                                visible
                                    .iter()
                                    .rev()
                                    .find(|previous| previous.range.start < cell.range.start)
                                    .map(|previous| previous.end_x)
                            })
                            .unwrap_or(origin);
                        cell.start_x = x;
                        cell.end_x = x;
                    }
                }
                let width = visible
                    .iter()
                    .map(|cell| (cell.end_x - cell.start_x).abs())
                    .fold(px(0.), |total, width| total + width);
                origin += width;
            }
        }
        let mut layout = LineLayout {
            runs,
            font_size,
            width: px(line.get_typographic_bounds().width as f32) + pen_x - native_x,
            ascent: max_ascent.into(),
            descent: max_descent.into(),
            len: text.len(),
            carets: Vec::new(),
            cells: Vec::new(),
            bidi_levels: Vec::new(),
            visual_lines: Vec::new(),
            rtl: font_runs
                .first()
                .and_then(|run| run.text_direction.rtl())
                .unwrap_or_else(|| {
                    unicode_bidi::BidiInfo::new(text, None)
                        .paragraphs
                        .first()
                        .is_some_and(|paragraph| paragraph.level.is_rtl())
                }),
        };
        if use_cells {
            layout.set_cells(text, cells);
        }
        layout
    }
}

#[derive(Debug, Clone)]
struct StringIndexConverter<'a> {
    text: &'a str,
    /// Index in UTF-8 bytes
    utf8_ix: usize,
    /// Index in UTF-16 code units
    utf16_ix: usize,
}

impl<'a> StringIndexConverter<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            utf8_ix: 0,
            utf16_ix: 0,
        }
    }

    fn advance_to_utf8_ix(&mut self, utf8_target: usize) {
        for (ix, c) in self.text[self.utf8_ix..].char_indices() {
            if self.utf8_ix + ix >= utf8_target {
                self.utf8_ix += ix;
                return;
            }
            self.utf16_ix += c.len_utf16();
        }
        self.utf8_ix = self.text.len();
    }

    fn advance_to_utf16_ix(&mut self, utf16_target: usize) {
        for (ix, c) in self.text[self.utf8_ix..].char_indices() {
            if self.utf16_ix >= utf16_target {
                self.utf8_ix += ix;
                return;
            }
            self.utf16_ix += c.len_utf16();
        }
        self.utf8_ix = self.text.len();
    }
}

// Adapted from Skia's pack4xHToMask, copyright Google Inc., BSD-3-Clause.
// See assets/skia-LICENSE.txt. CoreText normally supplies its own grayscale
// smoothing; synthesized outlines use Skia's horizontal four-sample filter.
fn synthetic_bold_mask(samples: &[u8], width: usize, height: usize) -> Vec<u8> {
    const FILTER: [[u32; 12]; 3] = [
        [3, 11, 28, 51, 64, 57, 36, 16, 5, 1, 0, 0],
        [0, 2, 8, 22, 43, 61, 61, 43, 22, 8, 2, 0],
        [0, 0, 1, 5, 16, 36, 57, 64, 51, 28, 11, 3],
    ];
    let mut mask = vec![0; width / 4 * height];
    for (row, output) in samples
        .chunks_exact(width)
        .zip(mask.chunks_exact_mut(width / 4))
    {
        for (x, alpha) in output.iter_mut().enumerate() {
            let mut channels = [0u32; 3];
            for tap in 0..12 {
                let sample = x as isize * 4 + tap as isize - 4;
                if let Some(value) = usize::try_from(sample)
                    .ok()
                    .and_then(|index| row.get(index))
                {
                    for channel in 0..3 {
                        channels[channel] += FILTER[channel][tap] * u32::from(*value);
                    }
                }
            }
            *alpha = (channels
                .into_iter()
                .map(|channel| (channel / 256).min(255))
                .sum::<u32>()
                / 3) as u8;
        }
    }
    mask
}

// Skia interpolates its synthetic bold stroke between 1/24 em at 9px and
// 1/32 em at 36px. Advances and real/variable bold faces remain unchanged.
fn synthetic_bold_stroke(size: f32) -> f32 {
    let interpolation = ((size - 9.) / 27.).clamp(0., 1.);
    size * (1. / 24. + interpolation * (1. / 32. - 1. / 24.))
}

impl From<Metrics> for FontMetrics {
    fn from(metrics: Metrics) -> Self {
        FontMetrics {
            units_per_em: metrics.units_per_em,
            ascent: metrics.ascent,
            descent: metrics.descent,
            line_gap: metrics.line_gap,
            underline_position: metrics.underline_position,
            underline_thickness: metrics.underline_thickness,
            cap_height: metrics.cap_height,
            x_height: metrics.x_height,
            bounding_box: metrics.bounding_box.into(),
        }
    }
}

impl From<RectF> for Bounds<f32> {
    fn from(rect: RectF) -> Self {
        Bounds {
            origin: point(rect.origin_x(), rect.origin_y()),
            size: size(rect.width(), rect.height()),
        }
    }
}

impl From<RectI> for Bounds<DevicePixels> {
    fn from(rect: RectI) -> Self {
        Bounds {
            origin: point(DevicePixels(rect.origin_x()), DevicePixels(rect.origin_y())),
            size: size(DevicePixels(rect.width()), DevicePixels(rect.height())),
        }
    }
}

impl From<Vector2I> for Size<DevicePixels> {
    fn from(value: Vector2I) -> Self {
        size(value.x().into(), value.y().into())
    }
}

impl From<RectI> for Bounds<i32> {
    fn from(rect: RectI) -> Self {
        Bounds {
            origin: point(rect.origin_x(), rect.origin_y()),
            size: size(rect.width(), rect.height()),
        }
    }
}

impl From<Point<u32>> for Vector2I {
    fn from(size: Point<u32>) -> Self {
        Vector2I::new(size.x as i32, size.y as i32)
    }
}

impl From<Vector2F> for Size<f32> {
    fn from(vec: Vector2F) -> Self {
        size(vec.x(), vec.y())
    }
}

impl From<FontWeight> for FontkitWeight {
    fn from(value: FontWeight) -> Self {
        FontkitWeight(value.0)
    }
}

impl From<FontStyle> for FontkitStyle {
    fn from(style: FontStyle) -> Self {
        match style {
            FontStyle::Normal => FontkitStyle::Normal,
            FontStyle::Italic => FontkitStyle::Italic,
            FontStyle::Oblique => FontkitStyle::Oblique,
        }
    }
}

// Some fonts may have no attributes despite `core_text` requiring them (and panicking).
// This is the same version as `core_text` has without `expect` calls.
mod lenient_font_attributes {
    use core_foundation::{
        base::{CFRetain, CFType, TCFType},
        string::{CFString, CFStringRef},
    };
    use core_text::font_descriptor::{
        CTFontDescriptor, CTFontDescriptorCopyAttribute, kCTFontFamilyNameAttribute,
    };

    pub fn family_name(descriptor: &CTFontDescriptor) -> Option<String> {
        unsafe { get_string_attribute(descriptor, kCTFontFamilyNameAttribute) }
    }

    fn get_string_attribute(
        descriptor: &CTFontDescriptor,
        attribute: CFStringRef,
    ) -> Option<String> {
        unsafe {
            let value = CTFontDescriptorCopyAttribute(descriptor.as_concrete_TypeRef(), attribute);
            if value.is_null() {
                return None;
            }

            let value = CFType::wrap_under_create_rule(value);
            assert!(value.instance_of::<CFString>());
            let s = wrap_under_get_rule(value.as_CFTypeRef() as CFStringRef);
            Some(s.to_string())
        }
    }

    unsafe fn wrap_under_get_rule(reference: CFStringRef) -> CFString {
        unsafe {
            assert!(!reference.is_null(), "Attempted to create a NULL object.");
            let reference = CFRetain(reference as *const ::std::os::raw::c_void) as CFStringRef;
            TCFType::wrap_under_create_rule(reference)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{FontRun, GlyphId, MacTextSystem, PlatformTextSystem, font, px};

    #[test]
    fn test_layout_line_bom_char() {
        let fonts = MacTextSystem::new();
        let font_id = fonts.font_id(&font("Helvetica")).unwrap();
        let line = "\u{feff}";
        let mut style = FontRun {
            font_size: px(16.),
            letter_spacing: crate::px(0.),
            trim_punctuation: false,
            text_direction: Default::default(),
            font_id,
            len: line.len(),
        };

        let layout = fonts.layout_line(line, px(16.), &[style]);
        assert_eq!(layout.len, line.len());
        assert!(layout.runs.is_empty());

        let line = "a\u{feff}b";
        style.len = line.len();
        let layout = fonts.layout_line(line, px(16.), &[style]);
        assert_eq!(layout.len, line.len());
        assert_eq!(layout.runs.len(), 1);
        assert_eq!(layout.runs[0].glyphs.len(), 2);
        assert_eq!(layout.runs[0].glyphs[0].id, GlyphId(68u32)); // a
        // There's no glyph for \u{feff}
        assert_eq!(layout.runs[0].glyphs[1].id, GlyphId(69u32)); // b

        let line = "\u{feff}ab";
        let font_runs = &[
            FontRun {
                font_size: px(16.),
                letter_spacing: crate::px(0.),
                trim_punctuation: false,
                text_direction: Default::default(),
                len: "\u{feff}".len(),
                font_id,
            },
            FontRun {
                font_size: px(16.),
                letter_spacing: crate::px(0.),
                trim_punctuation: false,
                text_direction: Default::default(),
                len: "ab".len(),
                font_id,
            },
        ];
        let layout = fonts.layout_line(line, px(16.), font_runs);
        assert_eq!(layout.len, line.len());
        assert_eq!(layout.runs.len(), 1);
        assert_eq!(layout.runs[0].glyphs.len(), 2);
        // There's no glyph for \u{feff}
        assert_eq!(layout.runs[0].glyphs[0].id, GlyphId(68u32)); // a
        assert_eq!(layout.runs[0].glyphs[1].id, GlyphId(69u32)); // b
    }

    #[test]
    fn test_layout_line_zwnj_insertion() {
        let fonts = MacTextSystem::new();
        let font_id = fonts.font_id(&font("Helvetica")).unwrap();

        let text = "hello world";
        let font_runs = &[
            FontRun {
                font_size: px(16.),
                letter_spacing: crate::px(0.),
                trim_punctuation: false,
                text_direction: Default::default(),
                font_id,
                len: 5,
            }, // "hello"
            FontRun {
                font_size: px(16.),
                letter_spacing: crate::px(0.),
                trim_punctuation: false,
                text_direction: Default::default(),
                font_id,
                len: 6,
            }, // " world"
        ];

        let layout = fonts.layout_line(text, px(16.), font_runs);
        assert_eq!(layout.len, text.len());

        for run in &layout.runs {
            for glyph in &run.glyphs {
                assert!(
                    glyph.index < text.len(),
                    "Glyph index {} is out of bounds for text length {}",
                    glyph.index,
                    text.len()
                );
            }
        }

        // Test with different font runs - should not insert ZWNJ
        let font_id2 = fonts.font_id(&font("Times")).unwrap_or(font_id);
        let font_runs_different = &[
            FontRun {
                font_size: px(16.),
                letter_spacing: crate::px(0.),
                trim_punctuation: false,
                text_direction: Default::default(),
                font_id,
                len: 5,
            }, // "hello"
            // " world"
            FontRun {
                font_size: px(16.),
                letter_spacing: crate::px(0.),
                trim_punctuation: false,
                text_direction: Default::default(),
                font_id: font_id2,
                len: 6,
            },
        ];

        let layout2 = fonts.layout_line(text, px(16.), font_runs_different);
        assert_eq!(layout2.len, text.len());

        for run in &layout2.runs {
            for glyph in &run.glyphs {
                assert!(
                    glyph.index < text.len(),
                    "Glyph index {} is out of bounds for text length {}",
                    glyph.index,
                    text.len()
                );
            }
        }
    }

    #[test]
    fn test_layout_line_zwnj_edge_cases() {
        let fonts = MacTextSystem::new();
        let font_id = fonts.font_id(&font("Helvetica")).unwrap();

        let text = "hello";
        let font_runs = &[FontRun {
            font_size: px(16.),
            letter_spacing: crate::px(0.),
            trim_punctuation: false,
            text_direction: Default::default(),
            font_id,
            len: 5,
        }];
        let layout = fonts.layout_line(text, px(16.), font_runs);
        assert_eq!(layout.len, text.len());

        let text = "abc";
        let font_runs = &[
            FontRun {
                font_size: px(16.),
                letter_spacing: crate::px(0.),
                trim_punctuation: false,
                text_direction: Default::default(),
                font_id,
                len: 1,
            }, // "a"
            FontRun {
                font_size: px(16.),
                letter_spacing: crate::px(0.),
                trim_punctuation: false,
                text_direction: Default::default(),
                font_id,
                len: 1,
            }, // "b"
            FontRun {
                font_size: px(16.),
                letter_spacing: crate::px(0.),
                trim_punctuation: false,
                text_direction: Default::default(),
                font_id,
                len: 1,
            }, // "c"
        ];
        let layout = fonts.layout_line(text, px(16.), font_runs);
        assert_eq!(layout.len, text.len());

        for run in &layout.runs {
            for glyph in &run.glyphs {
                assert!(
                    glyph.index < text.len(),
                    "Glyph index {} is out of bounds for text length {}",
                    glyph.index,
                    text.len()
                );
            }
        }

        // Test with empty text
        let text = "";
        let font_runs = &[];
        let layout = fonts.layout_line(text, px(16.), font_runs);
        assert_eq!(layout.len, 0);
        assert!(layout.runs.is_empty());
    }
}
