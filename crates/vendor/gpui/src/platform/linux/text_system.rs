use crate::{
    Bounds, DevicePixels, Font, FontFallbacks, FontFeatures, FontId, FontMetrics, FontRun,
    FontStyle, FontWeight, GlyphId, LineLayout, Pixels, PlatformTextSystem, Point,
    RenderGlyphParams, SUBPIXEL_VARIANTS_X, SUBPIXEL_VARIANTS_Y, ShapedGlyph, ShapedRun,
    SharedString, Size, TextCell, point, size,
};
use anyhow::{Context as _, Ok, Result};
use collections::HashMap;
use cosmic_text::{
    Attrs, AttrsList, CacheKey, Family, Font as CosmicTextFont, FontFeatures as CosmicFontFeatures,
    FontSystem, ShapeBuffer, ShapeLine, SwashCache,
};

use itertools::Itertools;
use parking_lot::RwLock;
use pathfinder_geometry::{
    rect::{RectF, RectI},
    vector::{Vector2F, Vector2I},
};
use smallvec::SmallVec;
use std::{borrow::Cow, sync::Arc};
use unicode_segmentation::UnicodeSegmentation;

pub(crate) struct CosmicTextSystem(RwLock<CosmicTextSystemState>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FontKey {
    family: SharedString,
    features: FontFeatures,
    weight: cosmic_text::fontdb::Weight,
    style: FontStyle,
    fallbacks: Option<FontFallbacks>,
    optical_size: Option<Pixels>,
    variations: Vec<(u32, u32)>,
}

impl FontKey {
    fn new(
        family: SharedString,
        features: FontFeatures,
        weight: cosmic_text::fontdb::Weight,
        style: FontStyle,
        fallbacks: Option<FontFallbacks>,
        optical_size: Option<Pixels>,
        variations: Vec<(u32, u32)>,
    ) -> Self {
        Self {
            family,
            features,
            weight,
            style,
            fallbacks,
            optical_size,
            variations,
        }
    }
}

struct CosmicTextSystemState {
    swash_cache: SwashCache,
    font_system: FontSystem,
    scratch: ShapeBuffer,
    /// Contains all already loaded fonts, including all faces. Indexed by `FontId`.
    loaded_fonts: Vec<LoadedFont>,
    /// Caches the `FontId`s associated with a specific family to avoid iterating the font database
    /// for every font face in a family.
    font_ids_by_family_cache: HashMap<FontKey, SmallVec<[FontId; 4]>>,
}

struct LoadedFont {
    font: Arc<CosmicTextFont>,
    features: CosmicFontFeatures,
    fallbacks: Option<FontFallbacks>,
    weight: cosmic_text::fontdb::Weight,
    style: FontStyle,
    synthetic_italic: bool,
    synthetic_bold: bool,
    optical_size: Option<f32>,
    variations: Vec<(u32, u32)>,
    is_known_emoji_font: bool,
}
impl LoadedFont {
    fn cache_key_flags(&self) -> cosmic_text::CacheKeyFlags {
        let mut flags = cosmic_text::CacheKeyFlags::empty();
        if self.synthetic_italic {
            flags |= cosmic_text::CacheKeyFlags::FAKE_ITALIC;
        }
        if self.synthetic_bold {
            flags |= cosmic_text::CacheKeyFlags::FAKE_BOLD;
        }
        flags
    }

    fn coordinates(&self) -> Vec<i16> {
        let settings =
            std::iter::once((u32::from_be_bytes(*b"wght"), self.font.variation_weight()))
                .chain(
                    self.optical_size
                        .map(|size| (u32::from_be_bytes(*b"opsz"), size)),
                )
                .chain(
                    self.variations
                        .iter()
                        .map(|(tag, bits)| (*tag, f32::from_bits(*bits))),
                );
        self.font
            .as_swash()
            .variations()
            .normalized_coords(settings)
            .collect()
    }
}

impl CosmicTextSystem {
    pub(crate) fn new() -> Self {
        // todo(linux) make font loading non-blocking
        let mut font_system = FontSystem::new();

        Self(RwLock::new(CosmicTextSystemState {
            font_system,
            swash_cache: SwashCache::new(),
            scratch: ShapeBuffer::default(),
            loaded_fonts: Vec::new(),
            font_ids_by_family_cache: HashMap::default(),
        }))
    }
}

impl Default for CosmicTextSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformTextSystem for CosmicTextSystem {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        self.0.write().add_fonts(fonts)
    }

    fn all_font_names(&self) -> Vec<String> {
        let mut result = self
            .0
            .read()
            .font_system
            .db()
            .faces()
            .filter_map(|face| face.families.first().map(|family| family.0.clone()))
            .collect_vec();
        result.sort();
        result.dedup();
        result
    }

    fn font_id(&self, font: &Font) -> Result<FontId> {
        // todo(linux): Do we need to use CosmicText's Font APIs? Can we consolidate this to use font_kit?
        let mut state = self.0.write();
        let weight = cosmic_text::fontdb::Weight(font.weight.0.round().clamp(1., 1000.) as u16);
        let variations = font
            .variations
            .iter()
            .map(|(tag, value)| {
                (
                    u32::from_be_bytes(tag.as_bytes().try_into().unwrap()),
                    value.to_bits(),
                )
            })
            .collect::<Vec<_>>();
        let key = FontKey::new(
            font.family.clone(),
            font.features.clone(),
            weight,
            font.style,
            font.fallbacks.clone(),
            font.optical_size,
            variations.clone(),
        );
        let candidates = if let Some(font_ids) = state.font_ids_by_family_cache.get(&key) {
            font_ids.as_slice()
        } else {
            let font_ids = state.load_family(
                &font.family,
                &font.features,
                weight,
                font.style,
                font.fallbacks.clone(),
                font.optical_size,
                variations.clone(),
            )?;
            state.font_ids_by_family_cache.insert(key.clone(), font_ids);
            state.font_ids_by_family_cache[&key].as_ref()
        };

        // todo(linux) ideally we would make fontdb's `find_best_match` pub instead of using font-kit here
        let candidate_properties = candidates
            .iter()
            .map(|font_id| {
                let database_id = state.loaded_font(*font_id).font.id();
                let face_info = state.font_system.db().face(database_id).expect("");
                face_info_into_properties(face_info)
            })
            .collect::<SmallVec<[_; 4]>>();

        let ix =
            font_kit::matching::find_best_match(&candidate_properties, &font_into_properties(font))
                .context("requested font family contains no font matching the other parameters")?;

        Ok(candidates[ix])
    }

    fn font_metrics(&self, font_id: FontId) -> FontMetrics {
        let lock = self.0.read();
        let font = lock.loaded_font(font_id);
        let coords = font.coordinates();
        let metrics = font.font.as_swash().metrics(&coords);

        FontMetrics {
            units_per_em: metrics.units_per_em as u32,
            ascent: metrics.ascent,
            descent: -metrics.descent, // todo(linux) confirm this is correct
            line_gap: metrics.leading,
            underline_position: metrics.underline_offset,
            underline_thickness: metrics.stroke_size,
            cap_height: metrics.cap_height,
            x_height: metrics.x_height,
            // todo(linux): Compute this correctly
            bounding_box: Bounds {
                origin: point(0.0, 0.0),
                size: size(metrics.max_width, metrics.ascent + metrics.descent),
            },
        }
    }

    fn typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Bounds<f32>> {
        let lock = self.0.read();
        let font = lock.loaded_font(font_id);
        let coords = font.coordinates();
        let glyph_metrics = font.font.as_swash().glyph_metrics(&coords);
        let glyph_id = glyph_id.0 as u16;
        // todo(linux): Compute this correctly
        // see https://github.com/servo/font-kit/blob/master/src/loaders/freetype.rs#L614-L620
        Ok(Bounds {
            origin: point(0.0, 0.0),
            size: size(
                glyph_metrics.advance_width(glyph_id),
                glyph_metrics.advance_height(glyph_id),
            ),
        })
    }

    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        self.0.read().advance(font_id, glyph_id)
    }

    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId> {
        self.0.read().glyph_for_char(font_id, ch)
    }

    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        self.0.write().raster_bounds(params)
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        self.0.write().rasterize_glyph(params, raster_bounds)
    }

    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        self.0.write().layout_line(text, font_size, runs)
    }

    fn layout_line_with_context(
        &self,
        text: &str,
        font_size: Pixels,
        runs: &[FontRun],
        before: &str,
        after: &str,
    ) -> LineLayout {
        let mut state = self.0.write();
        state.font_system.set_shaping_context(before, after);
        let result = state.layout_line(text, font_size, runs);
        state.font_system.set_shaping_context("", "");
        result
    }
}

