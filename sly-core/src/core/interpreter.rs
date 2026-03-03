use std::sync::Arc;
use crate::core::state::GlobalState;
use crate::core::agent;
use crate::io::events::Impulse;
use crate::error::Result;
use colored::*;

pub struct ImpulseInterpreter;

impl ImpulseInterpreter {
    pub async fn interpret(impulse: Impulse, state: Arc<GlobalState>) {
        // Record INTENT (Hickey: Data as the Truth)
        let op_name = format!("{:?}", impulse);
        let _ = state.memory_raw.record_event(
            &format!("EXEC:{}", op_name), 
            serde_json::json!({ "impulse": op_name })
        );

        let res = match impulse {
            Impulse::InitiateSession(input) => handle_initiate_session(input, state.clone()).await,
            Impulse::VisualInput(path, prompt) => handle_visual_input(path, prompt, state.clone()).await,
            Impulse::ThinkStep(session_id) => handle_think(session_id, state.clone()).await,
            Impulse::Observation(session_id, obs) => handle_observe(session_id, obs, state.clone()).await,
            Impulse::FileSystemEvent(event) => handle_fs_event(event, state.clone()).await,
            Impulse::ThoughtStream(session_id, query) => handle_thought_stream(session_id, query, state.clone()).await,
            Impulse::Undo(session_id) => handle_undo(session_id, state.clone()).await,
            Impulse::ExecuteWorkflow(name) => {
                let s = state.clone();
                tokio::spawn(async move {
                    let _ = crate::core::workflow::execute_workflow(&name, s).await;
                });
                Ok(())
            }
            Impulse::Terminate | Impulse::SystemInterrupt => handle_shutdown(state.clone()).await,
            Impulse::Error(e) => {
                eprintln!("{} System Error Impulse: {}", "⚠️".red(), e);
                Ok(())
            }
        };

        if let Err(e) = res {
            eprintln!("{} Impulse execution failed: {}", "⚠️".red(), e);
            let _ = state.memory_raw.record_event(
                &format!("ERROR:{}", op_name),
                serde_json::json!({ "error": e.to_string() })
            );
        }
    }
}

async fn handle_initiate_session(input: String, state: Arc<GlobalState>) -> Result<()> {
    let session = crate::core::session::AgentSession::new(input);
    let session_id = session.id.clone();
    state.memory_raw.create_session(&session).await?;
    println!("{} Persistent Session Initiated: {}", "🔋".green(), session_id);
    
    agent::step_agent_session(
        session_id, 
        state.clone(),
        state.config.max_autonomous_loops
    ).await;
    Ok(())
}

async fn handle_visual_input(path: String, prompt: String, state: Arc<GlobalState>) -> Result<()> {
    // 1. Create session with prompt
    let session = crate::core::session::AgentSession::new(prompt.clone());
    let session_id = session.id.clone();
    
    // 2. Attach visual artifact reference (path) to session metadata if needed
    // For now, we'll pass the path directly to the agent step
    state.memory_raw.create_session(&session).await?;
    
    println!("{} Visual Session Initiated: {} (Image: {})", "👁️".cyan(), session_id, path);
    
    // 3. Step agent with multimodal context
    // We need to modify step_agent_session to support image paths OR 
    // simply encode the image into a message and update session.
    
    let base64 = crate::io::vision::encode_image(&std::path::PathBuf::from(path))?;
    let multimodal_msg = format!("[MULTIMODAL_IMAGE:{}] {}", base64, prompt);
    
    // Update session with multimodal marker
    let session = session.with_message(multimodal_msg);
    state.memory_raw.update_session(&session).await?;

    agent::step_agent_session(
        session_id, 
        state.clone(),
        state.config.max_autonomous_loops
    ).await;
    Ok(())
}

async fn handle_think(session_id: String, state: Arc<GlobalState>) -> Result<()> {
    agent::step_agent_session(
        session_id, 
        state.clone(),
        state.config.max_autonomous_loops
    ).await;
    Ok(())
}

async fn handle_observe(session_id: String, observation: String, state: Arc<GlobalState>) -> Result<()> {
    if let Ok(Some(session)) = state.memory_raw.get_session(&session_id).await {
        let session = session.with_message(observation);
        state.memory_raw.update_session(&session).await?;
        
        agent::step_agent_session(
            session_id, 
            state.clone(),
            state.config.max_autonomous_loops
        ).await;
    }
     Ok(())
}

async fn handle_undo(session_id: String, state: Arc<GlobalState>) -> Result<()> {
    println!("{} [Undo] Rolling back session {}", "⏪".yellow(), session_id);
    if let Ok(Some(_)) = state.memory_raw.rollback_session(&session_id).await {
        
        // Also rollback the OverlayFS to clean up speculative writes
        state.overlay.rollback().map_err(|e| crate::error::SlyError::Overlay(e.to_string()))?;
        println!("{} Session and OverlayFS reverted.", "✅".green());
    }
    Ok(())
}

async fn handle_fs_event(event: notify::Event, state: Arc<GlobalState>) -> Result<()> {
    let paths: Vec<std::path::PathBuf> = event.paths;
    if !paths.is_empty() {
        println!("{} Executing Batch Ingest Impulse: {} paths", "📝".blue(), paths.len());
        crate::knowledge::ingest_batch(&state.memory_raw, &paths).await?;
    }
    Ok(())
}

async fn handle_thought_stream(session_id: String, query: String, state: Arc<GlobalState>) -> Result<()> {
    tokio::spawn(async move {
        println!("{} [Swarm] Spawning Parallel Analysis: {}", "🐝".yellow(), query);
        let _ = agent::step_thought_analysis(
            session_id,
            query,
            state.clone(),
        ).await;
    });
    Ok(())
}

async fn handle_shutdown(_state: Arc<GlobalState>) -> Result<()> {
    println!("{}", "🛑 Shutdown signal processed. Initializing cleanup...".red());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_state(name: &str) -> Arc<GlobalState> {
        let temp_dir = std::env::temp_dir().join(format!("s_p6_test_{}", name));
        if temp_dir.exists() { let _ = std::fs::remove_dir_all(&temp_dir); }
        std::fs::create_dir_all(&temp_dir).unwrap();
        let path = temp_dir.join("cozo").to_string_lossy().to_string();
        
        let state = GlobalState::new_for_tests(&path).await.expect("Failed to create state");
        Arc::new(state)
    }

    #[tokio::test]
    async fn test_interpret_logs_intent() -> Result<()> {
        let state = setup_state("logs_intent").await;
        let impulse = Impulse::Error("test_error_99".to_string());
        
        ImpulseInterpreter::interpret(impulse, state.clone()).await;
        
        let res = state.memory_raw.backend_run_script("?[op] := *event_log{op}")?;
        assert!(res.rows.iter().any(|r| {
            let s = format!("{:?}", r[0]);
            s.contains("EXEC") && s.contains("Error") && s.contains("test_error_99")
        }));
        Ok(())
    }
}
