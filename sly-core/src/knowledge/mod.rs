use crate::memory::{Memory, GraphNode};
use crate::error::Result;
use std::path::{Path, PathBuf};
use rayon::prelude::*;

pub mod scanner;
pub mod extractor;
pub mod compressor;
pub mod deduplicator;

use scanner::{Scanner, FileValue};
use extractor::Extractor;
pub use compressor::SymbolicCompressor;


// --- Pure Functions & Data Transformation ---

pub async fn ingest_file(memory: &Memory, path: &Path) -> Result<()> {
    ingest_batch(memory, &[path.to_path_buf()]).await
}

pub async fn ingest_batch(memory: &Memory, paths: &[PathBuf]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }

    // 1. Parallel Scan (IO-bound but sync)
    let files: Vec<FileValue> = paths.par_iter()
        .filter_map(|p| Scanner::scan_file(p).ok().flatten())
        .collect();

    // 2. Sequential/Async Filter (DB-bound)
    let mut candidates = Vec::new();
    for file in files {
        let reindex = should_reindex(memory, &file).await?;
        eprintln!("🔍 File: {:?}, Reindex: {}", file.path, reindex);
        if reindex {
            candidates.push(file);
        }
    }

    if candidates.is_empty() {
        eprintln!("ℹ️ No candidates for re-indexing.");
        return Ok(());
    }

    println!("📝 Re-indexing {} changed files in parallel...", candidates.len());

    // 3. Parallel Extraction (CPU-bound)
    let all_nodes_and_files: Vec<(Vec<GraphNode>, FileValue)> = candidates.into_par_iter()
        .map(|file| {
            let path_str = file.path.to_str().unwrap_or_default();
            let nodes = Extractor::extract_symbols(&file.content, &file.extension, path_str);
            (nodes, file)
        })
        .collect();

    // 4. Batch Commit (Side effects)
    for (nodes, file) in all_nodes_and_files {
        commit_nodes(memory, nodes, &file).await?;
    }
    
    Ok(())
}

async fn should_reindex(memory: &Memory, file: &FileValue) -> Result<bool> {
    let path_str = file.path.to_str().unwrap_or_default();
    if let Ok(Some((_, old_hash))) = memory.check_sync_status(path_str).await {
        if old_hash == file.hash {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn commit_nodes(memory: &Memory, nodes: Vec<GraphNode>, file: &FileValue) -> Result<()> {
    let path_str = file.path.to_str().unwrap_or_default();
    if !nodes.is_empty() {
        memory.batch_add_nodes(nodes).await?;
    }
    memory.update_sync_status(path_str, &file.hash).await?;
    Ok(())
}

pub async fn expand_symbol(memory: &Memory, path: &str, symbol: Option<&str>) -> Result<String> {
    let nodes = memory.get_symbols_for_path(path).await?;
    if nodes.is_empty() {
        return Ok(format!("No symbols found for path: {}", path));
    }

    if let Some(sym) = symbol {
        if let Some(node) = nodes.iter().find(|n| n.id.contains(sym)) {
            return Ok(node.content.clone());
        }
    }

    // Default to symbolic overview
    Ok(SymbolicCompressor::compress_nodes(&nodes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use crate::knowledge::scanner::FileValue;

    async fn setup_memory(name: &str) -> (Memory, PathBuf) {
        let temp_dir = std::env::temp_dir().join(format!("sly_kn_test_{}_v4", name));
        if temp_dir.exists() { let _ = fs::remove_dir_all(&temp_dir); }
        fs::create_dir_all(&temp_dir).unwrap();
        
        let path = temp_dir.join("cozo").to_string_lossy().to_string();
        let mem = Memory::new(&path, false).await.expect("Failed to create memory");
        (mem, temp_dir)
    }

    #[tokio::test]
    async fn test_should_reindex() -> Result<()> {
        let (mem, _tmp) = setup_memory("reindex").await;
        let file = FileValue {
            path: PathBuf::from("test.rs"),
            content: "foo".to_string(),
            hash: "h1".to_string(),
            extension: "rs".to_string(),
        };
        
        // Initial reindex should be true
        assert!(should_reindex(&mem, &file).await?);
        
        // Update sync status
        mem.update_sync_status("test.rs", "h1").await?;
        
        // Now it should be false
        assert!(!should_reindex(&mem, &file).await?);
        
        // Change hash
        let file2 = FileValue { hash: "h2".to_string(), ..file };
        assert!(should_reindex(&mem, &file2).await?);
        
        Ok(())
    }
}
