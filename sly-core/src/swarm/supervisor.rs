//! Swarm Supervisor
//!
//! Orchestrates multiple worker agents for parallel task execution.
//! Handles task distribution, result aggregation, and conflict resolution.

use crate::swarm::task::{SwarmTask, SwarmResult};
use crate::swarm::worker::SwarmWorker;
use futures::future::join_all;
use std::collections::HashMap;

/// Supervisor that orchestrates swarm workers
pub struct SwarmSupervisor {
    /// Maximum concurrent workers
    max_workers: usize,
    /// Default persona for workers
    default_persona: Option<String>,
}

impl SwarmSupervisor {
    pub fn new(max_workers: usize) -> Self {
        Self {
            max_workers,
            default_persona: None,
        }
    }

    pub fn with_persona(mut self, persona: &str) -> Self {
        self.default_persona = Some(persona.to_string());
        self
    }

    /// Distribute tasks to workers and collect results
    pub async fn distribute(&self, tasks: Vec<SwarmTask>) -> Vec<SwarmResult> {
        // Apply default persona to tasks without one
        let tasks: Vec<SwarmTask> = tasks
            .into_iter()
            .map(|mut t| {
                if t.persona.is_none() {
                    t.persona = self.default_persona.clone();
                }
                t
            })
            .collect();

        // Chunk tasks by max_workers for batching
        let mut all_results = Vec::new();
        
        for (batch_idx, batch) in tasks.chunks(self.max_workers).enumerate() {
            println!("🐝 Swarm batch {}: {} tasks", batch_idx + 1, batch.len());
            
            let futures: Vec<_> = batch
                .iter()
                .enumerate()
                .map(|(i, task)| {
                    let worker = SwarmWorker::new(&format!("worker-{}-{}", batch_idx, i));
                    let task = task.clone();
                    async move { worker.execute(task).await }
                })
                .collect();

            let batch_results = join_all(futures).await;
            all_results.extend(batch_results);
        }

        all_results
    }

    /// Partition a large task into subtasks
    pub fn partition_task(&self, instruction: &str, files: Vec<String>) -> Vec<SwarmTask> {
        files
            .into_iter()
            .enumerate()
            .map(|(i, file)| {
                SwarmTask::new(
                    &format!("subtask-{}", i),
                    &format!("{}\n\nTarget file: {}", instruction, file),
                )
                .with_files(vec![file])
            })
            .collect()
    }

    /// Aggregate results and detect conflicts
    pub fn aggregate(&self, results: &[SwarmResult]) -> AggregatedResult {
        let mut success_count = 0;
        let mut failure_count = 0;
        let mut files_modified: HashMap<String, Vec<String>> = HashMap::new();
        let mut total_duration_ms = 0u64;
        let mut outputs = Vec::new();

        for result in results {
            if result.success {
                success_count += 1;
                outputs.push(result.output.clone());
            } else {
                failure_count += 1;
            }
            total_duration_ms += result.duration_ms;

            // Track file modifications for conflict detection
            for file in &result.files_modified {
                files_modified
                    .entry(file.clone())
                    .or_default()
                    .push(result.task_id.clone());
            }
        }

        // Detect conflicts (files modified by multiple tasks)
        let conflicts: Vec<String> = files_modified
            .iter()
            .filter(|(_, tasks)| tasks.len() > 1)
            .map(|(file, _)| file.clone())
            .collect();

        AggregatedResult {
            total_tasks: results.len(),
            success_count,
            failure_count,
            conflicts,
            total_duration_ms,
            combined_output: outputs.join("\n---\n"),
        }
    }
}

/// Aggregated result from swarm execution
#[derive(Debug)]
pub struct AggregatedResult {
    pub total_tasks: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub conflicts: Vec<String>,
    pub total_duration_ms: u64,
    pub combined_output: String,
}

impl AggregatedResult {
    pub fn summary(&self) -> String {
        let conflict_msg = if self.conflicts.is_empty() {
            "No conflicts".to_string()
        } else {
            format!("⚠️ {} file conflicts", self.conflicts.len())
        };

        format!(
            "Swarm Complete: {}/{} succeeded, {} failed. {}. Duration: {}ms",
            self.success_count,
            self.total_tasks,
            self.failure_count,
            conflict_msg,
            self.total_duration_ms
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supervisor_creation() {
        let sup = SwarmSupervisor::new(4).with_persona("hickey");
        assert_eq!(sup.max_workers, 4);
        assert_eq!(sup.default_persona, Some("hickey".to_string()));
    }

    #[test]
    fn test_partition_task() {
        let sup = SwarmSupervisor::new(4);
        let tasks = sup.partition_task("Refactor", vec!["a.rs".into(), "b.rs".into()]);
        assert_eq!(tasks.len(), 2);
        assert!(tasks[0].instruction.contains("a.rs"));
    }

    #[test]
    fn test_aggregate_no_conflicts() {
        let sup = SwarmSupervisor::new(4);
        let results = vec![
            SwarmResult::success("t1", "ok".into(), vec!["a.rs".into()], 100),
            SwarmResult::success("t2", "ok".into(), vec!["b.rs".into()], 100),
        ];
        let agg = sup.aggregate(&results);
        assert!(agg.conflicts.is_empty());
        assert_eq!(agg.success_count, 2);
    }

    #[test]
    fn test_aggregate_with_conflicts() {
        let sup = SwarmSupervisor::new(4);
        let results = vec![
            SwarmResult::success("t1", "ok".into(), vec!["a.rs".into()], 100),
            SwarmResult::success("t2", "ok".into(), vec!["a.rs".into()], 100), // Same file!
        ];
        let agg = sup.aggregate(&results);
        assert_eq!(agg.conflicts.len(), 1);
        assert_eq!(agg.conflicts[0], "a.rs");
    }
}
