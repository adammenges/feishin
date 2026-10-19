use std::{borrow::Cow, sync::Arc};

use ::util::ResultExt;
use anyhow::{Context, Result};
use collections::HashMap;
use itertools::Itertools;
use parking_lot::{RwLock, RwLockUpgradableReadGuard};
use unicode_segmentation::UnicodeSegmentation as _;
use windows::{
    Win32::{
        Foundation::*,
        Globalization::GetUserDefaultLocaleName,
        Graphics::{
            Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP, Direct3D11::*, DirectWrite::*,
            Dxgi::Common::*, Gdi::LOGFONTW,
        },
        System::SystemServices::LOCALE_NAME_MAX_LENGTH,
        UI::WindowsAndMessaging::*,
    },
    core::*,
};
use windows_numerics::Vector2;

use crate::*;

#[derive(Clone, Debug)]
struct FontInfo {
    font_family: String,
    font_face: IDWriteFontFace3,
    features: IDWriteTypography,
    fallbacks: Option<IDWriteFontFallback>,
    is_system_font: bool,
    weight: FontWeight,
    style: FontStyle,
    optical_size: Option<Pixels>,
    variations: FontVariations,
}

pub(crate) struct DirectWriteTextSystem(RwLock<DirectWriteState>);

struct DirectWriteComponent {
    locale: String,
    factory: IDWriteFactory5,
    in_memory_loader: IDWriteInMemoryFontFileLoader,
    text_renderer: Arc<TextRendererWrapper>,

    gpu_state: GPUState,
}

struct GPUState {
    device: ID3D11Device,
    device_context: ID3D11DeviceContext,
    sampler: [Option<ID3D11SamplerState>; 1],
    blend_state: ID3D11BlendState,
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
}

struct DirectWriteState {
    components: DirectWriteComponent,
    system_ui_font_name: SharedString,
    system_font_collection: IDWriteFontCollection1,
    custom_font_collection: IDWriteFontCollection1,
    fonts: Vec<FontInfo>,
    font_selections: HashMap<Font, FontId>,
    font_id_by_identifier: HashMap<FontIdentifier, FontId>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct FontIdentifier {
    postscript_name: String,
    weight: i32,
    style: i32,
    simulations: i32,
    axes: Vec<(u32, u32)>,
}

impl DirectWriteComponent {
    pub fn new(directx_devices: &DirectXDevices) -> Result<Self> {
        // todo: ideally this would not be a large unsafe block but smaller isolated ones for easier auditing
        unsafe {
            let factory: IDWriteFactory5 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            // The `IDWriteInMemoryFontFileLoader` here is supported starting from
            // Windows 10 Creators Update, which consequently requires the entire
            // `DirectWriteTextSystem` to run on `win10 1703`+.
            let in_memory_loader = factory.CreateInMemoryFontFileLoader()?;
            factory.RegisterFontFileLoader(&in_memory_loader)?;
            let mut locale_vec = vec![0u16; LOCALE_NAME_MAX_LENGTH as usize];
            GetUserDefaultLocaleName(&mut locale_vec);
            let locale = String::from_utf16_lossy(&locale_vec);
            let text_renderer = Arc::new(TextRendererWrapper::new(&locale));

            let gpu_state = GPUState::new(directx_devices)?;

            Ok(DirectWriteComponent {
                locale,
                factory,
                in_memory_loader,
                text_renderer,
                gpu_state,
            })
        }
    }
}

impl GPUState {
    fn new(directx_devices: &DirectXDevices) -> Result<Self> {
        let device = directx_devices.device.clone();
        let device_context = directx_devices.device_context.clone();

        let blend_state = {
            let mut blend_state = None;
            let desc = D3D11_BLEND_DESC {
                AlphaToCoverageEnable: false.into(),
                IndependentBlendEnable: false.into(),
                RenderTarget: [
                    D3D11_RENDER_TARGET_BLEND_DESC {
                        BlendEnable: true.into(),
                        SrcBlend: D3D11_BLEND_ONE,
                        DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                        BlendOp: D3D11_BLEND_OP_ADD,
                        SrcBlendAlpha: D3D11_BLEND_ONE,
                        DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                        BlendOpAlpha: D3D11_BLEND_OP_ADD,
                        RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
                    },
                    Default::default(),
                    Default::default(),
                    Default::default(),
                    Default::default(),
                    Default::default(),
                    Default::default(),
                    Default::default(),
                ],
            };
            unsafe { device.CreateBlendState(&desc, Some(&mut blend_state)) }?;
            blend_state.unwrap()
        };

        let sampler = {
            let mut sampler = None;
            let desc = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
                AddressU: D3D11_TEXTURE_ADDRESS_BORDER,
                AddressV: D3D11_TEXTURE_ADDRESS_BORDER,
                AddressW: D3D11_TEXTURE_ADDRESS_BORDER,
                MipLODBias: 0.0,
                MaxAnisotropy: 1,
                ComparisonFunc: D3D11_COMPARISON_ALWAYS,
                BorderColor: [0.0, 0.0, 0.0, 0.0],
                MinLOD: 0.0,
                MaxLOD: 0.0,
            };
            unsafe { device.CreateSamplerState(&desc, Some(&mut sampler)) }?;
            [sampler]
        };

        let vertex_shader = {
            let source = shader_resources::RawShaderBytes::new(
                shader_resources::ShaderModule::EmojiRasterization,
                shader_resources::ShaderTarget::Vertex,
            )?;
            let mut shader = None;
            unsafe { device.CreateVertexShader(source.as_bytes(), None, Some(&mut shader)) }?;
            shader.unwrap()
        };

        let pixel_shader = {
            let source = shader_resources::RawShaderBytes::new(
                shader_resources::ShaderModule::EmojiRasterization,
                shader_resources::ShaderTarget::Fragment,
            )?;
            let mut shader = None;
            unsafe { device.CreatePixelShader(source.as_bytes(), None, Some(&mut shader)) }?;
            shader.unwrap()
        };

        Ok(Self {
            device,
            device_context,
            sampler,
            blend_state,
            vertex_shader,
            pixel_shader,
        })
    }
}

impl DirectWriteTextSystem {
    pub(crate) fn new(directx_devices: &DirectXDevices) -> Result<Self> {
        let components = DirectWriteComponent::new(directx_devices)?;
        let system_font_collection = unsafe {
            let mut result = std::mem::zeroed();
            components
                .factory
                .GetSystemFontCollection(false, &mut result, true)?;
            result.unwrap()
        };
        let custom_font_set =
            unsafe { components.factory.CreateFontSetBuilder()?.CreateFontSet()? };
        let custom_font_collection = unsafe {
            components
                .factory
                .CreateFontCollectionFromFontSet(&custom_font_set)?
        };
        let system_ui_font_name = get_system_ui_font_name();

        Ok(Self(RwLock::new(DirectWriteState {
            components,
            system_ui_font_name,
            system_font_collection,
            custom_font_collection,
            fonts: Vec::new(),
            font_selections: HashMap::default(),
            font_id_by_identifier: HashMap::default(),
        })))
    }

    pub(crate) fn handle_gpu_lost(&self, directx_devices: &DirectXDevices) {
        self.0.write().handle_gpu_lost(directx_devices);
    }
}

impl PlatformTextSystem for DirectWriteTextSystem {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        self.0.write().add_fonts(fonts)
    }

    fn all_font_names(&self) -> Vec<String> {
        self.0.read().all_font_names()
    }

    fn font_id(&self, font: &Font) -> Result<FontId> {
        let lock = self.0.upgradable_read();
        if let Some(font_id) = lock.font_selections.get(font) {
            Ok(*font_id)
        } else {
            let mut lock = RwLockUpgradableReadGuard::upgrade(lock);
            lock.resolve_font_id(font)
        }
    }

    fn font_metrics(&self, font_id: FontId) -> FontMetrics {
        self.0.read().font_metrics(font_id)
    }

    fn typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Bounds<f32>> {
        self.0.read().get_typographic_bounds(font_id, glyph_id)
    }

    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> anyhow::Result<Size<f32>> {
        self.0.read().get_advance(font_id, glyph_id)
    }

    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId> {
        self.0.read().glyph_for_char(font_id, ch)
    }

    fn glyph_raster_bounds(
        &self,
        params: &RenderGlyphParams,
    ) -> anyhow::Result<Bounds<DevicePixels>> {
        self.0.read().raster_bounds(params)
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> anyhow::Result<(Size<DevicePixels>, Vec<u8>)> {
        self.0.read().rasterize_glyph(params, raster_bounds)
    }

    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        self.0
            .write()
            .layout_line(text, font_size, runs)
            .log_err()
            .unwrap_or(LineLayout {
                font_size,
                ..Default::default()
            })
    }
}

