use crate::mcp::local::LocalMcp;
use crate::error::Result;
use async_trait::async_trait;
use serde_json::Value; // Added this import
use std::process::Stdio;
use tokio::process::Command;
use colored::Colorize;

pub struct SystemMcp;

#[async_trait]
impl LocalMcp for SystemMcp {
    fn name(&self) -> &str { "system" }

    fn tool_definitions(&self) -> String {
        r#"
<tool_def>
    <name>sly_run_task</name>
    <description>Spawns a recursive sub-process of Sly to execute a specific task autonomously. Use this to break down complex goals into smaller, isolated steps. Each sub-task runs in its own ephemeral container.</description>
    <parameters>
        {
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "The specific task instruction for the sub-agent."
                }
            },
            "required": ["task"]
        }
    </parameters>
</tool_def>
<tool_def>
    <name>sly_evolve</name>
    <description>Manages the self-evolution of the agent. Use this to verify code changes (compile check) or signal readiness for a restart after successful modifications.</description>
    <parameters>
        {
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["check", "restart"],
                    "description": "The evolution action to perform. 'check' runs cargo check. 'restart' signals the supervisor to reboot."
                }
            },
            "required": ["action"]
        }
    </parameters>
</tool_def>
"#.trim().to_string()
    }

    async fn execute(&self, tool_name: &str, args: &Value) -> Result<Value> {
        match tool_name {
            "sly_run_task" => {
                let task = args["task"].as_str().unwrap_or_default();
                if task.is_empty() {
                    return Ok(Value::String("Error: Task argument is empty.".to_string()));
                }

                println!("   {} Spawning Sub-Sly: {}", "🧬".cyan(), task);
                let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("sly"));

                let output = Command::new(exe)
                    .arg("session")
                    .arg(task)
                    .arg("--ephemeral") // Ensure isolation
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped()) // Capture stderr too
                    .output()
                    .await
                    .map_err(|e| crate::error::SlyError::Io(e))?;

                // We capture stdout as the result. 
                // Since CliAdapter ensures output to stdout, this should work.
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                
                let result_str = if output.status.success() {
                    format!("Sub-Task Output:\n{}\n", stdout)
                } else {
                    format!("Sub-Task Failed (Exit {}):\nSTDOUT:\n{}\nSTDERR:\n{}", 
                        output.status.code().unwrap_or(-1), stdout, stderr)
                };

                Ok(Value::String(result_str))
            }
            "sly_evolve" => {
                let action = args["action"].as_str().unwrap_or("check");
                
                if action == "restart" {
                    return Ok(Value::String("RESTART_REQUIRED".to_string()));
                }

                println!("   {} Running Evolution Check (Cargo)...", "🧬".cyan());
                
                let output = Command::new("cargo")
                    .arg("check")
                    .arg("--lib") // Check library integrity
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
                    .map_err(|e| crate::error::SlyError::Io(e))?;

                let logout = String::from_utf8_lossy(&output.stdout);
                let logerr = String::from_utf8_lossy(&output.stderr);

                if output.status.success() {
                     Ok(Value::String(format!("Evolution Check Passed:\n{}", logerr))) // Cargo check writes to stderr mostly
                } else {
                     Ok(Value::String(format!("Evolution Check Failed:\nSTDOUT:\n{}\nSTDERR:\n{}", logout, logerr)))
                }
            }
            _ => Ok(Value::String(format!("Unknown system tool: {}", tool_name))),
        }
    }
}
