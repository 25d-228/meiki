use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/src/lib/generated");
    meiki_application::export_typescript_contracts(&output)?;
    Ok(())
}
