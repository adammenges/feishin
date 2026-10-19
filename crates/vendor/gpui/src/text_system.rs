mod font_fallbacks;
mod font_features;
mod font_variations;
mod line;
mod line_layout;
mod line_wrapper;
mod text_spacing;

pub use font_fallbacks::*;
pub use font_features::*;
pub use font_variations::*;
pub use line::*;
pub use line_layout::*;
pub use line_wrapper::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub use text_spacing::*;

use crate::{
    Bounds, DevicePixels, Hsla, Pixels, PlatformTextSystem, Point, Result, SharedString, Size,
    StrikethroughStyle, UnderlineStyle, px,
};
use anyhow::{Context as _, anyhow};
use collections::FxHashMap;
use core::fmt;
use derive_more::{Add, Deref, FromStr, Sub};
use itertools::Itertools;
use parking_lot::{Mutex, RwLock, RwLockUpgradableReadGuard};
use smallvec::{SmallVec, smallvec};
use std::{
    borrow::Cow,
    cmp,
    fmt::{Debug, Display, Formatter},
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut, Range},
    sync::Arc,
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
};

/// An opaque identifier for a specific font.
#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
#[repr(C)]
pub struct FontId(pub usize);

/// An opaque identifier for a specific font family.
#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct FontFamilyId(pub usize);

pub(crate) const SUBPIXEL_VARIANTS_X: u8 = 4;

pub(crate) const SUBPIXEL_VARIANTS_Y: u8 =
    if cfg!(target_os = "windows") || cfg!(target_os = "linux") {
        1
    } else {
        SUBPIXEL_VARIANTS_X
    };

/// The GPUI text rendering sub system.
pub struct TextSystem {
    platform_text_system: Arc<dyn PlatformTextSystem>,
    fonts_revision: AtomicU64,
    font_ids_by_font: RwLock<FxHashMap<Font, Result<FontId>>>,
    font_metrics: RwLock<FxHashMap<FontId, FontMetrics>>,
    raster_bounds: RwLock<FxHashMap<RenderGlyphParams, Bounds<DevicePixels>>>,
    wrapper_pool: Mutex<FxHashMap<FontIdWithSize, Vec<LineWrapper>>>,
    font_runs_pool: Mutex<Vec<Vec<FontRun>>>,
    fallback_font_stack: SmallVec<[Font; 2]>,
}

impl TextSystem {
    pub(crate) fn new(platform_text_system: Arc<dyn PlatformTextSystem>) -> Self {
        TextSystem {
            platform_text_system,
            fonts_revision: AtomicU64::new(0),
            font_metrics: RwLock::default(),
            raster_bounds: RwLock::default(),
            font_ids_by_font: RwLock::default(),
            wrapper_pool: Mutex::default(),
            font_runs_pool: Mutex::default(),
            fallback_font_stack: smallvec![
                // TODO: Remove this when Linux have implemented setting fallbacks.
                font(".ZedMono"),
                font(".ZedSans"),
                font("Helvetica"),
                font("Segoe UI"),     // Windows
                font("Ubuntu"),       // Gnome (Ubuntu)
                font("Adwaita Sans"), // Gnome 47
                font("Cantarell"),    // Gnome
                font("Noto Sans"),    // KDE
                font("DejaVu Sans"),
                font("Arial"), // macOS, Windows
            ],
        }
    }

    /// Get a list of all available font names from the operating system.
    pub fn all_font_names(&self) -> Vec<String> {
        let mut names = self.platform_text_system.all_font_names();
        names.extend(
            self.fallback_font_stack
                .iter()
                .map(|font| font.family.to_string()),
        );
        names.retain(|name| {
            !matches!(
                name.as_str(),
                "Noto Sans JP Variable" | "Noto Sans Hebrew Variable"
            )
        });
        names.push(".SystemUIFont".to_string());
        names.sort();
        names.dedup();
        names
    }

    /// Add a font's data to the text system.
    pub fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        if fonts.is_empty() {
            return Ok(());
        }
        self.platform_text_system.add_fonts(fonts)?;
        self.font_ids_by_font.write().clear();
        self.fonts_revision.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Changes when font registration invalidates family matching and wrapping.
    pub fn fonts_revision(&self) -> u64 {
        self.fonts_revision.load(Ordering::Relaxed)
    }

    /// Get the FontId for the configure font family and style.
    fn font_id(&self, font: &Font) -> Result<FontId> {
        fn clone_font_id_result(font_id: &Result<FontId>) -> Result<FontId> {
            match font_id {
                Ok(font_id) => Ok(*font_id),
                Err(err) => Err(anyhow!("{err}")),
            }
        }

        let font_id = self
            .font_ids_by_font
            .read()
            .get(font)
            .map(clone_font_id_result);
        if let Some(font_id) = font_id {
            font_id
        } else {
            let font_id = self.platform_text_system.font_id(font);
            self.font_ids_by_font
                .write()
                .insert(font.clone(), clone_font_id_result(&font_id));
            font_id
        }
    }

    /// Get the Font for the Font Id.
    pub fn get_font_for_id(&self, id: FontId) -> Option<Font> {
        let lock = self.font_ids_by_font.read();
        lock.iter()
            .filter_map(|(font, result)| match result {
                Ok(font_id) if *font_id == id => Some(font.clone()),
                _ => None,
            })
            .next()
    }

