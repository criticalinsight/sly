use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use std::sync::Arc;

use crate::mcp::types::JsonRpcRequest;

/// Abstraction for sending/receiving JSON-RPC messages
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    async fn send(&self, message: &JsonRpcRequest) -> Result<()>;
    // Receive one message. Note: This assumes a request-response lockstep or a dedicated reading loop.
    // Real MCP is async, so client will likely have a "listen loop" separate from "send".
    async fn receive_line(&self) -> Result<Option<String>>;
}

/// Transport over Stdio of a spawned process
pub struct StdioTransport {
    stdin: Arc<Mutex<ChildStdin>>,
    reader: Arc<Mutex<BufReader<ChildStdout>>>,
    _process: Arc<Mutex<Child>>, // Shared handle
}

impl StdioTransport {
    pub fn new(command: &str, args: &[String]) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("Failed to spawn MCP server: {} {:?}", command, args))?;

        let stdin = child.stdin.take().context("Failed to open stdin")?;
        let stdout = child.stdout.take().context("Failed to open stdout")?;
        let reader = BufReader::new(stdout);

        Ok(Self {
            stdin: Arc::new(Mutex::new(stdin)),
            reader: Arc::new(Mutex::new(reader)),
            _process: Arc::new(Mutex::new(child)),
        })
    }
}

#[async_trait::async_trait]
impl Transport for StdioTransport {
    async fn send(&self, message: &JsonRpcRequest) -> Result<()> {
        let mut json = serde_json::to_string(message)?;
        json.push('\n');
        
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(json.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    async fn receive_line(&self) -> Result<Option<String>> {
        let mut reader = self.reader.lock().await;
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            return Ok(None);
        }
        Ok(Some(line))
    }
}
