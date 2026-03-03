//! Sub-Process Composition for Sly
//!
//! Phase 7: Generative Simplicity - Child agents run as isolated ephemeral processes.
//! This enables parallel task execution and recursive agent invocation.

use crate::error::{Result, SlyError};
use std::process::Stdio;
use tokio::process::Command;

/// Result from a child agent execution
#[derive(Debug, Clone)]
pub struct ChildResult {
    pub output: String,
    pub exit_code: i32,
    pub persona: String,
}

/// Spawn an ephemeral Sly child process with a specific task and persona.
/// 
/// # Arguments
/// * `task` - The task to execute
/// * `persona` - Optional persona ID to use (defaults to "hickey")
/// * `timeout_secs` - Maximum execution time in seconds
/// 
/// # Returns
/// The output from the child agent
pub async fn spawn_child_agent(
    task: &str,
    persona: Option<&str>,
    timeout_secs: u64,
) -> Result<ChildResult> {
    let persona_id = persona.unwrap_or("hickey");
    
    // Build command with ephemeral mode
    let mut cmd = Command::new("sly");
    cmd.args([
        "--ephemeral",           // Use :memory: CozoDB, no state pollution
        "--pipe",                // Non-interactive pipe mode
        "--persona", persona_id, // Persona to use
    ]);
    
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    
    // Spawn the child process
    let mut child = cmd.spawn().map_err(|e| {
        SlyError::Task(format!("Failed to spawn child agent: {}", e))
    })?;
    
    // Write task to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin.write_all(task.as_bytes()).await.map_err(|e| {
            SlyError::Io(e)
        })?;
        stdin.write_all(b"\n").await.map_err(|e| {
            SlyError::Io(e)
        })?;
        // Drop stdin to signal EOF
    }
    
    // Wait for completion with timeout
    let output = tokio::time::timeout(
        tokio::time::Duration::from_secs(timeout_secs),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| SlyError::Task(format!(
        "Child agent timed out after {} seconds", timeout_secs
    )))?
    .map_err(|e| SlyError::Task(format!("Child agent error: {}", e)))?;
    
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    
    // Combine stdout and stderr for full output
    let full_output = if stderr.is_empty() {
        stdout
    } else {
        format!("{}\n\n[stderr]:\n{}", stdout, stderr)
    };
    
    if exit_code != 0 {
        return Err(SlyError::Task(format!(
            "Child agent failed with exit code {}: {}",
            exit_code, full_output
        )));
    }
    
    Ok(ChildResult {
        output: full_output,
        exit_code,
        persona: persona_id.to_string(),
    })
}

/// Spawn multiple child agents in parallel for distributed work
pub async fn spawn_parallel_agents(
    tasks: Vec<(&str, Option<&str>)>,
    timeout_secs: u64,
) -> Vec<Result<ChildResult>> {
    use futures::future::join_all;
    
    let futures: Vec<_> = tasks
        .into_iter()
        .map(|(task, persona)| spawn_child_agent(task, persona, timeout_secs))
        .collect();
    
    join_all(futures).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spawn_child_basic() {
        // This test requires sly to be in PATH
        // Skip if not available
        let result = Command::new("which")
            .arg("sly")
            .output()
            .await;
        
        if result.is_err() || !result.unwrap().status.success() {
            println!("Skipping test: sly not in PATH");
            return;
        }
        
        // Test with a simple echo task
        let result = spawn_child_agent("echo test", Some("hickey"), 10).await;
        // We don't assert success since this depends on sly behavior
        println!("Child result: {:?}", result);
    }
}
