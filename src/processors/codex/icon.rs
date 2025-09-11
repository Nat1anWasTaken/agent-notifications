use anyhow::Error;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

const CODEX_ICON_BYTES: &[u8] = include_bytes!("../../../assets/codex-icon.png");

pub fn get_codex_icon_path() -> Result<PathBuf, Error> {
    let temp_dir = std::env::temp_dir();
    let icon_path = temp_dir.join("codex-icon.png");

    if !icon_path.exists() {
        let mut file = File::create(&icon_path)?;
        file.write_all(CODEX_ICON_BYTES)?;
    }

    Ok(icon_path)
}
