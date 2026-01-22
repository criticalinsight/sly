use uuid::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub messages: Vec<String>,
    pub depth: usize,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    Idle,
    Thinking,
    AwaitingObservation,
    Completed,
    Error(String),
}

impl AgentSession {
    pub fn new(initial_prompt: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            messages: vec![initial_prompt],
            depth: 0,
            status: SessionStatus::Idle,
        }
    }

    pub fn with_message(mut self, msg: String) -> Self {
        self.messages.push(msg);
        self
    }

    pub fn with_depth_increment(mut self) -> Self {
        self.depth += 1;
        self
    }

    pub fn with_status(mut self, status: SessionStatus) -> Self {
        self.status = status;
        self
    }
}