    /// Resolves the specified font, falling back to the default font stack if
    /// the font fails to load.
    ///
    /// # Panics
    ///
    /// Panics if the font and none of the fallbacks can be resolved.
    pub fn resolve_font(&self, font: &Font) -> FontId {
        if let Ok(font_id) = self.font_id(font) {
            return font_id;
        }
        if let Some(fallbacks) = &font.fallbacks {
            for (index, family) in fallbacks.fallback_list().iter().enumerate() {
                let fallback = Font {
                    family: family.clone().into(),
                    fallbacks: Some(FontFallbacks::from_fonts(
                        fallbacks.fallback_list()[index + 1..].to_vec(),
                    )),
                    ..font.clone()
                };
                if let Ok(font_id) = self.font_id(&fallback) {
                    return font_id;
                }
            }
        }
        for fallback in &self.fallback_font_stack {
            let fallback = Font {
                family: fallback.family.clone(),
                ..font.clone()
            };
            if let Ok(font_id) = self.font_id(&fallback) {
                return font_id;
            }
        }

        panic!(
            "failed to resolve font '{}' or any of the fallbacks: {}",
            font.family,
            self.fallback_font_stack
                .iter()
                .map(|fallback| &fallback.family)
                .join(", ")
        );
    }

    /// Get the bounding box for the given font and font size.
    /// A font's bounding box is the smallest rectangle that could enclose all glyphs
    /// in the font. superimposed over one another.
    pub fn bounding_box(&self, font_id: FontId, font_size: Pixels) -> Bounds<Pixels> {
        self.read_metrics(font_id, |metrics| metrics.bounding_box(font_size))
    }

    /// Get the typographic bounds for the given character, in the given font and size.
    pub fn typographic_bounds(
        &self,
        font_id: FontId,
        font_size: Pixels,
        character: char,
    ) -> Result<Bounds<Pixels>> {
        let glyph_id = self
            .platform_text_system
            .glyph_for_char(font_id, character)
            .with_context(|| format!("glyph not found for character '{character}'"))?;
        let bounds = self
            .platform_text_system
            .typographic_bounds(font_id, glyph_id)?;
        Ok(self.read_metrics(font_id, |metrics| {
            (bounds / metrics.units_per_em as f32 * font_size.0).map(px)
        }))
    }

    /// Get the advance width for the given character, in the given font and size.
    pub fn advance(&self, font_id: FontId, font_size: Pixels, ch: char) -> Result<Size<Pixels>> {
        let glyph_id = self
            .platform_text_system
            .glyph_for_char(font_id, ch)
            .with_context(|| format!("glyph not found for character '{ch}'"))?;
        let result = self.platform_text_system.advance(font_id, glyph_id)?
            / self.units_per_em(font_id) as f32;

        Ok(result * font_size)
    }

    /// Returns the width of an `em`.
    ///
    /// Uses the width of the `m` character in the given font and size.
    pub fn em_width(&self, font_id: FontId, font_size: Pixels) -> Result<Pixels> {
        Ok(self.typographic_bounds(font_id, font_size, 'm')?.size.width)
    }

    /// Returns the advance width of an `em`.
    ///
    /// Uses the advance width of the `m` character in the given font and size.
    pub fn em_advance(&self, font_id: FontId, font_size: Pixels) -> Result<Pixels> {
        Ok(self.advance(font_id, font_size, 'm')?.width)
    }

    /// Returns the width of an `ch`.
    ///
    /// Uses the width of the `0` character in the given font and size.
    pub fn ch_width(&self, font_id: FontId, font_size: Pixels) -> Result<Pixels> {
        Ok(self.typographic_bounds(font_id, font_size, '0')?.size.width)
    }

    /// Returns the advance width of an `ch`.
    ///
    /// Uses the advance width of the `0` character in the given font and size.
    pub fn ch_advance(&self, font_id: FontId, font_size: Pixels) -> Result<Pixels> {
        Ok(self.advance(font_id, font_size, '0')?.width)
    }

    /// Get the number of font size units per 'em square',
    /// Per MDN: "an abstract square whose height is the intended distance between
    /// lines of type in the same type size"
    pub fn units_per_em(&self, font_id: FontId) -> u32 {
        self.read_metrics(font_id, |metrics| metrics.units_per_em)
    }

    /// Get the height of a capital letter in the given font and size.
    pub fn cap_height(&self, font_id: FontId, font_size: Pixels) -> Pixels {
        self.read_metrics(font_id, |metrics| metrics.cap_height(font_size))
    }

    /// Get the height of the x character in the given font and size.
    pub fn x_height(&self, font_id: FontId, font_size: Pixels) -> Pixels {
        self.read_metrics(font_id, |metrics| metrics.x_height(font_size))
    }

    /// Get the recommended distance from the baseline for the given font
    pub fn ascent(&self, font_id: FontId, font_size: Pixels) -> Pixels {
        self.read_metrics(font_id, |metrics| metrics.ascent(font_size))
    }

