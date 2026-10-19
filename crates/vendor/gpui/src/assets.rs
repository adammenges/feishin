use crate::{DevicePixels, Pixels, Result, SharedString, Size, size};
use smallvec::SmallVec;

use image::{
    AnimationDecoder, Delay, DynamicImage, Frame, ImageDecoder, ImageFormat, ImageReader,
    codecs::{gif::GifDecoder, png::PngDecoder, webp::WebPDecoder},
    metadata::Orientation,
};
use std::{
    borrow::Cow,
    fmt,
    hash::Hash,
    sync::atomic::{AtomicUsize, Ordering::SeqCst},
};

/// A source of assets for this app to use.
pub trait AssetSource: 'static + Send + Sync {
    /// Load the given asset from the source path.
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>>;

    /// List the assets at the given path.
    fn list(&self, path: &str) -> Result<Vec<SharedString>>;
}

impl AssetSource for () {
    fn load(&self, _path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(None)
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

/// A unique identifier for the image cache
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ImageId(pub usize);

#[derive(PartialEq, Eq, Hash, Clone)]
pub(crate) struct RenderImageParams {
    pub(crate) image_id: ImageId,
    pub(crate) frame_index: usize,
}

/// A cached and processed image, in BGRA format
pub struct RenderImage {
    /// The ID associated with this image
    pub id: ImageId,
    /// The scale factor of this image on render.
    pub(crate) scale_factor: f32,
    data: SmallVec<[Frame; 1]>,
}

impl PartialEq for RenderImage {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for RenderImage {}

impl RenderImage {
    /// Create a new image from the given data.
    pub fn new(data: impl Into<SmallVec<[Frame; 1]>>) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

        Self {
            id: ImageId(NEXT_ID.fetch_add(1, SeqCst)),
            scale_factor: 1.0,
            data: data.into(),
        }
    }

    /// Convert this image into a byte slice.
    pub fn as_bytes(&self, frame_index: usize) -> Option<&[u8]> {
        self.data
            .get(frame_index)
            .map(|frame| frame.buffer().as_raw().as_slice())
    }

    /// Get the size of this image, in pixels.
    pub fn size(&self, frame_index: usize) -> Size<DevicePixels> {
        let (width, height) = self.data[frame_index].buffer().dimensions();
        size(width.into(), height.into())
    }

    /// Get the size of this image, in pixels for display, adjusted for the scale factor.
    pub(crate) fn render_size(&self, frame_index: usize) -> Size<Pixels> {
        self.size(frame_index)
            .map(|v| (v.0 as f32 / self.scale_factor).into())
    }

    /// Get the delay of this frame from the previous
    pub fn delay(&self, frame_index: usize) -> Delay {
        self.data[frame_index].delay()
    }

    /// Get the number of frames for this image.
    pub fn frame_count(&self) -> usize {
        self.data.len()
    }
}

impl fmt::Debug for RenderImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageData")
            .field("id", &self.id)
            .field("size", &self.size(0))
            .finish()
    }
}

/// Convert an embedded RGB or grayscale ICC profile to the renderer's sRGB space.
/// Invalid profiles fall back to the decoded pixels, as untagged artwork does.
pub fn color_managed_rgba8(image: DynamicImage, icc: Option<&[u8]>) -> image::RgbaImage {
    let gray = matches!(
        image.color(),
        image::ColorType::L8
            | image::ColorType::La8
            | image::ColorType::L16
            | image::ColorType::La16
    );
    color_managed_rgba8_with_gray(image, icc, gray)
}

fn color_managed_rgba8_with_gray(
    image: DynamicImage,
    icc: Option<&[u8]>,
    gray: bool,
) -> image::RgbaImage {
    use moxcms::{ColorProfile, DataColorSpace, Layout, TransformOptions};
    let mut rgba = image.into_rgba8();
    let Some(icc) = icc else { return rgba };
    let profile = match ColorProfile::new_from_slice(icc) {
        Ok(profile) => profile,
        Err(error) => {
            log::debug!("Artwork ICC profile is invalid: {error}");
            return rgba;
        }
    };
    let destination = ColorProfile::new_srgb();
    let converted = match profile.color_space {
        DataColorSpace::Rgb => profile
            .create_in_place_transform_8bit(Layout::Rgba, &destination, TransformOptions::default())
            .and_then(|transform| transform.transform(rgba.as_mut())),
        DataColorSpace::Gray if gray && rgba.width() > 0 => profile
            .create_transform_8bit(
                Layout::GrayAlpha,
                &destination,
                Layout::Rgba,
                TransformOptions::default(),
            )
            .and_then(|transform| {
                // A single scanline bounds temporary storage for grayscale expansion.
                let mut row = vec![0; rgba.width() as usize * 2];
                let pixels: &mut [u8] = rgba.as_mut();
                for pixels in pixels.chunks_exact_mut(row.len() * 2) {
                    for (source, target) in pixels.chunks_exact(4).zip(row.chunks_exact_mut(2)) {
                        target.copy_from_slice(&[source[0], source[3]]);
                    }
                    transform.transform(&row, pixels)?;
                }
                Ok(())
            }),
        // CMYK JPEG profiles use the JPEG-specific conversion below.
        _ => return rgba,
    };
    if let Err(error) = converted {
        log::debug!("Artwork ICC conversion failed: {error}");
    }
    rgba
}

