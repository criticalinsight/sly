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
        
        let memory_path = if is_ephemeral { ":memory:".to_string() } else { format!("{}/cozo", SLY_DIR) };
        let memory = Arc::new(Memory::new(&memory_path, false).await?);
        let memory_raw = memory.clone();
        let _config_clone = config.clone();
        
        let cortex = Arc::new(Cortex::new(config.clone(), "Supervisor/Background".to_string())?);
        let overlay = Arc::new(OverlayFS::new(&std::env::current_dir().map_err(|e| SlyError::Io(e))?, "supervisor_session")?);
        
        let bus = Arc::new(sly::core::bus::EventBus::new());
        let mut telegram_client = sly::io::telegram::TelegramClient::new(token.clone());
        if let Some(chat_id) = config.telegram_chat_id {
            telegram_client.set_chat_id(chat_id);
        }

        let state = Arc::new(GlobalState::new(
            config.clone(), 
            memory.clone() as Arc<dyn MemoryStore>, 
            memory_raw.clone(), 
            overlay, 
            cortex.clone(), 
            bus.clone(),
            Box::new(telegram_client) // Pass TelegramClient as AgentIO
        ));

        // Wire Up Adapters
        let mut _registry = sly::io::adapter::AdapterRegistry::new();
        // But TelegramClient implements SlyAdapter.
        
        println!("{} 🚀 Event-Driven Supervisor System Online", "⚡".yellow().bold());
        
        let (_priority_tx, priority_rx) = mpsc::channel(100);
        let (_background_tx, background_rx) = mpsc::channel(1000);
        
        // Bridge Legacy to Bus
        bus.bridge_impulse(priority_rx).await; // This will spawn a task
        bus.bridge_impulse(background_rx).await;


        cortex_loop(state).await;
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

    let bus = Arc::new(sly::core::bus::EventBus::new());
    let state = if is_ephemeral {
        Arc::new(GlobalState::new_transient().await?) // Transient already creates its own bus, let's fix that
    } else {
        match Memory::new(&format!("{}/cozo", SLY_DIR), false).await {
            Ok(memory) => {
                let memory_arc = Arc::new(memory);
                let memory_raw = memory_arc.clone();
                let cortex = Arc::new(Cortex::new(config.clone(), "Generic/Auto".to_string())?);
                let overlay = Arc::new(OverlayFS::new(&std::env::current_dir().map_err(|e| SlyError::Io(e))?, "godmode_session")?);
                let io: Box<dyn sly::io::interface::AgentIO> = Box::new(sly::io::cli::CliAdapter::new("cli_session"));
                Arc::new(GlobalState::new(config.clone(), memory_arc.clone() as Arc<dyn MemoryStore>, memory_raw.clone(), overlay, cortex.clone(), bus.clone(), io))
            },
            Err(_) => Arc::new(GlobalState::new_transient().await?)
        }
    };

    let (_priority_tx, priority_rx) = mpsc::channel(100);
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
        let _ = state.bus.publish(sly::core::bus::SlyEvent::Impulse(imp)).await;
    }

    // Bridge Legacy to Bus
    state.bus.bridge_impulse(priority_rx).await;
    state.bus.bridge_impulse(background_rx).await;

    let shutdown_bus = state.bus.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("\n{} Graceful shutdown requested...", "🛑".red());
        let _ = shutdown_bus.publish(sly::core::bus::SlyEvent::Impulse(sly::io::events::Impulse::SystemInterrupt)).await;
    });

    cortex_loop(state).await;

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
