use crate::error::{Result, SlyError};
use crate::core::state::GlobalState;
use crate::core::bus::SlyEvent;
use std::sync::Arc;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::process::Stdio;

pub async fn execute_workflow(name: &str, state: Arc<GlobalState>) -> Result<()> {
    let path = format!(".agent/workflows/{}.md", name);
    println!("🚀 Executing workflow: {} (Path: {})", name, path);
    if !Path::new(&path).exists() {
        let _ = state.bus.publish(SlyEvent::Error("system".to_string(), format!("Workflow not found: {}", name))).await;
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| SlyError::Io(e))?;
    
    let _ = state.bus.publish(SlyEvent::SystemStatus(format!("Executing Workflow: /{}", name))).await;

    let lines: Vec<&str> = content.lines().collect();
    let mut steps = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].starts_with("```bash") {
            let mut code = String::new();
            i += 1;
            while i < lines.len() && !lines[i].starts_with("```") {
                code.push_str(lines[i]);
                code.push('\n');
                i += 1;
            }
            if !code.trim().is_empty() {
                steps.push(code);
            }
        }
        i += 1;
    }

    for (idx, code) in steps.iter().enumerate() {
        let step_num = idx + 1;
        let first_line = code.lines().next().unwrap_or("...");
        
        let _ = state.bus.publish(SlyEvent::Action("system".to_string(), format!("Step {}/{}: {}", step_num, steps.len(), first_line))).await;
        
        // 2. Spawn Command
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(code)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| SlyError::Io(e))?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        
        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let _bus_clone = state.bus.clone();
        tokio::spawn(async move {
            while let Ok(Some(line)) = stdout_reader.next_line().await {
                println!("   [WF] {}", line);
            }
        });

        let _bus_err = state.bus.clone();
        tokio::spawn(async move {
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                eprintln!("   [WF ERR] {}", line);
            }
        });

        let status = child.wait().await.map_err(|e| SlyError::Io(e))?;
        if !status.success() {
             let _ = state.bus.publish(SlyEvent::Error("system".to_string(), format!("Workflow step {} failed with status {}", step_num, status))).await;
             return Err(SlyError::Task(format!("Workflow failed at step {}", step_num)));
        }
    }

    let _ = state.bus.publish(SlyEvent::SystemStatus(format!("Workflow /{} completed successfully.", name))).await;
    Ok(())
}
