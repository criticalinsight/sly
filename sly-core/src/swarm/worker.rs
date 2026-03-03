//! Swarm Worker
//!
//! An isolated worker agent that executes a single task.
//! Workers are stateless and ephemeral - they use :memory: CozoDB.

use crate::swarm::task::{SwarmTask, SwarmResult};
use std::time::Instant;
use tokio::process::Command;
use std::process::Stdio;

/// Execute a task as a worker (isolated subprocess)
pub struct SwarmWorker {
    _worker_id: String,
}

impl SwarmWorker {
    pub fn new(id: &str) -> Self {
        Self {
            _worker_id: id.to_string(),
        }
    }

    /// Execute a task in an isolated subprocess
    pub async fn execute(&self, task: SwarmTask) -> SwarmResult {
        let start = Instant::now();
        
        // Build command args
        let mut args = vec![
            "--ephemeral".to_string(),
            "--pipe".to_string(),
            "session".to_string(),
            task.instruction.clone(),
        ];
        
        if let Some(ref persona) = task.persona {
            args.insert(0, persona.clone());
            args.insert(0, "--persona".to_string());
        }
        
        // Spawn child sly process
        let mut cmd = Command::new("sly");
        cmd.args(&args);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        
        let result = tokio::time::timeout(
            tokio::time::Duration::from_secs(task.timeout_secs),
            cmd.output(),
        )
        .await;
        
        let duration_ms = start.elapsed().as_millis() as u64;
        
        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                
                if output.status.success() {
                    SwarmResult::success(
                        &task.id,
                        stdout,
                        task.target_files.clone(),
                        duration_ms,
                    )
                } else {
                    SwarmResult::failure(
                        &task.id,
                        format!("Exit {}: {}", output.status.code().unwrap_or(-1), stderr),
                        duration_ms,
                    )
                }
            }
            Ok(Err(e)) => {
                SwarmResult::failure(&task.id, format!("Spawn error: {}", e), duration_ms)
            }
            Err(_) => {
                SwarmResult::failure(
                    &task.id,
                    format!("Timeout after {} seconds", task.timeout_secs),
                    duration_ms,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worker_creation() {
        let worker = SwarmWorker::new("test-1");
        assert_eq!(worker.worker_id, "test-1");
    }
}
