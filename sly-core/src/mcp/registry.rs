use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::error::{Result, SlyError};
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

use crate::mcp::local::LocalMcp;

pub async fn get_tool_definitions(metadata: &[McpToolMetadata]) -> String {
    let mut all_tools = Vec::new();
    // External tools
    for meta in metadata {
        all_tools.push(serde_json::json!({
            "name": meta.name,
            "server": meta.server_name,
        }));
    }

    // Local Native Tools
    let mut native_defs = String::new();
    let local_tools: Vec<Box<dyn LocalMcp>> = vec![
        Box::new(crate::mcp::browser::BrowserMcp),
        Box::new(crate::mcp::cloud::CloudMcp),
        Box::new(crate::mcp::fetch::FetchMcp),
        Box::new(crate::mcp::system::SystemMcp),
    ];

    for tool in &local_tools {
        native_defs.push_str(&tool.tool_definitions());
        native_defs.push_str("\n");
    }

    // Meta-Tools (Hardcoded)
    native_defs.push_str(r#"
<tool_def>
    <name>ukr_search</name>
    <description>Universal Knowledge Retrieval: Broadcasts a search query to all connected MCP servers that expose search-like capabilities.</description>
    <parameters>
        {
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search term or topic"
                }
            },
            "required": ["query"]
        }
    </parameters>
</tool_def>
"#);

    format!(
        "\n## AVAILABLE MCP TOOLS\n\nExternal Tools:\n```json\n{}\n```\n\nNATIVE LOCAL TOOLS:\n{}\n",
        serde_json::to_string_pretty(&all_tools).unwrap_or_default(),
        native_defs
    )
}

pub async fn call_mcp_tool(
    metadata: &[McpToolMetadata],
    tool_name: &str, 
    args: serde_json::Value
) -> Result<serde_json::Value> {
    // Intercept UKR (Special Meta-Tool)
    if tool_name == "ukr_search" {
        let query = args["query"].as_str().unwrap_or_default();
        let mut results = Vec::new();
        for meta in metadata {
             if meta.name.to_lowercase().contains("search") || meta.name.to_lowercase().contains("query") {
                 match meta.client.call_tool(&meta.name, serde_json::json!({"query": query})).await {
                     Ok(res) => results.push(format!("#### Source: {} ({})\n{}\n", meta.server_name, meta.name, res)),
                     _ => {}
                 }
             }
        }
        let output = if results.is_empty() { 
            "No active MCP servers found with search capabilities.".to_string() 
        } else { 
            results.join("\n---\n") 
        };
        return Ok(serde_json::Value::String(output));
    }

    // Intercept Local Tools
    let local_tools: Vec<Box<dyn LocalMcp>> = vec![
        Box::new(crate::mcp::browser::BrowserMcp),
        Box::new(crate::mcp::cloud::CloudMcp),
        Box::new(crate::mcp::fetch::FetchMcp),
        Box::new(crate::mcp::system::SystemMcp),
    ];

    for tool in &local_tools {
        if tool_name.starts_with(tool.name()) {
            return tool.execute(tool_name, &args).await;
        }
    }

    let meta = metadata.iter()
        .find(|m| m.name == tool_name)
        .ok_or_else(|| SlyError::Mcp(format!("Tool not found: {}", tool_name)))?;

    println!("    Found tool {} on server {}", tool_name, meta.server_name);
    meta.client.call_tool(tool_name, args).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::transport::Transport;
    use crate::mcp::types::JsonRpcRequest;
    use async_trait::async_trait;
    use serde_json::json;

    struct MockTransport(Vec<String>);
    #[async_trait]
    impl Transport for MockTransport {
        async fn send(&self, _msg: &JsonRpcRequest) -> Result<()> { Ok(()) }
        async fn receive_line(&self) -> Result<Option<String>> {
            // Very simple mock: just return first response for now
            Ok(self.0.first().cloned())
        }
    }

    #[tokio::test]
    async fn test_get_all_tool_metadata() {
        let response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": [
                    { "name": "test_tool", "description": "desc", "inputSchema": {} }
                ]
            }
        });
        
        let transport = Box::new(MockTransport(vec![serde_json::to_string(&response).unwrap()]));
        let client = Arc::new(McpClient::new(transport));
        
        let mut clients = HashMap::new();
        clients.insert("test_server".to_string(), client);
        let mutex = Mutex::new(clients);
        
        let metadata = get_all_tool_metadata(&mutex).await;
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].name, "test_tool");
        assert_eq!(metadata[0].server_name, "test_server");
    }

    #[tokio::test]
    async fn test_get_tool_definitions() {
        // We don't need real client for this pure formatting function
        let metadata = vec![
            McpToolMetadata {
                name: "tool1".to_string(),
                server_name: "server1".to_string(),
                client: Arc::new(McpClient::new(Box::new(MockTransport(vec![])))),
            }
        ];
        
        let defs = get_tool_definitions(&metadata).await;
        assert!(defs.contains("tool1"));
        assert!(defs.contains("server1"));
    }
}