    /// Get the recommended distance below the baseline for the given font,
    /// in single spaced text.
    pub fn descent(&self, font_id: FontId, font_size: Pixels) -> Pixels {
        self.read_metrics(font_id, |metrics| metrics.descent(font_size))
    }

    /// Browser normal line boxes round each platform metric before summing it.
    pub fn normal_line_height(&self, font: &Font, font_size: Pixels, display_scale: f32) -> Pixels {
        let font_id = self.resolve_font(font);
        let scale = if cfg!(target_os = "linux") {
            1.
        } else if display_scale.is_finite() && display_scale > 0. {
            display_scale
        } else {
            1.
        };
        self.read_metrics(font_id, |metrics| {
            let factor = f64::from(font_size) * f64::from(scale) / f64::from(metrics.units_per_em);
            let height: f64 = [metrics.ascent, metrics.descent.abs(), metrics.line_gap]
                .into_iter()
                .map(|value| (f64::from(value) * factor).round() / f64::from(scale))
                .sum();
            px(height as f32)
        })
    }

    /// Get the underline thickness specified by the font.
    pub fn underline_thickness(&self, font_id: FontId, font_size: Pixels) -> Pixels {
        self.read_metrics(font_id, |metrics| metrics.underline_thickness(font_size))
    }

    /// Get the recommended baseline offset for the given font and line height.
    pub fn baseline_offset(
        &self,
        font_id: FontId,
        font_size: Pixels,
        line_height: Pixels,
    ) -> Pixels {
        let ascent = self.ascent(font_id, font_size);
        let descent = self.descent(font_id, font_size);
        let padding_top = (line_height - ascent - descent) / 2.;
        padding_top + ascent
    }

    fn read_metrics<T>(&self, font_id: FontId, read: impl FnOnce(&FontMetrics) -> T) -> T {
        let lock = self.font_metrics.upgradable_read();

        if let Some(metrics) = lock.get(&font_id) {
            read(metrics)
        } else {
            let mut lock = RwLockUpgradableReadGuard::upgrade(lock);
            let metrics = lock
                .entry(font_id)
                .or_insert_with(|| self.platform_text_system.font_metrics(font_id));
            read(metrics)
        }
    }

    /// Returns a handle to a line wrapper, for the given font and font size.
    pub fn line_wrapper(self: &Arc<Self>, font: Font, font_size: Pixels) -> LineWrapperHandle {
        let lock = &mut self.wrapper_pool.lock();
        let font_id = self.resolve_font(&font);
        let wrappers = lock
            .entry(FontIdWithSize { font_id, font_size })
            .or_default();
        let wrapper = wrappers.pop().unwrap_or_else(|| {
            LineWrapper::new(font_id, font_size, self.platform_text_system.clone())
        });

        LineWrapperHandle {
            wrapper: Some(wrapper),
            text_system: self.clone(),
        }
    }

    /// Get the rasterized size and location of a specific, rendered glyph.
    /// Rasterizes an actual platform glyph without a window for native font regression checks.
    #[cfg(any(test, feature = "test-support"))]
    pub fn rasterized_glyph_for_test(
        &self,
        font_id: FontId,
        glyph_id: GlyphId,
        font_size: Pixels,
        scale_factor: f32,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        let params = RenderGlyphParams {
            font_id,
            glyph_id,
            font_size,
            subpixel_variant: crate::point(0, 0),
            scale_factor,
            is_emoji: false,
        };
        self.rasterize_glyph(&params)
    }

    pub(crate) fn raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        let raster_bounds = self.raster_bounds.upgradable_read();
        if let Some(bounds) = raster_bounds.get(params) {
            Ok(*bounds)
        } else {
            let mut raster_bounds = RwLockUpgradableReadGuard::upgrade(raster_bounds);
            let bounds = self.platform_text_system.glyph_raster_bounds(params)?;
            raster_bounds.insert(params.clone(), bounds);
            Ok(bounds)
        }
    }

    pub(crate) fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        let raster_bounds = self.raster_bounds(params)?;
        self.platform_text_system
            .rasterize_glyph(params, raster_bounds)
    }
}

/// The GPUI text layout subsystem.
#[derive(Deref)]
pub struct WindowTextSystem {
    line_layout_cache: LineLayoutCache,
    optical_size_scale: AtomicU32,
    #[deref]
    text_system: Arc<TextSystem>,
}

impl WindowTextSystem {
    /// Creates a cached layout system, including for headless font measurement.
    pub fn new(text_system: Arc<TextSystem>) -> Self {
        Self {
            line_layout_cache: LineLayoutCache::new(text_system.platform_text_system.clone()),
            optical_size_scale: AtomicU32::new(1_f32.to_bits()),
            text_system,
        }
    }

