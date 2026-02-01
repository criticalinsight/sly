use crate::error::Result;
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait LocalMcp: Send + Sync {
    /// Name of the MCP module (used for tool routing prefix)
    fn name(&self) -> &str;
    
    /// Returns the XML-style tool definitions for the agent
    fn tool_definitions(&self) -> String;
    
    /// Executes a specific tool by name with the given arguments
    async fn execute(&self, tool_name: &str, arguments: &Value) -> Result<Value>;
}
