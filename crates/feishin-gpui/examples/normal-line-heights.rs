// Platform font metrics without a window, profile, account or playback.
use anyhow::{Result, ensure};
use gpui::{Application, FontWeight, font, px};
use std::borrow::Cow;
fn main() -> Result<()> {
    let application = Application::headless();
    let system = application.text_system();
    system.add_fonts(
        feishin_core::fonts::builtin_fonts()?
            .into_iter()
            .map(|font| Cow::Owned(font.bytes))
            .collect(),
    )?;
    #[cfg(target_os = "linux")]
    let bundled = include_str!("../assets/css-normal-line-heights-linux-reference.json");
    #[cfg(not(target_os = "linux"))]
    let bundled = include_str!("../assets/css-normal-line-heights-darwin-reference.json");
    let source = std::env::args()
        .nth(1)
        .map(std::fs::read_to_string)
        .transpose()?;
    let reference: serde_json::Value = serde_json::from_str(source.as_deref().unwrap_or(bundled))?;
    for case in reference["cases"].as_array().unwrap() {
        let size = case["size"].as_f64().unwrap() as f32;
        let mut face = font(case["family"].as_str().unwrap().to_owned());
        face.weight = FontWeight(case["weight"].as_f64().unwrap() as f32);
        let zoom = case["zoom"].as_f64().unwrap_or(1.) as f32;
        let height = if matches!(case["reset"].as_str().unwrap(), "normal" | "initial") {
            f32::from(system.normal_line_height(&face, px(size * zoom), 2.) / zoom)
        } else {
            2. * size
        };
        let expected = case["height"].as_f64().unwrap() as f32;
        ensure!(
            (height - expected).abs() < 0.01,
            "{case}: native {height}, original {expected}"
        );
    }
    println!(
        "{} original normal line-height cases match platform metrics",
        reference["cases"].as_array().unwrap().len()
    );
    Ok(())
}
