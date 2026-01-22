use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};
use crate::safety::OverlayFS;
use std::fs;

#[derive(Debug, Clone)]
pub enum FileSystemAction {
    Write { path: PathBuf, content: String },
    Delete { path: PathBuf },
}

/// Pure logic: Map a user-provided path to the physical path in the overlay.
pub fn map_to_overlay(base_dir: &Path, overlay_dir: &Path, path: &Path) -> Result<PathBuf> {
    let rel_path = if path.is_absolute() {
        if path.starts_with(base_dir) {
            path.strip_prefix(base_dir)?.to_path_buf()
        } else {
            return Err(anyhow!("Path {:?} is outside base directory {:?}", path, base_dir));
        }
    } else {
        path.to_path_buf()
    };
    
    Ok(overlay_dir.join(rel_path))
}

/// Execution logic: Apply a FileSystemAction to the Physical Overlay.
pub fn execute_action(overlay: &OverlayFS, action: FileSystemAction) -> Result<()> {
    match action {
        FileSystemAction::Write { path, content } => {
            let overlay_path = map_to_overlay(&overlay.base_dir(), &overlay.overlay_dir(), &path)?;
            
            if let Some(parent) = overlay_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(overlay_path, content)?;
        }
        FileSystemAction::Delete { path } => {
            let overlay_path = map_to_overlay(&overlay.base_dir(), &overlay.overlay_dir(), &path)?;
            if overlay_path.exists() {
                if overlay_path.is_dir() {
                    fs::remove_dir_all(overlay_path)?;
                } else {
                    fs::remove_file(overlay_path)?;
                }
            }
            // Note: In a real OverlayFS, we would also need to record a "Tombstone" 
            // if the file exists in the base. For now, we simple-shadow.
        }
    }
    Ok(())
}
