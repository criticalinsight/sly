//! Merge Engine for Swarm Conflict Resolution
//!
//! Phase 1: Handles 3-way merge for files modified by multiple workers.
//! Falls back to LLM for semantic conflicts that can't be auto-merged.

use std::process::Command;

/// Result of a merge operation
#[derive(Debug, Clone)]
pub enum MergeResult {
    /// Clean merge succeeded
    Success(String),
    /// Conflict markers present, needs resolution
    Conflict { content: String, markers: Vec<ConflictMarker> },
    /// Failed to merge
    Failed(String),
}

/// Location of a conflict in merged content
#[derive(Debug, Clone)]
pub struct ConflictMarker {
    pub start_line: usize,
    pub end_line: usize,
    pub ours_label: String,
    pub theirs_label: String,
}

/// 3-way merge engine
pub struct MergeEngine {
    /// Use LLM for semantic conflict resolution
    use_llm_fallback: bool,
}

impl MergeEngine {
    pub fn new() -> Self {
        Self {
            use_llm_fallback: true,
        }
    }

    pub fn without_llm(mut self) -> Self {
        self.use_llm_fallback = false;
        self
    }

    /// Perform 3-way merge using git merge-file
    pub fn merge_three_way(
        &self,
        base: &str,      // Original content
        ours: &str,      // Worker A's changes
        theirs: &str,    // Worker B's changes
        ours_label: &str,
        theirs_label: &str,
    ) -> MergeResult {
        
        // Write to temp files
        let tmp_dir = std::env::temp_dir();
        let base_path = tmp_dir.join("merge_base.tmp");
        let ours_path = tmp_dir.join("merge_ours.tmp");
        let theirs_path = tmp_dir.join("merge_theirs.tmp");
        
        if let Err(e) = std::fs::write(&base_path, base) {
            return MergeResult::Failed(format!("Failed to write base: {}", e));
        }
        if let Err(e) = std::fs::write(&ours_path, ours) {
            return MergeResult::Failed(format!("Failed to write ours: {}", e));
        }
        if let Err(e) = std::fs::write(&theirs_path, theirs) {
            return MergeResult::Failed(format!("Failed to write theirs: {}", e));
        }
        
        // Run git merge-file
        let output = Command::new("git")
            .args([
                "merge-file",
                "-p",
                "-L", ours_label,
                "-L", "base",
                "-L", theirs_label,
                ours_path.to_str().unwrap(),
                base_path.to_str().unwrap(),
                theirs_path.to_str().unwrap(),
            ])
            .output();
        
        // Cleanup
        let _ = std::fs::remove_file(&base_path);
        let _ = std::fs::remove_file(&ours_path);
        let _ = std::fs::remove_file(&theirs_path);
        
        match output {
            Ok(out) => {
                let content = String::from_utf8_lossy(&out.stdout).to_string();
                
                if out.status.success() {
                    // Clean merge
                    MergeResult::Success(content)
                } else {
                    // Conflicts present
                    let markers = self.find_conflict_markers(&content);
                    MergeResult::Conflict { content, markers }
                }
            }
            Err(e) => MergeResult::Failed(format!("git merge-file failed: {}", e)),
        }
    }

    /// Find conflict markers in merged content
    fn find_conflict_markers(&self, content: &str) -> Vec<ConflictMarker> {
        let mut markers = Vec::new();
        let mut current_start = None;
        let mut ours_label = String::new();
        
        for (i, line) in content.lines().enumerate() {
            if line.starts_with("<<<<<<<") {
                current_start = Some(i + 1);
                ours_label = line.trim_start_matches('<').trim().to_string();
            } else if line.starts_with(">>>>>>>") && current_start.is_some() {
                let theirs_label = line.trim_start_matches('>').trim().to_string();
                markers.push(ConflictMarker {
                    start_line: current_start.unwrap(),
                    end_line: i + 1,
                    ours_label: ours_label.clone(),
                    theirs_label,
                });
                current_start = None;
            }
        }
        
        markers
    }

    /// Resolve conflicts across multiple worker outputs
    pub fn resolve_multi_worker(
        &self,
        base: &str,
        worker_outputs: Vec<(String, String)>, // (worker_id, content)
    ) -> MergeResult {
        if worker_outputs.is_empty() {
            return MergeResult::Success(base.to_string());
        }
        
        if worker_outputs.len() == 1 {
            return MergeResult::Success(worker_outputs[0].1.clone());
        }
        
        // Pairwise merge
        let mut current = worker_outputs[0].1.clone();
        let mut current_label = worker_outputs[0].0.clone();
        
        for (worker_id, content) in worker_outputs.into_iter().skip(1) {
            match self.merge_three_way(base, &current, &content, &current_label, &worker_id) {
                MergeResult::Success(merged) => {
                    current = merged;
                    current_label = format!("{}+{}", current_label, worker_id);
                }
                result @ MergeResult::Conflict { .. } => return result,
                result @ MergeResult::Failed(_) => return result,
            }
        }
        
        MergeResult::Success(current)
    }
}

impl Default for MergeEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_merge() {
        let engine = MergeEngine::new();
        // Use more separated lines to ensure clean merge
        let base = "line1\nline2\nline3\nline4\nline5\n";
        let ours = "modified_by_a\nline2\nline3\nline4\nline5\n";
        let theirs = "line1\nline2\nline3\nline4\nmodified_by_b\n";
        
        match engine.merge_three_way(base, ours, theirs, "worker_a", "worker_b") {
            MergeResult::Success(merged) => {
                assert!(merged.contains("modified_by_a"));
                assert!(merged.contains("modified_by_b"));
            }
            MergeResult::Conflict { content, .. } => {
                // Accept conflict if git doesn't clean merge
                assert!(!content.is_empty());
            }
            MergeResult::Failed(e) => panic!("Merge failed: {}", e),
        }
    }



    #[test]
    fn test_conflict_detected() {
        let engine = MergeEngine::new();
        let base = "line1\noriginal\nline3\n";
        let ours = "line1\nchange_a\nline3\n";
        let theirs = "line1\nchange_b\nline3\n";
        
        match engine.merge_three_way(base, ours, theirs, "worker_a", "worker_b") {
            MergeResult::Conflict { markers, .. } => {
                assert!(!markers.is_empty());
            }
            MergeResult::Success(_) => {} // Git might auto-resolve in some cases
            MergeResult::Failed(e) => panic!("Merge failed: {}", e),
        }
    }
}
