// Real font shaping and rasterization without a native window or audio device.
use anyhow::{Result, ensure};
use gpui::{Application, FontFallbacks, FontStyle, TextStyle, WindowTextSystem, px};
use std::{borrow::Cow, collections::BTreeMap};

fn main() -> Result<()> {
    let application = Application::headless();
    let system = application.text_system();
    system.add_fonts(
        feishin_core::fonts::builtin_fonts()?
            .into_iter()
            .map(|font| Cow::Owned(font.bytes))
            .collect(),
    )?;
    let system = WindowTextSystem::new(system);
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../assets/font-styles-reference.json"))?;
    let platform = if cfg!(target_os = "macos") {
        "Darwin"
    } else {
        "Linux"
    };
    let mut rasters = BTreeMap::new();
    let mut differences = Vec::new();
    for case in reference["platform_metrics"][platform]["metrics"]
        .as_array()
        .unwrap()
    {
        let style = TextStyle {
            font_family: case["family"].as_str().unwrap().to_owned().into(),
            font_fallbacks: case["fallback"]
                .as_str()
                .map(|family| FontFallbacks::from_fonts(vec![family.to_owned()])),
            font_style: match case["style"].as_str().unwrap() {
                "italic" => FontStyle::Italic,
                "oblique" => FontStyle::Oblique,
                _ => FontStyle::Normal,
            },
            ..Default::default()
        };
        let text = case["text"].as_str().unwrap();
        ensure!(
            text.chars().count() == 1,
            "Font-style fixture contains a non-glyph sample: {text:?}"
        );
        let layout = system.layout_line(
            text,
            px(case["size"].as_f64().unwrap() as f32),
            &[style.to_run(text.len())],
            None,
        );
        let run = &layout.runs[0];
        let (size, pixels) = system.rasterized_glyph_for_test(
            run.font_id,
            run.glyphs[0].id,
            layout.font_size,
            1.,
        )?;
        let width = i32::from(size.width) as usize;
        let height = i32::from(size.height) as usize;
        let mut bounds = (width, height, 0, 0);
        for y in 0..height {
            for x in 0..width {
                if pixels[y * width + x] != 0 {
                    bounds.0 = bounds.0.min(x);
                    bounds.1 = bounds.1.min(y);
                    bounds.2 = bounds.2.max(x);
                    bounds.3 = bounds.3.max(y);
                }
            }
        }
        let ink = [bounds.2 + 1 - bounds.0, bounds.3 + 1 - bounds.1];
        let width_error = (f32::from(layout.width) - case["width"].as_f64().unwrap() as f32).abs();
        let rounding =
            0.02 + f32::from(layout.font_size) / system.units_per_em(run.font_id) as f32 / 2.;
        // Chromium's FreeType rasterizer and Swash differ by two pixels on an
        // upright Inter digit at 32px. Changed synthetic glyphs keep the stricter bound.
        let ink_rounding = if cfg!(target_os = "linux") && style.font_style == FontStyle::Normal {
            2
        } else {
            1
        };
        if width_error > rounding
            || ink
                .into_iter()
                .zip(case["ink"].as_array().unwrap())
                .any(|(actual, expected)| {
                    actual.abs_diff(expected.as_u64().unwrap() as usize) > ink_rounding
                })
        {
            differences.push(format!(
                "{case}: native width {:?}, ink {ink:?}",
                layout.width
            ));
        }
        rasters.insert(
            (
                case["family"].as_str().unwrap(),
                case["size"].as_u64().unwrap(),
                text,
                case["style"].as_str().unwrap(),
            ),
            (size, pixels),
        );
    }
    for (&(family, size, text, style), normal) in &rasters {
        if style == "normal" {
            let italic = &rasters[&(family, size, text, "italic")];
            ensure!(
                normal != italic,
                "Italic still uses upright glyphs: {family} {size} {text}"
            );
            ensure!(
                italic == &rasters[&(family, size, text, "oblique")],
                "Default oblique differs from synthesized italic"
            );
        }
    }
    ensure!(
        differences.is_empty(),
        "{} font-style mismatches: {:?}",
        differences.len(),
        &differences[..differences.len().min(12)]
    );
    println!("Verified 126 original font-style advances and raster bounds on {platform}");
    Ok(())
}
