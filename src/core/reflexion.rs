use crate::error::Result;
use crate::core::state::SlyConfig;
use crate::memory::Memory;
use crate::core::agent::step_agent_session;
use std::sync::Arc;
use colored::*;

pub async fn attempt_repair(
    session_id: &str,
    _config: &SlyConfig,
    memory: Arc<Memory>,
    cortex: Arc<crate::core::cortex::Cortex>,
    mcp_clients: Arc<tokio::sync::Mutex<std::collections::HashMap<String, Arc<crate::mcp::client::McpClient>>>>,
    metadata_cache: Arc<tokio::sync::Mutex<Vec<crate::mcp::registry::McpToolMetadata>>>,
    overlay: Arc<crate::safety::OverlayFS>,
    telegram: Option<Arc<tokio::sync::Mutex<crate::io::telegram::TelegramClient>>>,
    failed_command: &str,
    stderr: &str,
) -> Result<String> {
    println!("{} 🚑 Initiating Repair Sequence for Session {}", "🏥".red().bold(), session_id);

    // 1. Create a child session ID
    let repair_id = format!("{}_repair_{}", session_id, uuid::Uuid::new_v4().to_string().chars().take(4).collect::<String>());

    // 2. Hydrate the repair session with a specific goal
    let goal = format!(
        "CRITICAL FAILURE DETECTED.\n\nCommand: `{}`\nError Output:\n```\n{}\n```\n\nYOUR MISSION:\n1. Analyze the error.\n2. Fix the code/configuration.\n3. Verify the fix by running the command again.\n\nIf you succeed, report 'FIXED'. If you fail, report 'UNABLE_TO_FIX'.",
        failed_command, stderr
    );

    let mut session = crate::core::session::AgentSession::new(goal.clone());
    session.id = repair_id.clone();
    memory.create_session(&session).await?;

    if let Some(tg) = &telegram {
         let _ = tg.lock().await.send_message(&format!("🚑 <b>Self-Correction Triggered</b>\n\nI encountered an error running <code>{}</code>.\n\nSpawning sub-agent <code>{}</code> to fix it...", failed_command, repair_id)).await;
    }

    // 3. Run the sub-agent for a limited depth (e.g., 5 steps)
    // We must Box::pin this to avoid E0733 recursion error
    let repair_id_clone = repair_id.clone();
    let memory_clone = memory.clone();
    let cortex_clone = cortex.clone();
    let mcp_clients_clone = mcp_clients.clone();
    let metadata_cache_clone = metadata_cache.clone();
    let overlay_clone = overlay.clone();
    let telegram_clone = telegram.clone();

    Box::pin(async move {
        for _ in 0..5 {
            step_agent_session(
                repair_id_clone.clone(),
                memory_clone.clone(),
                cortex_clone.clone(),
                mcp_clients_clone.clone(),
                metadata_cache_clone.clone(),
                overlay_clone.clone(),
                telegram_clone.clone(),
                5 
            ).await;

            if let Ok(Some(s)) = memory_clone.get_session(&repair_id_clone).await {
                if matches!(s.status, crate::core::session::SessionStatus::Completed) {
                    break;
                }
            }
        }
    }).await;

    // 4. Retrieve the result
    let session_final = memory.get_session(&repair_id).await?.unwrap();
    let last_msg = session_final.messages.last().cloned().unwrap_or_default();

    println!("{} 🚑 Repair Sequence Complete. Result: {}", "✅".green(), last_msg.lines().last().unwrap_or("Unknown"));

    Ok(format!("**Reflexion Result (Sub-session {}):**\n{}", repair_id, last_msg))
}