    /// Keep variable-font optical sizing at the configured size during page zoom.
    pub fn set_optical_size_scale(&self, scale: f32) {
        if scale.is_finite()
            && scale > 0.
            && self
                .optical_size_scale
                .swap(scale.to_bits(), Ordering::Relaxed)
                != scale.to_bits()
        {
            self.text_system
                .fonts_revision
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Apply this window's optical size to fonts used by independent line wrappers.
    pub fn font_with_optical_size(&self, mut font: Font, font_size: Pixels) -> Font {
        let scale = f32::from_bits(self.optical_size_scale.load(Ordering::Relaxed));
        if scale != 1. && font.optical_size.is_none() {
            font.optical_size = Some(font_size / scale);
        }
        font
    }

    pub(crate) fn layout_index(&self) -> LineLayoutIndex {
        self.line_layout_cache.layout_index()
    }

    pub(crate) fn reuse_layouts(&self, index: Range<LineLayoutIndex>) {
        self.line_layout_cache.reuse_layouts(index)
    }

    pub(crate) fn truncate_layouts(&self, index: LineLayoutIndex) {
        self.line_layout_cache.truncate_layouts(index)
    }

    /// Shape the given line, at the given font_size, for painting to the screen.
    /// Subsets of the line can be styled independently with the `runs` parameter.
    ///
    /// Note that this method can only shape a single line of text. It will panic
    /// if the text contains newlines. If you need to shape multiple lines of text,
    /// use [`Self::shape_text`] instead.
    pub fn shape_line(
        &self,
        text: SharedString,
        font_size: Pixels,
        runs: &[TextRun],
        force_width: Option<Pixels>,
    ) -> ShapedLine {
        debug_assert!(
            text.find('\n').is_none(),
            "text argument should not contain newlines"
        );

        let mut decoration_runs = SmallVec::<[DecorationRun; 32]>::new();
        for run in runs {
            if let Some(last_run) = decoration_runs.last_mut()
                && last_run.color == run.color
                && last_run.underline == run.underline
                && last_run.overline == run.overline
                && last_run.strikethrough == run.strikethrough
                && last_run.background_color == run.background_color
            {
                last_run.len += run.len as u32;
                continue;
            }
            decoration_runs.push(DecorationRun {
                len: run.len as u32,
                color: run.color,
                background_color: run.background_color,
                underline: run.underline,
                overline: run.overline,
                strikethrough: run.strikethrough,
            });
        }

        let layout = self.layout_line(&text, font_size, runs, force_width);

        ShapedLine {
            layout,
            text,
            decoration_runs,
        }
    }

    /// Truncate tracked text at a grapheme boundary using its actual shaped advances.
    pub(crate) fn truncate_tracked_line(
        &self,
        text: SharedString,
        font_size: Pixels,
        width: Pixels,
        suffix: &str,
        runs: &mut Vec<TextRun>,
    ) -> SharedString {
        if self.layout_line(&text, font_size, runs, None).width <= width {
            return text;
        }
        use unicode_segmentation::UnicodeSegmentation;
        let boundaries = text
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let mut start = 0;
        let mut end = boundaries.len();
        while start < end {
            let middle = (start + end) / 2;
            let candidate = format!("{}{suffix}", &text[..boundaries[middle]]);
            let mut candidate_runs = runs.clone();
            line_wrapper::update_runs_after_truncation(&candidate, suffix, &mut candidate_runs);
            if self
                .layout_line(&candidate, font_size, &candidate_runs, None)
                .width
                <= width
            {
                start = middle + 1;
            } else {
                end = middle;
            }
        }
        let result = SharedString::from(format!(
            "{}{suffix}",
            &text[..boundaries[start.saturating_sub(1)]]
        ));
        line_wrapper::update_runs_after_truncation(&result, suffix, runs);
        result
    }

    /// Clamp complete physical rows, then truncate the final row with an ellipsis.
    pub(crate) fn clamp_text(
        &self,
        text: SharedString,
        font_size: Pixels,
        width: Pixels,
        limit: usize,
        wrap: crate::TextWrap,
        runs: &mut Vec<TextRun>,
    ) -> SharedString {
        if limit == 0 {
            runs.clear();
            return "".into();
        }
        let Ok(lines) = self.shape_text_with_wrap(
            text.clone(),
            font_size,
            runs,
            Some(width),
            Some(limit.saturating_add(1)),
            wrap,
        ) else {
            return text;
        };
        let mut count = 0;
        let mut offset = 0;
        let mut last_row = None;
        for line in &lines {
            for row in line.unwrapped_layout.fragments(&line.wrap_boundaries) {
                count += 1;
                if count == limit {
                    last_row = Some((offset + row.range.start, offset + line.len()));
                }
                if count > limit {
                    break;
                }
            }
            if count > limit {
                break;
            }
            offset += line.len() + 1;
        }
        if count <= limit {
            return text;
        }
        let Some((start, end)) = last_row else {
            return text;
        };
        let mut tail_runs = text_spacing::slice_runs(runs, start..end, start > 0);
        if let Some(last) = tail_runs.last_mut() {
            last.len += "…".len();
        } else if let Some(mut run) = runs.first().cloned() {
            run.len = "…".len();
            tail_runs.push(run);
        }
        let tail = self.truncate_tracked_line(
            format!("{}…", &text[start..end]).into(),
            font_size,
            width,
            "…",
            &mut tail_runs,
        );
        let mut prefix_runs = text_spacing::slice_runs(runs, 0..start, false);
        prefix_runs.extend(tail_runs);
        *runs = prefix_runs;
        format!("{}{tail}", &text[..start]).into()
    }

    /// Shape a multi line string of text, at the given font_size, for painting to the screen.
    /// Subsets of the text can be styled independently with the `runs` parameter.
    /// If `wrap_width` is provided, the line breaks will be adjusted to fit within the given width.
    pub fn shape_text(
        &self,
        text: SharedString,
        font_size: Pixels,
        runs: &[TextRun],
        wrap_width: Option<Pixels>,
        line_clamp: Option<usize>,
    ) -> Result<SmallVec<[WrappedLine; 1]>> {
        self.shape_text_with_wrap(
            text,
            font_size,
            runs,
            wrap_width,
            line_clamp,
            crate::TextWrap::Normal,
        )
    }

    /// Shape paragraph text with normal, emergency or per-grapheme word wrapping.
    pub fn shape_text_with_wrap(
        &self,
        text: SharedString,
        font_size: Pixels,
        runs: &[TextRun],
        wrap_width: Option<Pixels>,
        line_clamp: Option<usize>,
        wrap: crate::TextWrap,
    ) -> Result<SmallVec<[WrappedLine; 1]>> {
        let mut runs = runs.iter().filter(|run| run.len > 0).cloned().peekable();
        let mut font_runs = self.font_runs_pool.lock().pop().unwrap_or_default();

        let mut lines = SmallVec::new();
        let mut line_start = 0;
        let mut max_wrap_lines = line_clamp.unwrap_or(usize::MAX);
        let mut wrapped_lines = 0;

        let mut process_line = |line_text: SharedString| {
            font_runs.clear();
            let mut line_runs = Vec::new();
            // Wrapping must use shaped advances for automatic Latin direction too.
            // Summing isolated character widths loses kerning and ligatures.
            let shaped_wrap = wrap_width.is_some();
            let line_end = line_start + line_text.len();

            let mut decoration_runs = SmallVec::<[DecorationRun; 32]>::new();
            let mut run_start = line_start;
            while run_start < line_end {
                let Some(run) = runs.peek_mut() else {
                    break;
                };

                let run_len_within_line = cmp::min(line_end, run_start + run.len) - run_start;
                if shaped_wrap {
                    let mut line_run = run.clone();
                    line_run.len = run_len_within_line;
                    line_runs.push(line_run);
                }

                let decoration_changed = if let Some(last_run) = decoration_runs.last_mut()
                    && last_run.color == run.color
                    && last_run.underline == run.underline
                    && last_run.overline == run.overline
                    && last_run.strikethrough == run.strikethrough
                    && last_run.background_color == run.background_color
                {
                    last_run.len += run_len_within_line as u32;
                    false
                } else {
                    decoration_runs.push(DecorationRun {
                        len: run_len_within_line as u32,
                        color: run.color,
                        background_color: run.background_color,
                        underline: run.underline,
                        overline: run.overline,
                        strikethrough: run.strikethrough,
                    });
                    true
                };

                self.append_font_runs(
                    &line_text,
                    run_start - line_start..run_start - line_start + run_len_within_line,
                    font_size,
                    run,
                    !decoration_changed,
                    &mut font_runs,
                );

                if run_len_within_line == run.len {
                    runs.next();
                } else {
                    // Preserve the remainder of the run for the next line
                    run.len -= run_len_within_line;
                }
                run_start += run_len_within_line;
            }

            let layout = if let Some(width) = wrap_width
                && shaped_wrap
            {
                self.layout_cjk_wrapped_line(
                    &line_text,
                    font_size,
                    &line_runs,
                    width,
                    max_wrap_lines - wrapped_lines,
                    wrap,
                )
            } else {
                self.line_layout_cache.layout_wrapped_line(
                    &line_text,
                    font_size,
                    &font_runs,
                    wrap_width,
                    Some(max_wrap_lines - wrapped_lines),
                )
            };
            wrapped_lines += layout.wrap_boundaries.len();

            lines.push(WrappedLine {
                layout,
                decoration_runs,
                text: line_text,
            });

            // Skip `\n` character.
            line_start = line_end + 1;
            if let Some(run) = runs.peek_mut() {
                run.len -= 1;
                if run.len == 0 {
                    runs.next();
                }
            }
        };

        let mut split_lines = text.split('\n');
        let mut processed = false;

        if let Some(first_line) = split_lines.next()
            && let Some(second_line) = split_lines.next()
        {
            processed = true;
            process_line(first_line.to_string().into());
            process_line(second_line.to_string().into());
            for line_text in split_lines {
                process_line(line_text.to_string().into());
            }
        }

        if !processed {
            process_line(text);
        }

        self.font_runs_pool.lock().push(font_runs);

        Ok(lines)
    }

    pub(crate) fn finish_frame(&self) {
        self.line_layout_cache.finish_frame()
    }

    /// Layout the given line of text, at the given font_size.
    /// Subsets of the line can be styled independently with the `runs` parameter.
    /// Generally, you should prefer to use [`Self::shape_line`] instead, which
    /// can be painted directly.
    pub fn layout_line(
        &self,
        text: &str,
        font_size: Pixels,
        runs: &[TextRun],
        force_width: Option<Pixels>,
    ) -> Arc<LineLayout> {
        let mut last_run = None::<&TextRun>;
        let mut font_runs = self.font_runs_pool.lock().pop().unwrap_or_default();
        font_runs.clear();
        let mut run_start = 0;

        for run in runs.iter() {
            let decoration_changed = if let Some(last_run) = last_run
                && last_run.color == run.color
                && last_run.underline == run.underline
                && last_run.overline == run.overline
                && last_run.strikethrough == run.strikethrough
            // we do not consider differing background color relevant, as it does not affect glyphs
            // && last_run.background_color == run.background_color
            {
                false
            } else {
                last_run = Some(run);
                true
            };

            self.append_font_runs(
                text,
                run_start..run_start + run.len,
                font_size,
                run,
                !decoration_changed,
                &mut font_runs,
            );
            run_start += run.len;
        }

        let layout = self.line_layout_cache.layout_line(
            &SharedString::new(text),
            font_size,
            &font_runs,
            force_width,
        );

        self.font_runs_pool.lock().push(font_runs);

        layout
    }

    fn append_font_runs(
        &self,
        text: &str,
        range: Range<usize>,
        font_size: Pixels,
        run: &TextRun,
        mut merge: bool,
        output: &mut Vec<FontRun>,
    ) {
        let font_size = run.font_size.unwrap_or(font_size);
        let letter_spacing = run.letter_spacing + font_size * run.letter_spacing_fraction;
        let mut shaping_font = self.font_with_optical_size(run.font.clone(), font_size);
        if letter_spacing != px(0.) {
            let mut features = shaping_font.features.0.as_ref().clone();
            for tag in ["liga", "clig"] {
                if !features.iter().any(|(feature, _)| feature == tag) {
                    features.push((tag.into(), 0));
                }
            }
            shaping_font.features = FontFeatures(Arc::new(features));
        }
        let font_id = self.resolve_font(&shaping_font);
        let mut trimmed_font_id = None;
        let explicit_spacing = text_spacing::has_spacing_features(&run.font.features);
        let mut append = |len, trim| {
            if len == 0 {
                return;
            }
            let font_id = if trim {
                *trimmed_font_id.get_or_insert_with(|| {
                    let mut font = shaping_font.clone();
                    let mut features = font.features.0.as_ref().clone();
                    features.retain(|(tag, _)| tag != "halt");
                    features.push(("halt".into(), 1));
                    font.features = FontFeatures(Arc::new(features));
                    self.resolve_font(&font)
                })
            } else {
                font_id
            };
            let trim_punctuation = trim || explicit_spacing;
            if merge
                && let Some(last) = output.last_mut()
                && last.font_id == font_id
                && last.font_size == font_size
                && last.trim_punctuation == trim_punctuation
                && last.text_direction == run.text_direction
                && last.letter_spacing == letter_spacing
            {
                last.len += len;
            } else {
                output.push(FontRun {
                    font_size,
                    len,
                    font_id,
                    trim_punctuation,
                    letter_spacing,
                    text_direction: run.text_direction,
                });
            }
            merge = true;
        };
        if !text_spacing::allows_trim(run) {
            append(range.len(), false);
            return;
        }
        let mut start = range.start;
        let mut last_trim = false;
        for (offset, character) in text[range.clone()].char_indices() {
            let index = range.start + offset;
            let trim = text_spacing::should_trim(text, index, run.text_spacing_trim)
                // Proportional punctuation in the primary face keeps its original width.
                && self.advance(font_id, font_size, character)
                    .map_or(true, |advance| advance.width >= font_size * 0.95);
            if trim != last_trim {
                append(index - start, last_trim);
                start = index;
                last_trim = trim;
            }
        }
        append(range.end - start, last_trim);
    }
}

#[derive(Hash, Eq, PartialEq)]
struct FontIdWithSize {
    font_id: FontId,
    font_size: Pixels,
}

/// A handle into the text system, which can be used to compute the wrapped layout of text
pub struct LineWrapperHandle {
    wrapper: Option<LineWrapper>,
    text_system: Arc<TextSystem>,
}

impl Drop for LineWrapperHandle {
    fn drop(&mut self) {
        let mut state = self.text_system.wrapper_pool.lock();
        let wrapper = self.wrapper.take().unwrap();
        state
            .get_mut(&FontIdWithSize {
                font_id: wrapper.font_id,
                font_size: wrapper.font_size,
            })
            .unwrap()
            .push(wrapper);
    }
}

impl Deref for LineWrapperHandle {
    type Target = LineWrapper;

    fn deref(&self) -> &Self::Target {
        self.wrapper.as_ref().unwrap()
    }
}

impl DerefMut for LineWrapperHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.wrapper.as_mut().unwrap()
    }
}

