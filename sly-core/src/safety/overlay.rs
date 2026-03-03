use crate::error::{Result, SlyError};
use std::fs;
use std::path::{Path, PathBuf};

/// OverlayFS provides a safe, transactional layer over the filesystem.
/// 
/// - Reads: Check overlay first, then base.
/// - Writes: Always write to overlay.
/// - Commit: Copy overlay contents to base atomically (as much as possible).
/// - Rollback: Discard overlay.
pub struct OverlayFS {
    pub(crate) base_dir: PathBuf,
    pub(crate) overlay_dir: PathBuf,
}

impl OverlayFS {
    pub fn base_dir(&self) -> &Path { &self.base_dir }
    pub fn overlay_dir(&self) -> &Path { &self.overlay_dir }
    /// Creates a new OverlayFS. 
    /// `base_dir`: The real workspace (e.g., user's project).
    /// `overlay_id`: Unique ID for this transaction (e.g., task ID).
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

    /// Reads a file, transparently checking overlay then base.
    pub fn read_file(&self, path: &Path) -> Result<String> {
        let rel_path = self.get_relative_path(path)?;
        let overlay_path = self.overlay_dir.join(&rel_path);

        if overlay_path.exists() {
            return Ok(fs::read_to_string(overlay_path)?);
        }

        let base_path = self.base_dir.join(&rel_path);
        if base_path.exists() {
            return Ok(fs::read_to_string(base_path).map_err(|e| SlyError::Io(e))?);
        }

        Err(SlyError::Overlay(format!("File not found in overlay or base: {:?}", path)))
    }

    /// Writes a file to the overlay.
    pub fn write_file(&self, path: &Path, content: &str) -> Result<()> {
        let rel_path = self.get_relative_path(path)?;
        let overlay_path = self.overlay_dir.join(&rel_path);

        if let Some(parent) = overlay_path.parent() {
            fs::create_dir_all(parent).map_err(|e| SlyError::Io(e))?;
        }

        fs::write(overlay_path, content).map_err(|e| SlyError::Io(e))?;
        Ok(())
    }

    /// Commits changes from overlay to base.
    /// This effectively "merges" the overlay into the base.
    pub fn commit(&self) -> Result<()> {
        // Recursively copy overlay_dir to base_dir
        self.copy_dir_recursive(&self.overlay_dir, &self.base_dir)?;
        
        Ok(())
    }

    /// Generates a list of speculative facts representing the changes in the overlay.
    /// Useful for GleamDB with_facts speculative execution.
    pub fn generate_speculative_facts(&self) -> Result<serde_json::Value> {
        let mut facts = Vec::new();
        self.collect_speculative_facts(&self.overlay_dir, &mut facts)?;
        Ok(serde_json::Value::Array(facts))
    }

    fn collect_speculative_facts(&self, dir: &Path, facts: &mut Vec<serde_json::Value>) -> Result<()> {
        if !dir.exists() { return Ok(()); }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.collect_speculative_facts(&path, facts)?;
            } else {
                let content = fs::read_to_string(&path)?;
                let rel_path = path.strip_prefix(&self.overlay_dir).unwrap_or(&path);
                facts.push(serde_json::json!({
                    "e": 0, // Entity ID for the file/modification
                    "a": "file/speculative_content",
                    "v": { "type": "str", "value": content },
                    "path": rel_path.to_string_lossy()
                }));
            }
        }
        Ok(())
    }

    /// Discards the overlay (rollback).
    pub fn rollback(&self) -> Result<()> {
        if self.overlay_dir.exists() {
            fs::remove_dir_all(&self.overlay_dir).map_err(|e| SlyError::Io(e))?;
        }
        fs::create_dir_all(&self.overlay_dir).map_err(|e| SlyError::Io(e))?;
        Ok(())
    }

    /// Helper to handle absolute/relative paths and ensure they are within workspace
    fn get_relative_path(&self, path: &Path) -> Result<PathBuf> {
        if path.is_absolute() {
            if path.starts_with(&self.base_dir) {
                Ok(path.strip_prefix(&self.base_dir).map_err(|e| SlyError::Overlay(e.to_string()))?.to_path_buf())
            } else {
                // If it's absolute but NOT in base dir, we might reject it or handle it.
                // For safety, we only allow operations within base_dir.
                Err(SlyError::Overlay(format!("Path {:?} is outside base directory {:?}", path, self.base_dir)))
            }
        } else {
            Ok(path.to_path_buf())
        }
    }

    fn copy_dir_recursive(&self, src: &Path, dst: &Path) -> Result<()> {
        if !dst.exists() {
            fs::create_dir_all(dst)?;
        }

        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if ft.is_dir() {
                self.copy_dir_recursive(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
            }
        }
        Ok(())
    }
}

