use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Error;

const OPENCODE_ICON_BYTES: &[u8] = include_bytes!("../../../assets/opencode-icon.png");

pub fn get_opencode_icon_path() -> Result<PathBuf, Error> {
    let mut path = std::env::temp_dir();
    path.push("opencode-icon.png");

    if !path.exists() {
        let mut file = File::create(&path)?;
        file.write_all(OPENCODE_ICON_BYTES)?;
    }

    Ok(path)
}