impl DirectWriteState {
    fn resolve_font_id(&mut self, requested: &Font) -> Result<FontId> {
        if let Some(id) = self.font_selections.get(requested) {
            return Ok(*id);
        }
        let mut font = requested.clone();
        let mut font_id = self.select_font(&font);
        if font.variations.iter().next().is_some()
            && let Some(postscript) =
                get_postscript_name(&self.fonts[font_id.0].font_face, &self.components.locale)
                    .log_err()
            && let Some((family, weight)) = font.variations.fallback_companion(&postscript)
        {
            font.family = family.into();
            let mut settings = vec![("wght".to_owned(), weight)];
            settings.extend(
                font.variations
                    .iter()
                    .map(|(tag, value)| (tag.to_owned(), value)),
            );
            font.variations = FontVariations::try_from(settings).map_err(anyhow::Error::msg)?;
            font_id = self.select_font(&font);
        }
        if font.optical_size.is_some() || font.variations.iter().next().is_some() {
            let mut info = self.fonts[font_id.0].clone();
            info.optical_size = font.optical_size;
            info.variations = font.variations;
            info.font_face = super::font_variations::apply_to_face(
                &info.font_face,
                &info.variations,
                info.optical_size.map(f32::from),
            )?;
            font_id = FontId(self.fonts.len());
            self.fonts.push(info);
        }
        self.font_selections.insert(requested.clone(), font_id);
        Ok(font_id)
    }

    // Resolve explicit variation/optical instances before DirectWrite lays out
    // fallback spans, preserving the user's ordered family list and CSS defaults.
    fn resolve_variation_fallbacks(
        &mut self,
        text: &str,
        runs: &[FontRun],
    ) -> Result<Vec<FontRun>> {
        let mut output: Vec<FontRun> = Vec::new();
        let mut offset = 0;
        for run in runs {
            let fragment = &text[offset..offset + run.len];
            offset += run.len;
            let request = self
                .font_selections
                .iter()
                .find_map(|(font, id)| (*id == run.font_id).then(|| font.clone()));
            let Some(request) = request.filter(|font| {
                (font.variations.iter().next().is_some() || font.optical_size.is_some())
                    && font.fallbacks.is_some()
            }) else {
                output.push(*run);
                continue;
            };
            let start = output.len();
            for cluster in fragment.graphemes(true) {
                let mut selected = run.font_id;
                if !cluster.chars().all(|ch| {
                    self.glyph_for_char(selected, ch)
                        .is_some_and(|id| id.0 != 0)
                }) {
                    for family in request.fallbacks.as_ref().unwrap().fallback_list() {
                        if (family == "Noto Sans JP"
                            && !cluster
                                .chars()
                                .all(|ch| matches!(ch as u32, 0x3000..=0x9fff | 0xff00..=0xffef)))
                            || (family == "Noto Sans Hebrew"
                                && !cluster.chars().all(
                                    |ch| matches!(ch as u32, 0x0590..=0x05ff | 0xfb1d..=0xfb4f),
                                ))
                        {
                            continue;
                        }
                        let fallback = Font {
                            family: family.clone().into(),
                            fallbacks: None,
                            ..request.clone()
                        };
                        let id = self.resolve_font_id(&fallback)?;
                        let actual_family = &self.fonts[id.0].font_family;
                        if actual_family != family && actual_family != &format!("{family} Variable")
                        {
                            continue;
                        }
                        if cluster
                            .chars()
                            .all(|ch| self.glyph_for_char(id, ch).is_some_and(|id| id.0 != 0))
                        {
                            selected = id;
                            break;
                        }
                    }
                }
                if output.len() > start
                    && output.last().is_some_and(|last| last.font_id == selected)
                {
                    output.last_mut().unwrap().len += cluster.len();
                } else {
                    output.push(FontRun {
                        len: cluster.len(),
                        font_id: selected,
                        ..*run
                    });
                }
            }
        }
        Ok(output)
    }

    fn add_fonts(&mut self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        let builder = unsafe { self.components.factory.CreateFontSetBuilder()? };
        for font_data in fonts {
            match font_data {
                Cow::Borrowed(data) => unsafe {
                    let font_file = self
                        .components
                        .in_memory_loader
                        .CreateInMemoryFontFileReference(
                            &self.components.factory,
                            data.as_ptr() as _,
                            data.len() as _,
                            None,
                        )?;
                    builder.AddFontFile(&font_file)?;
                },
                Cow::Owned(data) => unsafe {
                    let font_file = self
                        .components
                        .in_memory_loader
                        .CreateInMemoryFontFileReference(
                            &self.components.factory,
                            data.as_ptr() as _,
                            data.len() as _,
                            None,
                        )?;
                    builder.AddFontFile(&font_file)?;
                },
            }
        }
        let imported = unsafe { builder.CreateFontSet()? };
        let mut names = collections::HashSet::default();
        for index in 0..unsafe { imported.GetFontCount() } {
            let face = unsafe { imported.GetFontFaceReference(index)?.CreateFontFace()? };
            names.insert(get_postscript_name(&face, &self.components.locale)?);
        }
        let previous = unsafe { self.custom_font_collection.GetFontSet()? };
        for index in 0..unsafe { previous.GetFontCount() } {
            let reference = unsafe { previous.GetFontFaceReference(index)? };
            let face = unsafe { reference.CreateFontFace()? };
            if !names.contains(&get_postscript_name(&face, &self.components.locale)?) {
                unsafe {
                    builder.AddFontFaceReference2(&reference)?;
                }
            }
        }
        let set = unsafe { builder.CreateFontSet()? };
        let collection = unsafe {
            self.components
                .factory
                .CreateFontCollectionFromFontSet(&set)?
        };
        self.custom_font_collection = collection;
        self.font_selections.clear();
        self.font_id_by_identifier.clear();

        Ok(())
    }

    fn generate_font_fallbacks(
        &self,
        fallbacks: &FontFallbacks,
    ) -> Result<Option<IDWriteFontFallback>> {
        if fallbacks.fallback_list().is_empty() {
            return Ok(None);
        }
        unsafe {
            let builder = self.components.factory.CreateFontFallbackBuilder()?;
            for family_name in fallbacks.fallback_list() {
                let Some((collection, fonts)) =
                    [&self.custom_font_collection, &self.system_font_collection]
                        .into_iter()
                        .find_map(|collection| {
                            let fonts = collection
                                .GetFontSet()
                                .and_then(|font_set| {
                                    font_set.GetMatchingFonts(
                                        &HSTRING::from(family_name),
                                        DWRITE_FONT_WEIGHT_NORMAL,
                                        DWRITE_FONT_STRETCH_NORMAL,
                                        DWRITE_FONT_STYLE_NORMAL,
                                    )
                                })
                                .log_err()?;
                            (fonts.GetFontCount() > 0).then_some((collection, fonts))
                        })
                else {
                    continue;
                };
                if fonts.GetFontCount() == 0 {
                    log::error!("No matching font found for {}", family_name);
                    continue;
                }
                let font = fonts.GetFontFaceReference(0)?.CreateFontFace()?;
                let mut count = 0;
                font.GetUnicodeRanges(None, &mut count).ok();
                if count == 0 {
                    continue;
                }
                let mut unicode_ranges = vec![DWRITE_UNICODE_RANGE::default(); count as usize];
                let Some(_) = font
                    .GetUnicodeRanges(Some(&mut unicode_ranges), &mut count)
                    .log_err()
                else {
                    continue;
                };
                let target_family_name = HSTRING::from(family_name);
                let collection: IDWriteFontCollection = collection.cast()?;
                builder.AddMapping(
                    &unicode_ranges,
                    &[target_family_name.as_ptr()],
                    Some(&collection),
                    None,
                    None,
                    1.0,
                )?;
            }
            let system_fallbacks = self.components.factory.GetSystemFontFallback()?;
            builder.AddMappings(&system_fallbacks)?;
            Ok(Some(builder.CreateFontFallback()?))
        }
    }

    unsafe fn generate_font_features(
        &self,
        font_features: &FontFeatures,
    ) -> Result<IDWriteTypography> {
        let direct_write_features = unsafe { self.components.factory.CreateTypography()? };
        apply_font_features(&direct_write_features, font_features)?;
        Ok(direct_write_features)
    }