impl CosmicTextSystemState {
    fn loaded_font(&self, font_id: FontId) -> &LoadedFont {
        &self.loaded_fonts[font_id.0]
    }

    #[profiling::function]
    fn add_fonts(&mut self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        for bytes in fonts {
            match bytes {
                Cow::Borrowed(embedded_font) => {
                    self.font_system
                        .add_private_font_data(embedded_font.to_vec());
                }
                Cow::Owned(bytes) => {
                    self.font_system.add_private_font_data(bytes);
                }
            }
        }
        self.font_ids_by_family_cache.clear();
        Ok(())
    }

    #[profiling::function]
    fn load_family(
        &mut self,
        name: &str,
        features: &FontFeatures,
        weight: cosmic_text::fontdb::Weight,
        style: FontStyle,
        fallbacks: Option<FontFallbacks>,
        optical_size: Option<Pixels>,
        variations: Vec<(u32, u32)>,
    ) -> Result<SmallVec<[FontId; 4]>> {
        // TODO: Determine the proper system UI font.
        let name = crate::text_system::font_name_with_fallbacks(name, "IBM Plex Sans");

        let families = self
            .font_system
            .db()
            .faces()
            .filter(|face| {
                self.font_system.font_is_current(face.id)
                    && face.families.iter().any(|family| *name == family.0)
            })
            .map(|face| (face.id, face.post_script_name.clone()))
            .collect::<SmallVec<[_; 4]>>();

        let mut loaded_font_ids = SmallVec::new();
        for (font_id, postscript_name) in families {
            let font = self
                .font_system
                .get_font_with_variations(font_id, weight, optical_size.map(f32::from), &variations)
                .context("Could not load font")?;

            // HACK: To let the storybook run and render Windows caption icons. We should actually do better font fallback.
            let allowed_bad_font_names = [
                "SegoeFluentIcons", // NOTE: Segoe fluent icons postscript name is inconsistent
                "Segoe Fluent Icons",
            ];

            if font.as_swash().charmap().map('m') == 0
                && !allowed_bad_font_names.contains(&postscript_name.as_str())
            {
                self.font_system.db_mut().remove_face(font.id());
                continue;
            };

            let synthetic_italic = style != FontStyle::Normal
                && self.font_system.db().face(font.id()).unwrap().style
                    == cosmic_text::Style::Normal;
            let synthetic_bold = weight.0 >= 600
                && self.font_system.db().face(font.id()).unwrap().weight.0 < 600
                && !font
                    .as_swash()
                    .variations()
                    .any(|axis| axis.tag() == u32::from_be_bytes(*b"wght"));
            let font_id = FontId(self.loaded_fonts.len());
            loaded_font_ids.push(font_id);
            self.loaded_fonts.push(LoadedFont {
                font,
                features: features.try_into()?,
                fallbacks: fallbacks.clone(),
                weight,
                style,
                synthetic_italic,
                synthetic_bold,
                optical_size: optical_size.map(f32::from),
                variations: variations.clone(),
                is_known_emoji_font: check_is_known_emoji_font(&postscript_name),
            });
        }

        Ok(loaded_font_ids)
    }

    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        let font = self.loaded_font(font_id);
        let coords = font.coordinates();
        let glyph_metrics = font.font.as_swash().glyph_metrics(&coords);
        Ok(Size {
            width: glyph_metrics.advance_width(glyph_id.0 as u16),
            height: glyph_metrics.advance_height(glyph_id.0 as u16),
        })
    }

    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId> {
        let glyph_id = self.loaded_font(font_id).font.as_swash().charmap().map(ch);
        if glyph_id == 0 {
            None
        } else {
            Some(GlyphId(glyph_id.into()))
        }
    }

    fn raster_bounds(&mut self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        let font = &self.loaded_fonts[params.font_id.0].font;
        let subpixel_shift = point(
            params.subpixel_variant.x as f32 / SUBPIXEL_VARIANTS_X as f32 / params.scale_factor,
            params.subpixel_variant.y as f32 / SUBPIXEL_VARIANTS_Y as f32 / params.scale_factor,
        );
        let image = self
            .swash_cache
            .get_image(
                &mut self.font_system,
                CacheKey::new(
                    font.id(),
                    params.glyph_id.0 as u16,
                    (params.font_size * params.scale_factor).into(),
                    (subpixel_shift.x, subpixel_shift.y.trunc()),
                    self.loaded_fonts[params.font_id.0].weight,
                    self.loaded_fonts[params.font_id.0].cache_key_flags(),
                )
                .0
                .with_optical_size(self.loaded_fonts[params.font_id.0].optical_size)
                .with_variations(self.loaded_fonts[params.font_id.0].variations.clone()),
            )
            .clone()
            .with_context(|| format!("no image for {params:?} in font {font:?}"))?;
        Ok(Bounds {
            origin: point(image.placement.left.into(), (-image.placement.top).into()),
            size: size(image.placement.width.into(), image.placement.height.into()),
        })
    }

    #[profiling::function]
    fn rasterize_glyph(
        &mut self,
        params: &RenderGlyphParams,
        glyph_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        if glyph_bounds.size.width.0 == 0 || glyph_bounds.size.height.0 == 0 {
            anyhow::bail!("glyph bounds are empty");
        } else {
            let bitmap_size = glyph_bounds.size;
            let font = &self.loaded_fonts[params.font_id.0].font;
            let subpixel_shift = point(
                params.subpixel_variant.x as f32 / SUBPIXEL_VARIANTS_X as f32 / params.scale_factor,
                params.subpixel_variant.y as f32 / SUBPIXEL_VARIANTS_Y as f32 / params.scale_factor,
            );
            let mut image = self
                .swash_cache
                .get_image(
                    &mut self.font_system,
                    CacheKey::new(
                        font.id(),
                        params.glyph_id.0 as u16,
                        (params.font_size * params.scale_factor).into(),
                        (subpixel_shift.x, subpixel_shift.y.trunc()),
                        self.loaded_fonts[params.font_id.0].weight,
                        self.loaded_fonts[params.font_id.0].cache_key_flags(),
                    )
                    .0
                    .with_optical_size(self.loaded_fonts[params.font_id.0].optical_size)
                    .with_variations(self.loaded_fonts[params.font_id.0].variations.clone()),
                )
                .clone()
                .with_context(|| format!("no image for {params:?} in font {font:?}"))?;

            if params.is_emoji {
                // Convert from RGBA to BGRA.
                for pixel in image.data.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
            }

            Ok((bitmap_size, image.data))
        }
    }

    /// This is used when cosmic_text has chosen a fallback font instead of using the requested
    /// font, typically to handle some unicode characters. When this happens, `loaded_fonts` may not
    /// yet have an entry for this fallback font, and so one is added.
    ///
    /// Preserve the actual weight, optical size and inherited features in the output font ID,
    /// including fallback runs, so later rasterization uses the shaped instance.
    fn font_id_for_cosmic_id(
        &mut self,
        id: cosmic_text::fontdb::ID,
        weight: cosmic_text::fontdb::Weight,
        optical_size: Option<f32>,
        variations: Vec<(u32, u32)>,
        style: FontStyle,
        features: CosmicFontFeatures,
        fallbacks: Option<FontFallbacks>,
    ) -> FontId {
        if let Some(ix) = self.loaded_fonts.iter().position(|loaded_font| {
            loaded_font.font.id() == id
                && loaded_font.weight == weight
                && loaded_font.optical_size == optical_size
                && loaded_font.variations == variations
                && loaded_font.style == style
                && loaded_font.features == features
                && loaded_font.fallbacks == fallbacks
        }) {
            FontId(ix)
        } else {
            let font = self
                .font_system
                .get_font_with_variations(id, weight, optical_size, &variations)
                .unwrap();
            let face = self.font_system.db().face(id).unwrap();

            let font_id = FontId(self.loaded_fonts.len());
            let synthetic_bold = weight.0 >= 600
                && face.weight.0 < 600
                && !font
                    .as_swash()
                    .variations()
                    .any(|axis| axis.tag() == u32::from_be_bytes(*b"wght"));
            self.loaded_fonts.push(LoadedFont {
                font,
                features,
                fallbacks,
                weight,
                style,
                synthetic_italic: style != FontStyle::Normal
                    && face.style == cosmic_text::Style::Normal,
                synthetic_bold,
                optical_size,
                variations,
                is_known_emoji_font: check_is_known_emoji_font(&face.post_script_name),
            });

            font_id
        }
    }

    #[profiling::function]
    fn layout_line(&mut self, text: &str, font_size: Pixels, font_runs: &[FontRun]) -> LineLayout {
        let optical_size = font_runs
            .first()
            .and_then(|run| self.loaded_font(run.font_id).optical_size)
            .unwrap_or(font_size.0);
        self.font_system.set_optical_size(Some(optical_size));
        let mut attrs_list = AttrsList::new(&Attrs::new());
        let mut offs = 0;
        for run in font_runs {
            let loaded_font = self.loaded_font(run.font_id);
            let font = self.font_system.db().face(loaded_font.font.id()).unwrap();

            attrs_list.add_span(
                offs..(offs + run.len),
                &Attrs::new()
                    .metadata(run.font_id.0)
                    .metrics(cosmic_text::Metrics::new(run.font_size.0, run.font_size.0))
                    .font_optical_size(loaded_font.optical_size)
                    .font_variations(loaded_font.variations.clone())
                    .family(Family::Name(&font.families.first().unwrap().0))
                    .font_fallbacks(loaded_font.fallbacks.as_ref().map_or_else(
                        Vec::new,
                        |fallbacks| {
                            fallbacks
                                .fallback_list()
                                .iter()
                                .map(|name| cosmic_text::FamilyOwned::new(Family::Name(name)))
                                .collect()
                        },
                    ))
                    .stretch(font.stretch)
                    .style(loaded_font.style.into())
                    .weight(loaded_font.weight)
                    .font_features(loaded_font.features.clone()),
            );
            offs += run.len;
        }

        let line = ShapeLine::new_with_direction(
            &mut self.font_system,
            text,
            &attrs_list,
            cosmic_text::Shaping::Advanced,
            4,
            font_runs.first().and_then(|run| run.text_direction.rtl()),
        );
        let mut layout_lines = Vec::with_capacity(1);
        line.layout_to_buffer(
            &mut self.scratch,
            font_size.0,
            None, // We do our own wrapping
            cosmic_text::Wrap::None,
            cosmic_text::Ellipsize::None,
            None,
            &mut layout_lines,
            None,
            cosmic_text::Hinting::Disabled,
        );
        let layout = layout_lines.first().unwrap();

        let mut runs: Vec<ShapedRun> = Vec::new();
        for glyph in &layout.glyphs {
            let mut font_id = FontId(glyph.metadata);
            let mut loaded_font = self.loaded_font(font_id);
            let optical_size = loaded_font.optical_size.unwrap_or(glyph.font_size);
            if loaded_font.font.id() != glyph.font_id
                || loaded_font.optical_size != Some(optical_size)
            {
                let features = loaded_font.features.clone();
                let fallbacks = loaded_font.fallbacks.clone();
                let style = loaded_font.style;
                let variations = loaded_font.variations.clone();
                font_id = self.font_id_for_cosmic_id(
                    glyph.font_id,
                    glyph.font_weight,
                    Some(optical_size),
                    variations,
                    style,
                    features,
                    fallbacks,
                );
                loaded_font = self.loaded_font(font_id);
            }
            let is_emoji = loaded_font.is_known_emoji_font;

            // HACK: Prevent crash caused by variation selectors.
            if glyph.glyph_id == 3 && is_emoji {
                continue;
            }

            let shaped_glyph = ShapedGlyph {
                position_is_advance: true,
                paint_offset: point(
                    (glyph.font_size * glyph.x_offset).into(),
                    (glyph.y - glyph.font_size * glyph.y_offset).into(),
                ),
                id: GlyphId(glyph.glyph_id as u32),
                position: point(glyph.x.into(), 0_f32.into()),
                index: glyph.start,
                is_emoji,
            };

            if let Some(last_run) = runs.last_mut().filter(|last_run| {
                last_run.font_id == font_id && last_run.font_size == Pixels::from(glyph.font_size)
            }) {
                last_run.glyphs.push(shaped_glyph);
            } else {
                runs.push(ShapedRun {
                    font_size: Pixels::from(glyph.font_size),
                    font_id,
                    glyphs: vec![shaped_glyph],
                });
            }
        }

        let mut clusters: HashMap<(usize, usize), (f32, f32, bool)> = HashMap::default();
        for glyph in &layout.glyphs {
            clusters
                .entry((glyph.start, glyph.end))
                .and_modify(|cell| {
                    cell.0 = cell.0.min(glyph.x);
                    cell.1 = cell.1.max(glyph.x + glyph.w);
                })
                .or_insert((glyph.x, glyph.x + glyph.w, glyph.level.is_rtl()));
        }
        let cells = clusters
            .into_iter()
            .flat_map(|((start, end), (left, right, rtl))| {
                let indices = text[start..end]
                    .grapheme_indices(true)
                    .map(|(index, _)| start + index)
                    .chain(std::iter::once(end))
                    .collect::<Vec<_>>();
                let count = indices.len().saturating_sub(1).max(1);
                indices
                    .windows(2)
                    .enumerate()
                    .map(move |(index, range)| {
                        let step = (right - left) / count as f32;
                        let (start_x, end_x) = if rtl {
                            (
                                right - step * index as f32,
                                right - step * (index + 1) as f32,
                            )
                        } else {
                            (left + step * index as f32, left + step * (index + 1) as f32)
                        };
                        TextCell {
                            range: range[0]..range[1],
                            start_x: start_x.into(),
                            end_x: end_x.into(),
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut result = LineLayout {
            font_size,
            width: layout.w.into(),
            ascent: layout.max_ascent.into(),
            descent: layout.max_descent.into(),
            runs,
            len: text.len(),
            carets: Vec::new(),
            cells: Vec::new(),
            bidi_levels: Vec::new(),
            visual_lines: Vec::new(),
            rtl: line.rtl,
        };
        result.set_cells(text, cells);
        result
    }
}

impl TryFrom<&FontFeatures> for CosmicFontFeatures {
    type Error = anyhow::Error;

    fn try_from(features: &FontFeatures) -> Result<Self> {
        let mut result = CosmicFontFeatures::new();
        for feature in features.0.iter() {
            let name_bytes: [u8; 4] = feature
                .0
                .as_bytes()
                .try_into()
                .context("Incorrect feature flag format")?;

            let tag = cosmic_text::FeatureTag::new(&name_bytes);

            result.set(tag, feature.1);
        }
        Ok(result)
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

impl From<FontWeight> for cosmic_text::Weight {
    fn from(value: FontWeight) -> Self {
        cosmic_text::Weight(value.0 as u16)
    }
}

impl From<FontStyle> for cosmic_text::Style {
    fn from(style: FontStyle) -> Self {
        match style {
            FontStyle::Normal => cosmic_text::Style::Normal,
            FontStyle::Italic => cosmic_text::Style::Italic,
            FontStyle::Oblique => cosmic_text::Style::Oblique,
        }
    }
}

fn font_into_properties(font: &crate::Font) -> font_kit::properties::Properties {
    font_kit::properties::Properties {
        style: match font.style {
            crate::FontStyle::Normal => font_kit::properties::Style::Normal,
            crate::FontStyle::Italic => font_kit::properties::Style::Italic,
            crate::FontStyle::Oblique => font_kit::properties::Style::Oblique,
        },
        weight: font_kit::properties::Weight(font.weight.0),
        stretch: Default::default(),
    }
}

fn face_info_into_properties(
    face_info: &cosmic_text::fontdb::FaceInfo,
) -> font_kit::properties::Properties {
    font_kit::properties::Properties {
        style: match face_info.style {
            cosmic_text::Style::Normal => font_kit::properties::Style::Normal,
            cosmic_text::Style::Italic => font_kit::properties::Style::Italic,
            cosmic_text::Style::Oblique => font_kit::properties::Style::Oblique,
        },
        // both libs use the same values for weight
        weight: font_kit::properties::Weight(face_info.weight.0.into()),
        stretch: match face_info.stretch {
            cosmic_text::Stretch::Condensed => font_kit::properties::Stretch::CONDENSED,
            cosmic_text::Stretch::Expanded => font_kit::properties::Stretch::EXPANDED,
            cosmic_text::Stretch::ExtraCondensed => font_kit::properties::Stretch::EXTRA_CONDENSED,
            cosmic_text::Stretch::ExtraExpanded => font_kit::properties::Stretch::EXTRA_EXPANDED,
            cosmic_text::Stretch::Normal => font_kit::properties::Stretch::NORMAL,
            cosmic_text::Stretch::SemiCondensed => font_kit::properties::Stretch::SEMI_CONDENSED,
            cosmic_text::Stretch::SemiExpanded => font_kit::properties::Stretch::SEMI_EXPANDED,
            cosmic_text::Stretch::UltraCondensed => font_kit::properties::Stretch::ULTRA_CONDENSED,
            cosmic_text::Stretch::UltraExpanded => font_kit::properties::Stretch::ULTRA_EXPANDED,
        },
    }
}

fn check_is_known_emoji_font(postscript_name: &str) -> bool {
    // TODO: Include other common emoji fonts
    postscript_name == "NotoColorEmoji"
}
