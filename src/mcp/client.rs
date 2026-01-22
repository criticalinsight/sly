use anyhow::{anyhow, Context, Result};
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
                sampling: None,
            },
            client_info: ClientInfo {
                name: "sly-mcp-client".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: Some(serde_json::to_value(params)?),
            id: Some(Value::Number(serde_json::Number::from(1))),
        };

        self.transport.send(&request).await?;

        // Wait for response (Blocking for now during init)
        // In a real async actor system, we'd have a correlation map.
        // For simplified "Design First", we assume simple sequential init.
        if let Some(line) = self.transport.receive_line().await? {
            let response: JsonRpcResponse = serde_json::from_str(&line)?;
            
            if let Some(error) = response.error {
                 return Err(anyhow!("MCP Initialize Error: {}", error.message));
            }

            if let Some(result) = response.result {
                 // Store server info from result["serverInfo"] / ["capabilities"]
                 let info: ClientInfo = serde_json::from_value(result["serverInfo"].clone())
                     .context("Missing serverInfo in initialize response")?;
                 
                 let caps: ClientCapabilities = serde_json::from_value(result["capabilities"].clone())
                     .context("Missing capabilities in initialize response")?;

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

        Err(anyhow!("MCP Initialize unexpected response or timeout"))
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
            let response: JsonRpcResponse = serde_json::from_str(&line)?;
            if let Some(result) = response.result {
                if let Some(tools_val) = result.get("tools") {
                     let tools: Vec<Tool> = serde_json::from_value(tools_val.clone())?;
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
            let response: JsonRpcResponse = serde_json::from_str(&line)?;
            if let Some(error) = response.error {
                return Err(anyhow!("Tool Call Error: {}", error.message));
            }
            if let Some(result) = response.result {
                return Ok(result);
            }
        }
        Err(anyhow!("Tool call returned no result"))
    }
}