    unsafe fn get_font_id_from_font_collection(
        &mut self,
        family_name: &str,
        font_weight: FontWeight,
        font_style: FontStyle,
        font_features: &FontFeatures,
        font_fallbacks: Option<&FontFallbacks>,
        is_system_font: bool,
    ) -> Option<FontId> {
        let collection = if is_system_font {
            &self.system_font_collection
        } else {
            &self.custom_font_collection
        };
        let fontset = unsafe { collection.GetFontSet().log_err()? };
        let font = unsafe {
            fontset
                .GetMatchingFonts(
                    &HSTRING::from(family_name),
                    font_weight.into(),
                    DWRITE_FONT_STRETCH_NORMAL,
                    font_style.into(),
                )
                .log_err()?
        };
        let total_number = unsafe { font.GetFontCount() };
        for index in 0..total_number {
            let Some(font_face_ref) = (unsafe { font.GetFontFaceReference(index).log_err() })
            else {
                continue;
            };
            let Some(font_face) = unsafe { font_face_ref.CreateFontFace() }
                .map_err(anyhow::Error::from)
                .and_then(|face| {
                    super::font_variations::synthesize_style(
                        face,
                        font_style != FontStyle::Normal,
                        font_weight.0,
                    )
                })
                .log_err()
            else {
                continue;
            };
            let Some(identifier) = get_font_identifier(&font_face, &self.components.locale) else {
                continue;
            };
            let Some(direct_write_features) =
                (unsafe { self.generate_font_features(font_features).log_err() })
            else {
                continue;
            };
            let fallbacks = font_fallbacks
                .and_then(|fallbacks| self.generate_font_fallbacks(fallbacks).log_err().flatten());
            let font_info = FontInfo {
                font_family: family_name.to_owned(),
                font_face,
                features: direct_write_features,
                fallbacks,
                is_system_font,
                weight: font_weight,
                style: font_style,
                optical_size: None,
                variations: FontVariations::default(),
            };
            let font_id = FontId(self.fonts.len());
            self.fonts.push(font_info);
            self.font_id_by_identifier.insert(identifier, font_id);
            return Some(font_id);
        }
        None
    }

    unsafe fn update_system_font_collection(&mut self) {
        let mut collection = unsafe { std::mem::zeroed() };
        if unsafe {
            self.components
                .factory
                .GetSystemFontCollection(false, &mut collection, true)
                .log_err()
                .is_some()
        } {
            self.system_font_collection = collection.unwrap();
        }
    }

    fn select_font(&mut self, target_font: &Font) -> FontId {
        unsafe {
            if target_font.family == ".SystemUIFont" {
                let family = self.system_ui_font_name.clone();
                self.find_font_id(
                    family.as_ref(),
                    target_font.weight,
                    target_font.style,
                    &target_font.features,
                    target_font.fallbacks.as_ref(),
                )
                .unwrap()
            } else {
                let family = self.system_ui_font_name.clone();
                self.find_font_id(
                    font_name_with_fallbacks(target_font.family.as_ref(), family.as_ref()),
                    target_font.weight,
                    target_font.style,
                    &target_font.features,
                    target_font.fallbacks.as_ref(),
                )
                .unwrap_or_else(|| {
                    #[cfg(any(test, feature = "test-support"))]
                    {
                        panic!("ERROR: {} font not found!", target_font.family);
                    }
                    #[cfg(not(any(test, feature = "test-support")))]
                    {
                        log::error!("{} not found, use {} instead.", target_font.family, family);
                        self.get_font_id_from_font_collection(
                            family.as_ref(),
                            target_font.weight,
                            target_font.style,
                            &target_font.features,
                            target_font.fallbacks.as_ref(),
                            true,
                        )
                        .unwrap()
                    }
                })
            }
        }
    }

    unsafe fn find_font_id(
        &mut self,
        family_name: &str,
        weight: FontWeight,
        style: FontStyle,
        features: &FontFeatures,
        fallbacks: Option<&FontFallbacks>,
    ) -> Option<FontId> {
        // try to find target font in custom font collection first
        unsafe {
            self.get_font_id_from_font_collection(
                family_name,
                weight,
                style,
                features,
                fallbacks,
                false,
            )
            .or_else(|| {
                self.get_font_id_from_font_collection(
                    family_name,
                    weight,
                    style,
                    features,
                    fallbacks,
                    true,
                )
            })
            .or_else(|| {
                self.update_system_font_collection();
                self.get_font_id_from_font_collection(
                    family_name,
                    weight,
                    style,
                    features,
                    fallbacks,
                    true,
                )
            })
        }
    }

    fn layout_line(
        &mut self,
        text: &str,
        font_size: Pixels,
        font_runs: &[FontRun],
    ) -> Result<LineLayout> {
        if font_runs.is_empty() {
            return Ok(LineLayout {
                font_size,
                ..Default::default()
            });
        }
        let resolved = self.resolve_variation_fallbacks(text, font_runs)?;
        let font_runs = resolved.as_slice();
        unsafe {
            let text_renderer = self.components.text_renderer.clone();
            let text_wide = text.encode_utf16().collect_vec();

            let mut utf8_offset = 0usize;
            let mut utf16_offset = 0u32;
            let text_layout = {
                let first_run = &font_runs[0];
                let font_info = &self.fonts[first_run.font_id.0];
                let collection = if font_info.is_system_font {
                    &self.system_font_collection
                } else {
                    &self.custom_font_collection
                };
                let format: IDWriteTextFormat1 = self
                    .components
                    .factory
                    .CreateTextFormat(
                        &HSTRING::from(&font_info.font_family),
                        collection,
                        font_info.weight.into(),
                        font_info.style.into(),
                        DWRITE_FONT_STRETCH_NORMAL,
                        first_run.font_size.0,
                        &HSTRING::from(&self.components.locale),
                    )?
                    .cast()?;
                if let Some(ref fallbacks) = font_info.fallbacks {
                    format.SetFontFallback(fallbacks)?;
                }
                super::font_variations::enable_variations(
                    &format,
                    &font_info.font_face,
                    &font_info.variations,
                    font_info.optical_size.unwrap_or(first_run.font_size).0,
                )?;
                let rtl = first_run.text_direction.rtl().unwrap_or_else(|| {
                    unicode_bidi::ParagraphBidiInfo::new(text, None)
                        .paragraph_level
                        .is_rtl()
                });
                format.SetReadingDirection(if rtl {
                    DWRITE_READING_DIRECTION_RIGHT_TO_LEFT
                } else {
                    DWRITE_READING_DIRECTION_LEFT_TO_RIGHT
                })?;
                format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;

                let layout = self.components.factory.CreateTextLayout(
                    &text_wide,
                    &format,
                    f32::INFINITY,
                    f32::INFINITY,
                )?;
                let current_text = &text[utf8_offset..(utf8_offset + first_run.len)];
                utf8_offset += first_run.len;
                let current_text_utf16_length = current_text.encode_utf16().count() as u32;
                let text_range = DWRITE_TEXT_RANGE {
                    startPosition: utf16_offset,
                    length: current_text_utf16_length,
                };
                layout.SetTypography(&font_info.features, text_range)?;
                utf16_offset += current_text_utf16_length;

                layout
            };

            let mut first_run = true;
            for run in font_runs {
                if first_run {
                    first_run = false;
                    continue;
                }
                let font_info = &self.fonts[run.font_id.0];
                let current_text = &text[utf8_offset..(utf8_offset + run.len)];
                utf8_offset += run.len;
                let current_text_utf16_length = current_text.encode_utf16().count() as u32;

                let collection = if font_info.is_system_font {
                    &self.system_font_collection
                } else {
                    &self.custom_font_collection
                };
                let text_range = DWRITE_TEXT_RANGE {
                    startPosition: utf16_offset,
                    length: current_text_utf16_length,
                };
                utf16_offset += current_text_utf16_length;
                text_layout.SetFontCollection(collection, text_range)?;
                text_layout
                    .SetFontFamilyName(&HSTRING::from(&font_info.font_family), text_range)?;
                text_layout.SetFontSize(run.font_size.0, text_range)?;
                text_layout.SetFontStyle(font_info.style.into(), text_range)?;
                text_layout.SetFontWeight(font_info.weight.into(), text_range)?;
                if let Ok(layout) = text_layout.cast::<IDWriteTextLayout4>() {
                    layout.SetFontAxisValues(
                        &super::font_variations::instance_axes(
                            &font_info.font_face,
                            &font_info.variations,
                            Some(font_info.optical_size.unwrap_or(run.font_size).0),
                        )?,
                        text_range,
                    )?;
                }
                text_layout.SetTypography(&font_info.features, text_range)?;
            }

            let mut line_metrics = vec![DWRITE_LINE_METRICS::default(); 4];
            let mut line_count = 0u32;
            text_layout.GetLineMetrics(Some(&mut line_metrics), &mut line_count as _)?;
            let ascent = px(line_metrics[0].baseline);
            let descent = px(line_metrics[0].height - line_metrics[0].baseline);
            let mut metrics = DWRITE_TEXT_METRICS::default();
            text_layout.GetMetrics(&mut metrics)?;
            let width = px(metrics.widthIncludingTrailingWhitespace);
            // RTL leading alignment must use the paragraph's finite width.
            text_layout.SetMaxWidth(width.0.max(1.))?;
            let mut runs = Vec::new();
            let mut cells = Vec::new();
            let renderer_context = RendererContext {
                text_system: self,
                index_converter: StringIndexConverter::new(text),
                runs: &mut runs,
                cells: &mut cells,
            };
            text_layout.Draw(
                Some(&renderer_context as *const _ as _),
                &text_renderer.0,
                0.0,
                0.0,
            )?;
            let mut layout = LineLayout {
                font_size,
                width,
                ascent,
                descent,
                runs,
                len: text.len(),
                carets: Vec::new(),
                cells: Vec::new(),
                bidi_levels: Vec::new(),
                visual_lines: Vec::new(),
                rtl: font_runs[0].text_direction.rtl().unwrap_or_else(|| {
                    unicode_bidi::ParagraphBidiInfo::new(text, None)
                        .paragraph_level
                        .is_rtl()
                }),
            };
            layout.set_cells(text, cells);
            Ok(layout)
        }
    }

