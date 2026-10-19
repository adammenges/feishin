// Real platform wrapping and caret cells compared with original Chromium fonts.
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
        "../assets/font-spacing-wraps-electron-reference.json"
    ))?;
    let mut differences = 0;
    for (index, case) in reference["metrics"].as_array().unwrap().iter().enumerate() {
        let style = TextStyle {
            font_family: "Inter".into(),
            font_fallbacks: Some(FontFallbacks::from_fonts(vec![
                "Noto Sans JP".into(),
                "Noto Sans Hebrew".into(),
            ])),
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
            font_weight: FontWeight(400.),
            letter_spacing: gpui::px(0.),
            text_spacing_trim: TextSpacingTrim::parse(case["spacing"].as_str().unwrap()).unwrap(),
            ..Default::default()
        };
        let text = case["text"].as_str().unwrap();
        let size = case["size"].as_f64().unwrap() as f32;
        let line_height = size * 1.5;
        let lines = system.shape_text(
            text.to_owned().into(),
            px(size),
            &[style.to_run(text.len())],
            Some(px(case["wrap"].as_f64().unwrap() as f32)),
            None,
        )?;
        let platform = if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            "Linux"
        } else {
            "Darwin"
        };
        let expected = reference["platform_metrics"][platform]
            .get(index)
            .unwrap_or(case);
        let expected = expected["positions"].as_array().unwrap();
        let first_y = expected[0][1].as_f64().unwrap() as f32;
        let mut actual = Vec::new();
        let mut y = 0_f32;
        for line in &lines {
            let starts = std::iter::once((0, px(0.)))
                .chain(line.wrap_boundaries().iter().map(|boundary| {
                    let glyph = &line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix];
                    (glyph.index, glyph.position.x)
                }))
                .collect::<Vec<_>>();
            for (byte, character) in line.text.char_indices() {
                let row = starts.partition_point(|(index, _)| *index <= byte) - 1;
                let x = line.unwrapped_layout.x_for_index(byte);
                let end = line
                    .unwrapped_layout
                    .x_for_index(byte + character.len_utf8());
                let caret = line.position_for_index(byte, px(line_height)).unwrap();
                ensure!(
                    (caret.x - (x - starts[row].1)).abs() < px(0.01)
                        && caret.y == px(row as f32 * line_height),
                    "Wrapped caret moved to the wrong line: {case}, byte {byte}"
                );
                actual.push([
                    f32::from(x - starts[row].1),
                    y + row as f32 * line_height,
                    f32::from(end - x),
                ]);
            }
            y += (starts.len() as f32) * line_height;
            if actual.len() < expected.len() && text.chars().nth(actual.len()) == Some('\n') {
                // A newline has no visible glyph; its range can have browser-specific width.
                let item = &expected[actual.len()];
                actual.push([
                    item[0].as_f64().unwrap() as f32,
                    item[1].as_f64().unwrap() as f32 - first_y,
                    item[2].as_f64().unwrap() as f32,
                ]);
            }
        }
        ensure!(
            actual.len() == expected.len(),
            "Character count changed: {case}"
        );
        let horizontal_error = actual
            .iter()
            .zip(expected)
            .map(|(actual, expected)| {
                (actual[0] - expected[0].as_f64().unwrap() as f32)
                    .abs()
                    .max((actual[2] - expected[2].as_f64().unwrap() as f32).abs())
            })
            .fold(0_f32, f32::max);
        let row_error = actual
            .iter()
            .zip(expected)
            .map(|(actual, expected)| {
                (actual[1] - expected[1].as_f64().unwrap() as f32 + first_y).abs()
            })
            .fold(0_f32, f32::max);
        // Static Noto instances round variable GPOS advances to whole font units.
        // Bound that horizontal error separately; line breaks retain a strict check.
        let horizontal_bound = 1. / 64.
            + lines
                .iter()
                .flat_map(|line| line.runs())
                .map(|run| {
                    run.glyphs.len() as f32 * size / system.units_per_em(run.font_id) as f32 / 2.
                })
                .sum::<f32>();
        if row_error > 0.04 || horizontal_error > horizontal_bound {
            differences += 1;
            if differences <= 10 {
                eprintln!(
                    "Wrapped punctuation differs: {case}, native {actual:?}, horizontal error {horizontal_error} (bound {horizontal_bound}), row error {row_error}"
                );
            }
        }
    }
    ensure!(
        differences == 0,
        "{differences} wrapped punctuation cases differed"
    );
    for text in [
        "one two three four five six seven eight",
        "日本語（音楽）日本語（音楽）",
    ] {
        let style = TextStyle {
            font_family: "Inter".into(),
            ..Default::default()
        };
        let runs = [style.to_run(text.len())];
        let single = system.shape_text(
            text.to_owned().into(),
            px(32.),
            &runs,
            Some(px(100.)),
            Some(1),
        )?;
        let three = system.shape_text(
            text.to_owned().into(),
            px(32.),
            &runs,
            Some(px(100.)),
            Some(3),
        )?;
        let single_again = system.shape_text(
            text.to_owned().into(),
            px(32.),
            &runs,
            Some(px(100.)),
            Some(1),
        )?;
        ensure!(
            single[0].wrap_boundaries().is_empty()
                && three[0].wrap_boundaries().len() == 2
                && single_again[0].wrap_boundaries().is_empty(),
            "Changing title line count reused a stale text layout"
        );
    }
    println!(
        "{} wrapped punctuation and forced-line layouts matched Chromium",
        reference["metrics"].as_array().unwrap().len()
    );
    Ok(())
}
