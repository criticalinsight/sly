use crate::error::{Result, SlyError};
use std::fs;
use std::path::{Path, PathBuf};

/// OverlayFS provides a safe, transactional layer over the filesystem.
pub struct OverlayFS {
    pub(crate) base_dir: PathBuf,
    pub(crate) overlay_dir: PathBuf,
}

impl OverlayFS {
    pub fn new(base_dir: &Path, overlay_id: &str) -> Result<Self> {
        let temp_dir = std::env::temp_dir().join("sly_overlays").join(overlay_id);
        
        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir)?;
        }
        fs::create_dir_all(&temp_dir)?;

        Ok(Self {
            base_dir: base_dir.to_path_buf(),
            overlay_dir: temp_dir,
        })
    }

    pub fn write_file(&self, path: &Path, content: &str) -> Result<()> {
        let overlay_path = self.map_to_overlay(path)?;

        if let Some(parent) = overlay_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(overlay_path, content)?;
        Ok(())
    }

    pub fn rollback(&self) -> Result<()> {
        if self.overlay_dir.exists() {
            fs::remove_dir_all(&self.overlay_dir)?;
        }
        fs::create_dir_all(&self.overlay_dir)?;
        Ok(())
    }

    fn get_relative_path(&self, path: &Path) -> Result<PathBuf> {
        if path.is_absolute() {
            if path.starts_with(&self.base_dir) {
                Ok(path.strip_prefix(&self.base_dir).map_err(|e| SlyError::Overlay(e.to_string()))?.to_path_buf())
            } else {
                Err(SlyError::Overlay(format!("Outside base: {:?}", path)))
            }
        } else {
            Ok(path.to_path_buf())
        }
    }

    fn map_to_overlay(&self, path: &Path) -> Result<PathBuf> {
        let rel_path = self.get_relative_path(path)?;
        Ok(self.overlay_dir.join(rel_path))
    }
}
