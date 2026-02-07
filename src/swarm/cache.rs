//! Content-Addressed Cache for Memoization
//!
//! Phase 6: Cache parsed ASTs and embeddings to avoid redundant analysis.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// Cache entry with content hash
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    pub content_hash: u64,
    pub data: T,
    pub created_at: u64,
}

/// Content-addressed cache
pub struct ContentCache<T> {
    entries: HashMap<PathBuf, CacheEntry<T>>,
    max_entries: usize,
}

impl<T: Clone> ContentCache<T> {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
        }
    }

    /// Get cached data if content hash matches
    pub fn get(&self, path: &Path, content: &str) -> Option<T> {
        let hash = hash_content(content);
        self.entries.get(path).and_then(|entry| {
            if entry.content_hash == hash {
                Some(entry.data.clone())
            } else {
                None
            }
        })
    }

    /// Store data with content hash
    pub fn set(&mut self, path: PathBuf, content: &str, data: T) {
        // Evict oldest if at capacity
        if self.entries.len() >= self.max_entries {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.created_at)
                .map(|(k, _)| k.clone());
            
            if let Some(key) = oldest {
                self.entries.remove(&key);
            }
        }

        self.entries.insert(
            path,
            CacheEntry {
                content_hash: hash_content(content),
                data,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            },
        );
    }

    /// Invalidate cache entry
    pub fn invalidate(&mut self, path: &Path) {
        self.entries.remove(path);
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of cached entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Hash content for cache key
fn hash_content(content: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Cached file metadata for swarm context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFileAnalysis {
    pub imports: Vec<String>,
    pub exports: Vec<String>,
    pub functions: Vec<String>,
    pub types: Vec<String>,
}

/// Global analysis cache (singleton pattern via lazy_static or OnceCell)
pub struct AnalysisCache {
    file_analysis: ContentCache<CachedFileAnalysis>,
}

impl AnalysisCache {
    pub fn new() -> Self {
        Self {
            file_analysis: ContentCache::new(1000), // Cache up to 1000 files
        }
    }

    pub fn get_analysis(&self, path: &Path, content: &str) -> Option<CachedFileAnalysis> {
        self.file_analysis.get(path, content)
    }

    pub fn cache_analysis(&mut self, path: PathBuf, content: &str, analysis: CachedFileAnalysis) {
        self.file_analysis.set(path, content, analysis);
    }

    pub fn invalidate(&mut self, path: &Path) {
        self.file_analysis.invalidate(path);
    }
}

impl Default for AnalysisCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_hit() {
        let mut cache = ContentCache::<String>::new(10);
        let path = PathBuf::from("test.rs");
        let content = "fn main() {}";
        
        cache.set(path.clone(), content, "cached_data".to_string());
        
        assert_eq!(cache.get(&path, content), Some("cached_data".to_string()));
    }

    #[test]
    fn test_cache_miss_on_change() {
        let mut cache = ContentCache::<String>::new(10);
        let path = PathBuf::from("test.rs");
        
        cache.set(path.clone(), "fn main() {}", "old_data".to_string());
        
        // Different content = cache miss
        assert_eq!(cache.get(&path, "fn main() { println!() }"), None);
    }

    #[test]
    fn test_eviction() {
        let mut cache = ContentCache::<u32>::new(2);
        
        cache.set(PathBuf::from("a.rs"), "a", 1);
        cache.set(PathBuf::from("b.rs"), "b", 2);
        cache.set(PathBuf::from("c.rs"), "c", 3); // Should evict oldest
        
        assert_eq!(cache.len(), 2);
    }
}
