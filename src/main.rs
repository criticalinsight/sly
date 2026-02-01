use sly::memory::{Memory, MemoryStore};
use sly::core::state::{GlobalState, SlyConfig};
use sly::core::r#loop::cortex_loop;
use sly::io::watcher::setup_watcher;
use sly::safety::OverlayFS;
use sly::core::cortex::Cortex;

use tokio::sync::mpsc;
use colored::*;
use std::env;
use std::fs;
use std::path::{Path};
use std::sync::Arc;
use tokio::time::Duration;

pub const SLY_DIR: &str = ".sly";

use sly::error::{Result, SlyError};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let args: Vec<String> = env::args().collect();
    
    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("sly {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h" || a == "help") {
        println!("Sly - Autonomous Agent (v{})", env!("CARGO_PKG_VERSION"));
        println!("Usage: sly [init | supervisor | session <query> | --version | --help] [--ephemeral]");
        return Ok(());
    }

    if args.iter().any(|a| a == "init") {
        let no_services = args.iter().any(|a| a == "--no-services");
        return init_workspace(no_services);
    }

    let config = SlyConfig::load();
    let is_ephemeral = args.iter().any(|a| a == "--ephemeral");

    // -- Supervisor Mode --
    if args.iter().any(|a| a == "supervisor") {
        let token = env::var("TELEGRAM_BOT_TOKEN")
            .map(|t| t.trim().to_string())
            .map_err(|_| SlyError::Task("TELEGRAM_BOT_TOKEN not found in .env".to_string()))?;
        
        let masked = if token.len() > 10 { format!("{}...{}", &token[0..5], &token[token.len()-5..]) } else { "???".to_string() };
        println!("🔑 Supervisor Token: {}", masked);
        println!("🔢 Token Bytes: {:?}", token.as_bytes());
        
        let memory_path = if is_ephemeral { ":memory:".to_string() } else { format!("{}/cozo", SLY_DIR) };
        let memory = Arc::new(Memory::new(&memory_path, false).await?);
        let memory_raw = memory.clone();
        
        let cortex = Arc::new(Cortex::new(config.clone(), "Supervisor/Background".to_string())?);
        let overlay = Arc::new(OverlayFS::new(&std::env::current_dir().map_err(|e| SlyError::Io(e))?, "supervisor_session")?);
        
        let (priority_tx, priority_rx) = mpsc::channel(100);
        let (_background_tx, background_rx) = mpsc::channel(1000);
        
        let telegram = Some(Arc::new(tokio::sync::Mutex::new(sly::io::telegram::TelegramClient::new(token.clone()))));
        let state = Arc::new(GlobalState::new(config.clone(), memory.clone() as Arc<dyn MemoryStore>, memory_raw.clone(), overlay, cortex.clone(), telegram));

        let supervisor = sly::core::supervisor::Supervisor::new(token, priority_tx.clone(), cortex.clone(), memory_raw.clone());
        
        println!("{} 🚀 Unified Supervisor System Initializing...", "⚡".yellow().bold());
        if is_ephemeral { println!("{} Running in Ephemeral Mode", "🧪".yellow()); }
        
        tokio::select! {
            res = supervisor.run() => {
                if let Err(e) = res { eprintln!("Supervisor Crash: {}", e); }
            }
            _ = cortex_loop(priority_rx, background_rx, state) => {
                println!("Cortex loop terminated.");
            }
        }
        return Ok(());
    }

    // -- Session/CLI Mode --
    let mut initial_impulse = None;
    if args.len() > 1 {
        if args[1] == "session" && args.len() > 2 {
            initial_impulse = Some(sly::io::events::Impulse::InitiateSession(args[2..].join(" ")));
        } else if args[1].starts_with('/') {
            initial_impulse = Some(sly::io::events::Impulse::InitiateSession(args[1..].join(" ")));
        }
    }

    let state = if is_ephemeral {
        println!("{} Running in Ephemeral Mode (In-Memory only)", "🧪".yellow());
        Arc::new(GlobalState::new_transient().await?)
    } else {
        match Memory::new(&format!("{}/cozo", SLY_DIR), false).await {
            Ok(memory) => {
                let memory_arc = Arc::new(memory);
                let memory_raw = memory_arc.clone();
                let memory_store: Arc<dyn MemoryStore> = memory_arc.clone();
                let cortex = Arc::new(Cortex::new(config.clone(), "Generic/Auto".to_string())?);
                let overlay = Arc::new(OverlayFS::new(&std::env::current_dir().map_err(|e| SlyError::Io(e))?, "godmode_session")?);
                
                let telegram = if let Ok(token) = env::var("TELEGRAM_BOT_TOKEN") {
                    let mut client = sly::io::telegram::TelegramClient::new(token.trim().to_string());
                    if let Some(chat_id) = config.telegram_chat_id {
                        client.set_chat_id(chat_id);
                    }
                    Some(Arc::new(tokio::sync::Mutex::new(client)))
                } else {
                    None
                };

                Arc::new(GlobalState::new(config.clone(), memory_store, memory_raw.clone(), overlay, cortex, telegram))
            },
            Err(e) if e.to_string().contains("locked") || e.to_string().contains("Resource temporarily unavailable") => {
                println!("{} Database is locked by another process.", "⚠️".red());
                println!("{} Falling back to Ephemeral Mode...", "🧠".yellow());
                Arc::new(GlobalState::new_transient().await?)
            },
            Err(e) => return Err(e),
        }
    };

    let (priority_tx, priority_rx) = mpsc::channel(100);
    let (background_tx, background_rx) = mpsc::channel(1000);

    {
        let mut clients = state.mcp_clients.lock().await;
        for (name, server_config) in &config.mcp_servers {
             println!("   {} Starting MCP Server: {} ({})", "🔌".cyan(), name, server_config.command);
             match sly::mcp::transport::StdioTransport::new(&server_config.command, &server_config.args) {
                 Ok(transport) => {
                     let client = Arc::new(sly::mcp::client::McpClient::new(Box::new(transport)));
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
    
    // Dynamic Discovery
    if let Err(e) = sly::mcp::discovery::discover_and_start_servers(state.mcp_clients.clone()).await {
        eprintln!("   {} MCP Discovery failed: {}", "⚠️".red(), e);
    }

    let _watcher = setup_watcher(Path::new("."), background_tx.clone())?;
    println!("{} Safety Shield (OverlayFS) Active", "🛡️".green());
    println!("{}", "🚀 Godmode Activated: Event Bus Online".green().bold());

    if let Some(imp) = initial_impulse {
        priority_tx.send(imp).await.map_err(|e| SlyError::Task(format!("Failed to send initial impulse: {}", e)))?;
    }

    let shutdown_tx = priority_tx.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("\n{} Graceful shutdown requested...", "🛑".red());
        let _ = shutdown_tx.send(sly::io::events::Impulse::SystemInterrupt).await;
    });

    cortex_loop(priority_rx, background_rx, state).await;

    Ok(())
}

fn init_workspace(no_services: bool) -> Result<()> {
    let sly_path = Path::new(SLY_DIR);
    if sly_path.exists() {
        println!("{}", "✅ Sly is already alive in this workspace.".green());
    } else {
        fs::create_dir_all(sly_path.join("cozo")).map_err(|e| SlyError::Io(e))?;
        let config = SlyConfig::default();
        let toml = toml::to_string_pretty(&config).map_err(|e| SlyError::Task(format!("TOML error: {}", e)))?;
        fs::write(sly_path.join("config.toml"), toml).map_err(|e| SlyError::Io(e))?;
        
        let gitignore_path = Path::new(".gitignore");
        let mut gitignore = if gitignore_path.exists() {
            fs::read_to_string(gitignore_path).map_err(|e| SlyError::Io(e))?
        } else {
            String::new()
        };
        if !gitignore.contains(".sly") {
            gitignore.push_str("\n# Sly Agent Data\n.sly/\n");
            fs::write(".gitignore", gitignore).map_err(|e| SlyError::Io(e))?;
        }
        println!("{}", "🧬 DNA REPLICATION COMPLETE.".green().bold());
        
        let env_path = Path::new(".env");
        if !env_path.exists() {
            let env_template = "# Sly Environment Configuration\n\n# 1. AI Cortex (Required)\nGEMINI_API_KEY=your_gemini_api_key_here\n\n# 2. Remote Control (Optional)\nTELEGRAM_BOT_TOKEN=your_telegram_bot_token_here\n# TELEGRAM_CHAT_ID=auto_detected_on_first_message\n";
            fs::write(env_path, env_template).map_err(|e| SlyError::Io(e))?;
            println!("{} Created .env template. Please add your GEMINI_API_KEY.", "📝".yellow());
        }

        println!("\n{} Next steps:", "🚀".blue());
        println!("  1. Edit .env and set your API keys.");
        println!("  2. Run 'sly' to start the agent.");
    }
    
    if !no_services {
        launch_background_services();
    } else {
        println!("{} Skipping background services (--no-services)", "ℹ️".blue());
    }

    Ok(())
}

fn launch_background_services() {
    use std::process::{Command, Stdio};
    use std::fs::File;

    println!("{} {} Initiating Background Services...", "🛰️".magenta(), "Sly".bold());
    let out_path = "/tmp/sly_supervisor.out";
    let err_path = "/tmp/sly_supervisor.err";
    
    let stdout = File::create(out_path).unwrap();
    let stderr = File::create(err_path).unwrap();
    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("sly"));
    
    match Command::new(&exe)
        .arg("supervisor")
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn() {
            Ok(child) => println!("   {} Supervisor launched (PID: {})", "🟢".green(), child.id()),
            Err(e) => eprintln!("   {} Failed to launch supervisor: {}", "🔴".red(), e),
        }

    match Command::new("cargo")
        .args(["run", "-p", "sly-monitor"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn() {
            Ok(child) => println!("   {} Monitor launched (PID: {})", "🟢".green(), child.id()),
            Err(e) => eprintln!("   {} Failed to launch monitor: {}", "🔴".red(), e),
        }
}
