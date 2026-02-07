//! Shared Context Layer for Swarm Workers
//!
//! Phase 2: Pre-analyzed codebase state shared across all workers.
//! Workers receive read-only snapshot, write to isolated overlays.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// File metadata for indexed codebase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub language: String,
    pub size_bytes: u64,
    pub hash: String,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
}

/// Dependency edge in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub from: PathBuf,
    pub to: PathBuf,
    pub kind: DependencyKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyKind {
    Import,
    TypeReference,
    FunctionCall,
    Inheritance,
}

/// Pre-analyzed codebase context (immutable, shared)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmContext {
    /// All indexed files
    pub files: HashMap<PathBuf, FileMetadata>,
    /// Dependency graph edges
    pub dependencies: Vec<DependencyEdge>,
    /// Root directory
    pub root: PathBuf,
    /// Timestamp of analysis
    pub analyzed_at: u64,
}

impl SwarmContext {
    /// Analyze a codebase and build context
    pub fn analyze(root: &Path) -> std::io::Result<Self> {
        let mut files = HashMap::new();
        let mut dependencies = Vec::new();
        
        // Walk source directories
        let extensions = ["rs", "ts", "tsx", "js", "jsx", "py", "go", "gleam", "ex", "exs"];
        
        fn walk_dir(
            dir: &Path,
            root: &Path,
            extensions: &[&str],
            files: &mut HashMap<PathBuf, FileMetadata>,
        ) -> std::io::Result<()> {
            if !dir.is_dir() {
                return Ok(());
            }
            
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_dir() {
                    // Skip common non-source dirs
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if !["node_modules", "target", ".git", "__pycache__", "dist", "build"]
                        .contains(&name.as_ref())
                    {
                        walk_dir(&path, root, extensions, files)?;
                    }
                } else if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if extensions.contains(&ext.to_string_lossy().as_ref()) {
                            let content = std::fs::read_to_string(&path).unwrap_or_default();
                            let hash = format!("{:x}", md5_hash(&content));
                            let imports = extract_imports(&content, &ext.to_string_lossy());
                            
                            files.insert(
                                path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
                                FileMetadata {
                                    path: path.clone(),
                                    language: ext.to_string_lossy().to_string(),
                                    size_bytes: content.len() as u64,
                                    hash,
                                    imports,
                                    exports: Vec::new(), // TODO: extract exports
                                },
                            );
                        }
                    }
                }
            }
            Ok(())
        }
        
        walk_dir(root, root, &extensions, &mut files)?;
        
        // Build dependency edges from imports
        for (path, meta) in &files {
            for imp in &meta.imports {
                // Try to resolve import to a file
                if let Some(target) = resolve_import(imp, path, &files) {
                    dependencies.push(DependencyEdge {
                        from: path.clone(),
                        to: target,
                        kind: DependencyKind::Import,
                    });
                }
            }
        }
        
        Ok(Self {
            files,
            dependencies,
            root: root.to_path_buf(),
            analyzed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    /// Get all files that depend on the given file
    pub fn dependents(&self, path: &Path) -> Vec<PathBuf> {
        self.dependencies
            .iter()
            .filter(|e| e.to == path)
            .map(|e| e.from.clone())
            .collect()
    }

    /// Get all dependencies of a file
    pub fn dependencies_of(&self, path: &Path) -> Vec<PathBuf> {
        self.dependencies
            .iter()
            .filter(|e| e.from == path)
            .map(|e| e.to.clone())
            .collect()
    }

    /// Serialize context for worker consumption
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Simple MD5 hash (for content addressing)
fn md5_hash(content: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Extract import statements from source code
fn extract_imports(content: &str, lang: &str) -> Vec<String> {
    let mut imports = Vec::new();
    
    for line in content.lines() {
        let line = line.trim();
        match lang {
            "rs" => {
                if line.starts_with("use ") || line.starts_with("mod ") {
                    imports.push(line.to_string());
                }
            }
            "ts" | "tsx" | "js" | "jsx" => {
                if line.starts_with("import ") || line.starts_with("require(") {
                    imports.push(line.to_string());
                }
            }
            "py" => {
                if line.starts_with("import ") || line.starts_with("from ") {
                    imports.push(line.to_string());
                }
            }
            "go" => {
                if line.starts_with("import ") {
                    imports.push(line.to_string());
                }
            }
            _ => {}
        }
    }
    
    imports
}

/// Try to resolve an import to a file path
fn resolve_import(
    _import: &str,
    _from: &Path,
    _files: &HashMap<PathBuf, FileMetadata>,
) -> Option<PathBuf> {
    // Simplified: in reality, this needs language-specific resolution
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_imports() {
        let content = "use std::collections::HashMap;\nmod tests;\nfn main() {}";
        let imports = extract_imports(content, "rs");
        assert_eq!(imports.len(), 2);
    }

    #[test]
    fn test_extract_ts_imports() {
        let content = "import { foo } from './bar';\nconst x = 1;";
        let imports = extract_imports(content, "ts");
        assert_eq!(imports.len(), 1);
    }
}
