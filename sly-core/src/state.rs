use crate::memory::Memory;
use crate::safety::OverlayFS;
use crate::cortex::Cortex;
use crate::error::{Result};
use crate::io::CliAdapter;

#[derive(Debug, Clone)]
pub struct SlyConfig {
    pub primary_model: String,
    pub max_autonomous_loops: usize,
}

impl Default for SlyConfig {
    fn default() -> Self {
        Self {
            primary_model: "gemini-3-flash".to_string(),
            max_autonomous_loops: 50,
        }
    }
}

pub struct GlobalState {
    pub config: SlyConfig,
    pub memory: Memory,
    pub overlay: OverlayFS,
    pub cortex: Cortex,
    pub io: CliAdapter,
}

impl GlobalState {
    pub fn new_transient() -> Result<Self> {
        let config = SlyConfig::default();
        let memory = Memory::new("/tmp/sly_mem")?;
        let overlay = OverlayFS::new(std::path::Path::new("."), "transient")?;
        let cortex = Cortex::new(config.clone(), "".to_string())?;
        let io = CliAdapter::new();
        
        Ok(Self { config, memory, overlay, cortex, io })
    }
}
