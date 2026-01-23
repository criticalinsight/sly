use sly::memory::{Memory, MemoryStore};
use sly::core::state::{GlobalState, SlyConfig};
use sly::core::r#loop::cortex_loop;
use sly::io::watcher::setup_watcher;
use sly::safety::OverlayFS;
// use sly::knowledge::KnowledgeEngine; // Removed
use sly::core::cortex::Cortex;

use tokio::sync::mpsc;
use anyhow::{Context, Result};
use colored::*;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path};
// use std::process::Command;
use std::sync::Arc;
use tokio::time::Duration;

pub const SLY_DIR: &str = ".sly";

// Modules are now re-exported from lib.rs (sly crate)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    WriteFile { path: String, content: String },
    ExecShell { command: String, context: String },
    QueryMemory { query: String },
    Commit,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("sly {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h" || a == "help") {
        println!("Sly - Autonomous Agent (v{})", env!("CARGO_PKG_VERSION"));
        println!("Usage: sly [init | supervisor | session <query> | --version | --help]");
        return Ok(());
    }

    if args.iter().any(|a| a == "init") {
        return init_workspace();
    }

    if args.iter().any(|a| a == "fact") {
        if args.len() < 4 {
            eprintln!("Usage: sly fact <operation> <json_data>");
            return Ok(());
        }
        let op = &args[2];
        let data: serde_json::Value = serde_json::from_str(&args[3])
            .context("Invalid JSON data for fact")?;
        
        let outbox = Path::new(SLY_DIR).join("outbox");
        fs::create_dir_all(&outbox)?;
        
        // Write as unique file to outbox
        let id = uuid::Uuid::new_v4();
        let file_path = outbox.join(format!("{}.json", id));
        let fact = serde_json::json!({
            "op": op,
            "data": data,
            "ts": chrono::Utc::now().timestamp_millis()
        });
        fs::write(file_path, serde_json::to_string(&fact)?)?;
        
        println!("{} Fact Queued for Broadcast: {}", "📩".cyan(), op);
        return Ok(());
    }

    if args.iter().any(|a| a == "supervisor") {
        if args.iter().any(|a| a == "install") {
            return sly::core::supervisor::Supervisor::install_service();
        }
        dotenvy::dotenv().ok();
        let token = env::var("TELEGRAM_BOT_TOKEN")
            .context("TELEGRAM_BOT_TOKEN not found in .env")?;
        let supervisor = sly::core::supervisor::Supervisor::new(token);
        return supervisor.run().await;
    }

    let mut initial_impulse = None;
    if args.len() > 2 && args[1] == "session" {
        initial_impulse = Some(sly::io::events::Impulse::InitiateSession(args[2..].join(" ")));
    }


    // 1. Initialize State and Memory (Only for Agent execution)
    let config = SlyConfig::load(); 
    let memory = Arc::new(Memory::new(&format!("{}/cozo", SLY_DIR), false).await.context("Failed to init memory")?);
    let memory_raw = memory.clone();
    let memory_store: Arc<dyn MemoryStore> = memory.clone();

    println!("{} Scanning Local Environment for New Knowledge...", "🧠".cyan());
    
    // Bootstrap Skills (Critical since sly-learn is disabled)
    // Pure function call (no manager object)
    if let Err(e) = sly::knowledge::ensure_skills_loaded(&memory).await {
        eprintln!("{} Failed to bootstrap skills: {}", "⚠️".yellow(), e);
    } else {
        println!("   {} Native Skills Loaded", "🧩".green());
    }
    
    // Project Fingerprinting (Moved UP)
    let fp = sly::fingerprint::ProjectFingerprint::detect(Path::new("."));
    println!("   {} Detected Tech Stack: {}", "🔍".yellow(), fp.tech_stack.join(", "));
    println!("   {} Project Type: {:?}", "📁".blue(), fp.project_type);
    
    let tech_stack_str = if fp.tech_stack.is_empty() {
        "Unknown/Generic".to_string()
    } else {
        fp.tech_stack.join(", ")
    };

    // Cortex (Ownership of Arc<Memory> REMOVED)
    let cortex = Arc::new(Cortex::new(config.clone(), tech_stack_str)?);



    // Safety Shield
    let overlay = Arc::new(OverlayFS::new(&std::env::current_dir()?, "godmode_session")?);
    println!("{} Safety Shield (OverlayFS) Active", "🛡️".green());

    let state = Arc::new(GlobalState::new(config.clone(), memory_store, memory_raw.clone(), overlay, cortex));

    // Phase 6: Register Core Handlers (Dynamic Dispatch)
    sly::core::interpreter::DirectiveInterpreter::register_core_handlers(state.clone()).await;

    // 2. Setup Event Bus (Nervous System QoS)
    let (priority_tx, priority_rx) = mpsc::channel(100);
    let (background_tx, background_rx) = mpsc::channel(1000);

    // 3. Start Background Services
    


    // Start MCP Clients
    {
        let mut clients = state.mcp_clients.lock().await;
        for (name, server_config) in &config.mcp_servers {
             println!("   {} Starting MCP Server: {} ({})", "🔌".cyan(), name, server_config.command);
             match sly::mcp::transport::StdioTransport::new(&server_config.command, &server_config.args) {
                 Ok(transport) => {
                     let client = Arc::new(sly::mcp::client::McpClient::new(Box::new(transport)));
                     // Timeout the init handshake to avoid hanging boot
                     match tokio::time::timeout(Duration::from_secs(5), client.initialize()).await {
                         Ok(Ok(_)) => {
                             println!("     {} Connected to {}", "✅".green(), name);
                             clients.insert(name.clone(), client);
                         },
                         Ok(Err(e)) => eprintln!("     {} Handshake failed for {}: {}", "⚠️".red(), name, e),
                         Err(_) => eprintln!("     {} Connection timed out for {}", "⚠️".red(), name),
                     }
                 },
                 Err(e) => eprintln!("     {} Failed to spawn {}: {}", "⚠️".red(), name, e),
             }
        }
    }



    // 4. Setup File Watcher
    let _watcher = setup_watcher(Path::new("."), background_tx.clone())?;
    
    // 5. Start Cortex Loop (Godmode)

    println!("{}", "🚀 Godmode Activated: Event Bus Online".green().bold());
    println!("{}", "   - Priority Channel: READY".yellow());
    println!("{}", "   - Background Channel: READY".blue());
    println!("{}", "   - API Server: DISABLED".bright_black());
    println!("{}", "   - Janitor: DISABLED".bright_black());
    


    if let Some(imp) = initial_impulse {
        priority_tx.send(imp).await?;
    }

    // Graceful Shutdown Signal Handler (Hickey: Capture state change as event)
    let shutdown_tx = priority_tx.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("\n{} Graceful shutdown requested...", "🛑".red());
        let _ = shutdown_tx.send(sly::io::events::Impulse::SystemInterrupt).await;
    });

    cortex_loop(priority_rx, background_rx, state).await;

    Ok(())
}

fn init_workspace() -> Result<()> {
    let sly_path = Path::new(SLY_DIR);
    if sly_path.exists() {
        println!("{}", "✅ Sly is already alive in this workspace.".green());
        return Ok(());
    }
    fs::create_dir_all(sly_path.join("cozo"))?;
    fs::create_dir_all(sly_path.join("shadow"))?;
    let config = SlyConfig::default();
    let toml = toml::to_string_pretty(&config)?;
    fs::write(sly_path.join("config.toml"), toml)?;
    
    let gitignore_path = Path::new(".gitignore");
    let mut gitignore = if gitignore_path.exists() {
        fs::read_to_string(gitignore_path)?
    } else {
        String::new()
    };
    if !gitignore.contains(".sly") {
        gitignore.push_str("\n# Sly Agent Data\n.sly/\n");
        fs::write(".gitignore", gitignore)?;
    }
    println!("{}", "🧬 DNA REPLICATION COMPLETE.".green().bold());
    Ok(())
}


