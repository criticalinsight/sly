//! Transactional overlay filesystem.
//!
//! All agent file writes go into a temporary overlay directory.
//! [`OverlayFS::commit`] copies overlay files to the real working
//! directory. [`OverlayFS::rollback`] wipes the overlay.
//! Absolute paths outside `base_dir` are rejected.

use crate::error::{Result, SlyError};
use std::fs;
use std::path::{Path, PathBuf};

/// A sandboxed, rollback-capable filesystem layer.
pub struct OverlayFS {
    pub(crate) base_dir: PathBuf,
    pub(crate) overlay_dir: PathBuf,
    pub(crate) scratchpad_dir: PathBuf,
}

impl OverlayFS {
    /// Create a fresh overlay directory under the system temp dir.
    pub fn new(base_dir: &Path, overlay_id: &str) -> Result<Self> {
        let temp_dir = std::env::temp_dir().join("sly_overlays").join(overlay_id);
        let scratchpad_dir = std::env::temp_dir().join("sly_scratchpad").join(overlay_id);
        
        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir)?;
        }
        fs::create_dir_all(&temp_dir)?;

        if scratchpad_dir.exists() {
            fs::remove_dir_all(&scratchpad_dir)?;
        }
        fs::create_dir_all(&scratchpad_dir)?;

        let base_dir_canon = fs::canonicalize(base_dir).unwrap_or_else(|_| base_dir.to_path_buf());
        Self::clone_base_to_scratchpad(&base_dir_canon, &scratchpad_dir)?;

        Ok(Self {
            base_dir: base_dir_canon,
            overlay_dir: temp_dir,
            scratchpad_dir,
        })
    }

    fn clone_base_to_scratchpad(src: &Path, dst: &Path) -> Result<()> {
        if !src.exists() { return Ok(()); }
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            
            // Rich Hickey insight: Omit large, ephemeral caches from the projection.
            if name_str == ".git" || name_str == "target" || name_str == "node_modules" {
                continue;
            }
            
            let dst_path = dst.join(name);
            if path.is_dir() {
                fs::create_dir_all(&dst_path)?;
                Self::clone_base_to_scratchpad(&path, &dst_path)?;
            } else {
                fs::copy(&path, &dst_path)?;
            }
        }
        Ok(())
    }

    /// Write a file into the overlay. Rejects absolute paths outside `base_dir`.
    pub fn write_file(&self, path: &Path, content: &str) -> Result<()> {
        let rel_path = self.get_relative_path(path)?;
        let overlay_path = self.overlay_dir.join(&rel_path);
        let scratchpad_path = self.scratchpad_dir.join(&rel_path);

        if let Some(parent) = overlay_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(parent) = scratchpad_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&overlay_path, content)?;
        fs::write(&scratchpad_path, content)?;
        Ok(())
    }

    /// Copy all overlay files to the real `base_dir`.
    ///
    /// Returns the list of relative paths that were committed.
    /// The overlay is cleared after a successful commit.
    pub fn commit(&self) -> Result<Vec<String>> {
        let mut committed = Vec::new();
        self.copy_dir_recursive(&self.overlay_dir, &self.base_dir, &mut committed)?;
        // Clear overlay after commit
        self.rollback()?;
        Ok(committed)
    }

    /// Recursively copy `src` into `dst`, collecting relative paths.
    fn copy_dir_recursive(
        &self,
        src: &Path,
        dst: &Path,
        committed: &mut Vec<String>,
    ) -> Result<()> {
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let src_path = entry.path();
            let file_name = entry.file_name();
            let dst_path = dst.join(&file_name);

            if file_type.is_dir() {
                fs::create_dir_all(&dst_path)?;
                self.copy_dir_recursive(&src_path, &dst_path, committed)?;
            } else {
                if let Some(parent) = dst_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&src_path, &dst_path)?;
                // Record relative path from base_dir
                let rel = dst_path
                    .strip_prefix(&self.base_dir)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| dst_path.display().to_string());
                committed.push(rel);
            }
        }
        Ok(())
    }

    /// List all files currently in the overlay.
    pub fn list_files(&self) -> Vec<String> {
        let mut files = Vec::new();
        self.collect_files(&self.overlay_dir, &mut files);
        files
    }

    fn collect_files(&self, dir: &Path, files: &mut Vec<String>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    self.collect_files(&path, files);
                } else if let Ok(rel) = path.strip_prefix(&self.overlay_dir) {
                    files.push(rel.display().to_string());
                }
            }
        }
    }

    /// Destroy all overlay contents and recreate an empty directory.
    pub fn rollback(&self) -> Result<()> {
        if self.overlay_dir.exists() {
            fs::remove_dir_all(&self.overlay_dir)?;
        }
        fs::create_dir_all(&self.overlay_dir)?;

        if self.scratchpad_dir.exists() {
            fs::remove_dir_all(&self.scratchpad_dir)?;
        }
        fs::create_dir_all(&self.scratchpad_dir)?;
        Self::clone_base_to_scratchpad(&self.base_dir, &self.scratchpad_dir)?;
        
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

    #[test]
    fn test_overlay_commit_copies_to_base() {
        let base = std::env::temp_dir().join(format!("sly_commit_test_{}", OV_COUNTER.fetch_add(1, Ordering::SeqCst)));
        fs::create_dir_all(&base).unwrap();
        let ov = OverlayFS::new(&base, &format!("commit_{}", OV_COUNTER.fetch_add(1, Ordering::SeqCst))).unwrap();

        ov.write_file(Path::new("hello.txt"), "world").unwrap();
        ov.write_file(Path::new("sub/nested.txt"), "deep").unwrap();

        let committed = ov.commit().unwrap();
        assert_eq!(committed.len(), 2);
        assert_eq!(fs::read_to_string(base.join("hello.txt")).unwrap(), "world");
        assert_eq!(fs::read_to_string(base.join("sub/nested.txt")).unwrap(), "deep");
        // Overlay should be cleared after commit
        assert!(ov.list_files().is_empty());

        fs::remove_dir_all(&base).ok();
        fs::remove_dir_all(&ov.overlay_dir).ok();
    }

    #[test]
    fn test_overlay_list_files() {
        let ov = test_overlay("list");
        assert!(ov.list_files().is_empty());
        ov.write_file(Path::new("a.txt"), "1").unwrap();
        ov.write_file(Path::new("b.txt"), "2").unwrap();
        let files = ov.list_files();
        assert_eq!(files.len(), 2);
        fs::remove_dir_all(&ov.overlay_dir).ok();
    }
}
