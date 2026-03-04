//! Global configuration and state bundle.
//!
//! [`GlobalState`] is the single mutable root passed through the
//! entire OODA loop by `&mut` reference. No `Arc`, no `Mutex`.

use crate::memory::Memory;
use crate::safety::OverlayFS;
use crate::cortex::Cortex;
use crate::error::Result;
use crate::io::CliAdapter;

/// Runtime configuration for the Sly engine.
#[derive(Debug, Clone)]
pub struct SlyConfig {
    /// The LLM model identifier (e.g. `gemini-3-flash`).
    pub primary_model: String,
    /// Maximum OODA iterations per user query.
    pub max_autonomous_loops: usize,
}

impl Default for SlyConfig {
    fn default() -> Self {
        Self {
            primary_model: "qwen3:8b".to_string(),
            max_autonomous_loops: 50,
        }
    }
}

/// The single mutable root of the entire engine.
///
/// Passed by `&mut` reference through the OODA loop.
/// No `Arc<Mutex<_>>`, no shared ownership.
pub struct GlobalState {
    pub config: SlyConfig,
    pub memory: Memory,
    pub overlay: OverlayFS,
    pub cortex: Cortex,
    pub io: CliAdapter,
}

impl GlobalState {
    /// Bootstrap a transient (in-memory) engine for development.
    pub fn new_transient() -> Result<Self> {
        let config = SlyConfig::default();
        let memory = Memory::new("/tmp/sly_mem")?;
        let overlay = OverlayFS::new(std::path::Path::new("."), "transient")?;
        let cortex = Cortex::new(config.clone(), String::new())?;
        let io = CliAdapter::new();

        Ok(Self { config, memory, overlay, cortex, io })
    }
}
