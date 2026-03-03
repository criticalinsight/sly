//! Swarm Task definitions
//!
//! Pure data structures for task distribution and result aggregation.

use serde::{Deserialize, Serialize};

/// A task to be executed by a swarm worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmTask {
    /// Unique task identifier
    pub id: String,
    /// Task description/instruction
    pub instruction: String,
    /// Optional persona to use
    pub persona: Option<String>,
    /// Target files (if applicable)
    pub target_files: Vec<String>,
    /// Maximum execution time in seconds
    pub timeout_secs: u64,
}

impl SwarmTask {
    pub fn new(id: &str, instruction: &str) -> Self {
        Self {
            id: id.to_string(),
            instruction: instruction.to_string(),
            persona: None,
            target_files: Vec::new(),
            timeout_secs: 300, // 5 minute default
        }
    }

    pub fn with_persona(mut self, persona: &str) -> Self {
        self.persona = Some(persona.to_string());
        self
    }

    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.target_files = files;
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

/// Result from a swarm worker execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmResult {
    /// Task ID this result belongs to
    pub task_id: String,
    /// Whether execution succeeded
    pub success: bool,
    /// Output content
    pub output: String,
    /// Files modified (for merge resolution)
    pub files_modified: Vec<String>,
    /// Execution time in milliseconds
    pub duration_ms: u64,
    /// Error message if failed
    pub error: Option<String>,
}

impl SwarmResult {
    pub fn success(task_id: &str, output: String, files: Vec<String>, duration_ms: u64) -> Self {
        Self {
            task_id: task_id.to_string(),
            success: true,
            output,
            files_modified: files,
            duration_ms,
            error: None,
        }
    }

    pub fn failure(task_id: &str, error: String, duration_ms: u64) -> Self {
        Self {
            task_id: task_id.to_string(),
            success: false,
            output: String::new(),
            files_modified: Vec::new(),
            duration_ms,
            error: Some(error),
        }
    }
}