    fn font_metrics(&self, font_id: FontId) -> FontMetrics {
        unsafe {
            let font_info = &self.fonts[font_id.0];
            let mut metrics = std::mem::zeroed();
            font_info.font_face.GetMetrics(&mut metrics);

            FontMetrics {
                units_per_em: metrics.Base.designUnitsPerEm as _,
                ascent: metrics.Base.ascent as _,
                descent: -(metrics.Base.descent as f32),
                line_gap: metrics.Base.lineGap as _,
                underline_position: metrics.Base.underlinePosition as _,
                underline_thickness: metrics.Base.underlineThickness as _,
                cap_height: metrics.Base.capHeight as _,
                x_height: metrics.Base.xHeight as _,
                bounding_box: Bounds {
                    origin: Point {
                        x: metrics.glyphBoxLeft as _,
                        y: metrics.glyphBoxBottom as _,
                    },
                    size: Size {
                        width: (metrics.glyphBoxRight - metrics.glyphBoxLeft) as _,
                        height: (metrics.glyphBoxTop - metrics.glyphBoxBottom) as _,
                    },
                },
            }
        }
    }

    fn create_glyph_run_analysis(
        &self,
        params: &RenderGlyphParams,
    ) -> Result<IDWriteGlyphRunAnalysis> {
        let font = &self.fonts[params.font_id.0];
        let glyph_id = [params.glyph_id.0 as u16];
        let advance = [0.0];
        let offset = [DWRITE_GLYPH_OFFSET::default()];
        let glyph_run = DWRITE_GLYPH_RUN {
            fontFace: unsafe { std::mem::transmute_copy(&font.font_face) },
            fontEmSize: params.font_size.0,
            glyphCount: 1,
            glyphIndices: glyph_id.as_ptr(),
            glyphAdvances: advance.as_ptr(),
            glyphOffsets: offset.as_ptr(),
            isSideways: BOOL(0),
            bidiLevel: 0,
        };
        let transform = DWRITE_MATRIX {
            m11: params.scale_factor,
            m12: 0.0,
            m21: 0.0,
            m22: params.scale_factor,
            dx: 0.0,
            dy: 0.0,
        };
        let baseline_origin_x =
            params.subpixel_variant.x as f32 / SUBPIXEL_VARIANTS_X as f32 / params.scale_factor;
        let baseline_origin_y =
            params.subpixel_variant.y as f32 / SUBPIXEL_VARIANTS_Y as f32 / params.scale_factor;

        let mut rendering_mode = DWRITE_RENDERING_MODE1::default();
        let mut grid_fit_mode = DWRITE_GRID_FIT_MODE::default();
        unsafe {
            font.font_face.GetRecommendedRenderingMode(
                params.font_size.0,
                // Using 96 as scale is applied by the transform
                96.0,
                96.0,
                Some(&transform),
                false,
                DWRITE_OUTLINE_THRESHOLD_ANTIALIASED,
                DWRITE_MEASURING_MODE_NATURAL,
                None,
                &mut rendering_mode,
                &mut grid_fit_mode,
            )?;
        }
        let rendering_mode = match rendering_mode {
            DWRITE_RENDERING_MODE1_OUTLINE => DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC,
            m => m,
        };

        let glyph_analysis = unsafe {
            self.components.factory.CreateGlyphRunAnalysis(
                &glyph_run,
                Some(&transform),
                rendering_mode,
                DWRITE_MEASURING_MODE_NATURAL,
                grid_fit_mode,
                DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE,
                baseline_origin_x,
                baseline_origin_y,
            )
        }?;
        Ok(glyph_analysis)
    }

    fn raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        let glyph_analysis = self.create_glyph_run_analysis(params)?;

