// Production platform font shaping, no window, playback, profile, or account access.
use anyhow::{Result, ensure};
use gpui::{Application, FontFeatures, FontWeight, TextStyle, WindowTextSystem, px};
use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

fn main() -> Result<()> {
    let application = Application::headless();
    let text_system = application.text_system();
    let fonts = feishin_core::fonts::builtin_fonts()?
        .into_iter()
        .map(|font| Cow::Owned(font.bytes))
        .collect();
    text_system.add_fonts(fonts)?;
    let text_system = WindowTextSystem::new(text_system);
    let source = std::env::args_os()
        .nth(1)
        .map(std::fs::read_to_string)
        .transpose()?;
    let reference: serde_json::Value = serde_json::from_str(
        source
            .as_deref()
            .unwrap_or(include_str!("../assets/font-features-reference.json")),
    )?;
    let mut largest_error = 0_f32;
    let mut differences = 0;
    let mut strokes = BTreeMap::<u32, Vec<(u32, u64)>>::new();
    let mut scaled_glyphs = BTreeMap::new();
    for (index, case) in reference["metrics"].as_array().unwrap().iter().enumerate() {
        let mut features = feishin_core::fonts::numeric_features(case["numeric"].as_str().unwrap())
            .unwrap()
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        features.extend(
            feishin_core::fonts::feature_settings(case["features"].as_str().unwrap()).unwrap(),
        );
        let style = TextStyle {
            font_family: case["font"].as_str().unwrap().to_owned().into(),
            font_features: FontFeatures(Arc::new(features.into_iter().collect())),
            font_weight: FontWeight(case["weight"].as_f64().unwrap_or(400.) as f32),
            ..Default::default()
        };
        let text = case["text"].as_str().unwrap();
        let layout = text_system.layout_line(
            text,
            px(case["size"].as_f64().unwrap() as f32),
            &[style.to_run(text.len())],
            None,
        );
        let expected_width = if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            reference["platform_widths"]["Linux"][index]
                .as_f64()
                .unwrap_or_else(|| case["width"].as_f64().unwrap())
        } else {
            case["width"].as_f64().unwrap()
        };
        let error = (f32::from(layout.width) - expected_width as f32).abs();
        if case["weight"].is_number() && case["numeric"] == "normal" && text == "0" {
            let run = &layout.runs[0];
            let (_, pixels) = text_system.rasterized_glyph_for_test(
                run.font_id,
                run.glyphs[0].id,
                layout.font_size,
                1.,
            )?;
            let ink = pixels.iter().map(|byte| u64::from(*byte)).sum();
            strokes
                .entry(case["size"].as_u64().unwrap() as u32)
                .or_default()
                .push((case["weight"].as_u64().unwrap() as u32, ink));
            if case["weight"] == 400 && matches!(case["size"].as_u64(), Some(16 | 32)) {
                let size = case["size"].as_u64().unwrap() as u32;
                scaled_glyphs.insert(
                    size,
                    text_system.rasterized_glyph_for_test(
                        run.font_id,
                        run.glyphs[0].id,
                        layout.font_size,
                        if size == 16 { 2. } else { 1. },
                    )?,
                );
            }
        }
        largest_error = largest_error.max(error);
        let tolerance = if case["weight"].is_number()
            && cfg!(any(target_os = "linux", target_os = "freebsd"))
        {
            // HarfRust rounds variable advances to integer font units. Chromium retains fractions.
            // Bound each glyph to half a font unit, plus the browser's final 1/64px layout rounding.
            1. / 64.
                + layout
                    .runs
                    .iter()
                    .map(|run| {
                        run.glyphs.len() as f32 * f32::from(layout.font_size)
                            / text_system.units_per_em(run.font_id) as f32
                            / 2.
                    })
                    .sum::<f32>()
        } else {
            0.02
        };
        if error > tolerance {
            differences += 1;
            if differences <= 12 {
                eprintln!(
                    "Font width differs from Chromium: {case}, platform width {expected_width}, native {:?}, error {error}, permitted rounding {tolerance}",
                    layout.width
                );
            }
        }
    }
    for (size, strokes) in strokes {
        ensure!(
            strokes.len() == 9
                && strokes
                    .windows(2)
                    .all(|pair| pair[0].0 < pair[1].0 && pair[0].1 < pair[1].1),
            "Variable glyph pixels do not retain their requested weights at {size}px: {strokes:?}"
        );
    }
    ensure!(
        scaled_glyphs.len() == 2 && scaled_glyphs[&16] != scaled_glyphs[&32],
        "Different logical optical sizes reused the same glyph at equal physical size"
    );
    ensure!(
        differences == 0,
        "{differences} font measurements differ from Chromium; largest error {largest_error}px"
    );
    println!(
        "Verified {} original-font measurements against Chromium; maximum width difference {largest_error}px",
        reference["metrics"].as_array().unwrap().len()
    );
    Ok(())
}
