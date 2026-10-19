// Actual platform font registration, shaping and rasterization, without a window.
use anyhow::{Result, ensure};
use gpui::{Application, TextStyle, WindowTextSystem, px};
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
    let window_system = WindowTextSystem::new(system.clone());
    let style = TextStyle {
        font_family: "Feishin Reload Probe".into(),
        ..Default::default()
    };
    let text = "HHH";
    let run = style.to_run(text.len());
    // Cache a failed lookup before importing this family.
    let missing = window_system.layout_line(text, px(32.), std::slice::from_ref(&run), None);
    system.add_fonts(vec![Cow::Borrowed(include_bytes!(
        "../assets/font-reload-1.ttf"
    ))])?;
    let first = window_system.layout_line(text, px(32.), std::slice::from_ref(&run), None);
    ensure!(
        first.runs[0].font_id != missing.runs[0].font_id,
        "Font import reused a cached missing-family fallback"
    );
    let first_raster = window_system.rasterized_glyph_for_test(
        first.runs[0].font_id,
        first.runs[0].glyphs[0].id,
        px(32.),
        1.,
    )?;
    system.add_fonts(vec![Cow::Borrowed(include_bytes!(
        "../assets/font-reload-2.ttf"
    ))])?;
    let second = window_system.layout_line(text, px(32.), &[run], None);
    ensure!(
        second.runs[0].font_id != first.runs[0].font_id,
        "Same-family reload reused its earlier font identifier"
    );
    ensure!(
        (second.width - first.width * 2.).abs() <= px(0.05),
        "Same-family reload kept stale metrics: {:?} -> {:?}",
        first.width,
        second.width
    );
    let second_raster = window_system.rasterized_glyph_for_test(
        second.runs[0].font_id,
        second.runs[0].glyphs[0].id,
        px(32.),
        1.,
    )?;
    ensure!(
        second_raster != first_raster,
        "Reload kept stale glyph pixels"
    );
    ensure!(
        window_system.rasterized_glyph_for_test(
            first.runs[0].font_id,
            first.runs[0].glyphs[0].id,
            px(32.),
            1.
        )? == first_raster,
        "Reload invalidated an earlier layout's live font identifier"
    );
    println!("Late font import and same-face reload preserve new metrics/pixels and old layouts");
    Ok(())
}