        let bounds = unsafe { glyph_analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_ALIASED_1x1)? };

        if bounds.right < bounds.left {
            Ok(Bounds {
                origin: point(0.into(), 0.into()),
                size: size(0.into(), 0.into()),
            })
        } else {
            Ok(Bounds {
                origin: point(bounds.left.into(), bounds.top.into()),
                size: size(
                    (bounds.right - bounds.left).into(),
                    (bounds.bottom - bounds.top).into(),
                ),
            })
        }
    }

    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId> {
        let font_info = &self.fonts[font_id.0];
        let codepoints = [ch as u32];
        let mut glyph_indices = vec![0u16; 1];
        unsafe {
            font_info
                .font_face
                .GetGlyphIndices(codepoints.as_ptr(), 1, glyph_indices.as_mut_ptr())
                .log_err()
        }
        .map(|_| GlyphId(glyph_indices[0] as u32))
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        glyph_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        if glyph_bounds.size.width.0 == 0 || glyph_bounds.size.height.0 == 0 {
            anyhow::bail!("glyph bounds are empty");
        }

        let bitmap_data = if params.is_emoji {
            if let Ok(color) = self.rasterize_color(params, glyph_bounds) {
                color
            } else {
                let monochrome = self.rasterize_monochrome(params, glyph_bounds)?;
                monochrome
                    .into_iter()
                    .flat_map(|pixel| [0, 0, 0, pixel])
                    .collect::<Vec<_>>()
            }
        } else {
            self.rasterize_monochrome(params, glyph_bounds)?
        };

        Ok((glyph_bounds.size, bitmap_data))
    }

    fn rasterize_monochrome(
        &self,
        params: &RenderGlyphParams,
        glyph_bounds: Bounds<DevicePixels>,
    ) -> Result<Vec<u8>> {
        let mut bitmap_data =
            vec![0u8; glyph_bounds.size.width.0 as usize * glyph_bounds.size.height.0 as usize];

        let glyph_analysis = self.create_glyph_run_analysis(params)?;
        unsafe {
            glyph_analysis.CreateAlphaTexture(
                DWRITE_TEXTURE_ALIASED_1x1,
                &RECT {
                    left: glyph_bounds.origin.x.0,
                    top: glyph_bounds.origin.y.0,
                    right: glyph_bounds.size.width.0 + glyph_bounds.origin.x.0,
                    bottom: glyph_bounds.size.height.0 + glyph_bounds.origin.y.0,
                },
                &mut bitmap_data,
            )?;
        }

        Ok(bitmap_data)
    }

    fn rasterize_color(
        &self,
        params: &RenderGlyphParams,
        glyph_bounds: Bounds<DevicePixels>,
    ) -> Result<Vec<u8>> {
        let bitmap_size = glyph_bounds.size;
        let subpixel_shift = params
            .subpixel_variant
            .map(|v| v as f32 / SUBPIXEL_VARIANTS_X as f32);
        let baseline_origin_x = subpixel_shift.x / params.scale_factor;
        let baseline_origin_y = subpixel_shift.y / params.scale_factor;

        let transform = DWRITE_MATRIX {
            m11: params.scale_factor,
            m12: 0.0,
            m21: 0.0,
            m22: params.scale_factor,
            dx: 0.0,
            dy: 0.0,
        };

        let font = &self.fonts[params.font_id.0];
        let glyph_id = [params.glyph_id.0 as u16];
        let advance = [glyph_bounds.size.width.0 as f32];
        let offset = [DWRITE_GLYPH_OFFSET {
            advanceOffset: -glyph_bounds.origin.x.0 as f32 / params.scale_factor,
            ascenderOffset: glyph_bounds.origin.y.0 as f32 / params.scale_factor,
        }];
        let glyph_run = DWRITE_GLYPH_RUN {
            fontFace: unsafe { std::mem::transmute_copy(&font.font_face) },
            fontEmSize: params.font_size.0,
            glyphCount: 1,
            glyphIndices: glyph_id.as_ptr(),
            glyphAdvances: advance.as_ptr(),
            glyphOffsets: offset.as_ptr(),
            isSideways: BOOL(0),
            bidiLevel: 0,
        };

        // todo: support formats other than COLR
        let color_enumerator = unsafe {
            self.components.factory.TranslateColorGlyphRun(
                Vector2::new(baseline_origin_x, baseline_origin_y),
                &glyph_run,
                None,
                DWRITE_GLYPH_IMAGE_FORMATS_COLR,
                DWRITE_MEASURING_MODE_NATURAL,
                Some(&transform),
                0,
            )
        }?;

        let mut glyph_layers = Vec::new();
        loop {
            let color_run = unsafe { color_enumerator.GetCurrentRun() }?;
            let color_run = unsafe { &*color_run };
            let image_format = color_run.glyphImageFormat & !DWRITE_GLYPH_IMAGE_FORMATS_TRUETYPE;
            if image_format == DWRITE_GLYPH_IMAGE_FORMATS_COLR {
                let color_analysis = unsafe {
                    self.components.factory.CreateGlyphRunAnalysis(
                        &color_run.Base.glyphRun as *const _,
                        Some(&transform),
                        DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC,
                        DWRITE_MEASURING_MODE_NATURAL,
                        DWRITE_GRID_FIT_MODE_DEFAULT,
                        DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE,
                        baseline_origin_x,
                        baseline_origin_y,
                    )
                }?;

                let color_bounds =
                    unsafe { color_analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_ALIASED_1x1) }?;

                let color_size = size(
                    color_bounds.right - color_bounds.left,
                    color_bounds.bottom - color_bounds.top,
                );
                if color_size.width > 0 && color_size.height > 0 {
                    let mut alpha_data = vec![0u8; (color_size.width * color_size.height) as usize];
                    unsafe {
                        color_analysis.CreateAlphaTexture(
                            DWRITE_TEXTURE_ALIASED_1x1,
                            &color_bounds,
                            &mut alpha_data,
                        )
                    }?;

                    let run_color = {
                        let run_color = color_run.Base.runColor;
                        Rgba {
                            r: run_color.r,
                            g: run_color.g,
                            b: run_color.b,
                            a: run_color.a,
                        }
                    };
                    let bounds = bounds(point(color_bounds.left, color_bounds.top), color_size);
                    glyph_layers.push(GlyphLayerTexture::new(
                        &self.components.gpu_state,
                        run_color,
                        bounds,
                        &alpha_data,
                    )?);
                }
            }

            let has_next = unsafe { color_enumerator.MoveNext() }
                .map(|e| e.as_bool())
                .unwrap_or(false);
            if !has_next {
                break;
            }
        }

        let gpu_state = &self.components.gpu_state;
        let params_buffer = {
            let desc = D3D11_BUFFER_DESC {
                ByteWidth: std::mem::size_of::<GlyphLayerTextureParams>() as u32,
                Usage: D3D11_USAGE_DYNAMIC,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                MiscFlags: 0,
                StructureByteStride: 0,
            };

            let mut buffer = None;
            unsafe {
                gpu_state
                    .device
                    .CreateBuffer(&desc, None, Some(&mut buffer))
            }?;
            [buffer]
        };

        let render_target_texture = {
            let mut texture = None;
            let desc = D3D11_TEXTURE2D_DESC {
                Width: bitmap_size.width.0 as u32,
                Height: bitmap_size.height.0 as u32,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            unsafe {
                gpu_state
                    .device
                    .CreateTexture2D(&desc, None, Some(&mut texture))
            }?;
            texture.unwrap()
        };

        let render_target_view = {
            let desc = D3D11_RENDER_TARGET_VIEW_DESC {
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
                Anonymous: D3D11_RENDER_TARGET_VIEW_DESC_0 {
                    Texture2D: D3D11_TEX2D_RTV { MipSlice: 0 },
                },
            };
            let mut rtv = None;
            unsafe {
                gpu_state.device.CreateRenderTargetView(
                    &render_target_texture,
                    Some(&desc),
                    Some(&mut rtv),
                )
            }?;
            [rtv]
        };

        let staging_texture = {
            let mut texture = None;
            let desc = D3D11_TEXTURE2D_DESC {
                Width: bitmap_size.width.0 as u32,
                Height: bitmap_size.height.0 as u32,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };
            unsafe {
                gpu_state
                    .device
                    .CreateTexture2D(&desc, None, Some(&mut texture))
            }?;
            texture.unwrap()
        };

        let device_context = &gpu_state.device_context;
        unsafe { device_context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP) };
        unsafe { device_context.VSSetShader(&gpu_state.vertex_shader, None) };
        unsafe { device_context.PSSetShader(&gpu_state.pixel_shader, None) };
        unsafe { device_context.VSSetConstantBuffers(0, Some(&params_buffer)) };
        unsafe { device_context.PSSetConstantBuffers(0, Some(&params_buffer)) };
        unsafe { device_context.OMSetRenderTargets(Some(&render_target_view), None) };
        unsafe { device_context.PSSetSamplers(0, Some(&gpu_state.sampler)) };
        unsafe { device_context.OMSetBlendState(&gpu_state.blend_state, None, 0xffffffff) };

        let crate::FontInfo {
            gamma_ratios,
            grayscale_enhanced_contrast,
        } = DirectXRenderer::get_font_info();

        for layer in glyph_layers {
            let params = GlyphLayerTextureParams {
                run_color: layer.run_color,
                bounds: layer.bounds,
                gamma_ratios: *gamma_ratios,
                grayscale_enhanced_contrast: *grayscale_enhanced_contrast,
                _pad: [0f32; 3],
            };
            unsafe {
                let mut dest = std::mem::zeroed();
                gpu_state.device_context.Map(
                    params_buffer[0].as_ref().unwrap(),
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut dest),
                )?;
                std::ptr::copy_nonoverlapping(&params as *const _, dest.pData as *mut _, 1);
                gpu_state
                    .device_context
                    .Unmap(params_buffer[0].as_ref().unwrap(), 0);
            };

            let texture = [Some(layer.texture_view)];
            unsafe { device_context.PSSetShaderResources(0, Some(&texture)) };

            let viewport = [D3D11_VIEWPORT {
                TopLeftX: layer.bounds.origin.x as f32,
                TopLeftY: layer.bounds.origin.y as f32,
                Width: layer.bounds.size.width as f32,
                Height: layer.bounds.size.height as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }];
            unsafe { device_context.RSSetViewports(Some(&viewport)) };

            unsafe { device_context.Draw(4, 0) };
        }

        unsafe { device_context.CopyResource(&staging_texture, &render_target_texture) };

        let mapped_data = {
            let mut mapped_data = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                device_context.Map(
                    &staging_texture,
                    0,
                    D3D11_MAP_READ,
                    0,
                    Some(&mut mapped_data),
                )
            }?;
            mapped_data
        };
        let mut rasterized =
            vec![0u8; (bitmap_size.width.0 as u32 * bitmap_size.height.0 as u32 * 4) as usize];

        for y in 0..bitmap_size.height.0 as usize {
            let width = bitmap_size.width.0 as usize;
            unsafe {
                std::ptr::copy_nonoverlapping::<u8>(
                    (mapped_data.pData as *const u8).byte_add(mapped_data.RowPitch as usize * y),
                    rasterized
                        .as_mut_ptr()
                        .byte_add(width * y * std::mem::size_of::<u32>()),
                    width * std::mem::size_of::<u32>(),
                )
            };
        }

        // Convert from premultiplied to straight alpha
        for chunk in rasterized.chunks_exact_mut(4) {
            let b = chunk[0] as f32;
            let g = chunk[1] as f32;
            let r = chunk[2] as f32;
            let a = chunk[3] as f32;
            if a > 0.0 {
                let inv_a = 255.0 / a;
                chunk[0] = (b * inv_a).clamp(0.0, 255.0) as u8;
                chunk[1] = (g * inv_a).clamp(0.0, 255.0) as u8;
                chunk[2] = (r * inv_a).clamp(0.0, 255.0) as u8;
            }
        }

        Ok(rasterized)
    }

    fn get_typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Bounds<f32>> {
        unsafe {
            let font = &self.fonts[font_id.0].font_face;
            let glyph_indices = [glyph_id.0 as u16];
            let mut metrics = [DWRITE_GLYPH_METRICS::default()];
            font.GetDesignGlyphMetrics(glyph_indices.as_ptr(), 1, metrics.as_mut_ptr(), false)?;

            let metrics = &metrics[0];
            let advance_width = metrics.advanceWidth as i32;
            let advance_height = metrics.advanceHeight as i32;
            let left_side_bearing = metrics.leftSideBearing;
            let right_side_bearing = metrics.rightSideBearing;
            let top_side_bearing = metrics.topSideBearing;
            let bottom_side_bearing = metrics.bottomSideBearing;
            let vertical_origin_y = metrics.verticalOriginY;

            let y_offset = vertical_origin_y + bottom_side_bearing - advance_height;
            let width = advance_width - (left_side_bearing + right_side_bearing);
            let height = advance_height - (top_side_bearing + bottom_side_bearing);

            Ok(Bounds {
                origin: Point {
                    x: left_side_bearing as f32,
                    y: y_offset as f32,
                },
                size: Size {
                    width: width as f32,
                    height: height as f32,
                },
            })
        }
    }

    fn get_advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        unsafe {
            let font = &self.fonts[font_id.0].font_face;
            let glyph_indices = [glyph_id.0 as u16];
            let mut metrics = [DWRITE_GLYPH_METRICS::default()];
            font.GetDesignGlyphMetrics(glyph_indices.as_ptr(), 1, metrics.as_mut_ptr(), false)?;

            let metrics = &metrics[0];

            Ok(Size {
                width: metrics.advanceWidth as f32,
                height: 0.0,
            })
        }
    }

    fn all_font_names(&self) -> Vec<String> {
        let mut result =
            get_font_names_from_collection(&self.system_font_collection, &self.components.locale);
        result.extend(get_font_names_from_collection(
            &self.custom_font_collection,
            &self.components.locale,
        ));
        result
    }

    fn handle_gpu_lost(&mut self, directx_devices: &DirectXDevices) {
        try_to_recover_from_device_lost(
            || GPUState::new(directx_devices).context("Recreating GPU state for DirectWrite"),
            |gpu_state| self.components.gpu_state = gpu_state,
            || {
                log::error!(
                    "Failed to recreate GPU state for DirectWrite after multiple attempts."
                );
                // Do something here?
                // At this point, the device loss is considered unrecoverable.
            },
        );
    }
}

