use anyhow::Error;
use notify_rust::Notification;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[allow(dead_code)]
const CLAUDE_ICON_BYTES: &[u8] = include_bytes!("../../../assets/claude-icon.png");

#[allow(dead_code)]
fn get_claude_icon_temp_path() -> Result<PathBuf, Error> {
    let temp_dir = std::env::temp_dir();
    let icon_path = temp_dir.join("claude-code-icon.png");

    if !icon_path.exists() {
        let mut file = File::create(&icon_path)?;
        file.write_all(CLAUDE_ICON_BYTES)?;
    }

    Ok(icon_path)
}

pub fn set_claude_icon(notification: &mut Notification) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        notification.appname("Claude Code");
        notification.icon("NSInfo");
    }

    #[cfg(not(target_os = "macos"))]
    {
        let icon_path = get_claude_icon_temp_path()?;
        notification.icon(&icon_path.to_string_lossy());
    }

    Ok(())
}