/// The degree of blackness or stroke thickness of a font. This value ranges from 100.0 to 900.0,
/// with 400.0 as normal.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Add, Sub, FromStr)]
#[serde(transparent)]
pub struct FontWeight(pub f32);

impl Display for FontWeight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<f32> for FontWeight {
    fn from(weight: f32) -> Self {
        FontWeight(weight)
    }
}

impl Default for FontWeight {
    #[inline]
    fn default() -> FontWeight {
        FontWeight::NORMAL
    }
}

impl Hash for FontWeight {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(u32::from_be_bytes(self.0.to_be_bytes()));
    }
}

impl Eq for FontWeight {}

impl FontWeight {
    /// Thin weight (100), the thinnest value.
    pub const THIN: FontWeight = FontWeight(100.0);
    /// Extra light weight (200).
    pub const EXTRA_LIGHT: FontWeight = FontWeight(200.0);
    /// Light weight (300).
    pub const LIGHT: FontWeight = FontWeight(300.0);
    /// Normal (400).
    pub const NORMAL: FontWeight = FontWeight(400.0);
    /// Medium weight (500, higher than normal).
    pub const MEDIUM: FontWeight = FontWeight(500.0);
    /// Semibold weight (600).
    pub const SEMIBOLD: FontWeight = FontWeight(600.0);
    /// Bold weight (700).
    pub const BOLD: FontWeight = FontWeight(700.0);
    /// Extra-bold weight (800).
    pub const EXTRA_BOLD: FontWeight = FontWeight(800.0);
    /// Black weight (900), the thickest value.
    pub const BLACK: FontWeight = FontWeight(900.0);

