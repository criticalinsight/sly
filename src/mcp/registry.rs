use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::{anyhow, Result};
use crate::mcp::client::McpClient;

#[derive(Clone)]
pub struct McpToolMetadata {
    pub name: String,
    pub server_name: String,
    pub client: Arc<McpClient>,
}

pub async fn get_all_tool_metadata(clients_mutex: &Mutex<HashMap<String, Arc<McpClient>>>) -> Vec<McpToolMetadata> {
    let clients = clients_mutex.lock().await;
    let mut metadata = Vec::new();
    
    for (name, client) in clients.iter() {
        if let Ok(tools) = client.list_tools().await {
            for tool in tools {
                metadata.push(McpToolMetadata {
                    name: tool.name,
                    server_name: name.clone(),
                    client: client.clone(),
                });
            }
        }
    }
    metadata
}

pub async fn get_tool_definitions(metadata: &[McpToolMetadata]) -> String {
    if metadata.is_empty() {
        return String::new();
    }

    let mut all_tools = Vec::new();
    for meta in metadata {
        // We need the full tool object for the definition. 
        // For now, let's assume we can reconstruct or we should have stored it.
        // Let's just use a placeholder or simpler JSON if we don't have the full Tool object here.
        // Actually, let's modify McpToolMetadata to include the full Tool object.
        all_tools.push(serde_json::json!({
            "name": meta.name,
            "server": meta.server_name,
        }));
    }

    format!(
        "\n## AVAILABLE MCP TOOLS\n\nYou have access to the following external tools:\n```json\n{}\n```\n",
        serde_json::to_string_pretty(&all_tools).unwrap_or_default()
    )
}

pub async fn call_mcp_tool(
    metadata: &[McpToolMetadata],
    tool_name: &str, 
    args: serde_json::Value
) -> Result<serde_json::Value> {
    let meta = metadata.iter()
        .find(|m| m.name == tool_name)
        .ok_or_else(|| anyhow!("Tool not found: {}", tool_name))?;

    println!("    Found tool {} on server {}", tool_name, meta.server_name);
    meta.client.call_tool(tool_name, args).await
}
