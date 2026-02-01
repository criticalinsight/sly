use crate::error::{Result, SlyError};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::mcp::transport::Transport;
use crate::mcp::types::{
    ClientCapabilities, ClientInfo, InitializeParams, JsonRpcRequest, JsonRpcResponse, Tool,
};

pub struct McpClient {
    transport: Arc<Box<dyn Transport>>,
    server_info: Arc<Mutex<Option<ClientInfo>>>,
    server_capabilities: Arc<Mutex<Option<ClientCapabilities>>>,
}

impl McpClient {
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Self {
            transport: Arc::new(transport),
            server_info: Arc::new(Mutex::new(None)),
            server_capabilities: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ClientCapabilities {
                roots: Some(std::collections::HashMap::new()),
                sampling: Some(std::collections::HashMap::new()),
            },
            client_info: ClientInfo {
                name: "sly-mcp-client".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: Some(serde_json::to_value(params).map_err(|e| SlyError::Mcp(e.to_string()))?),
            id: Some(Value::Number(serde_json::Number::from(1))),
        };

        self.transport.send(&request).await?;

        if let Some(line) = self.transport.receive_line().await? {
            let response: JsonRpcResponse = serde_json::from_str(&line).map_err(|e| SlyError::Mcp(e.to_string()))?;
            
            if let Some(error) = response.error {
                 return Err(SlyError::Mcp(format!("MCP Initialize Error: {}", error.message)));
            }

            if let Some(result) = response.result {
                 // Store server info from result["serverInfo"] / ["capabilities"]
                 let info: ClientInfo = serde_json::from_value(result["serverInfo"].clone())
                     .map_err(|_| SlyError::Mcp("Missing serverInfo in initialize response".to_string()))?;
                 
                 let caps: ClientCapabilities = serde_json::from_value(result["capabilities"].clone())
                     .map_err(|_| SlyError::Mcp("Missing capabilities in initialize response".to_string()))?;

                 *self.server_info.lock().await = Some(info);
                 *self.server_capabilities.lock().await = Some(caps);

                 // Send initialized notification
                 let notification = JsonRpcRequest {
                     jsonrpc: "2.0".to_string(),
                     method: "notifications/initialized".to_string(),
                     params: None,
                     id: None,
                 };
                 self.transport.send(&notification).await?;

                 return Ok(());
            }
        }

        Err(SlyError::Mcp("MCP Initialize unexpected response or timeout".to_string()))
    }

    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/list".to_string(),
            params: None,
            id: Some(Value::Number(serde_json::Number::from(2))),
        };
        self.transport.send(&request).await?;

        if let Some(line) = self.transport.receive_line().await? {
            let response: JsonRpcResponse = serde_json::from_str(&line).map_err(|e| SlyError::Mcp(e.to_string()))?;
            if let Some(result) = response.result {
                if let Some(tools_val) = result.get("tools") {
                     let tools: Vec<Tool> = serde_json::from_value(tools_val.clone()).map_err(|e| SlyError::Mcp(e.to_string()))?;
                     return Ok(tools);
                }
            }
        }
        Ok(vec![])
    }

    pub async fn call_tool(&self, name: &str, args: Value) -> Result<Value> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": name,
                "arguments": args
            })),
            id: Some(Value::String(Uuid::new_v4().to_string())),
        };
        self.transport.send(&request).await?;

        if let Some(line) = self.transport.receive_line().await? {
            let response: JsonRpcResponse = serde_json::from_str(&line).map_err(|e| SlyError::Mcp(e.to_string()))?;
            if let Some(error) = response.error {
                return Err(SlyError::Mcp(format!("Tool Call Error: {}", error.message)));
            }
            if let Some(result) = response.result {
                return Ok(result);
            }
        }
        Err(SlyError::Mcp("Tool call returned no result".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use tokio::sync::Mutex;
    use std::collections::VecDeque;

    struct MockTransport {
        sent: Arc<Mutex<Vec<JsonRpcRequest>>>,
        to_receive: Arc<Mutex<VecDeque<String>>>,
    }

    impl MockTransport {
        fn new(responses: Vec<String>) -> Self {
            Self {
                sent: Arc::new(Mutex::new(Vec::new())),
                to_receive: Arc::new(Mutex::new(responses.into())),
            }
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn send(&self, message: &JsonRpcRequest) -> Result<()> {
            self.sent.lock().await.push(message.clone());
            Ok(())
        }
        async fn receive_line(&self) -> Result<Option<String>> {
            Ok(self.to_receive.lock().await.pop_front())
        }
    }

    #[tokio::test]
    async fn test_client_initialize() -> Result<()> {
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "serverInfo": { "name": "test-server", "version": "1.0.0" },
                "capabilities": { "roots": {}, "sampling": {} }
            }
        });
        
        let transport = Box::new(MockTransport::new(vec![serde_json::to_string(&response)?]));
        let client = McpClient::new(transport);
        
        client.initialize().await?;
        
        let info = client.server_info.lock().await;
        assert_eq!(info.as_ref().unwrap().name, "test-server");
        Ok(())
    }

    #[tokio::test]
    async fn test_client_list_tools() -> Result<()> {
        let response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": [
                    { "name": "echo", "description": "Echoes input", "inputSchema": {} }
                ]
            }
        });
        
        let transport = Box::new(MockTransport::new(vec![serde_json::to_string(&response)?]));
        let client = McpClient::new(transport);
        
        let tools = client.list_tools().await?;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
        Ok(())
    }

    #[tokio::test]
    async fn test_client_call_tool() -> Result<()> {
        let response = json!({
            "jsonrpc": "2.0",
            "id": "any-id",
            "result": { "output": "hello" }
        });
        
        let transport = Box::new(MockTransport::new(vec![serde_json::to_string(&response)?]));
        let client = McpClient::new(transport);
        
        let result = client.call_tool("echo", json!({"msg": "hello"})).await?;
        assert_eq!(result["output"], "hello");
        Ok(())
    }
}
