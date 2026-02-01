use std::path::{Path, PathBuf};
use crate::error::{Result, SlyError};
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
            path.strip_prefix(base_dir).map_err(|e| SlyError::Overlay(e.to_string()))?.to_path_buf()
        } else {
            return Err(SlyError::Overlay(format!("Path {:?} is outside base directory {:?}", path, base_dir)));
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
            let overlay_path = map_to_overlay(overlay.base_dir(), overlay.overlay_dir(), &path)?;
            
            if let Some(parent) = overlay_path.parent() {
                fs::create_dir_all(parent).map_err(|e| SlyError::Io(e))?;
            }
            fs::write(overlay_path, content).map_err(|e| SlyError::Io(e))?;
        }
        FileSystemAction::Delete { path } => {
            let overlay_path = map_to_overlay(overlay.base_dir(), overlay.overlay_dir(), &path)?;
            if overlay_path.exists() {
                if overlay_path.is_dir() {
                    fs::remove_dir_all(overlay_path).map_err(|e| SlyError::Io(e))?;
                } else {
                    fs::remove_file(overlay_path).map_err(|e| SlyError::Io(e))?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_to_overlay_relative() {
        let base = Path::new("/work");
        let overlay = Path::new("/tmp/overlay");
        let path = Path::new("src/main.rs");
        let mapped = map_to_overlay(base, overlay, path).unwrap();
        assert_eq!(mapped, Path::new("/tmp/overlay/src/main.rs"));
    }

    #[test]
    fn test_map_to_overlay_absolute_valid() {
        let base = Path::new("/work");
        let overlay = Path::new("/tmp/overlay");
        let path = Path::new("/work/src/main.rs");
        let mapped = map_to_overlay(base, overlay, path).unwrap();
        assert_eq!(mapped, Path::new("/tmp/overlay/src/main.rs"));
    }

    #[test]
    fn test_map_to_overlay_outside_base() {
        let base = Path::new("/work");
        let overlay = Path::new("/tmp/overlay");
        let path = Path::new("/etc/passwd");
        assert!(map_to_overlay(base, overlay, path).is_err());
    }

    #[test]
    fn test_execute_write_action() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("sly_fs_test");
        if temp_dir.exists() { fs::remove_dir_all(&temp_dir)?; }
        fs::create_dir_all(&temp_dir)?;

        let overlay = OverlayFS::new(&temp_dir, "fs_test")?;
        let action = FileSystemAction::Write {
            path: PathBuf::from("test.txt"),
            content: "hello".to_string(),
        };

        execute_action(&overlay, action)?;
        
        let overlay_file = overlay.overlay_dir().join("test.txt");
        assert!(overlay_file.exists());
        assert_eq!(fs::read_to_string(overlay_file)?, "hello");
        Ok(())
    }
}