    /// All of the font weights, in order from thinnest to thickest.
    pub const ALL: [FontWeight; 9] = [
        Self::THIN,
        Self::EXTRA_LIGHT,
        Self::LIGHT,
        Self::NORMAL,
        Self::MEDIUM,
        Self::SEMIBOLD,
        Self::BOLD,
        Self::EXTRA_BOLD,
        Self::BLACK,
    ];
}

impl schemars::JsonSchema for FontWeight {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "FontWeight".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        use schemars::json_schema;
        json_schema!({
            "type": "number",
            "minimum": Self::THIN,
            "maximum": Self::BLACK,
            "default": Self::default(),
            "description": "Font weight value between 100 (thin) and 900 (black)"
        })
    }
}

/// Allows italic or oblique faces to be selected.
#[derive(Clone, Copy, Eq, PartialEq, Debug, Hash, Default, Serialize, Deserialize, JsonSchema)]
pub enum FontStyle {
    /// A face that is neither italic not obliqued.
    #[default]
    Normal,
    /// A form that is generally cursive in nature.
    Italic,
    /// A typically-sloped version of the regular face.
    Oblique,
}

impl Display for FontStyle {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

/// Base direction used when resolving a paragraph's bidirectional text.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum TextDirection {
    /// Resolve the base direction from the paragraph's first strong character.
    #[default]
    Auto,
    /// Resolve the paragraph in a left-to-right context.
    Ltr,
    /// Resolve the paragraph in a right-to-left context.
    Rtl,
}

