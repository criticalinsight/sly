use std::path::{Path, PathBuf};
use std::fs;
use sha2::{Sha256, Digest};
use crate::error::Result;

pub struct FileValue {
    pub path: PathBuf,
    pub content: String,
    pub hash: String,
    pub extension: String,
}

pub struct Scanner;

impl Scanner {
    pub fn scan_file(path: &Path) -> Result<Option<FileValue>> {
        if !path.is_file() {
            return Ok(None);
        }

        let extension = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Skip if not code/markdown
        if !matches!(extension.as_str(), "rs" | "js" | "ts" | "py" | "md" | "txt") {
            return Ok(None);
        }

        let content = fs::read_to_string(path)?;
        
        // Calculate Hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = hex::encode(hasher.finalize());

        Ok(Some(FileValue {
            path: path.to_path_buf(),
            content,
            hash,
            extension,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_scan_file_valid() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("sly_scan_test");
        if temp_dir.exists() { fs::remove_dir_all(&temp_dir)?; }
        fs::create_dir_all(&temp_dir)?;
        let file_path = temp_dir.join("test.rs");
        fs::write(&file_path, "fn main() {}")?;
        
        let val = Scanner::scan_file(&file_path)?.unwrap();
        assert_eq!(val.extension, "rs");
        assert_eq!(val.content, "fn main() {}");
        assert!(!val.hash.is_empty());
        Ok(())
    }

    #[test]
    fn test_scan_file_invalid_ext() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("sly_scan_test_ext");
        if temp_dir.exists() { fs::remove_dir_all(&temp_dir)?; }
        fs::create_dir_all(&temp_dir)?;
        let file_path = temp_dir.join("test.bin");
        fs::write(&file_path, vec![0, 1, 2])?;
        
        let val = Scanner::scan_file(&file_path)?;
        assert!(val.is_none());
        Ok(())
    }
}