// Ensure cleanup on drop if not committed? 
// Ideally yes, but strict transactional logic (commit consumed) is safer.
// For now, let's leave drop explicit or rely on OS temp cleanup, 
// to avoid accidental data loss if the struct is dropped prematurely.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_transaction() -> Result<()> {
        let temp_root = std::env::temp_dir().join("sly_test_safety_tx");
        if temp_root.exists() {
            fs::remove_dir_all(&temp_root)?;
        }
        fs::create_dir_all(&temp_root)?;

        let base_file = temp_root.join("config.toml");
        fs::write(&base_file, "version = 1")?;

        let overlay = OverlayFS::new(&temp_root, "tx_1")?;

        // 1. Read base through overlay
        assert_eq!(overlay.read_file(&Path::new("config.toml"))?, "version = 1");

        // 2. Write to overlay (shadowed)
        overlay.write_file(&Path::new("config.toml"), "version = 2")?;
        
        // 3. Read should show new version
        assert_eq!(overlay.read_file(&Path::new("config.toml"))?, "version = 2");

        // 4. Base should still be old
        assert_eq!(fs::read_to_string(&base_file)?, "version = 1");

        // 5. Commit
        overlay.commit()?;

        // 6. Base should now be updated
        assert_eq!(fs::read_to_string(&base_file)?, "version = 2");

        Ok(())
    }

    #[test]
    fn test_overlay_rollback() -> Result<()> {
        let temp_root = std::env::temp_dir().join("sly_test_safety_rollback");
        if temp_root.exists() {
            fs::remove_dir_all(&temp_root)?;
        }
        fs::create_dir_all(&temp_root)?;

        let base_file = temp_root.join("config.toml");
        fs::write(&base_file, "initial")?;

        let overlay = OverlayFS::new(&temp_root, "tx_rollback")?;
        overlay.write_file(&Path::new("config.toml"), "changed")?;
        assert_eq!(overlay.read_file(&Path::new("config.toml"))?, "changed");

        overlay.rollback()?;
        
        // After rollback, read should show base version (since overlay dir is gone)
        // Wait, the current implementation of read_file checks if overlay_dir exists?
        // Let's re-verify read_file logic.
        
        let overlay2 = OverlayFS {
            base_dir: temp_root.clone(),
            overlay_dir: std::env::temp_dir().join("sly_overlays").join("tx_rollback"),
        };
        assert_eq!(overlay2.read_file(&Path::new("config.toml"))?, "initial");

        Ok(())
    }

    #[test]
    fn test_path_security() -> Result<()> {
        let temp_root = std::env::temp_dir().join("sly_test_safety_security");
        let overlay = OverlayFS::new(&temp_root, "tx_sec")?;

        // Path outside base
        let outside_path = Path::new("/etc/passwd");
        assert!(overlay.read_file(outside_path).is_err());
        assert!(overlay.write_file(outside_path, "hack").is_err());

        Ok(())
    }

    #[test]
    fn test_nested_directories() -> Result<()> {
        let temp_root = std::env::temp_dir().join("sly_test_safety_nested");
        if temp_root.exists() {
            fs::remove_dir_all(&temp_root)?;
        }
        fs::create_dir_all(&temp_root)?;

        let overlay = OverlayFS::new(&temp_root, "tx_nested")?;
        let nested_path = Path::new("src/core/mod.rs");
        
        overlay.write_file(nested_path, "pub mod parser;")?;
        assert_eq!(overlay.read_file(nested_path)?, "pub mod parser;");
        
        overlay.commit()?;
        assert_eq!(fs::read_to_string(temp_root.join(nested_path))?, "pub mod parser;");

        Ok(())
    }
}
