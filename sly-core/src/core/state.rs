use std::sync::Arc;
use crate::memory::MemoryStore;
use crate::safety::OverlayFS;
use super::cortex::Cortex;
use crate::error::{Result, SlyError};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use colored::Colorize;
use crate::mcp::registry::McpToolMetadata;

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
    pub telegram_chat_id: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
}

impl SlyConfig {
    pub fn load() -> Self {
        let path = std::path::Path::new(".sly/config.toml");
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    match Self::from_str(&content) {
                        Ok(config) => return config,
                        Err(e) => eprintln!("{} Failed to parse config.toml: {}", "⚠️".red(), e),
                    }
                }
                Err(e) => eprintln!("{} Failed to read config.toml: {}", "⚠️".red(), e),
            }
        }
        Self::default()
    }

    pub fn from_str(content: &str) -> Result<Self> {
        toml::from_str(content).map_err(|e| SlyError::Task(format!("Config parse error: {}", e)))
    }
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
            telegram_chat_id: None,
        }
    }
}

#[derive(Clone)]
pub struct GlobalState {
    pub config: Arc<SlyConfig>,
    pub memory: Arc<dyn MemoryStore>,
    pub memory_raw: Arc<crate::memory::Memory>,
    pub overlay: Arc<OverlayFS>,
    pub cortex: Arc<Cortex>,
    pub mcp_clients: Arc<tokio::sync::Mutex<HashMap<String, Arc<crate::mcp::client::McpClient>>>>,
    pub metadata_cache: Arc<tokio::sync::Mutex<Vec<McpToolMetadata>>>,
    pub bus: crate::core::bus::ArcBus,
    pub io: Arc<tokio::sync::Mutex<Box<dyn crate::io::interface::AgentIO>>>,
}

impl GlobalState {
    pub fn new(
        config: SlyConfig,
        memory: Arc<dyn MemoryStore>,
        memory_raw: Arc<crate::memory::Memory>,
        overlay: Arc<OverlayFS>,
        cortex: Arc<Cortex>,
        bus: crate::core::bus::ArcBus,
        io: Box<dyn crate::io::interface::AgentIO>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            memory,
            memory_raw,
            overlay,
            cortex,
            mcp_clients: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            metadata_cache: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            bus,
            io: Arc::new(tokio::sync::Mutex::new(io)),
        }
    }

    pub async fn new_transient() -> Result<Self> {
        let config = SlyConfig::default();
        let memory_raw = Arc::new(crate::memory::Memory::new_transient().await?);
        let overlay = Arc::new(OverlayFS::new(&std::env::current_dir().map_err(|e| SlyError::Io(e))?, "transient")?);
        let cortex = Arc::new(Cortex::new(config.clone(), "rust".to_string())?);
        let bus = Arc::new(crate::core::bus::EventBus::new());
        let io = Box::new(crate::io::cli::CliAdapter::new("transient_session"));
        Ok(Self::new(config, memory_raw.clone() as Arc<dyn MemoryStore>, memory_raw, overlay, cortex, bus, io))
    }

    #[cfg(test)]
    pub async fn new_for_tests(path: &str) -> Result<Self> {
        if std::env::var("GEMINI_API_KEY").is_err() {
            std::env::set_var("GEMINI_API_KEY", "test-key");
        }
        let config = SlyConfig::default();
        let memory_raw = Arc::new(crate::memory::Memory::new_light(path, false).await?);
        let overlay = Arc::new(OverlayFS::new(std::path::Path::new(path), "test-overlay").map_err(|e| SlyError::Overlay(e.to_string()))?);
        let cortex = Arc::new(Cortex::new(config.clone(), "rust".to_string())?);
        let bus = Arc::new(crate::core::bus::EventBus::new());
        let io = Box::new(crate::io::cli::CliAdapter::new("test_session"));
        Ok(Self::new(config, memory_raw.clone() as Arc<dyn MemoryStore>, memory_raw, overlay, cortex, bus, io))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = SlyConfig::default();
        assert_eq!(config.project_name, "sly");
        assert_eq!(config.primary_model, "gemini-3-flash");
        assert!(config.autonomous_mode);
        assert_eq!(config.role, SlyRole::Executor);
    }
}
