//! Transactional overlay filesystem.
//!
//! All agent file writes go into a temporary overlay directory.
//! [`OverlayFS::rollback`] wipes the overlay without touching the
//! real filesystem. Absolute paths outside `base_dir` are rejected.

use crate::error::{Result, SlyError};
use std::fs;
use std::path::{Path, PathBuf};

/// A sandboxed, rollback-capable filesystem layer.
pub struct OverlayFS {
    pub(crate) base_dir: PathBuf,
    pub(crate) overlay_dir: PathBuf,
}

impl OverlayFS {
    /// Create a fresh overlay directory under the system temp dir.
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

    /// Write a file into the overlay. Rejects absolute paths outside `base_dir`.
    pub fn write_file(&self, path: &Path, content: &str) -> Result<()> {
        let overlay_path = self.map_to_overlay(path)?;

        if let Some(parent) = overlay_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(overlay_path, content)?;
        Ok(())
    }

    /// Destroy all overlay contents and recreate an empty directory.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    static OV_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn test_overlay(label: &str) -> OverlayFS {
        let id = format!("{}_{}", label, OV_COUNTER.fetch_add(1, Ordering::SeqCst));
        OverlayFS::new(Path::new("/tmp/sly_test_base"), &id).unwrap()
    }

    #[test]
    fn test_overlay_creates_dir() {
        let ov = test_overlay("create");
        assert!(ov.overlay_dir.exists());
        fs::remove_dir_all(&ov.overlay_dir).ok();
    }

    #[test]
    fn test_overlay_write_and_read() {
        let ov = test_overlay("write");
        ov.write_file(Path::new("test.txt"), "hello").unwrap();
        let content = fs::read_to_string(ov.overlay_dir.join("test.txt")).unwrap();
        assert_eq!(content, "hello");
        fs::remove_dir_all(&ov.overlay_dir).ok();
    }

    #[test]
    fn test_overlay_rollback() {
        let ov = test_overlay("rollback");
        // Write a file using relative path
        ov.write_file(Path::new("gone.txt"), "data").unwrap();
        let file_path = ov.overlay_dir.join("gone.txt");
        assert!(file_path.exists(), "File should exist after write: {:?}", file_path);
        ov.rollback().unwrap();
        assert!(!file_path.exists(), "File should be gone after rollback");
        assert!(ov.overlay_dir.exists(), "Overlay dir should still exist");
        fs::remove_dir_all(&ov.overlay_dir).ok();
    }

    #[test]
    fn test_overlay_rejects_outside_base() {
        let ov = test_overlay("reject");
        let result = ov.write_file(Path::new("/etc/passwd"), "bad");
        assert!(result.is_err());
        fs::remove_dir_all(&ov.overlay_dir).ok();
    }
}
