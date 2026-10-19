//! Read-only audit of an Electron/native preference export.
fn main() -> anyhow::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Supply a settings fixture path"))?;
    let bytes = std::fs::read(path)?;
    let preview = feishin_core::migration::preview(&Default::default(), &bytes)?;
    println!(
        "{}",
        serde_json::json!({"omitted":preview.omitted,"settings":preview.settings})
    );
    Ok(())
}
