// Actual platform punctuation shaping and caret positions without a window.
use anyhow::{Result, ensure};
use gpui::{
    Application, FontFallbacks, FontFeatures, FontWeight, TextSpacingTrim, TextStyle,
    WindowTextSystem, px,
};
use std::{borrow::Cow, sync::Arc};

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
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../assets/font-spacing-electron-reference.json"
    ))?;
    let mut differences = 0;
    let mut largest_error = 0_f32;
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
            font_features: FontFeatures(Arc::new(
                std::iter::once(("tnum".into(), 1))
                    .chain(
                        feishin_core::fonts::feature_settings(
                            case["features"].as_str().unwrap_or("normal"),
                        )
                        .unwrap(),
                    )
                    .collect(),
            )),
            font_weight: FontWeight(case["weight"].as_f64().unwrap() as f32),
            letter_spacing: gpui::px(0.),
            text_spacing_trim: TextSpacingTrim::parse(case["spacing"].as_str().unwrap()).unwrap(),
            ..Default::default()
        };
        let text = case["text"].as_str().unwrap();
        let size = px(case["size"].as_f64().unwrap() as f32);
        let layout = system.layout_line(text, size, &[style.to_run(text.len())], None);
        let platform = if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            "Linux"
        } else {
            "Darwin"
        };
        let expected = reference["platform_metrics"][platform]
            .get(index)
            .unwrap_or(case);
        // Derived static Noto faces round fractional advances to integer font units.
        let tolerance = 1. / 64.
            + layout
                .runs
                .iter()
                .map(|run| {
                    run.glyphs.len() as f32 * f32::from(size)
                        / system.units_per_em(run.font_id) as f32
                        / 2.
                })
                .sum::<f32>();
        let mut error =
            (f32::from(layout.width) - expected["width"].as_f64().unwrap() as f32).abs();
        for ((byte, character), position) in text
            .char_indices()
            .zip(expected["positions"].as_array().unwrap())
        {
            let x = f32::from(layout.x_for_index(byte));
            let end = f32::from(layout.x_for_index(byte + character.len_utf8()));
            error = error.max((x - position[0].as_f64().unwrap() as f32).abs());
            error = error.max((end - x - position[1].as_f64().unwrap() as f32).abs());
            if end > x {
                ensure!(
                    layout.index_for_x(px((x + end) * 0.5)) == Some(byte),
                    "Punctuation hit cell changed: {case}, byte {byte}"
                );
                ensure!(
                    layout.closest_index_for_x(px(x + (end - x) * 0.25)) == byte
                        && layout.closest_index_for_x(px(x + (end - x) * 0.75))
                            == byte + character.len_utf8(),
                    "Punctuation caret affinity changed: {case}, byte {byte}"
                );
            }
        }
        largest_error = largest_error.max(error);
        if error > tolerance {
            differences += 1;
            if differences <= 12 {
                eprintln!(
                    "Punctuation differs from Chromium: {case}, native width {:?}, positions {:?}, error {error}",
                    layout.width,
                    text.char_indices()
                        .map(|(byte, _)| f32::from(layout.x_for_index(byte)))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
    ensure!(
        differences == 0,
        "{differences} punctuation cases differed; maximum error {largest_error}px"
    );
    let features: serde_json::Value =
        serde_json::from_str(include_str!("../assets/font-features-reference.json"))?;
    let expected = ["Inter", "Poppins"]
        .into_iter()
        .map(|family| {
            features["metrics"]
                .as_array()
                .unwrap()
                .iter()
                .find(|case| {
                    case["font"] == family
                        && case["size"] == 32
                        && case["numeric"] == "normal"
                        && case["features"] == "normal"
                        && case["text"] == "0"
                        && case["weight"].as_u64().unwrap_or(400) == 400
                })
                .unwrap()["width"]
                .as_f64()
                .unwrap() as f32
        })
        .sum::<f32>();
    let runs = ["Inter", "Poppins"].map(|family| {
        TextStyle {
            font_family: family.into(),
            ..Default::default()
        }
        .to_run(1)
    });
    let adjacent = system.layout_line("00", px(32.), &runs, None);
    ensure!(
        (f32::from(adjacent.width) - expected).abs() < 0.04
            && adjacent.font_id_for_index(0) != adjacent.font_id_for_index(1),
        "Adjacent spans with the same color lost their distinct fonts"
    );
    for size in [14., 20., 32.] {
        let style = TextStyle {
            font_family: "Noto Sans JP".into(),
            letter_spacing: gpui::px(0.),
            text_spacing_trim: TextSpacingTrim::Normal,
            ..Default::default()
        };
        let full = system.layout_line("（日）", px(size), &[style.to_run(9)], None);
        let trimmed_style = TextStyle {
            letter_spacing: gpui::px(0.),
            text_spacing_trim: TextSpacingTrim::TrimStart,
            ..style
        };
        let trimmed = system.layout_line("（日）", px(size), &[trimmed_style.to_run(9)], None);
        let original = &full.runs[0].glyphs[0];
        let alternate = &trimmed.runs[0].glyphs[0];
        let ink =
            system.rasterized_glyph_for_test(full.runs[0].font_id, original.id, px(size), 1.)?;
        let shifted = system.rasterized_glyph_for_test(
            trimmed.runs[0].font_id,
            alternate.id,
            px(size),
            1.,
        )?;
        ensure!(
            ink == shifted
                && (alternate.position.x + alternate.paint_offset.x
                    - original.position.x
                    - original.paint_offset.x
                    + px(size / 2.))
                .abs()
                    < px(0.02),
            "Opening punctuation lost its actual glyph ink or drawing displacement"
        );
    }
    println!(
        "{} punctuation layouts and caret cells matched Chromium; maximum error {largest_error}px",
        reference["metrics"].as_array().unwrap().len()
    );
    Ok(())
}
