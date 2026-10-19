// Independent Electron rows, cell positions and pointer affinity for mixed-direction text.
use anyhow::{Result, ensure};
use gpui::{
    Application, FontFallbacks, FontFeatures, TextDirection, TextSpacingTrim, TextStyle, TextWrap,
    WindowTextSystem, px,
};
use std::{borrow::Cow, sync::Arc};

fn main() -> Result<()> {
    let app = Application::headless();
    let system = app.text_system();
    system.add_fonts(
        feishin_core::fonts::builtin_fonts()?
            .into_iter()
            .map(|font| Cow::Owned(font.bytes))
            .collect(),
    )?;
    let system = WindowTextSystem::new(system);
    let word_wraps = std::env::args().any(|arg| arg == "--word-wraps");
    let keep_all = std::env::args().any(|arg| arg == "--keep-all");
    let tracking = std::env::args().any(|arg| arg == "--tracking");
    let reference: serde_json::Value = serde_json::from_str(if tracking {
        include_str!("../assets/font-spacing-tracking-wraps-electron-reference.json")
    } else if keep_all {
        include_str!("../assets/font-spacing-keep-all-electron-reference.json")
    } else if word_wraps {
        include_str!("../assets/font-spacing-word-wraps-electron-reference.json")
    } else {
        include_str!("../assets/font-spacing-bidi-wraps-electron-reference.json")
    })?;
    let mut differences = 0;
    let mut audit = Vec::new();
    for (index, case) in reference["metrics"].as_array().unwrap().iter().enumerate() {
        let style = TextStyle {
            font_family: case["families"][0].as_str().unwrap().to_owned().into(),
            font_fallbacks: Some(FontFallbacks::from_fonts(vec![
                "Noto Sans JP".into(),
                "Noto Sans Hebrew".into(),
            ])),
            font_features: FontFeatures(Arc::new(vec![("tnum".into(), 1)])),
            font_weight: gpui::FontWeight(case["weight"].as_f64().unwrap_or(400.) as f32),
            letter_spacing: px(case["tracking"].as_f64().unwrap_or(0.) as f32),
            text_spacing_trim: TextSpacingTrim::Normal,
            text_direction: if case["direction"] == "rtl" {
                TextDirection::Rtl
            } else {
                TextDirection::Ltr
            },
            ..Default::default()
        };
        let platform = if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            "Linux"
        } else {
            "Darwin"
        };
        let expected = reference["platform_metrics"][platform]
            .get(index)
            .unwrap_or(case);
        let text = case["text"].as_str().unwrap();
        let size = px(case["size"].as_f64().unwrap() as f32);
        let line_height = size * 1.5;
        let wrap = match case["wrap_mode"].as_str() {
            Some("break-word") => TextWrap::BreakWord,
            Some("break-all") => TextWrap::BreakAll,
            Some("keep-all") => TextWrap::KeepAll,
            Some("keep-all-break-word" | "keep-all-anywhere") => TextWrap::KeepAllBreakWord,
            _ => TextWrap::Normal,
        };
        let lines = system.shape_text_with_wrap(
            text.to_owned().into(),
            size,
            &[style.to_run(text.len())],
            Some(px(case["wrap"].as_f64().unwrap() as f32)),
            None,
            wrap,
        )?;
        let mut actual = Vec::new();
        let mut y = 0.;
        for line in &lines {
            let extracted = line.shaped_rows();
            ensure!(
                extracted
                    .iter()
                    .map(|row| row.text.as_ref())
                    .collect::<String>()
                    == line.text.as_ref(),
                "Extracted rows must preserve source bytes"
            );
            let rows = line.unwrapped_layout.fragments(line.wrap_boundaries());
            for (row, fragment) in extracted.iter().zip(&rows) {
                ensure!(
                    row.width == fragment.width && row.rtl == line.unwrapped_layout.rtl,
                    "Extracted rows must preserve paragraph geometry"
                );
                for cell in &row.cells {
                    let original = line
                        .unwrapped_layout
                        .cells
                        .iter()
                        .find(|source| {
                            source.range.start == fragment.range.start + cell.range.start
                        })
                        .unwrap();
                    ensure!(
                        cell.start_x + fragment.x == original.start_x
                            && cell.end_x + fragment.x == original.end_x,
                        "Extracted bidi cells must preserve visual edges"
                    );
                }
            }
            for (byte, _) in line.text.char_indices() {
                let cell = line
                    .unwrapped_layout
                    .cells
                    .iter()
                    .find(|cell| cell.range.contains(&byte))
                    .unwrap();
                let position = line.position_for_index(byte, line_height).unwrap();
                let origin = line.unwrapped_layout.x_for_index(byte) - position.x;
                let row = &rows[(position.y / line_height) as usize];
                let aligned = row.alignment_offset(
                    gpui::TextAlign::Start,
                    px(case["wrap"].as_f64().unwrap() as f32),
                    line.unwrapped_layout.rtl,
                );
                actual.push([
                    f32::from(cell.start_x.min(cell.end_x) - origin + aligned),
                    y + f32::from(position.y),
                    f32::from((cell.end_x - cell.start_x).abs()),
                ]);
            }
            y += f32::from(line.size(line_height).height);
        }
        let bound = 1. / 64.
            + lines
                .iter()
                .flat_map(|line| line.runs())
                .map(|run| {
                    run.glyphs.len() as f32 * f32::from(size)
                        / system.units_per_em(run.font_id) as f32
                        / 2.
                })
                .sum::<f32>();
        let mut cursor = 0;
        let baseline = expected["positions"][0][1].as_f64().unwrap() as f32;
        let mut failed = false;
        for (character, position) in text.chars().zip(expected["positions"].as_array().unwrap()) {
            if character == '\n' {
                continue;
            }
            let value = actual[cursor];
            cursor += 1;
            let x = position[0].as_f64().unwrap() as f32;
            let y = position[1].as_f64().unwrap() as f32 - baseline;
            if (value[0] - x).abs() > bound
                || (value[1] - y).abs() > 0.04
                || (value[2] - position[2].as_f64().unwrap() as f32).abs() > bound
            {
                failed = true;
            }
        }
        if failed {
            differences += 1;
            if differences <= 5 {
                eprintln!("Wrapped bidi geometry differs: {case}, native {actual:?}");
            }
        }
        audit.push(serde_json::json!({"case":case,"expected":expected,"actual":actual}));
    }
    std::fs::write(
        "target/native-text-wraps-audit.json",
        serde_json::to_vec_pretty(&audit)?,
    )?;
    ensure!(
        differences == 0,
        "{differences} original wrapped bidi layouts differed"
    );
    println!(
        "{} original wrapped bidi layouts matched Electron",
        audit.len()
    );
    Ok(())
}
