// Actual shaping and glyph pixels for faces that only contain a regular weight.
use anyhow::{Result, ensure};
use gpui::{Application, FontStyle, FontWeight, TextStyle, WindowTextSystem, px};
use std::{borrow::Cow, collections::BTreeMap};

fn main() -> Result<()> {
    let application = Application::headless();
    let system = application.text_system();
    system.add_fonts(vec![
        Cow::Borrowed(include_bytes!("../assets/font-reload-1.ttf")),
        Cow::Owned(
            feishin_core::fonts::decode(include_bytes!(
                "../../feishin-core/assets/fonts/noto-jp-400.woff2"
            ))?
            .bytes,
        ),
        Cow::Owned(
            feishin_core::fonts::decode(include_bytes!(
                "../../feishin-core/assets/fonts/noto-hebrew-400.woff2"
            ))?
            .bytes,
        ),
        Cow::Owned(
            feishin_core::fonts::decode(include_bytes!(
                "../../../assets/fonts/Poppins-Regular.woff2"
            ))?
            .bytes,
        ),
        Cow::Owned(
            feishin_core::fonts::decode(include_bytes!(
                "../../../assets/fonts/Poppins-Bold.woff2"
            ))?
            .bytes,
        ),
        Cow::Owned(
            feishin_core::fonts::decode(include_bytes!(
                "../../../assets/fonts/Inter-VariableFont_opsz,wght.woff2"
            ))?
            .bytes,
        ),
    ])?;
    let system = WindowTextSystem::new(system);
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../assets/font-synthesis-reference.json"))?;
    let platform = if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    let cases = reference["platform_metrics"][platform]["metrics"]
        .as_array()
        .unwrap();
    let mut rasters = BTreeMap::new();
    let mut failures = vec![];
    for case in cases {
        let family = case["family"].as_str().unwrap();
        let text = case["text"].as_str().unwrap();
        let style = case["style"].as_str().unwrap();
        let weight = case["weight"].as_u64().unwrap();
        let size = case["size"].as_u64().unwrap();
        let run = TextStyle {
            font_family: family.to_owned().into(),
            font_weight: FontWeight(weight as f32),
            font_style: if style == "italic" {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            },
            ..Default::default()
        }
        .to_run(text.len());
        let layout = system.layout_line(text, px(size as f32), &[run], None);
        let run = &layout.runs[0];
        let (dimensions, pixels) = system.rasterized_glyph_for_test(
            run.font_id,
            run.glyphs[0].id,
            layout.font_size,
            1.,
        )?;
        let width = i32::from(dimensions.width) as usize;
        let height = i32::from(dimensions.height) as usize;
        let mut ink = (width, height, 0usize, 0usize);
        for y in 0..height {
            for x in 0..width {
                if pixels[y * width + x] != 0 {
                    ink.0 = ink.0.min(x);
                    ink.1 = ink.1.min(y);
                    ink.2 = ink.2.max(x);
                    ink.3 = ink.3.max(y);
                }
            }
        }
        let ink = [ink.2 + 1 - ink.0, ink.3 + 1 - ink.1];
        let expected = case["ink"].as_array().unwrap();
        let rounding = if cfg!(target_os = "linux") && style == "normal" {
            2
        } else {
            1
        };
        let advance_rounding = 1. / 64.
            + if cfg!(target_os = "linux") && family == "Inter" {
                size as f64 / f64::from(system.units_per_em(run.font_id)) / 2.
            } else {
                0.
            };
        if (f64::from(f32::from(layout.width)) - case["width"].as_f64().unwrap()).abs()
            > advance_rounding
            || ink
                .into_iter()
                .zip(expected)
                .any(|(a, b)| a.abs_diff(b.as_u64().unwrap() as usize) > rounding)
        {
            failures.push(format!("{family} {text} {weight} {style} {size}: width={:?}, ink={ink:?}, expected={expected:?}", layout.width));
        }
        rasters.insert(
            (family, text, style, size, weight),
            (dimensions, pixels, case["mass"].as_f64().unwrap()),
        );
    }
    for (&(family, text, style, size, weight), actual) in &rasters {
        let baseline = &rasters[&(family, text, style, size, 400)];
        let original_changed = (actual.2 - baseline.2).abs() > 0.001;
        let native_changed = actual.0 != baseline.0 || actual.1 != baseline.1;
        if original_changed != native_changed {
            failures.push(format!("{family} {text} {weight} {style} {size}: native changed={native_changed}, original changed={original_changed}"));
        }
    }
    ensure!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures
            .iter()
            .take(24)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    println!(
        "{} original synthetic font advances, ink bounds and weight pixel changes verified on {platform}",
        cases.len()
    );
    Ok(())
}
