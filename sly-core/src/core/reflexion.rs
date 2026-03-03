use crate::error::Result;
use std::sync::Arc;
use colored::*;

pub async fn attempt_repair(
    session_id: &str,
    state: Arc<crate::core::state::GlobalState>,
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
    state.memory_raw.create_session(&session).await?;

    let _ = state.bus.publish(crate::core::bus::SlyEvent::Thought(session_id.to_string(), format!("🚑 Self-Correction Triggered for error in `{}`. Spawning sub-agent `{}`...", failed_command, repair_id))).await;

    // 3. Run the sub-agent for a limited depth (e.g., 5 steps)
    // We must Box::pin this to avoid E0733 recursion error
    let state_clone = state.clone();
    let repair_id_clone = repair_id.clone();

    Box::pin(async move {
        for _ in 0..5 {
            crate::core::agent::step_agent_session(
                repair_id_clone.clone(),
                state_clone.clone(),
                5 
            ).await;

            if let Ok(Some(s)) = state_clone.memory_raw.get_session(&repair_id_clone).await {
                if matches!(s.status, crate::core::session::SessionStatus::Completed) {
                    break;
                }
            }
        }
    }).await;

    // 4. Retrieve the result
    let session_final = state.memory_raw.get_session(&repair_id).await?.unwrap();
    let last_msg = session_final.messages.last().cloned().unwrap_or_default();

    println!("{} 🚑 Repair Sequence Complete. Result: {}", "✅".green(), last_msg.lines().last().unwrap_or("Unknown"));

    Ok(format!("**Reflexion Result (Sub-session {}):**\n{}", repair_id, last_msg))
}
