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
        println!("\nUsage: sly [COMMAND] [OPTIONS]");
        println!("\nCommands:");
        println!("  init              Initialize a Sly workspace");
        println!("  supervisor        Run in supervisor mode (Telegram)");
        println!("  session <query>   Run a one-shot session");
        println!("  swarm <task>      Parallel swarm execution [--workers N]");
        println!("  vision <prompt>   Capture screen and reason about it");
        println!("");

        println!("Options:");
        println!("  --ephemeral       Use in-memory storage (no persistence)");
        println!("  --pipe            Non-interactive pipe mode (JSON I/O)");
        println!("  --persona <id>    Use specific persona (hickey, sierra, etc.)");
        println!("  --mcp-server      Run as MCP server (JSON-RPC over stdio)");
        println!("  --version, -v     Show version");
        println!("  --help, -h        Show this help");
        return Ok(());
    }

    if args.iter().any(|a| a == "init") {
        let no_services = args.iter().any(|a| a == "--no-services");
        return init_workspace(no_services);
    }

    // -- Swarm Mode --
    if args.iter().any(|a| a == "swarm") {
        let workers: usize = args.windows(2)
            .find(|w| w[0] == "--workers")
            .and_then(|w| w[1].parse().ok())
            .unwrap_or(4);
        
        let task_idx = args.iter().position(|a| a == "swarm").unwrap_or(0);
        let task_instruction = args[task_idx + 1..].iter()
            .filter(|a| !a.starts_with("--"))
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        
        if task_instruction.is_empty() {
            println!("Usage: sly swarm <task> [--workers N]");
            return Ok(());
        }
        
        return run_swarm(task_instruction, workers).await;
    }

    // -- Vision Mode --
    if args.iter().any(|a| a == "vision") {
        let prompt_idx = args.iter().position(|a| a == "vision").unwrap_or(0);
        let prompt = args[prompt_idx + 1..].join(" ");
        let prompt = if prompt.is_empty() { "Describe what is on my screen.".to_string() } else { prompt };

        println!("{} Capturing workspace...", "📸".cyan());
        match sly::io::vision::capture_screen() {
            Ok(path) => {
                let config = SlyConfig::load();
                let bus = Arc::new(sly::core::bus::EventBus::new());
                let memory_path = format!("{}/cozo", SLY_DIR);
                let memory: Arc<Memory> = Arc::new(Memory::new(&memory_path, false).await?);
                let memory_raw = memory.clone();
                let cortex = Arc::new(Cortex::new(config.clone(), "Vision/Multimodal".to_string())?);
                let overlay = Arc::new(OverlayFS::new(&std::env::current_dir().unwrap(), "vision_session")?);
                let io: Box<dyn sly::io::interface::AgentIO> = Box::new(sly::io::cli::CliAdapter::new("vision_cli"));
                
                let state = Arc::new(GlobalState::new(config, memory as Arc<dyn MemoryStore>, memory_raw, overlay, cortex, bus.clone(), io));
                
                println!("{} Image captured: {}", "✅".green(), path.display());
                let impulse = sly::io::events::Impulse::VisualInput(path.to_string_lossy().to_string(), prompt);
                
                // We need to run the loop or just interpret it once. 
                // Since 'vision' is a CLI command, we want to run a session.
                sly::core::interpreter::ImpulseInterpreter::interpret(impulse, state.clone()).await;
                
                // For a one-shot CLI, we might need to wait for the session to finish or use a direct call.
                // But interpretation sparks an agent loop.
                return Ok(());
            },
            Err(e) => {
                eprintln!("{} Vision failed: {}", "🔴".red(), e);
                return Ok(());
            }
        }
    }


    let config = SlyConfig::load();
    let is_ephemeral = args.iter().any(|a| a == "--ephemeral");
    let is_pipe_mode = args.iter().any(|a| a == "--pipe");
    let is_mcp_server = args.iter().any(|a| a == "--mcp-server");
    
    // Extract --persona <id>
    let persona_id: Option<String> = args.windows(2)
        .find(|w| w[0] == "--persona")
        .map(|w| w[1].clone());

    // -- MCP Server Mode --
    if is_mcp_server {
        println!("{{\"jsonrpc\":\"2.0\",\"result\":\"sly-mcp-server-{}\"}}", env!("CARGO_PKG_VERSION"));
        // TODO: Implement full MCP server protocol
        // For now, just echo that we're ready
        return run_mcp_server(config, is_ephemeral, persona_id).await;
    }

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

    // Select I/O adapter based on flags
    let io_adapter: Box<dyn sly::io::interface::AgentIO> = if is_pipe_mode {
        Box::new(sly::io::cli::CliAdapter::pipe("cli_pipe_session"))
    } else {
        Box::new(sly::io::cli::CliAdapter::new("cli_session"))
    };


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
                let io: Box<dyn sly::io::interface::AgentIO> = io_adapter;
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

    // Launch Gleam Knowledge Server
    let db_path = format!("{}/gleam", SLY_DIR);
    match Command::new("gleam")
        .current_dir("/Users/brixelectronics/Documents/mac/sly_knowledge")
        .args(["run", "--", "server", &db_path, "4000"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn() {
            Ok(child) => println!("   {} Gleam Knowledge Server launched (PID: {}, Port: 4000)", "🟢".green(), child.id()),
            Err(e) => eprintln!("   {} Failed to launch Gleam Knowledge Server: {}", "🔴".red(), e),
        }
}

/// MCP Server Mode: Run as JSON-RPC server over stdio
/// This enables IDE integration via Model Context Protocol
async fn run_mcp_server(
    config: SlyConfig,
    is_ephemeral: bool,
    _persona_id: Option<String>,
) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, BufReader, AsyncWriteExt};
    use serde_json::{json, Value};

    let memory_path = if is_ephemeral { ":memory:".to_string() } else { format!("{}/cozo", SLY_DIR) };
    let memory: Arc<Memory> = Arc::new(Memory::new(&memory_path, false).await?);
    let cortex = Arc::new(Cortex::new(config.clone(), "MCP/Server".to_string())?);

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    // Simple MCP server loop
    while let Ok(Some(line)) = lines.next_line().await {
        let request: std::result::Result<Value, _> = serde_json::from_str(&line);
        
        let response = match request {
            Ok(req) => {
                let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
                let id = req.get("id").cloned().unwrap_or(json!(null));
                
                match method {
                    "initialize" => json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "protocolVersion": "2024-11-05",
                            "capabilities": {
                                "tools": {}
                            },
                            "serverInfo": {
                                "name": "sly",
                                "version": env!("CARGO_PKG_VERSION")
                            }
                        }
                    }),
                    "tools/list" => json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "tools": [
                                {
                                    "name": "sly_task",
                                    "description": "Execute a coding task with Sly agent",
                                    "inputSchema": {
                                        "type": "object",
                                        "properties": {
                                            "task": { "type": "string", "description": "Task to execute" },
                                            "persona": { "type": "string", "description": "Persona to use (optional)" }
                                        },
                                        "required": ["task"]
                                    }
                                },
                                {
                                    "name": "sly_gleam_query_as_of",
                                    "description": "Query codebase knowledge at a specific point in history (temporal query)",
                                    "inputSchema": {
                                        "type": "object",
                                        "properties": {
                                            "clauses": { "type": "array", "description": "GleamDB query clauses" },
                                            "as_of": { "type": "integer", "description": "Transaction ID to query at" }
                                        },
                                        "required": ["clauses", "as_of"]
                                    }
                                },
                                {
                                    "name": "sly_gleam_subscribe",
                                    "description": "Set up a reactive subscription to facts in GleamDB",
                                    "inputSchema": {
                                        "type": "object",
                                        "properties": {
                                            "clauses": { "type": "array", "description": "GleamDB query clauses to watch" }
                                        },
                                        "required": ["clauses"]
                                    }
                                }
                            ]
                        }
                    }),
                    "tools/call" => {
                        let params = req.get("params").cloned().unwrap_or(json!({}));
                        let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let args = params.get("arguments").cloned().unwrap_or(json!({}));
                        
                        match tool_name {
                            "sly_task" => {
                                let task = args.get("task").and_then(|t| t.as_str()).unwrap_or("");
                                // Build messages array for Cortex
                                let messages = vec![json!({
                                    "role": "user",
                                    "parts": [{ "text": task }]
                                })];
                                // Execute via Cortex
                                match cortex.generate(messages, sly::core::cortex::ThinkingLevel::Automatic, None).await {
                                    Ok(result) => json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "result": {
                                            "content": [{ "type": "text", "text": result }]
                                        }
                                    }),
                                    Err(e) => {
                                        let err_msg = format!("{}", e);
                                        json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "error": { "code": -32000, "message": err_msg }
                                        })
                                    }
                                }
                            },
                            "sly_gleam_query_as_of" => {
                                let clauses = args.get("clauses").cloned().unwrap_or(json!([]));
                                let as_of = args.get("as_of").and_then(|v| v.as_u64()).unwrap_or(0);
                                match memory.recall_as_of(clauses, as_of).await {
                                    Ok(results) => {
                                        let text = serde_json::to_string_pretty(&results).unwrap_or_default();
                                        json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "result": {
                                                "content": [{ "type": "text", "text": text }]
                                            }
                                        })
                                    },
                                    Err(e) => {
                                        let err_msg = format!("{}", e);
                                        json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "error": { "code": -32000, "message": err_msg }
                                        })
                                    }
                                }
                            },
                            "sly_gleam_subscribe" => {
                                let clauses = args.get("clauses").cloned().unwrap_or(json!([]));
                                // We need access to the raw backend for generic subscribe
                                // Memory doesn't expose generic subscribe yet, let's fix that.
                                match memory.subscribe_to_clauses(clauses).await {
                                    Ok(_) => json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "result": {
                                            "content": [{ "type": "text", "text": "Successfully subscribed to GleamDB fact stream." }]
                                        }
                                    }),
                                    Err(e) => {
                                        let err_msg = format!("{}", e);
                                        json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "error": { "code": -32000, "message": err_msg }
                                        })
                                    }
                                }
                            },
                            _ => json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": { "code": -32601, "message": format!("Unknown tool: {}", tool_name) }
                            })
                        }
                    },
                    "notifications/initialized" | "initialized" => {
                        // Notification, no response needed
                        continue;
                    },
                    _ => json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32601, "message": format!("Unknown method: {}", method) }
                    })
                }
            },
            Err(e) => json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32700, "message": format!("Parse error: {}", e) }
            })
        };

        let response_str = serde_json::to_string(&response).unwrap_or_default();
        stdout.write_all(response_str.as_bytes()).await.map_err(|e| SlyError::Io(e))?;
        stdout.write_all(b"\n").await.map_err(|e| SlyError::Io(e))?;
        stdout.flush().await.map_err(|e| SlyError::Io(e))?;
    }

    Ok(())
}

/// Swarm Mode: Distribute task across parallel workers
async fn run_swarm(instruction: String, max_workers: usize) -> Result<()> {
    use sly::swarm::{SwarmSupervisor, SwarmTask};
    use colored::*;

    println!("{} Starting Swarm with {} workers", "🐝".yellow(), max_workers);
    println!("   Task: {}", instruction.bright_white());

    let supervisor = SwarmSupervisor::new(max_workers);
    
    // For now, create a single task - future: auto-partition by files
    let tasks = vec![
        SwarmTask::new("main-task", &instruction)
            .with_timeout(300)
    ];

    let results = supervisor.distribute(tasks).await;
    let aggregated = supervisor.aggregate(&results);

    println!("\n{}", aggregated.summary().green());
    
    if !aggregated.conflicts.is_empty() {
        println!("{} Conflicting files:", "⚠️".yellow());
        for conflict in &aggregated.conflicts {
            println!("   - {}", conflict);
        }
    }

    if aggregated.failure_count > 0 {
        for result in &results {
            if !result.success {
                if let Some(ref err) = result.error {
                    println!("{} Task {} failed: {}", "❌".red(), result.task_id, err);
                }
            }
        }
    }

    Ok(())
}
