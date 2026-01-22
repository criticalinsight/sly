use crate::core::parser::{parse_action, AgentAction};
use crate::mcp::registry;
use colored::*;
use std::sync::Arc;
use std::collections::HashMap;


pub async fn step_agent_session(
    session_id: String, 
    memory: Arc<crate::memory::Memory>,
    cortex: Arc<crate::core::cortex::Cortex>,
    mcp_clients: Arc<tokio::sync::Mutex<HashMap<String, Arc<crate::mcp::client::McpClient>>>>,
    overlay: Arc<crate::safety::OverlayFS>,
    max_loops: usize,
) {
    let mut session = match memory.get_session(&session_id).await {
        Ok(Some(s)) => s,
        _ => return,
    };

    if session.depth >= max_loops {
        println!("{} Session {} reached max depth", "⚠️".red(), session_id);
        return;
    }

    // 1. Fetch Metadata once (Value-Oriented)
    let tool_metadata = registry::get_all_tool_metadata(&mcp_clients).await;
    let tool_defs = registry::get_tool_definitions(&tool_metadata).await;
    
    let full_context = session.messages.join("\n\n");
    let mut prompt = full_context;

    if session.depth == 0 {
        if !tool_defs.is_empty() {
            prompt = format!("{}\n\n{}", prompt, tool_defs);
        }
        // Inject Datalog Schema for grounding
        prompt = format!("{}\n\n## KNOWLEDGE GRAPH SCHEMA (Datalog Ready)\nNodes: `nodes {{ id => content, type, path, embedding }}`\nEdges: `edges {{ parent => child }}`\n", prompt);
    }

    println!("{} [Session {}] Thinking...", "🤔".magenta(), session_id);
    match cortex.generate(&prompt, crate::core::cortex::ThinkingLevel::High).await {
        Ok(response) => {
            println!("{}\n{}", "🤖 Sly (Managed Session):".green().bold(), response);
            
            // Functional Update
            let step_depth = session.depth;
            session = session.with_message(format!("**Sly (Step {}):**\n{}", step_depth, response.clone()))
                             .with_depth_increment();
            
            match parse_action(&response) {
                Ok(actions) => {
                    for action in actions {
                        // Pass ownership and get new session back
                        session = handle_action(action, session, &tool_metadata, overlay.clone()).await;
                    }
                }
                Err(e) => {
                    eprintln!("Parse error: {}", e);
                    session = session.with_status(crate::core::session::SessionStatus::Error(e.to_string()));
                }
            }
            let _ = memory.update_session(&session).await;
        }
        Err(e) => {
            eprintln!("Cortex error: {}", e);
        }
    }
}

// Update signature to use &[McpToolMetadata]
async fn handle_action(
    action: AgentAction, 
    session: crate::core::session::AgentSession, 
    tool_metadata: &[registry::McpToolMetadata],
    overlay: Arc<crate::safety::OverlayFS>,
) -> crate::core::session::AgentSession {
    match action {
        AgentAction::CallTool { tool_name, arguments } => {
            println!("{} 🛠️  Calling Tool: {}...", "⚙️".cyan(), tool_name);
            match registry::call_mcp_tool(tool_metadata, &tool_name, arguments).await {
                Ok(tool_output) => {
                    session.with_message(format!("**Observation (Tool '{}'):**\n```json\n{}\n```", tool_name, tool_output))
                }
                Err(e) => {
                    session.with_message(format!("**Observation (Error from '{}'):**\n{}", tool_name, e))
                }
            }
        }
        AgentAction::WriteFile { path, content } => {
             use crate::core::fs::{FileSystemAction, execute_action};
             let fs_action = FileSystemAction::Write { 
                 path: std::path::PathBuf::from(&path), 
                 content 
             };
             println!("{} 📝 FileSystemAction: {:?}", "💾".blue(), fs_action);
              // Unbundled execute_action
             match execute_action(&overlay, fs_action) {
                 Ok(_) => {
                     session.with_message(format!("**Observation:** Action successfully executed in OverlayFS."))
                 }
                 Err(e) => {
                     eprintln!("     {} Action Failed: {}", "⚠️".red(), e);
                     session.with_message(format!("**Observation (Error):** Failed to execute action: {}", e))
                 }
             }
        }
        AgentAction::ExecShell { command, .. } => {
             println!("{} 🐚 ExecShell: {}", "💻".blue(), command);
             match tokio::process::Command::new("sh").arg("-c").arg(&command).output().await {
                 Ok(output) => {
                     let result = format!("Exit Code: {}\nSTDOUT:\n{}\nSTDERR:\n{}", 
                        output.status.code().unwrap_or(-1), 
                        String::from_utf8_lossy(&output.stdout), 
                        String::from_utf8_lossy(&output.stderr));
                     session.with_message(format!("**Observation (Shell '{}'):**\n```\n{}\n```", command, result))
                 }
                 Err(e) => session.with_message(format!("**Observation (Error):** Command '{}' failed: {}", command, e)),
             }
        }
        AgentAction::QueryMemory { query: _query, .. } => {
            session.with_message("**Observation:** Memory query not yet implemented in unbundled OODA step.".to_string())
        }
        AgentAction::CommitOverlay { message } => {
            println!("{} 🚀 Committing Overlay: {}", "📦".green().bold(), message);
            match overlay.commit() {
                Ok(_) => {
                    session.with_message("**Observation:** Overlay committed to workspace successfully.".to_string())
                           .with_status(crate::core::session::SessionStatus::Completed)
                }
                Err(e) => {
                    session.with_message(format!("**Observation (Commit Error):** {}", e))
                }
            }
        }
        AgentAction::Answer { .. } => {
            session.with_status(crate::core::session::SessionStatus::Completed)
        }
        _ => session
    }
}

