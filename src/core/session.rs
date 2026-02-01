use uuid::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub messages: Vec<String>,
    pub depth: usize,
    pub status: SessionStatus,
    pub snapshots: Vec<Vec<String>>, // Stack of message history snapshots
    pub last_action_result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    Idle,
    Thinking,
    AwaitingObservation,
    PendingCommit,
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
            snapshots: Vec::new(),
            last_action_result: None,
        }
    }

    pub fn checkpoint(mut self) -> Self {
        self.snapshots.push(self.messages.clone());
        self
    }

    pub fn with_snapshot(mut self, history: Vec<String>) -> Self {
        self.snapshots.push(history);
        self
    }

    pub fn rollback(mut self) -> Self {
        if let Some(history) = self.snapshots.pop() {
            self.messages = history;
        }
        self.status = SessionStatus::Idle; // Reset status on rollback
        self
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new() {
        let session = AgentSession::new("hello".to_string());
        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0], "hello");
        assert_eq!(session.depth, 0);
        assert_eq!(session.status, SessionStatus::Idle);
    }

    #[test]
    fn test_session_builder() {
        let session = AgentSession::new("1".to_string())
            .with_message("2".to_string())
            .with_depth_increment()
            .with_status(SessionStatus::Completed);
            
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.depth, 1);
        assert_eq!(session.status, SessionStatus::Completed);
    }
}