impl Drop for DirectWriteState {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .components
                .factory
                .UnregisterFontFileLoader(&self.components.in_memory_loader);
        }
    }
}

struct GlyphLayerTexture {
    run_color: Rgba,
    bounds: Bounds<i32>,
    texture_view: ID3D11ShaderResourceView,
    // holding on to the texture to not RAII drop it
    _texture: ID3D11Texture2D,
}

impl GlyphLayerTexture {
    pub fn new(
        gpu_state: &GPUState,
        run_color: Rgba,
        bounds: Bounds<i32>,
        alpha_data: &[u8],
    ) -> Result<Self> {
        let texture_size = bounds.size;

        let desc = D3D11_TEXTURE2D_DESC {
            Width: texture_size.width as u32,
            Height: texture_size.height as u32,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };

        let texture = {
            let mut texture: Option<ID3D11Texture2D> = None;
            unsafe {
                gpu_state
                    .device
                    .CreateTexture2D(&desc, None, Some(&mut texture))?
            };
            texture.unwrap()
        };
        let texture_view = {
            let mut view: Option<ID3D11ShaderResourceView> = None;
            unsafe {
                gpu_state
                    .device
                    .CreateShaderResourceView(&texture, None, Some(&mut view))?
            };
            view.unwrap()
        };

        unsafe {
            gpu_state.device_context.UpdateSubresource(
                &texture,
                0,
                None,
                alpha_data.as_ptr() as _,
                texture_size.width as u32,
                0,
            )
        };

        Ok(GlyphLayerTexture {
            run_color,
            bounds,
            texture_view,
            _texture: texture,
        })
    }
}

#[repr(C)]
struct GlyphLayerTextureParams {
    bounds: Bounds<i32>,
    run_color: Rgba,
    gamma_ratios: [f32; 4],
    grayscale_enhanced_contrast: f32,
    _pad: [f32; 3],
}

struct TextRendererWrapper(pub IDWriteTextRenderer);

impl TextRendererWrapper {
    pub fn new(locale_str: &str) -> Self {
        let inner = TextRenderer::new(locale_str);
        TextRendererWrapper(inner.into())
    }
}

#[implement(IDWriteTextRenderer)]
struct TextRenderer {
    locale: String,
}

impl TextRenderer {
    pub fn new(locale_str: &str) -> Self {
        TextRenderer {
            locale: locale_str.to_owned(),
        }
    }
}

struct RendererContext<'t, 'a, 'b> {
    text_system: &'t mut DirectWriteState,
    index_converter: StringIndexConverter<'a>,
    runs: &'b mut Vec<ShapedRun>,
    cells: &'b mut Vec<TextCell>,
}

#[derive(Debug)]
struct ClusterAnalyzer<'t> {
    utf16_idx: usize,
    glyph_idx: usize,
    glyph_count: usize,
    cluster_map: &'t [u16],
}

impl<'t> ClusterAnalyzer<'t> {
    pub fn new(cluster_map: &'t [u16], glyph_count: usize) -> Self {
        ClusterAnalyzer {
            utf16_idx: 0,
            glyph_idx: 0,
            glyph_count,
            cluster_map,
        }
    }
}

impl Iterator for ClusterAnalyzer<'_> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<(usize, usize)> {
        if self.utf16_idx >= self.cluster_map.len() {
            return None; // No more clusters
        }
        let start_utf16_idx = self.utf16_idx;
        let current_glyph = self.cluster_map[start_utf16_idx] as usize;

        // Find the end of current cluster (where glyph index changes)
        let mut end_utf16_idx = start_utf16_idx + 1;
        while end_utf16_idx < self.cluster_map.len()
            && self.cluster_map[end_utf16_idx] as usize == current_glyph
        {
            end_utf16_idx += 1;
        }

        let utf16_len = end_utf16_idx - start_utf16_idx;

        // Calculate glyph count for this cluster
        let next_glyph = if end_utf16_idx < self.cluster_map.len() {
            self.cluster_map[end_utf16_idx] as usize
        } else {
            self.glyph_count
        };

        let glyph_count = next_glyph - current_glyph;

        // Update state for next call
        self.utf16_idx = end_utf16_idx;
        self.glyph_idx = next_glyph;

        Some((utf16_len, glyph_count))
    }
}

#[allow(non_snake_case)]
impl IDWritePixelSnapping_Impl for TextRenderer_Impl {
    fn IsPixelSnappingDisabled(
        &self,
        _clientdrawingcontext: *const ::core::ffi::c_void,
    ) -> windows::core::Result<BOOL> {
        Ok(BOOL(0))
    }

    fn GetCurrentTransform(
        &self,
        _clientdrawingcontext: *const ::core::ffi::c_void,
        transform: *mut DWRITE_MATRIX,
    ) -> windows::core::Result<()> {
        unsafe {
            *transform = DWRITE_MATRIX {
                m11: 1.0,
                m12: 0.0,
                m21: 0.0,
                m22: 1.0,
                dx: 0.0,
                dy: 0.0,
            };
        }
        Ok(())
    }

    fn GetPixelsPerDip(
        &self,
        _clientdrawingcontext: *const ::core::ffi::c_void,
    ) -> windows::core::Result<f32> {
        Ok(1.0)
    }
}

