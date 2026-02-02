use async_trait::async_trait;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct InputMessage {
    pub content: String,
    pub sender: String,
    pub session_id: String, // Useful for the agent to know which session context to load
}

/// The fundamental I/O interface for Sly agent.
/// This trait abstracts over CLI pipes, Telegram Webhooks, or MCP JSON-RPC.
#[async_trait]
pub trait AgentIO: Send + Sync {
    /// Blocking call to fetch the next message from this interface.
    /// Returns None if the stream is closed (e.g. CLI EOF).
    async fn next_message(&mut self) -> Result<Option<InputMessage>>;

    /// Send a response back to the user.
    async fn send_message(&mut self, content: &str) -> Result<()>;
}