fn color_managed_cmyk_jpeg(bytes: &[u8], icc: &[u8]) -> Option<image::RgbaImage> {
    use moxcms::{ColorProfile, DataColorSpace, Layout, TransformOptions};
    use zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions};
    let profile = ColorProfile::new_from_slice(icc).ok()?;
    if profile.color_space != DataColorSpace::Cmyk {
        return None;
    }
    let result = (|| -> Result<image::RgbaImage> {
        let mut decoder = zune_jpeg::JpegDecoder::new_with_options(
            ZCursor::new(bytes),
            DecoderOptions::default()
                .set_strict_mode(false)
                .set_max_width(usize::MAX)
                .set_max_height(usize::MAX),
        );
        decoder
            .decode_headers()
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        let color = decoder
            .input_colorspace()
            .ok_or_else(|| anyhow::anyhow!("Missing JPEG color space"))?;
        anyhow::ensure!(
            matches!(color, ColorSpace::CMYK | ColorSpace::YCCK),
            "CMYK profile on non-CMYK JPEG"
        );
        let (width, height) = decoder
            .dimensions()
            .ok_or_else(|| anyhow::anyhow!("Missing JPEG dimensions"))?;
        decoder.set_options(decoder.options().jpeg_set_out_colorspace(color));
        let mut pixels = decoder
            .decode()
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        // Adobe JPEG stores inverted CMYK; YCCK stores the CMY components as YCbCr.
        for pixel in pixels.chunks_exact_mut(4) {
            if color == ColorSpace::CMYK {
                for channel in pixel {
                    *channel = 255 - *channel;
                }
            } else {
                let (y, cb, cr) = (
                    f64::from(pixel[0]),
                    f64::from(pixel[1]) - 128.,
                    f64::from(pixel[2]) - 128.,
                );
                pixel[0] = (y + 1.402 * cr).round().clamp(0., 255.) as u8;
                pixel[1] = (y - 0.344136 * cb - 0.714136 * cr).round().clamp(0., 255.) as u8;
                pixel[2] = (y + 1.772 * cb).round().clamp(0., 255.) as u8;
                pixel[3] = 255 - pixel[3];
            }
        }
        // Chromium combines the black component into RGB before applying the
        // profile. Match its integer division, then interpret those RGB values
        // as inverted CMY with no remaining black component.
        for pixel in pixels.chunks_exact_mut(4) {
            let black = u16::from(255 - pixel[3]);
            for channel in &mut pixel[..3] {
                *channel = 255 - (u16::from(255 - *channel) * black / 255) as u8;
            }
            pixel[3] = 0;
        }
        let transform = profile.create_transform_8bit(
            Layout::Rgba,
            &ColorProfile::new_srgb(),
            Layout::Rgba,
            TransformOptions::default(),
        )?;
        // Reuse the decoded buffer and limit staged ICC temporary storage to one row.
        let mut row = vec![0; width * 4];
        for pixels in pixels.chunks_exact_mut(row.len()) {
            row.copy_from_slice(pixels);
            transform.transform(&row, pixels)?;
            for pixel in pixels.chunks_exact_mut(4) {
                pixel[3] = 255;
            }
        }
        image::RgbaImage::from_raw(width as u32, height as u32, pixels)
            .ok_or_else(|| anyhow::anyhow!("Invalid CMYK JPEG pixel buffer"))
    })();
    match result {
        Ok(image) => Some(image),
        Err(error) => {
            log::debug!("CMYK artwork ICC conversion failed: {error}");
            None
        }
    }
}

/// Decode native raster assets with their display orientation, colors and frame timing.
pub(crate) fn decode_raster_image(
    bytes: &[u8],
    format: ImageFormat,
) -> image::ImageResult<SmallVec<[Frame; 1]>> {
    decode_raster_image_with_limits(bytes, format, image::Limits::default())
}