#[allow(non_snake_case)]
impl IDWriteTextRenderer_Impl for TextRenderer_Impl {
    fn DrawGlyphRun(
        &self,
        clientdrawingcontext: *const ::core::ffi::c_void,
        baselineoriginx: f32,
        _baselineoriginy: f32,
        _measuringmode: DWRITE_MEASURING_MODE,
        glyphrun: *const DWRITE_GLYPH_RUN,
        glyphrundescription: *const DWRITE_GLYPH_RUN_DESCRIPTION,
        _clientdrawingeffect: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let glyphrun = unsafe { &*glyphrun };
        let glyph_count = glyphrun.glyphCount as usize;
        if glyph_count == 0 || glyphrun.fontFace.is_none() {
            return Ok(());
        }
        let desc = unsafe { &*glyphrundescription };
        let context = unsafe {
            &mut *(clientdrawingcontext as *const RendererContext as *mut RendererContext)
        };
        let font_face = glyphrun.fontFace.as_ref().unwrap();
        // This `cast()` action here should never fail since we are running on Win10+, and
        // `IDWriteFontFace3` requires Win10
        let font_face = &font_face.cast::<IDWriteFontFace3>().unwrap();
        let Some((font_identifier, font_struct, color_font)) =
            get_font_identifier_and_font_struct(font_face, &self.locale)
        else {
            return Ok(());
        };

        let font_id = if let Some(id) = context
            .text_system
            .font_id_by_identifier
            .get(&font_identifier)
        {
            *id
        } else {
            let id = FontId(context.text_system.fonts.len());
            context.text_system.fonts.push(FontInfo {
                font_family: font_struct.family.to_string(),
                font_face: font_face.clone(),
                features: unsafe { context.text_system.components.factory.CreateTypography()? },
                fallbacks: None,
                is_system_font: true,
                weight: font_struct.weight,
                style: font_struct.style,
                optical_size: font_struct.optical_size,
                variations: font_struct.variations,
            });
            context
                .text_system
                .font_id_by_identifier
                .insert(font_identifier, id);
            id
        };

        let glyph_ids = unsafe { std::slice::from_raw_parts(glyphrun.glyphIndices, glyph_count) };
        let glyph_advances =
            unsafe { std::slice::from_raw_parts(glyphrun.glyphAdvances, glyph_count) };
        let glyph_offsets =
            unsafe { std::slice::from_raw_parts(glyphrun.glyphOffsets, glyph_count) };
        let cluster_map =
            unsafe { std::slice::from_raw_parts(desc.clusterMap, desc.stringLength as usize) };

        let cluster_analyzer = ClusterAnalyzer::new(cluster_map, glyph_count);
        let mut utf16_idx = desc.textPosition as usize;
        let mut glyph_idx = 0;
        let mut glyphs = Vec::with_capacity(glyph_count);
        let rtl = glyphrun.bidiLevel % 2 != 0;
        let mut pen = baselineoriginx;
        for (cluster_utf16_len, cluster_glyph_count) in cluster_analyzer {
            if utf16_idx < context.index_converter.utf16_ix {
                context.index_converter = StringIndexConverter::new(context.index_converter.text);
            }
            context.index_converter.advance_to_utf16_ix(utf16_idx);
            let start = context.index_converter.utf8_ix;
            utf16_idx += cluster_utf16_len;
            let leading = pen;
            for (cluster_glyph_idx, glyph_id) in glyph_ids
                [glyph_idx..(glyph_idx + cluster_glyph_count)]
                .iter()
                .enumerate()
            {
                let id = GlyphId(*glyph_id as u32);
                let is_emoji = color_font
                    && is_color_glyph(font_face, id, &context.text_system.components.factory);
                let this_glyph_idx = glyph_idx + cluster_glyph_idx;
                let advance = glyph_advances[this_glyph_idx];
                if rtl {
                    pen -= advance;
                }
                glyphs.push(ShapedGlyph {
                    position_is_advance: true,
                    paint_offset: point(
                        px(if rtl {
                            -glyph_offsets[this_glyph_idx].advanceOffset
                        } else {
                            glyph_offsets[this_glyph_idx].advanceOffset
                        }),
                        px(-glyph_offsets[this_glyph_idx].ascenderOffset),
                    ),
                    id,
                    position: point(px(pen), px(0.0)),
                    index: start,
                    is_emoji,
                });
                if !rtl {
                    pen += advance;
                }
            }
            context.index_converter.advance_to_utf16_ix(utf16_idx);
            let end = context.index_converter.utf8_ix;
            let fragment = &context.index_converter.text[start..end];
            let count = fragment.graphemes(true).count().max(1) as f32;
            for (part, (index, grapheme)) in fragment.grapheme_indices(true).enumerate() {
                context.cells.push(TextCell {
                    range: start + index..start + index + grapheme.len(),
                    start_x: px(leading + (pen - leading) * part as f32 / count),
                    end_x: px(leading + (pen - leading) * (part + 1) as f32 / count),
                });
            }
            glyph_idx += cluster_glyph_count;
        }
        context.runs.push(ShapedRun {
            font_id,
            font_size: px(glyphrun.fontEmSize),
            glyphs,
        });
        Ok(())
    }

    fn DrawUnderline(
        &self,
        _clientdrawingcontext: *const ::core::ffi::c_void,
        _baselineoriginx: f32,
        _baselineoriginy: f32,
        _underline: *const DWRITE_UNDERLINE,
        _clientdrawingeffect: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        Err(windows::core::Error::new(
            E_NOTIMPL,
            "DrawUnderline unimplemented",
        ))
    }

    fn DrawStrikethrough(
        &self,
        _clientdrawingcontext: *const ::core::ffi::c_void,
        _baselineoriginx: f32,
        _baselineoriginy: f32,
        _strikethrough: *const DWRITE_STRIKETHROUGH,
        _clientdrawingeffect: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        Err(windows::core::Error::new(
            E_NOTIMPL,
            "DrawStrikethrough unimplemented",
        ))
    }

    fn DrawInlineObject(
        &self,
        _clientdrawingcontext: *const ::core::ffi::c_void,
        _originx: f32,
        _originy: f32,
        _inlineobject: windows::core::Ref<IDWriteInlineObject>,
        _issideways: BOOL,
        _isrighttoleft: BOOL,
        _clientdrawingeffect: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        Err(windows::core::Error::new(
            E_NOTIMPL,
            "DrawInlineObject unimplemented",
        ))
    }
}

struct StringIndexConverter<'a> {
    text: &'a str,
    utf8_ix: usize,
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

    #[allow(dead_code)]
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

impl Into<DWRITE_FONT_STYLE> for FontStyle {
    fn into(self) -> DWRITE_FONT_STYLE {
        match self {
            FontStyle::Normal => DWRITE_FONT_STYLE_NORMAL,
            FontStyle::Italic => DWRITE_FONT_STYLE_ITALIC,
            FontStyle::Oblique => DWRITE_FONT_STYLE_OBLIQUE,
        }
    }
}

impl From<DWRITE_FONT_STYLE> for FontStyle {
    fn from(value: DWRITE_FONT_STYLE) -> Self {
        match value.0 {
            0 => FontStyle::Normal,
            1 => FontStyle::Italic,
            2 => FontStyle::Oblique,
            _ => unreachable!(),
        }
    }
}

impl Into<DWRITE_FONT_WEIGHT> for FontWeight {
    fn into(self) -> DWRITE_FONT_WEIGHT {
        DWRITE_FONT_WEIGHT(self.0 as i32)
    }
}

impl From<DWRITE_FONT_WEIGHT> for FontWeight {
    fn from(value: DWRITE_FONT_WEIGHT) -> Self {
        FontWeight(value.0 as f32)
    }
}

fn get_font_names_from_collection(
    collection: &IDWriteFontCollection1,
    locale: &str,
) -> Vec<String> {
    unsafe {
        let mut result = Vec::new();
        let family_count = collection.GetFontFamilyCount();
        for index in 0..family_count {
            let Some(font_family) = collection.GetFontFamily(index).log_err() else {
                continue;
            };
            let Some(localized_family_name) = font_family.GetFamilyNames().log_err() else {
                continue;
            };
            let Some(family_name) = get_name(localized_family_name, locale).log_err() else {
                continue;
            };
            result.push(family_name);
        }

        result
    }
}

fn get_font_identifier_and_font_struct(
    font_face: &IDWriteFontFace3,
    locale: &str,
) -> Option<(FontIdentifier, Font, bool)> {
    let postscript_name = get_postscript_name(font_face, locale).log_err()?;
    let localized_family_name = unsafe { font_face.GetFamilyNames().log_err() }?;
    let family_name = get_name(localized_family_name, locale).log_err()?;
    let weight = unsafe { font_face.GetWeight() };
    let style = unsafe { font_face.GetStyle() };
    let simulations = unsafe { font_face.GetSimulations() };
    let identifier = FontIdentifier {
        postscript_name,
        weight: weight.0,
        style: style.0,
        simulations: simulations.0,
        axes: super::font_variations::axes(font_face).log_err()?,
    };
    let font_struct = Font {
        family: family_name.into(),
        features: FontFeatures::default(),
        variations: crate::FontVariations::default(),
        weight: weight.into(),
        style: if style == DWRITE_FONT_STYLE_NORMAL
            && simulations.0 & DWRITE_FONT_SIMULATIONS_OBLIQUE.0 != 0
        {
            FontStyle::Oblique
        } else {
            style.into()
        },
        fallbacks: None,
        optical_size: None,
    };
    let is_emoji = unsafe { font_face.IsColorFont().as_bool() };
    Some((identifier, font_struct, is_emoji))
}

