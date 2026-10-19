// Actual letter spacing, shaping and carets compared with the original Electron runtime.
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
        "../assets/font-spacing-tracking-electron-reference.json"
    ))?;
    let mut differences = 0;
    let mut audit = Vec::new();
    for (index, case) in reference["metrics"].as_array().unwrap().iter().enumerate() {
        let style = TextStyle {
            font_family: case["families"][0].as_str().unwrap().to_owned().into(),
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
            font_weight: FontWeight(case["weight"].as_f64().unwrap() as f32),
            letter_spacing: px(case["tracking"].as_f64().unwrap() as f32),
            text_spacing_trim: TextSpacingTrim::Normal,
            text_direction: if case["direction"] == "rtl" {
                gpui::TextDirection::Rtl
            } else {
                gpui::TextDirection::Ltr
            },
            ..Default::default()
        };
        let text = case["text"].as_str().unwrap();
        let size = px(case["size"].as_f64().unwrap() as f32);
        let layout = system.layout_line(text, size, &[style.to_run(text.len())], None);
        for cell in &layout.cells {
            if (cell.end_x - cell.start_x).abs() <= px(0.01) {
                continue;
            }
            for (fraction, index, edge) in [
                (0.1, cell.range.start, cell.start_x),
                (0.9, cell.range.end, cell.end_x),
            ] {
                let hit = cell.start_x + (cell.end_x - cell.start_x) * fraction;
                let (actual, affinity) = layout.caret_for_x(hit);
                ensure!(
                    actual == index && (layout.caret_x(actual, affinity) - edge).abs() < px(0.01),
                    "Wrong pointer edge for {text:?}: {cell:?}, {fraction}: {actual}, {affinity:?}, actual x {:?}, carets {:?}, cells {:?}",
                    layout.caret_x(actual, affinity),
                    layout.carets,
                    layout.cells
                );
            }
        }
        for right in [false, true] {
            let (mut index, mut affinity) = layout.caret_for_x(if right {
                px(-1.)
            } else {
                layout.width + px(1.)
            });
            let mut steps = 0;
            while let Some((next, visual)) = layout.visual_caret(index, affinity, right) {
                let delta = layout.caret_x(next, visual) - layout.caret_x(index, affinity);
                ensure!(
                    if right {
                        delta > px(0.01)
                    } else {
                        delta < px(-0.01)
                    },
                    "Visual navigation stalled for {text:?}: {index}, {affinity:?} -> {next}, {visual:?}"
                );
                (index, affinity) = (next, visual);
                steps += 1;
                ensure!(
                    steps <= layout.cells.len(),
                    "Visual navigation cycled for {text:?}"
                );
            }
            let edge = if right { layout.width } else { px(0.) };
            ensure!(
                (layout.caret_x(index, affinity) - edge).abs() < bound_for_edge(&layout),
                "Visual navigation missed paragraph edge for {text:?}"
            );
        }
        let platform = if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            "Linux"
        } else {
            "Darwin"
        };
        let expected = reference["platform_metrics"][platform]
            .get(index)
            .unwrap_or(case);
        let carets = text
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(text.len()))
            .map(|index| f32::from(layout.x_for_index(index)))
            .collect::<Vec<_>>();
        let bound = 1. / 64.
            + layout
                .runs
                .iter()
                .map(|run| {
                    run.glyphs.len() as f32 * f32::from(size)
                        / system.units_per_em(run.font_id) as f32
                        / 2.
                })
                .sum::<f32>();
        let error = carets
            .iter()
            .zip(expected["carets"].as_array().unwrap())
            .map(|(actual, expected)| (*actual - expected.as_f64().unwrap() as f32).abs())
            .fold(
                (f32::from(layout.width) - expected["width"].as_f64().unwrap() as f32).abs(),
                f32::max,
            );
        if error > bound {
            differences += 1;
            if differences <= 5 {
                eprintln!("Caret positions differ: {case}, native {carets:?}, error {error}");
            }
        }
        audit.push(serde_json::json!({"case":case,"expected":expected,"carets":carets,"error":error,
            "native_carets":layout.carets.iter().map(|caret| serde_json::json!({"index":caret.index,"x":f32::from(caret.x),"secondary":f32::from(caret.secondary_x)})).collect::<Vec<_>>(),
            "glyphs":layout.runs.iter().flat_map(|run| run.glyphs.iter()).map(|glyph|
                serde_json::json!({"index":glyph.index,"x":f32::from(glyph.position.x),"offset":f32::from(glyph.paint_offset.x)})
            ).collect::<Vec<_>>() }));
    }
    std::fs::write(
        "target/native-font-tracking-audit.json",
        serde_json::to_vec_pretty(&audit)?,
    )?;
    ensure!(
        differences == 0,
        "{differences} original letter spacing and caret layouts differed"
    );
    println!(
        "{} original letter spacing and caret layouts matched Electron",
        audit.len()
    );
    Ok(())
}

fn bound_for_edge(layout: &gpui::LineLayout) -> gpui::Pixels {
    // The last native cell can differ from rounded platform paragraph width.
    px(1. / 32.) + layout.font_size / 1000.
}