impl TextDirection {
    /// Explicit paragraph direction, or None for automatic detection.
    pub fn rtl(self) -> Option<bool> {
        match self {
            Self::Auto => None,
            Self::Ltr => Some(false),
            Self::Rtl => Some(true),
        }
    }
}

/// A styled run of text, for use in [`crate::TextLayout`].
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    /// An inline line height; None inherits the paragraph line height.
    pub line_height: Option<Pixels>,
    /// Font size for this run; None inherits the paragraph size.
    pub font_size: Option<Pixels>,
    /// A number of utf8 bytes
    pub len: usize,
    /// The font to use for this run.
    pub font: Font,
    /// Contextual fullwidth punctuation spacing.
    pub text_spacing_trim: TextSpacingTrim,
    /// Additional spacing per typographic character.
    pub letter_spacing: Pixels,

    /// Inherited percentage tracking, resolved after a run's font-size override.
    pub letter_spacing_fraction: f32,

    /// Paragraph reading direction, inherited from the text style.
    pub text_direction: TextDirection,
    /// The color
    pub color: Hsla,
    /// The background color (if any)
    pub background_color: Option<Hsla>,
    /// The underline style (if any)
    pub underline: Option<UnderlineStyle>,

    /// The overline style of the text.
    pub overline: Option<UnderlineStyle>,
    /// The strikethrough style (if any)
    pub strikethrough: Option<StrikethroughStyle>,
}

#[cfg(all(target_os = "macos", test))]
impl TextRun {
    fn with_len(&self, len: usize) -> Self {
        let mut this = self.clone();
        this.len = len;
        this
    }
}

/// An identifier for a specific glyph, as returned by [`WindowTextSystem::layout_line`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct GlyphId(pub(crate) u32);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RenderGlyphParams {
    pub(crate) font_id: FontId,
    pub(crate) glyph_id: GlyphId,
    pub(crate) font_size: Pixels,
    pub(crate) subpixel_variant: Point<u8>,
    pub(crate) scale_factor: f32,
    pub(crate) is_emoji: bool,
}

impl Eq for RenderGlyphParams {}

impl Hash for RenderGlyphParams {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.font_id.0.hash(state);
        self.glyph_id.0.hash(state);
        self.font_size.0.to_bits().hash(state);
        self.subpixel_variant.hash(state);
        self.scale_factor.to_bits().hash(state);
        self.is_emoji.hash(state);
    }
}

