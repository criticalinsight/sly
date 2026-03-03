//! Progress Streaming and Events
//!
//! Phase 5: Real-time progress events from workers to supervisor.

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Events emitted during swarm execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwarmEvent {
    /// Swarm execution started
    SwarmStarted {
        total_tasks: usize,
        max_workers: usize,
    },
    /// Worker started a task
    WorkerStarted {
        worker_id: String,
        task_id: String,
        instruction: String,
    },
    /// Worker progress update
    WorkerProgress {
        worker_id: String,
        task_id: String,
        percent: u8,
        message: String,
    },
    /// Worker completed a task
    WorkerCompleted {
        worker_id: String,
        task_id: String,
        success: bool,
        duration_ms: u64,
    },
    /// Conflict detected between workers
    ConflictDetected {
        file: String,
        workers: Vec<String>,
    },
    /// Merge operation performed
    MergePerformed {
        file: String,
        success: bool,
    },
    /// Batch completed
    BatchCompleted {
        batch_index: usize,
        success_count: usize,
        failure_count: usize,
    },
    /// Swarm execution complete
    SwarmComplete {
        total_tasks: usize,
        success_count: usize,
        failure_count: usize,
        conflicts_resolved: usize,
        duration_ms: u64,
    },
    /// Rollback triggered
    RollbackTriggered {
        reason: String,
        affected_workers: Vec<String>,
    },
}

/// Event channel for streaming progress
pub type EventSender = mpsc::Sender<SwarmEvent>;
pub type EventReceiver = mpsc::Receiver<SwarmEvent>;

/// Create a new event channel
pub fn event_channel(buffer: usize) -> (EventSender, EventReceiver) {
    mpsc::channel(buffer)
}

/// CLI progress printer
pub struct CliProgressPrinter;

impl CliProgressPrinter {
    pub async fn run(mut rx: EventReceiver) {
        use colored::*;
        
        while let Some(event) = rx.recv().await {
            match event {
                SwarmEvent::SwarmStarted { total_tasks, max_workers } => {
                    println!(
                        "{} Swarm started: {} tasks, {} workers",
                        "🐝".yellow(),
                        total_tasks,
                        max_workers
                    );
                }
                SwarmEvent::WorkerStarted { worker_id, task_id, .. } => {
                    println!(
                        "   {} {} started task {}",
                        "▶".cyan(),
                        worker_id,
                        task_id
                    );
                }
                SwarmEvent::WorkerProgress { worker_id, percent, message, .. } => {
                    println!(
                        "   {} {} [{}%] {}",
                        "⋯".blue(),
                        worker_id,
                        percent,
                        message
                    );
                }
                SwarmEvent::WorkerCompleted { worker_id, task_id, success, duration_ms } => {
                    let icon = if success { "✓".green() } else { "✗".red() };
                    println!(
                        "   {} {} completed {} ({}ms)",
                        icon,
                        worker_id,
                        task_id,
                        duration_ms
                    );
                }
                SwarmEvent::ConflictDetected { file, workers } => {
                    println!(
                        "   {} Conflict: {} modified by {:?}",
                        "⚠".yellow(),
                        file,
                        workers
                    );
                }
                SwarmEvent::MergePerformed { file, success } => {
                    let icon = if success { "⊕".green() } else { "⊖".red() };
                    println!("   {} Merged: {}", icon, file);
                }
                SwarmEvent::BatchCompleted { batch_index, success_count, failure_count } => {
                    println!(
                        "   {} Batch {}: {}/{} succeeded",
                        "◆".magenta(),
                        batch_index + 1,
                        success_count,
                        success_count + failure_count
                    );
                }
                SwarmEvent::SwarmComplete { success_count, failure_count, conflicts_resolved, duration_ms, .. } => {
                    println!(
                        "\n{} Complete: {} succeeded, {} failed, {} conflicts resolved ({}ms)",
                        "🏁".green(),
                        success_count,
                        failure_count,
                        conflicts_resolved,
                        duration_ms
                    );
                }
                SwarmEvent::RollbackTriggered { reason, .. } => {
                    println!("{} Rollback: {}", "⟲".red(), reason);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_channel() {
        let (tx, mut rx) = event_channel(10);
        
        tx.send(SwarmEvent::SwarmStarted { total_tasks: 5, max_workers: 2 })
            .await
            .unwrap();
        
        let event = rx.recv().await.unwrap();
        match event {
            SwarmEvent::SwarmStarted { total_tasks, .. } => {
                assert_eq!(total_tasks, 5);
            }
            _ => panic!("Wrong event type"),
        }
    }
}
