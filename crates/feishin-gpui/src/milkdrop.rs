use anyhow::{Context as _, Result, bail, ensure};
use feishin_core::visualizer::{self, Capture, number};
use glow::HasContext;
use gpui::RenderImage;
use projectm_sys as pm;
use serde_json::Value;
use std::{
    ffi::{CStr, CString, c_char, c_void},
    sync::Arc,
    time::Instant,
};
use surfman::{
    Connection, Context, ContextAttributeFlags, ContextAttributes, Device, GLVersion,
    SurfaceAccess, SurfaceType,
};

fn surface_error(error: surfman::Error) -> anyhow::Error {
    anyhow::anyhow!("OpenGL context: {error:?}")
}

struct Target {
    device: Device,
    context: Context,
}
impl Target {
    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        if let Some(mut surface) = self
            .device
            .unbind_surface_from_context(&mut self.context)
            .map_err(surface_error)?
        {
            self.device
                .destroy_surface(&mut self.context, &mut surface)
                .map_err(surface_error)?;
        }
        let surface = self
            .device
            .create_surface(
                &self.context,
                SurfaceAccess::GPUOnly,
                SurfaceType::Generic {
                    size: euclid::default::Size2D::new(width, height),
                },
            )
            .map_err(surface_error)?;
        if let Err((error, mut surface)) = self
            .device
            .bind_surface_to_context(&mut self.context, surface)
        {
            let _ = self.device.destroy_surface(&mut self.context, &mut surface);
            return Err(surface_error(error));
        }
        Ok(())
    }
}
impl Drop for Target {
    fn drop(&mut self) {
        if let Err(error) = self.device.destroy_context(&mut self.context) {
            log::warn!("MilkDrop context cleanup failed: {error:?}");
        }
    }
}

