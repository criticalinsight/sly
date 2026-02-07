use async_trait::async_trait;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct InputMessage {
    pub content: String,
    pub sender: String,
    pub session_id: String,
    /// Optional metadata for MCP mode (tool calls, etc.)
    pub metadata: Option<serde_json::Value>,
}

/// I/O Modality identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoModality {
    Cli,
    CliPipe,  // Non-interactive pipe mode (stdin/stdout JSON)
    Telegram,
    McpServer,
}

/// The fundamental I/O interface for Sly agent.
/// This trait abstracts over CLI pipes, Telegram Webhooks, or MCP JSON-RPC.
#[async_trait]
pub trait AgentIO: Send + Sync {
    /// Returns the modality of this I/O adapter.
    fn modality(&self) -> IoModality;

    /// Blocking call to fetch the next message from this interface.
    /// Returns None if the stream is closed (e.g. CLI EOF).
    async fn next_message(&mut self) -> Result<Option<InputMessage>>;

    /// Send a response back to the user.
    async fn send_message(&mut self, content: &str) -> Result<()>;

    /// Send structured response (for MCP mode). Default: serialize to string.
    async fn send_structured(&mut self, response: &serde_json::Value) -> Result<()> {
        self.send_message(&serde_json::to_string_pretty(response).unwrap_or_default()).await
    }

    /// Check if this adapter supports streaming responses.
    fn supports_streaming(&self) -> bool {
        false
    }
}