#[inline]
fn get_font_identifier(font_face: &IDWriteFontFace3, locale: &str) -> Option<FontIdentifier> {
    let weight = unsafe { font_face.GetWeight().0 };
    let style = unsafe { font_face.GetStyle().0 };
    let simulations = unsafe { font_face.GetSimulations().0 };
    let axes = super::font_variations::axes(font_face).log_err()?;
    get_postscript_name(font_face, locale)
        .log_err()
        .map(|postscript_name| FontIdentifier {
            postscript_name,
            weight,
            style,
            simulations,
            axes,
        })
}

#[inline]
fn get_postscript_name(font_face: &IDWriteFontFace3, locale: &str) -> Result<String> {
    let mut info = None;
    let mut exists = BOOL(0);
    unsafe {
        font_face.GetInformationalStrings(
            DWRITE_INFORMATIONAL_STRING_POSTSCRIPT_NAME,
            &mut info,
            &mut exists,
        )?
    };
    if !exists.as_bool() || info.is_none() {
        anyhow::bail!("No postscript name found for font face");
    }

    get_name(info.unwrap(), locale)
}

// https://learn.microsoft.com/en-us/windows/win32/api/dwrite/ne-dwrite-dwrite_font_feature_tag
fn apply_font_features(
    direct_write_features: &IDWriteTypography,
    features: &FontFeatures,
) -> Result<()> {
    let tag_values = features.tag_value_list();
    if tag_values.is_empty() {
        return Ok(());
    }

    // All of these features are enabled by default by DirectWrite.
    // If you want to (and can) peek into the source of DirectWrite
    let mut feature_liga = make_direct_write_feature("liga", 1);
    let mut feature_clig = make_direct_write_feature("clig", 1);
    let mut feature_calt = make_direct_write_feature("calt", 1);

    for (tag, value) in tag_values {
        if tag.as_str() == "liga" && *value == 0 {
            feature_liga.parameter = 0;
            continue;
        }
        if tag.as_str() == "clig" && *value == 0 {
            feature_clig.parameter = 0;
            continue;
        }
        if tag.as_str() == "calt" && *value == 0 {
            feature_calt.parameter = 0;
            continue;
        }

        unsafe {
            direct_write_features.AddFontFeature(make_direct_write_feature(tag, *value))?;
        }
    }
    unsafe {
        direct_write_features.AddFontFeature(feature_liga)?;
        direct_write_features.AddFontFeature(feature_clig)?;
        direct_write_features.AddFontFeature(feature_calt)?;
    }

    Ok(())
}

#[inline]
const fn make_direct_write_feature(feature_name: &str, parameter: u32) -> DWRITE_FONT_FEATURE {
    let tag = make_direct_write_tag(feature_name);
    DWRITE_FONT_FEATURE {
        nameTag: tag,
        parameter,
    }
}

#[inline]
const fn make_open_type_tag(tag_name: &str) -> u32 {
    let bytes = tag_name.as_bytes();
    debug_assert!(bytes.len() == 4);
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[inline]
const fn make_direct_write_tag(tag_name: &str) -> DWRITE_FONT_FEATURE_TAG {
    DWRITE_FONT_FEATURE_TAG(make_open_type_tag(tag_name))
}

#[inline]
fn get_name(string: IDWriteLocalizedStrings, locale: &str) -> Result<String> {
    let mut locale_name_index = 0u32;
    let mut exists = BOOL(0);
    unsafe {
        string.FindLocaleName(
            &HSTRING::from(locale),
            &mut locale_name_index,
            &mut exists as _,
        )?
    };
    if !exists.as_bool() {
        unsafe {
            string.FindLocaleName(
                DEFAULT_LOCALE_NAME,
                &mut locale_name_index as _,
                &mut exists as _,
            )?
        };
        anyhow::ensure!(exists.as_bool(), "No localised string for {locale}");
    }

    let name_length = unsafe { string.GetStringLength(locale_name_index) }? as usize;
    let mut name_vec = vec![0u16; name_length + 1];
    unsafe {
        string.GetString(locale_name_index, &mut name_vec)?;
    }

    Ok(String::from_utf16_lossy(&name_vec[..name_length]))
}

fn get_system_ui_font_name() -> SharedString {
    unsafe {
        let mut info: LOGFONTW = std::mem::zeroed();
        let font_family = if SystemParametersInfoW(
            SPI_GETICONTITLELOGFONT,
            std::mem::size_of::<LOGFONTW>() as u32,
            Some(&mut info as *mut _ as _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .log_err()
        .is_none()
        {
            // https://learn.microsoft.com/en-us/windows/win32/uxguide/vis-fonts
            // Segoe UI is the Windows font intended for user interface text strings.
            "Segoe UI".into()
        } else {
            let font_name = String::from_utf16_lossy(&info.lfFaceName);
            font_name.trim_matches(char::from(0)).to_owned().into()
        };
        log::info!("Use {} as UI font.", font_family);
        font_family
    }
}

// One would think that with newer DirectWrite method: IDWriteFontFace4::GetGlyphImageFormats
// but that doesn't seem to work for some glyphs, say ❤
fn is_color_glyph(
    font_face: &IDWriteFontFace3,
    glyph_id: GlyphId,
    factory: &IDWriteFactory5,
) -> bool {
    let glyph_run = DWRITE_GLYPH_RUN {
        fontFace: unsafe { std::mem::transmute_copy(font_face) },
        fontEmSize: 14.0,
        glyphCount: 1,
        glyphIndices: &(glyph_id.0 as u16),
        glyphAdvances: &0.0,
        glyphOffsets: &DWRITE_GLYPH_OFFSET {
            advanceOffset: 0.0,
            ascenderOffset: 0.0,
        },
        isSideways: BOOL(0),
        bidiLevel: 0,
    };
    unsafe {
        factory.TranslateColorGlyphRun(
            Vector2::default(),
            &glyph_run as _,
            None,
            DWRITE_GLYPH_IMAGE_FORMATS_COLR
                | DWRITE_GLYPH_IMAGE_FORMATS_SVG
                | DWRITE_GLYPH_IMAGE_FORMATS_PNG
                | DWRITE_GLYPH_IMAGE_FORMATS_JPEG
                | DWRITE_GLYPH_IMAGE_FORMATS_PREMULTIPLIED_B8G8R8A8,
            DWRITE_MEASURING_MODE_NATURAL,
            None,
            0,
        )
    }
    .is_ok()
}

const DEFAULT_LOCALE_NAME: PCWSTR = windows::core::w!("en-US");

#[cfg(test)]
mod tests {
    use crate::platform::windows::direct_write::ClusterAnalyzer;

    #[test]
    fn test_cluster_map() {
        let cluster_map = [0];
        let mut analyzer = ClusterAnalyzer::new(&cluster_map, 1);
        let next = analyzer.next();
        assert_eq!(next, Some((1, 1)));
        let next = analyzer.next();
        assert_eq!(next, None);

        let cluster_map = [0, 1, 2];
        let mut analyzer = ClusterAnalyzer::new(&cluster_map, 3);
        let next = analyzer.next();
        assert_eq!(next, Some((1, 1)));
        let next = analyzer.next();
        assert_eq!(next, Some((1, 1)));
        let next = analyzer.next();
        assert_eq!(next, Some((1, 1)));
        let next = analyzer.next();
        assert_eq!(next, None);
        // 👨‍👩‍👧‍👦👩‍💻
        let cluster_map = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 4, 4, 4, 4];
        let mut analyzer = ClusterAnalyzer::new(&cluster_map, 5);
        let next = analyzer.next();
        assert_eq!(next, Some((11, 4)));
        let next = analyzer.next();
        assert_eq!(next, Some((5, 1)));
        let next = analyzer.next();
        assert_eq!(next, None);
        // 👩‍💻
        let cluster_map = [0, 0, 0, 0, 0];
        let mut analyzer = ClusterAnalyzer::new(&cluster_map, 1);
        let next = analyzer.next();
        assert_eq!(next, Some((5, 1)));
        let next = analyzer.next();
        assert_eq!(next, None);
    }
}