pub struct Renderer {
    target: Target,
    gl: glow::Context,
    instance: pm::projectm_handle,
    error: Box<Option<String>>,
    preset: String,
    size: (i32, i32),
    frame_at: Instant,
    image: Option<Arc<RenderImage>>,
}
unsafe extern "C" fn preset_failed(
    _name: *const c_char,
    message: *const c_char,
    data: *mut c_void,
) {
    if data.is_null() {
        return;
    }
    // projectM calls synchronously; the boxed error outlives the registered callback.
    let error = unsafe { &mut *data.cast::<Option<String>>() };
    *error = Some(if message.is_null() {
        "Preset failed".into()
    } else {
        unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned()
    });
}
impl Renderer {
    pub fn take_image(&mut self) -> Option<Arc<RenderImage>> {
        self.image.take()
    }
    pub fn new(data_dir: &std::path::Path) -> Result<Self> {
        let textures = visualizer::install_milkdrop_textures(data_dir)?;
        let connection = Connection::new().map_err(surface_error)?;
        let adapter = connection.create_adapter().map_err(surface_error)?;
        let device = connection.create_device(&adapter).map_err(surface_error)?;
        let descriptor = device
            .create_context_descriptor(&ContextAttributes {
                version: GLVersion::new(
                    if cfg!(target_os = "macos") { 4 } else { 3 },
                    if cfg!(target_os = "macos") { 1 } else { 3 },
                ),
                flags: ContextAttributeFlags::ALPHA,
            })
            .map_err(surface_error)?;
        let context = device
            .create_context(&descriptor, None)
            .map_err(surface_error)?;
        let mut target = Target { device, context };
        target.resize(640, 480)?;
        target
            .device
            .make_context_current(&target.context)
            .map_err(surface_error)?;
        // All OpenGL calls remain on the GPUI window thread with this context current.
        let gl = unsafe {
            glow::Context::from_loader_function(|name| {
                target.device.get_proc_address(&target.context, name)
            })
        };
        let instance = unsafe { pm::projectm_create() };
        if instance.is_null() {
            bail!("Could not initialize projectM with OpenGL 3.3 or later")
        }
        let mut error = Box::new(None);
        let texture_path = CString::new(textures.to_string_lossy().as_bytes())?;
        unsafe {
            pm::projectm_set_texture_search_paths(
                instance,
                [texture_path.as_ptr()].as_mut_ptr(),
                1,
            );
            pm::projectm_set_preset_switch_failed_event_callback(
                instance,
                Some(preset_failed),
                (&mut *error as *mut Option<String>).cast(),
            );
            pm::projectm_set_preset_locked(instance, true);
            pm::projectm_set_hard_cut_enabled(instance, false);
            pm::projectm_set_window_size(instance, 640, 480);
            // Isolate errors left by projectM's capability probes from Surfman's checks.
            while gl.get_error() != glow::NO_ERROR {}
        }
        let renderer = Self {
            target,
            gl,
            instance,
            error,
            preset: String::new(),
            size: (640, 480),
            frame_at: Instant::now(),
            image: None,
        };
        renderer
            .target
            .device
            .make_no_context_current()
            .map_err(surface_error)?;
        Ok(renderer)
    }
    pub fn frame(
        &mut self,
        width: f32,
        height: f32,
        options: &Value,
        capture: &Capture,
        playing: bool,
    ) -> Result<(Arc<RenderImage>, Option<Arc<RenderImage>>)> {
        let fps = number(options, "maxFPS", 0.);
        if fps > 0.
            && self.frame_at.elapsed().as_secs_f32() < 1. / fps
            && let Some(image) = &self.image
        {
            return Ok((image.clone(), None));
        }
        self.target
            .device
            .make_context_current(&self.target.context)
            .map_err(surface_error)?;
        let result = self.render(width, height, options, capture, playing);
        let release = self.target.device.make_no_context_current();
        let image = result?;
        release.map_err(surface_error)?;
        let old = self.image.replace(image.clone());
        self.frame_at = Instant::now();
        Ok((image, old))
    }
    fn render(
        &mut self,
        width: f32,
        height: f32,
        options: &Value,
        capture: &Capture,
        playing: bool,
    ) -> Result<Arc<RenderImage>> {
        let (width, height) = render_size(width, height);
        if self.size != (width, height) {
            self.target.resize(width, height)?;
            self.target
                .device
                .make_context_current(&self.target.context)
                .map_err(surface_error)?;
            unsafe { pm::projectm_set_window_size(self.instance, width as usize, height as usize) };
            self.size = (width, height);
        }
        let name = options["currentPreset"]
            .as_str()
            .filter(|name| visualizer::milkdrop_data(options, name).is_some())
            .unwrap_or(&visualizer::milkdrop_presets().names[0]);
        let fps = number(options, "maxFPS", 0.).max(1.) as i32;
        unsafe {
            pm::projectm_set_fps(self.instance, if fps == 1 { 60 } else { fps });
            pm::projectm_set_soft_cut_duration(
                self.instance,
                number(options, "blendTime", 2.5) as f64,
            );
        }
        if self.preset != name {
            let data = CString::new(
                visualizer::milkdrop_data(options, name).context("Preset is missing")?,
            )?;
            *self.error = None;
            unsafe {
                pm::projectm_load_preset_data(
                    self.instance,
                    data.as_ptr(),
                    !self.preset.is_empty() && number(options, "blendTime", 2.5) > 0.,
                )
            };
            if let Some(error) = self.error.take() {
                log::warn!("MilkDrop preset {name} failed: {error}");
                let summary = error.split("\nSource:\n").next().unwrap_or(&error);
                bail!("{name}: {summary}")
            }
            let error = unsafe { self.gl.get_error() };
            ensure!(
                error == glow::NO_ERROR,
                "Preset initialization OpenGL error 0x{error:04x}"
            );
            self.preset = name.into();
        }
        let frames = capture.take_frames();
        let max = unsafe { pm::projectm_pcm_get_max_samples() }.max(1) as usize;
        if playing && !frames.is_empty() {
            for frames in frames.chunks(max) {
                let interleaved: Vec<f32> = frames
                    .iter()
                    .flat_map(|frame| frame.iter().copied())
                    .collect();
                unsafe {
                    pm::projectm_pcm_add_float(
                        self.instance,
                        interleaved.as_ptr(),
                        frames.len() as u32,
                        pm::projectm_channels_PROJECTM_STEREO,
                    )
                };
            }
        } else {
            let silence = vec![0.; max * 2];
            unsafe {
                pm::projectm_pcm_add_float(
                    self.instance,
                    silence.as_ptr(),
                    max as u32,
                    pm::projectm_channels_PROJECTM_STEREO,
                )
            };
        }
        let fbo = self
            .target
            .device
            .context_surface_info(&self.target.context)
            .map_err(surface_error)?
            .context("Missing offscreen surface")?
            .framebuffer_object;
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, fbo);
            self.gl.viewport(0, 0, width, height);
            pm::projectm_opengl_render_frame_fbo(self.instance, fbo.map_or(0, |fbo| fbo.0.get()));
            let error = self.gl.get_error();
            ensure!(
                error == glow::NO_ERROR,
                "projectM frame OpenGL error 0x{error:04x}"
            );
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, fbo);
            self.gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
            self.gl.read_pixels(
                0,
                0,
                width,
                height,
                glow::BGRA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut pixels)),
            );
            let error = self.gl.get_error();
            ensure!(
                error == glow::NO_ERROR,
                "Frame transfer OpenGL error 0x{error:04x}"
            );
        }
        flip_rows(&mut pixels, width as usize, height as usize);
        let image = image::RgbaImage::from_raw(width as u32, height as u32, pixels)
            .context("Invalid visualizer frame")?;
        // GPUI RenderImage uses BGRA bytes, including when carried by an RgbaImage.
        Ok(Arc::new(RenderImage::new(vec![image::Frame::new(image)])))
    }
}
impl Drop for Renderer {
    fn drop(&mut self) {
        if self
            .target
            .device
            .make_context_current(&self.target.context)
            .is_ok()
        {
            unsafe { pm::projectm_destroy(self.instance) };
            let _ = self.target.device.make_no_context_current();
        } else {
            log::warn!("Could not activate the MilkDrop context for cleanup");
        }
    }
}
fn render_size(width: f32, height: f32) -> (i32, i32) {
    let width = if width.is_finite() {
        width.max(64.)
    } else {
        640.
    };
    let height = if height.is_finite() {
        height.max(64.)
    } else {
        480.
    };
    // ponytail: CPU transfer capped at 1080p; use native texture interop if profiling requires more.
    let scale = (1920. / width).min(1080. / height).min(1.);
    (
        (width * scale).round().max(1.) as i32,
        (height * scale).round().max(1.) as i32,
    )
}
fn flip_rows(pixels: &mut [u8], width: usize, height: usize) {
    let row = width * 4;
    for y in 0..height / 2 {
        let (first, rest) = pixels.split_at_mut((height - 1 - y) * row);
        first[y * row..(y + 1) * row].swap_with_slice(&mut rest[..row]);
    }
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Requires an offscreen OpenGL 3.3 context; run explicitly on a GPU or Mesa host"]
    fn projectm_offscreen_gpu_smoke() {
        let dir = std::env::temp_dir().join(format!("feishin-offscreen-{}", uuid::Uuid::new_v4()));
        let mut renderer = Renderer::new(&dir).unwrap();
        let cloud = image::open(dir.join("milkdrop-textures-v1/clouds2.jpg")).unwrap();
        assert_eq!((cloud.width(), cloud.height()), (128, 128));
        let mut options = visualizer::Config::default().butterchurn;
        options["blendTime"] = serde_json::json!(0);
        let capture = Capture::default();
        capture.enable(true);
        let mut failures = vec![];
        let mut dark = vec![];
        for name in &visualizer::milkdrop_presets().names {
            if std::env::var_os("FEISHIN_GPU_AUDIT").is_some() {
                eprintln!("GPU preset: {name}");
            }
            options["currentPreset"] = serde_json::json!(name);
            let mut rendered = false;
            for frame in 0..8 {
                let samples: Vec<f32> = (0..1024)
                    .map(|i| (i as f32 * 0.05 + frame as f32).sin() * 0.25)
                    .collect();
                capture.push(&samples, 2);
                let (image, _) = match renderer.frame(96., 64., &options, &capture, true) {
                    Ok(frame) => frame,
                    Err(error) => {
                        failures.push(format!("{name}: {error}"));
                        break;
                    }
                };
                assert_eq!(image.as_bytes(0).unwrap().len(), 96 * 64 * 4);
                rendered |= image
                    .as_bytes(0)
                    .unwrap()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| pixel[..3].iter().any(|byte| *byte > 8));
            }
            if !rendered {
                dark.push(name.clone());
            }
        }
        assert!(
            failures.is_empty(),
            "Preset shader failures:\n{}",
            failures.join("\n")
        );
        assert!(
            dark.is_empty(),
            "Every preset must produce visible pixels: {dark:?}"
        );
        options["blendTime"] = serde_json::json!(2.5);
        options["currentPreset"] = serde_json::json!(&visualizer::milkdrop_presets().names[0]);
        renderer.frame(160., 90., &options, &capture, true).unwrap();
        options["currentPreset"] = serde_json::json!("Invalid imported shader");
        options["customPresets"] = serde_json::json!({
            "Invalid imported shader": "MILKDROP_PRESET_VERSION=200\n[preset00]\nPSVERSION=3\nwarp_1=`shader_body { ret = missing_function(uv); }\n"
        });
        let error = renderer
            .frame(160., 90., &options, &capture, true)
            .err()
            .unwrap();
        assert!(error.to_string().contains("Invalid imported shader"));
        assert!(!error.to_string().contains("Source:"));
        options["currentPreset"] = serde_json::json!(&visualizer::milkdrop_presets().names[0]);
        renderer.frame(160., 90., &options, &capture, true).unwrap();
        drop(renderer);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn offscreen_frame_dimensions_and_bgra_orientation() {
        assert_eq!(render_size(3840., 2160.), (1920, 1080));
        assert_eq!(render_size(f32::NAN, 0.), (640, 64));
        let mut pixels = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        flip_rows(&mut pixels, 2, 2);
        assert_eq!(
            pixels,
            vec![9, 10, 11, 255, 13, 14, 15, 255, 1, 2, 3, 255, 5, 6, 7, 255]
        );
    }
}
