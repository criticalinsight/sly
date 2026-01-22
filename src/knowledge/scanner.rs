use std::path::{Path, PathBuf};
use std::fs;
use sha2::{Sha256, Digest};
use anyhow::Result;

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