/// Decode oriented, color-managed BGRA frames with dimension and total frame storage limits.
pub fn decode_raster_image_with_limits(
    bytes: &[u8],
    format: ImageFormat,
    limits: image::Limits,
) -> image::ImageResult<SmallVec<[Frame; 1]>> {
    fn configure(
        decoder: &mut impl ImageDecoder,
        limits: &image::Limits,
    ) -> image::ImageResult<()> {
        let mut limits = limits.clone();
        let (width, height) = decoder.dimensions();
        limits.check_dimensions(width, height)?;
        limits.reserve(
            decoder
                .total_bytes()
                .max(u64::from(width) * u64::from(height) * 4),
        )?;
        decoder.set_limits(limits)
    }
    fn collect_frames(
        frames: image::Frames<'_>,
        mut limits: image::Limits,
    ) -> image::ImageResult<SmallVec<[Frame; 1]>> {
        let mut result = SmallVec::new();
        for frame in frames {
            let frame = frame?;
            limits.reserve(frame.buffer().len() as u64)?;
            result.push(frame);
        }
        Ok(result)
    }
    use std::io::Cursor;
    let mut animated_gray = false;
    let (mut frames, orientation, animated_icc) = match format {
        ImageFormat::Png => {
            let mut decoder = PngDecoder::new(Cursor::new(bytes))?;
            let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
            let icc = decoder.icc_profile().ok().flatten();
            configure(&mut decoder, &limits)?;
            if decoder.is_apng()? {
                animated_gray = matches!(
                    decoder.color_type(),
                    image::ColorType::L8
                        | image::ColorType::La8
                        | image::ColorType::L16
                        | image::ColorType::La16
                );
                (
                    collect_frames(decoder.apng()?.into_frames(), limits.clone())?,
                    orientation,
                    icc,
                )
            } else {
                (
                    SmallVec::from_elem(
                        Frame::new(color_managed_rgba8(
                            DynamicImage::from_decoder(decoder)?,
                            icc.as_deref(),
                        )),
                        1,
                    ),
                    orientation,
                    None,
                )
            }
        }
        ImageFormat::Gif => {
            let mut decoder = GifDecoder::new(Cursor::new(bytes))?;
            configure(&mut decoder, &limits)?;
            let icc = decoder.icc_profile().ok().flatten();
            (
                collect_frames(decoder.into_frames(), limits.clone())?,
                Orientation::NoTransforms,
                icc,
            )
        }
        ImageFormat::WebP => {
            let mut decoder = WebPDecoder::new(Cursor::new(bytes))?;
            configure(&mut decoder, &limits)?;
            let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
            let icc = decoder.icc_profile().ok().flatten();
            if decoder.has_animation() {
                let _ = decoder.set_background_color(image::Rgba([0, 0, 0, 0]));
                (
                    collect_frames(decoder.into_frames(), limits.clone())?,
                    orientation,
                    icc,
                )
            } else {
                (
                    SmallVec::from_elem(
                        Frame::new(color_managed_rgba8(
                            DynamicImage::from_decoder(decoder)?,
                            icc.as_deref(),
                        )),
                        1,
                    ),
                    orientation,
                    None,
                )
            }
        }
        _ => {
            let mut decoder =
                ImageReader::with_format(Cursor::new(bytes), format).into_decoder()?;
            let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
            let icc = decoder.icc_profile().ok().flatten();
            configure(&mut decoder, &limits)?;
            let cmyk = (format == ImageFormat::Jpeg)
                .then(|| {
                    icc.as_deref()
                        .and_then(|icc| color_managed_cmyk_jpeg(bytes, icc))
                })
                .flatten();
            let rgba = if let Some(cmyk) = cmyk {
                cmyk
            } else {
                color_managed_rgba8(DynamicImage::from_decoder(decoder)?, icc.as_deref())
            };
            (SmallVec::from_elem(Frame::new(rgba), 1), orientation, None)
        }
    };
    for frame in &mut frames {
        if let Some(icc) = animated_icc.as_deref() {
            *frame.buffer_mut() = color_managed_rgba8_with_gray(
                DynamicImage::ImageRgba8(std::mem::take(frame.buffer_mut())),
                Some(icc),
                animated_gray,
            );
        }
        if orientation != Orientation::NoTransforms {
            let mut image = DynamicImage::ImageRgba8(std::mem::take(frame.buffer_mut()));
            image.apply_orientation(orientation);
            *frame.buffer_mut() = image.into_rgba8();
        }
        for pixel in frame.buffer_mut().chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
    }
    Ok(frames)
}
