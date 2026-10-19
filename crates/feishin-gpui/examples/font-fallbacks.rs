// Actual platform shaping and glyph pixels, without a window or account access.
use anyhow::{Result, ensure};
use gpui::{Application, FontFallbacks, FontFeatures, FontWeight, TextStyle, WindowTextSystem, px};
use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

fn main() -> Result<()> {
    let application = Application::headless();
    let text_system = application.text_system();
    text_system.add_fonts(
        feishin_core::fonts::builtin_fonts()?
            .into_iter()
            .map(|font| Cow::Owned(font.bytes))
            .collect(),
    )?;
    let text_system = WindowTextSystem::new(text_system);
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../assets/font-fallbacks-reference.json"))?;
    let mut largest_error = 0_f32;
    let mut differences = 0;
    let mut strokes = BTreeMap::<(String, u32), BTreeMap<u32, u64>>::new();
    for (index, case) in reference["metrics"].as_array().unwrap().iter().enumerate() {
        let families = case["families"].as_array().unwrap();
        let style = TextStyle {
            font_family: families[0].as_str().unwrap().to_owned().into(),
            font_fallbacks: Some(FontFallbacks::from_fonts(
                families[1..]
                    .iter()
                    .map(|family| family.as_str().unwrap().to_owned())
                    .collect(),
            )),
            font_features: FontFeatures(Arc::new(vec![("tnum".into(), 1)])),
            font_weight: FontWeight(case["weight"].as_f64().unwrap() as f32),
            ..Default::default()
        };
        let text = case["text"].as_str().unwrap();
        let size = px(case["size"].as_f64().unwrap() as f32);
        let layout = text_system.layout_line(text, size, &[style.to_run(text.len())], None);
        let expected = if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            reference["platform_widths"]["Linux"][index]
                .as_f64()
                .unwrap_or_else(|| case["width"].as_f64().unwrap())
        } else {
            case["width"].as_f64().unwrap()
        };
        let error = (f32::from(layout.width) - expected as f32).abs();
        largest_error = largest_error.max(error);
        // The original variable fonts have fractional advances; fixed CSS fallback
        // faces quantize them to integer font units, as required by static OpenType.
        let tolerance = if families[0] == "Symbola" {
            0.02
        } else {
            1. / 64.
                + layout
                    .runs
                    .iter()
                    .map(|run| {
                        run.glyphs.len() as f32 * f32::from(size)
                            / text_system.units_per_em(run.font_id) as f32
                            / 2.
                    })
                    .sum::<f32>()
        };
        if error > tolerance {
            differences += 1;
            if differences <= 12 {
                eprintln!(
                    "Fallback differs from Chromium: {case}, platform width {expected}, native {:?}, error {error}",
                    layout.width
                );
            }
        }
        if matches!(text, "日" | "ש") {
            let family = if text == "日" {
                "Noto Sans JP"
            } else {
                "Noto Sans Hebrew"
            };
            let expected_style = TextStyle {
                font_family: family.into(),
                font_fallbacks: None,
                ..style.clone()
            };
            let direct =
                text_system.layout_line(text, size, &[expected_style.to_run(text.len())], None);
            ensure!(
                layout.runs.len() == 1
                    && layout.runs[0].glyphs.len() == 1
                    && direct.runs.len() == 1
                    && direct.runs[0].glyphs.len() == 1,
                "Fallback probe was not a single glyph"
            );
            let raster = |line: &gpui::LineLayout| {
                let run = &line.runs[0];
                text_system.rasterized_glyph_for_test(run.font_id, run.glyphs[0].id, size, 1.)
            };
            let actual_pixels = raster(&layout)?;
            ensure!(
                actual_pixels == raster(&direct)?,
                "Embedded fallback pixels or weight were lost: {case}"
            );
            let ink = actual_pixels.1.iter().map(|byte| u64::from(*byte)).sum();
            strokes
                .entry((family.into(), case["size"].as_u64().unwrap() as u32))
                .or_default()
                .insert(case["weight"].as_u64().unwrap() as u32, ink);
        }
    }
    for (key, strokes) in strokes {
        ensure!(
            strokes[&100] == strokes[&200]
                && strokes[&200] == strokes[&300]
                && strokes[&300] == strokes[&400]
                && strokes[&800] == strokes[&900],
            "CSS fallback weight endpoints differ: {key:?} {strokes:?}"
        );
        let original_weights = strokes
            .range(400..=800)
            .map(|(_, ink)| *ink)
            .collect::<Vec<_>>();
        ensure!(
            original_weights.windows(2).all(|pair| pair[0] < pair[1]),
            "Fallback weight outlines are not preserved: {key:?} {strokes:?}"
        );
    }
    ensure!(
        differences == 0,
        "{differences} fallback measurements differ from Chromium; largest error {largest_error}px"
    );
    println!(
        "Verified {} original CSS fallback measurements and actual embedded glyph pixels; maximum width difference {largest_error}px",
        reference["metrics"].as_array().unwrap().len()
    );
    Ok(())
}