/// The configuration details for identifying a specific font.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Font {
    /// The font family name.
    ///
    /// The special name ".SystemUIFont" is used to identify the system UI font, which varies based on platform.
    pub family: SharedString,

    /// The font features to use.
    pub features: FontFeatures,

    /// Explicit variation coordinates, independent of face selection.
    pub variations: FontVariations,

    /// The fallbacks fonts to use.
    pub fallbacks: Option<FontFallbacks>,

    /// The font weight.
    pub weight: FontWeight,

    /// The font style.
    pub style: FontStyle,

    /// The unzoomed optical size; `None` follows the drawing size.
    pub optical_size: Option<Pixels>,
}

/// Get a [`Font`] for a given name.
pub fn font(family: impl Into<SharedString>) -> Font {
    Font {
        family: family.into(),
        features: FontFeatures::default(),
        variations: FontVariations::default(),
        weight: FontWeight::default(),
        style: FontStyle::default(),
        fallbacks: None,
        optical_size: None,
    }
}

impl Font {
    /// Set this Font to be bold
    pub fn bold(mut self) -> Self {
        self.weight = FontWeight::BOLD;
        self
    }

    /// Set this Font to be italic
    pub fn italic(mut self) -> Self {
        self.style = FontStyle::Italic;
        self
    }
}

/// A struct for storing font metrics.
/// It is used to define the measurements of a typeface.
#[derive(Clone, Copy, Debug)]
pub struct FontMetrics {
    /// The number of font units that make up the "em square",
    /// a scalable grid for determining the size of a typeface.
    pub(crate) units_per_em: u32,

    /// The vertical distance from the baseline of the font to the top of the glyph covers.
    pub(crate) ascent: f32,

    /// The vertical distance from the baseline of the font to the bottom of the glyph covers.
    pub(crate) descent: f32,

    /// The recommended additional space to add between lines of type.
    pub(crate) line_gap: f32,

    /// The suggested position of the underline.
    pub(crate) underline_position: f32,

    /// The suggested thickness of the underline.
    pub(crate) underline_thickness: f32,

    /// The height of a capital letter measured from the baseline of the font.
    pub(crate) cap_height: f32,

    /// The height of a lowercase x.
    pub(crate) x_height: f32,

    /// The outer limits of the area that the font covers.
    /// Corresponds to the xMin / xMax / yMin / yMax values in the OpenType `head` table
    pub(crate) bounding_box: Bounds<f32>,
}

impl FontMetrics {
    /// Returns the vertical distance from the baseline of the font to the top of the glyph covers in pixels.
    pub fn ascent(&self, font_size: Pixels) -> Pixels {
        Pixels((self.ascent / self.units_per_em as f32) * font_size.0)
    }

    /// Returns the vertical distance from the baseline of the font to the bottom of the glyph covers in pixels.
    pub fn descent(&self, font_size: Pixels) -> Pixels {
        Pixels((self.descent / self.units_per_em as f32) * font_size.0)
    }

    /// Returns the recommended additional space to add between lines of type in pixels.
    pub fn line_gap(&self, font_size: Pixels) -> Pixels {
        Pixels((self.line_gap / self.units_per_em as f32) * font_size.0)
    }

    /// Returns the suggested position of the underline in pixels.
    pub fn underline_position(&self, font_size: Pixels) -> Pixels {
        Pixels((self.underline_position / self.units_per_em as f32) * font_size.0)
    }

    /// Returns the suggested thickness of the underline in pixels.
    pub fn underline_thickness(&self, font_size: Pixels) -> Pixels {
        Pixels((self.underline_thickness / self.units_per_em as f32) * font_size.0)
    }

    /// Returns the height of a capital letter measured from the baseline of the font in pixels.
    pub fn cap_height(&self, font_size: Pixels) -> Pixels {
        Pixels((self.cap_height / self.units_per_em as f32) * font_size.0)
    }

    /// Returns the height of a lowercase x in pixels.
    pub fn x_height(&self, font_size: Pixels) -> Pixels {
        Pixels((self.x_height / self.units_per_em as f32) * font_size.0)
    }

    /// Returns the outer limits of the area that the font covers in pixels.
    pub fn bounding_box(&self, font_size: Pixels) -> Bounds<Pixels> {
        (self.bounding_box / self.units_per_em as f32 * font_size.0).map(px)
    }
}

#[allow(unused)]
pub(crate) fn font_name_with_fallbacks<'a>(name: &'a str, system: &'a str) -> &'a str {
    // Note: the "Zed Plex" fonts were deprecated as we are not allowed to use "Plex"
    // in a derived font name. They are essentially indistinguishable from IBM Plex/Lilex,
    // and so retained here for backward compatibility.
    match name {
        ".SystemUIFont" => system,
        "sans-serif" | "system-ui" | "ui-sans-serif" | "ui-rounded" => system,
        "serif" | "ui-serif" => {
            if cfg!(target_os = "macos") {
                "Times"
            } else if cfg!(target_os = "windows") {
                "Times New Roman"
            } else {
                "DejaVu Serif"
            }
        }
        "monospace" | "ui-monospace" => {
            if cfg!(target_os = "macos") {
                "Menlo"
            } else if cfg!(target_os = "windows") {
                "Consolas"
            } else {
                "DejaVu Sans Mono"
            }
        }
        ".ZedSans" | "Zed Plex Sans" => "IBM Plex Sans",
        ".ZedMono" | "Zed Plex Mono" => "Lilex",
        _ => name,
    }
}
