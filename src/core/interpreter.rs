use std::sync::Arc;
use crate::core::directives::Directive;
use crate::core::state::GlobalState;
use crate::core::agent;
use crate::core::bus::DirectiveHandler;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use colored::*;

pub struct DirectiveInterpreter;

impl DirectiveInterpreter {
    pub async fn interpret(directive: Directive, state: Arc<GlobalState>) {
        // Record INTENT (Hickey: Data as the Truth)
        let _ = state.memory_raw.record_event(
            &format!("EXEC:{}", directive.type_name), 
            directive.payload.clone()
        );

        if let Err(e) = state.bus.dispatch(&directive.type_name, directive.payload, state.clone()).await {
            eprintln!("{} Directive dispatch failed: {}", "⚠️".red(), e);
            let _ = state.memory_raw.record_event(
                &format!("ERROR:{}", directive.type_name),
                serde_json::json!({ "error": e.to_string() })
            );
        }
    }

    pub async fn register_core_handlers(state: Arc<GlobalState>) {
        let bus = &state.bus;
        bus.register("initiate_session", InitiateSessionHandler).await;
        bus.register("think", ThinkHandler).await;
        bus.register("observe", ObserveHandler).await;
        bus.register("ingest_file", IngestFileHandler).await;
        bus.register("fs_batch", FsBatchHandler).await;
        bus.register("bootstrap_skills", BootstrapSkillsHandler).await;
        bus.register("shutdown", ShutdownHandler).await;
    }
}

struct InitiateSessionHandler;
#[async_trait]
impl DirectiveHandler for InitiateSessionHandler {
    async fn handle(&self, data: Value, state: Arc<GlobalState>) -> Result<()> {
        let input = data["input"].as_str().unwrap_or_default().to_string();
        let session = crate::core::session::AgentSession::new(input);
        let session_id = session.id.clone();
        state.memory_raw.create_session(&session).await?;
        println!("{} Persistent Session Initiated: {}", "🔋".green(), session_id);
        
        // Unbundled call
        agent::step_agent_session(
            session_id, 
            state.memory_raw.clone(),
            state.cortex.clone(),
            state.mcp_clients.clone(),
            state.overlay.clone(),
            state.config.max_autonomous_loops
        ).await;
        Ok(())
    }
}

struct ThinkHandler;
#[async_trait]
impl DirectiveHandler for ThinkHandler {
    async fn handle(&self, data: Value, state: Arc<GlobalState>) -> Result<()> {
        let session_id = data["session_id"].as_str().unwrap_or_default().to_string();
        
        // Unbundled call
        agent::step_agent_session(
            session_id, 
            state.memory_raw.clone(),
            state.cortex.clone(),
            state.mcp_clients.clone(),
            state.overlay.clone(),
            state.config.max_autonomous_loops
        ).await;
        Ok(())
    }
}

struct ObserveHandler;
#[async_trait]
impl DirectiveHandler for ObserveHandler {
    async fn handle(&self, data: Value, state: Arc<GlobalState>) -> Result<()> {
        let session_id = data["session_id"].as_str().unwrap_or_default().to_string();
        let observation = data["observation"].as_str().unwrap_or_default().to_string();
        if let Ok(Some(session)) = state.memory_raw.get_session(&session_id).await {
            let session = session.with_message(observation);
            state.memory_raw.update_session(&session).await?;
            
            // Unbundled call
            agent::step_agent_session(
                session_id, 
                state.memory_raw.clone(),
                state.cortex.clone(),
                state.mcp_clients.clone(),
                state.overlay.clone(),
                state.config.max_autonomous_loops
            ).await;
        }
        Ok(())
    }
}

struct IngestFileHandler;
#[async_trait]
impl DirectiveHandler for IngestFileHandler {
    async fn handle(&self, data: Value, state: Arc<GlobalState>) -> Result<()> {
        let path_str = data["path"].as_str().unwrap_or_default();
        let path = std::path::PathBuf::from(path_str);
        println!("{} Executing Ingest Directive: {:?}", "📝".blue(), path);
        crate::knowledge::ingest_file(&state.memory_raw, &path).await?;
        Ok(())
    }
}

struct FsBatchHandler;
#[async_trait]
impl DirectiveHandler for FsBatchHandler {
    async fn handle(&self, data: Value, state: Arc<GlobalState>) -> Result<()> {
        let paths: Vec<std::path::PathBuf> = data["paths"].as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|p| p.as_str().map(std::path::PathBuf::from))
            .collect();
        
        if !paths.is_empty() {
            println!("{} Executing Batch Ingest Directive: {} paths", "📝".blue(), paths.len());
            crate::knowledge::ingest_batch(&state.memory_raw, &paths).await?;
        }
        Ok(())
    }
}

struct BootstrapSkillsHandler;
#[async_trait]
impl DirectiveHandler for BootstrapSkillsHandler {
    async fn handle(&self, _data: Value, state: Arc<GlobalState>) -> Result<()> {
        crate::knowledge::ensure_skills_loaded(&state.memory_raw).await?;
        println!("{} Skills DB Loaded (Data-Driven Bus)", "📦".purple());
        Ok(())
    }
}

struct ShutdownHandler;
#[async_trait]
impl DirectiveHandler for ShutdownHandler {
    async fn handle(&self, _data: Value, _state: Arc<GlobalState>) -> Result<()> {
        println!("{}", "🛑 Shutdown directive received. Initializing cleanup...".red());
        Ok(())
    }
}
