use std::sync::Arc;
// use tokio::sync::RwLock;
use crate::memory::MemoryStore;
use crate::safety::OverlayFS;
use super::cortex::Cortex;
use std::collections::HashMap;


use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SlyRole {
    Supervisor,
    #[default]
    Executor,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SlyConfig {
    pub project_name: String,
    pub primary_model: String,
    pub fallback_model: String,
    #[serde(default)]
    pub autonomous_mode: bool,
    #[serde(default)]
    pub max_autonomous_loops: usize,
    #[serde(default)]
    pub role: SlyRole,
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
}

impl Default for SlyConfig {
    fn default() -> Self {
        Self {
            project_name: "sly".to_string(),
            primary_model: "gemini-3-flash".to_string(),
            fallback_model: "gemini-3-flash".to_string(),
            autonomous_mode: true,
            max_autonomous_loops: 50,
            role: SlyRole::Executor,
            mcp_servers: HashMap::new(),
        }
    }
}

// use super::session::SessionStore; // Removed Phase 5

#[derive(Clone)]
pub struct GlobalState {
    pub config: Arc<SlyConfig>,
    pub memory: Arc<dyn MemoryStore>,
    pub memory_raw: Arc<crate::memory::Memory>,
    pub overlay: Arc<OverlayFS>,
    pub cortex: Arc<Cortex>,
    pub bus: Arc<crate::core::bus::DirectiveBus>,
    pub mcp_clients: Arc<tokio::sync::Mutex<HashMap<String, Arc<crate::mcp::client::McpClient>>>>,
}

impl GlobalState {
    pub fn new(
        config: SlyConfig,
        memory: Arc<dyn MemoryStore>,
        memory_raw: Arc<crate::memory::Memory>,
        overlay: Arc<OverlayFS>,
        cortex: Arc<Cortex>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            memory,
            memory_raw,
            overlay,
            cortex,
            bus: Arc::new(crate::core::bus::DirectiveBus::new()),
            mcp_clients: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    // MCP Logic moved to crate::mcp::registry
}

