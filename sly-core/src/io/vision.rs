use std::process::Command;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use crate::error::{Result, SlyError};
use base64::{Engine as _, engine::general_purpose};
use std::fs;

/// Captures the primary screen using MacOS 'screencapture' utility.
/// Returns the path to the temporary image file.
pub fn capture_screen() -> Result<PathBuf> {
    let temp_file = NamedTempFile::new()
        .map_err(|e| SlyError::Io(e))?
        .into_temp_path();
    
    // We add the extension because screencapture might expect it or it's cleaner
    let path = temp_file.to_path_buf().with_extension("png");
    
    // -x: mute sound
    // -t png: format
    let status = Command::new("screencapture")
        .arg("-x")
        .arg("-t")
        .arg("png")
        .arg(&path)
        .status()?;

    if !status.success() {
        return Err(SlyError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "screencapture failed",
        )));
    }

    Ok(path)
}

/// Encodes an image file to Base64 for Gemini ingestion.
pub fn encode_image(path: &PathBuf) -> Result<String> {
    let data = fs::read(path)?;
    Ok(general_purpose::STANDARD.encode(data))
}

pub struct VisionFrame {
    pub path: PathBuf,
    pub base64: String,
}

impl VisionFrame {
    pub fn capture() -> Result<Self> {
        let path = capture_screen()?;
        let base64 = encode_image(&path)?;
        Ok(Self { path, base64 })
    }
}
