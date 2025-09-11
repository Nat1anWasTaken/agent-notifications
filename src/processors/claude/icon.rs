use anyhow::Error;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

const CLAUDE_ICON_BYTES: &[u8] = include_bytes!("../../../assets/claude-icon.png");

pub fn get_claude_icon_temp_path() -> Result<PathBuf, Error> {
    let temp_dir = std::env::temp_dir();
    let icon_path = temp_dir.join("claude-code-icon.png");

    if !icon_path.exists() {
        let mut file = File::create(&icon_path)?;
        file.write_all(CLAUDE_ICON_BYTES)?;
    }

    Ok(icon_path)
}
