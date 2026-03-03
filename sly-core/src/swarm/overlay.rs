//! Transactional Overlay for Rollback
//!
//! Phase 4: Each worker writes to isolated overlay.
//! Supervisor commits all or rolls back on failure.

use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// A pending file change in the overlay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayChange {
    pub original_content: Option<String>,
    pub new_content: String,
    pub worker_id: String,
}

/// Transactional overlay for a worker
#[derive(Debug, Clone)]
pub struct WorkerOverlay {
    worker_id: String,
    changes: HashMap<PathBuf, OverlayChange>,
    _committed: bool,
}

impl WorkerOverlay {
    pub fn new(worker_id: &str) -> Self {
        Self {
            worker_id: worker_id.to_string(),
            changes: HashMap::new(),
            _committed: false,
        }
    }

    /// Record a file write
    pub fn write(&mut self, path: PathBuf, original: Option<String>, new_content: String) {
        self.changes.insert(
            path,
            OverlayChange {
                original_content: original,
                new_content,
                worker_id: self.worker_id.clone(),
            },
        );
    }

    /// Get pending changes
    pub fn changes(&self) -> &HashMap<PathBuf, OverlayChange> {
        &self.changes
    }

    /// Check if overlay has changes
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    /// Get files modified
    pub fn files_modified(&self) -> Vec<PathBuf> {
        self.changes.keys().cloned().collect()
    }
}

/// Commit coordinator for atomic swarm commits
pub struct CommitCoordinator {
    overlays: Vec<WorkerOverlay>,
}

impl CommitCoordinator {
    pub fn new() -> Self {
        Self {
            overlays: Vec::new(),
        }
    }

    /// Add a worker's overlay
    pub fn add_overlay(&mut self, overlay: WorkerOverlay) {
        self.overlays.push(overlay);
    }

    /// Check for conflicts (same file modified by multiple workers)
    pub fn detect_conflicts(&self) -> Vec<(PathBuf, Vec<String>)> {
        let mut file_workers: HashMap<PathBuf, Vec<String>> = HashMap::new();
        
        for overlay in &self.overlays {
            for path in overlay.files_modified() {
                file_workers
                    .entry(path)
                    .or_default()
                    .push(overlay.worker_id.clone());
            }
        }
        
        file_workers
            .into_iter()
            .filter(|(_, workers)| workers.len() > 1)
            .collect()
    }

    /// Commit all overlays atomically
    pub fn commit_all(&self) -> Result<CommitResult, RollbackReason> {
        let conflicts = self.detect_conflicts();
        
        if !conflicts.is_empty() {
            return Err(RollbackReason::UnresolvedConflicts(conflicts));
        }
        
        let mut files_written = Vec::new();
        
        // Write all changes
        for overlay in &self.overlays {
            for (path, change) in overlay.changes() {
                if let Err(e) = std::fs::write(path, &change.new_content) {
                    // Rollback already-written files
                    self.rollback(&files_written);
                    return Err(RollbackReason::WriteError(path.clone(), e.to_string()));
                }
                files_written.push((path.clone(), change.original_content.clone()));
            }
        }
        
        Ok(CommitResult {
            files_written: files_written.len(),
            workers_committed: self.overlays.len(),
        })
    }

    /// Rollback written files
    fn rollback(&self, files: &[(PathBuf, Option<String>)]) {
        for (path, original) in files {
            if let Some(content) = original {
                let _ = std::fs::write(path, content);
            } else {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    /// Validate all overlays can be committed
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        
        for (file, workers) in self.detect_conflicts() {
            errors.push(ValidationError::Conflict {
                file,
                workers,
            });
        }
        
        errors
    }
}

impl Default for CommitCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct CommitResult {
    pub files_written: usize,
    pub workers_committed: usize,
}

#[derive(Debug)]
pub enum RollbackReason {
    UnresolvedConflicts(Vec<(PathBuf, Vec<String>)>),
    WriteError(PathBuf, String),
    ValidationFailed(Vec<ValidationError>),
}

#[derive(Debug)]
pub enum ValidationError {
    Conflict { file: PathBuf, workers: Vec<String> },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_write() {
        let mut overlay = WorkerOverlay::new("worker-1");
        overlay.write(
            PathBuf::from("test.rs"),
            Some("old".to_string()),
            "new".to_string(),
        );
        
        assert!(overlay.has_changes());
        assert_eq!(overlay.files_modified().len(), 1);
    }

    #[test]
    fn test_conflict_detection() {
        let mut coord = CommitCoordinator::new();
        
        let mut overlay1 = WorkerOverlay::new("worker-1");
        overlay1.write(PathBuf::from("shared.rs"), None, "content1".to_string());
        
        let mut overlay2 = WorkerOverlay::new("worker-2");
        overlay2.write(PathBuf::from("shared.rs"), None, "content2".to_string());
        
        coord.add_overlay(overlay1);
        coord.add_overlay(overlay2);
        
        let conflicts = coord.detect_conflicts();
        assert_eq!(conflicts.len(), 1);
    }

    #[test]
    fn test_no_conflict() {
        let mut coord = CommitCoordinator::new();
        
        let mut overlay1 = WorkerOverlay::new("worker-1");
        overlay1.write(PathBuf::from("a.rs"), None, "content1".to_string());
        
        let mut overlay2 = WorkerOverlay::new("worker-2");
        overlay2.write(PathBuf::from("b.rs"), None, "content2".to_string());
        
        coord.add_overlay(overlay1);
        coord.add_overlay(overlay2);
        
        let conflicts = coord.detect_conflicts();
        assert!(conflicts.is_empty());
    }
}
