use crate::error::{Result, SlyError};
use crate::mcp::client::McpClient;
use crate::mcp::transport::StdioTransport;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use walkdir::WalkDir;

pub async fn discover_and_start_servers(
    clients_mutex: Arc<Mutex<HashMap<String, Arc<McpClient>>>>
) -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| SlyError::Task("Could not find home directory".to_string()))?;
    let mcp_dir = home.join(".sly").join("mcp");

    if !mcp_dir.exists() {
        std::fs::create_dir_all(&mcp_dir).map_err(|e| SlyError::Io(e))?;
        println!("   📂 Created MCP discovery directory: {}", mcp_dir.display());
    }

    println!("   🔍 Scanning for local MCP servers in {}...", mcp_dir.display());

    for entry in WalkDir::new(&mcp_dir).max_depth(1).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path();
            let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            
            // Skip non-executables or hidden files
            if name.starts_with('.') { continue; }

            // Check if already registered
            {
                let clients = clients_mutex.lock().await;
                if clients.contains_key(&name) { continue; }
            }

            println!("   🔌 Found potential MCP server: {}", name);
            
            // Attempt to spawn
            match StdioTransport::new(path.to_str().unwrap(), &[]) {
                Ok(transport) => {
                    let client = Arc::new(McpClient::new(Box::new(transport)));
                    match tokio::time::timeout(std::time::Duration::from_secs(5), client.initialize()).await {
                        Ok(Ok(_)) => {
                            println!("     ✅ Connected to discovered server: {}", name);
                            clients_mutex.lock().await.insert(name, client);
                        },
                        Ok(Err(e)) => eprintln!("     ⚠️ Handshake failed for {}: {}", name, e),
                        Err(_) => eprintln!("     ⚠️ Connection timed out for discovered server {}", name),
                    }
                },
                Err(e) => eprintln!("     ⚠️ Failed to spawn discovered server {}: {}", name, e),
            }
        }
    }

    Ok(())
}
