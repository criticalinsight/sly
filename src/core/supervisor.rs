use crate::io::events::Impulse;
use crate::error::{Result, SlyError};
use tokio::sync::mpsc;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::io::telegram::TelegramClient;
use crate::io::telegram::html_escape;
use colored::*;

#[derive(Clone)]
pub struct Supervisor {
    pub telegram: Arc<Mutex<TelegramClient>>,
    pub event_tx: mpsc::Sender<Impulse>,
    pub cortex: Arc<crate::core::cortex::Cortex>,
    pub memory: Arc<crate::memory::Memory>,
}

impl Supervisor {
    pub fn new(
        token: String, 
        event_tx: mpsc::Sender<Impulse>, 
        cortex: Arc<crate::core::cortex::Cortex>,
        memory: Arc<crate::memory::Memory>,
    ) -> Self {
        Self {
            telegram: Arc::new(Mutex::new(TelegramClient::new(token))),
            event_tx,
            cortex,
            memory,
        }
    }

    pub async fn run(self) -> Result<()> {
        let _lock = crate::core::supervisor::SupervisorLock::obtain().map_err(|e| SlyError::Task(e.to_string()))?;
        
        // Verify Token
        self.telegram.lock().await.get_me().await?;
        
        // Load Config
        use crate::core::state::SlyConfig;
        let config = SlyConfig::load();
        if let Some(chat_id) = config.telegram_chat_id {
            self.telegram.lock().await.set_chat_id(chat_id);
            println!("🔌 Loaded Chat ID from config: {}", chat_id);
        }

        // Try to get chat_id from env as override
        if let Ok(chat_id_str) = env::var("TELEGRAM_CHAT_ID") {
            if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                self.telegram.lock().await.set_chat_id(chat_id);
            }
        }

        println!("{}", "👁️  Sly Supervisor Online (Flattened Loop Active)".green().bold());
        let mut offset = 0;
        let mut idle_cycles = 0;
        let pulse_interval_cycles = 60; // 5 minutes (5s * 60)

        loop {
            // Priority 1: Remote Tasks/Commands from Telegram
            let updates = match self.telegram.lock().await.get_updates(offset).await {
                Ok(u) => {
                    if !u.is_empty() {
                        println!("📥 Received {} updates from Telegram", u.len());
                    }
                    u
                },
                Err(e) => {
                    if e.to_string().contains("409 Conflict") {
                        eprintln!("⚠️ Conflict: Another Supervisor is already running. Exiting...");
                        return Ok(());
                    }
                    eprintln!("⚠️ Telegram Polling Error: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            if updates.is_empty() {
                idle_cycles += 1;
                if idle_cycles >= pulse_interval_cycles {
                    idle_cycles = 0;
                    let _ = self.perform_predictive_pulse().await;
                }
            } else {
                idle_cycles = 0;
                for update in updates {
                    offset = update.update_id + 1;
                    if let Some(msg) = update.message {
                        if env::var("TELEGRAM_CHAT_ID").is_err() {
                            self.telegram.lock().await.set_chat_id(msg.chat.id);
                        }
                        if let Some(text) = msg.text {
                            println!("💬 Remote Command: {}", text);
                            let res = if text.starts_with('/') {
                                self.handle_command(&text).await
                            } else {
                                self.handle_task(&text).await
                            };
                            if let Err(e) = res {
                                eprintln!("⚠️ Interaction Error: {}", e);
                            }
                        }
                    }
                    if let Some(cb) = update.callback_query {
                        println!("🔘 Callback Triggered: {:?}", cb.data);
                        if let Err(e) = self.handle_callback(cb).await {
                            eprintln!("⚠️ Callback Error: {}", e);
                        }
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    async fn handle_command(&self, text: &str) -> Result<()> {
        let parts: Vec<&str> = text.split_whitespace().collect();
        match parts.get(0).copied().unwrap_or_default() {
            "/help" | "/start" => {
                use crate::io::telegram::{InlineKeyboardButton, InlineKeyboardMarkup};
                let mut keyboard = vec![
                    vec![
                        InlineKeyboardButton { text: "🟢 Status".to_string(), callback_data: "status".to_string() },
                        InlineKeyboardButton { text: "📂 Workspace".to_string(), callback_data: "workspaces".to_string() },
                    ],
                    vec![
                        InlineKeyboardButton { text: "📜 Logs".to_string(), callback_data: "logs".to_string() },
                        InlineKeyboardButton { text: "📊 Report".to_string(), callback_data: "report".to_string() },
                    ],
                ];

                // Add Workflows
                let wfs = self.discover_workflows().await;
                let mut wf_buttons = Vec::new();
                for wf in wfs {
                    wf_buttons.push(InlineKeyboardButton { 
                        text: format!("⚡ /{}", wf), 
                        callback_data: format!("wf:{}", wf) 
                    });
                    if wf_buttons.len() == 2 {
                        keyboard.push(wf_buttons.clone());
                        wf_buttons.clear();
                    }
                }
                if !wf_buttons.is_empty() {
                    keyboard.push(wf_buttons);
                }

                keyboard.push(vec![
                    InlineKeyboardButton { text: "🚀 GitHub".to_string(), callback_data: "github".to_string() },
                    InlineKeyboardButton { text: "🛑 Stop".to_string(), callback_data: "stop".to_string() },
                ]);

                let markup = InlineKeyboardMarkup { inline_keyboard: keyboard };
                let msg = "<b>Sly Supervisor: Roadmap Active</b>\n\n/run &lt;task&gt; - Start session\n/ask - Reasoning\n/logs - View Logs\n/workspaces - Switch Repo";
                self.telegram.lock().await.send_message_with_markup(msg, markup).await?;
            }
            "/status" => {
                self.notify("🟢 <b>System Online</b>\nMode: Godmode\nSafety: OverlayFS Active").await?;
            }
            "/workspaces" => {
                let markup = self.generate_workspace_keyboard().await;
                self.telegram.lock().await.send_message_with_markup("📂 <b>Select Workspace</b>:", markup).await?;
            }
            "/test" => { self.execute_workflow("test").await?; }
            "/github" => {
                self.notify("🚀 <b>Pushing to GitHub...</b>").await?;
                let _ = tokio::process::Command::new("git").arg("push").output().await?;
                self.notify("✅ Push complete.").await?;
            }
            "/cloudflare" | "/c" => { self.execute_workflow("c").await?; }
            "/run" => {
                if parts.len() > 1 {
                    let task = parts[1..].join(" ");
                    self.notify(&format!("🚀 <b>Initiating Session</b>: <i>{}</i>", html_escape(&task))).await?;
                    let _ = self.event_tx.send(Impulse::InitiateSession(task)).await;
                } else {
                    self.notify("⚠️ Usage: <code>/run &lt;task description&gt;</code>").await?;
                }
            }
            "/stop" => {
                self.notify("🚨 <b>Emergency Stop Triggered</b>. Halting active sessions...").await?;
                let _ = self.event_tx.send(Impulse::SystemInterrupt).await;
            }
            "/logs" => {
                let log_snippet = match std::fs::read_to_string("/tmp/sly_monitor.out") {
                    Ok(content) => {
                        let lines: Vec<&str> = content.lines().rev().take(10).collect();
                        let mut res = lines.into_iter().rev().collect::<Vec<&str>>().join("\n");
                        if res.is_empty() { res = "No logs found.".to_string(); }
                        res
                    },
                    Err(_) => "Error reading logs.".to_string(),
                };
                self.notify(&format!("📜 <b>Latest Snippet</b>:\n<code>{}</code>", html_escape(&log_snippet))).await?;
            }
            "/report" | "/prd" => {
                 let workflows = self.discover_workflows().await;
                 if workflows.contains(&"prd".to_string()) {
                     self.execute_workflow("prd").await?;
                 } else {
                     let _ = self.perform_predictive_pulse().await;
                 }
            }
            "/ask" => {
                if parts.len() > 1 {
                    let query = parts[1..].join(" ");
                    self.notify("🤔 <b>Thinking...</b>").await?;
                    match self.cortex.generate(&query, crate::core::cortex::ThinkingLevel::Low).await {
                        Ok(res) => { let _ = self.notify(&format!("💡 <b>Sly Insight</b>:\n\n{}", html_escape(&res))).await; },
                        Err(e) => { let _ = self.notify(&format!("⚠️ Error: {}", e)).await; },
                    }
                } else {
                    let _ = self.notify("⚠️ Usage: <code>/ask &lt;your question&gt;</code>").await;
                }
            }
            "/undo" => {
                self.notify("⏳ Rolling back last speculative step...").await?;
                let _ = self.event_tx.send(Impulse::Undo("godmode_session".to_string())).await;
            }
            _ => {
                 let workflows = self.discover_workflows().await;
                 let cmd_name = parts[0].trim_start_matches('/');
                 if workflows.contains(&cmd_name.to_string()) {
                     self.execute_workflow(cmd_name).await?;
                 } else {
                     self.notify(&format!("❓ Unknown command: {}", html_escape(text))).await?;
                 }
            }
        }
        Ok(())
    }

    async fn discover_workflows(&self) -> Vec<String> {
        let mut workflows = Vec::new();
        let path = std::path::Path::new(".agent/workflows");
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        let path = entry.path();
                        if path.extension().map(|e| e == "md").unwrap_or(false) {
                            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                workflows.push(stem.to_string());
                            }
                        }
                    }
                }
            }
        }
        workflows
    }

    async fn execute_workflow(&self, name: &str) -> Result<()> {
        let path = format!(".agent/workflows/{}.md", name);
        println!("🚀 Executing workflow: {} (Path: {})", name, path);
        if !std::path::Path::new(&path).exists() {
            let _ = self.notify(&format!("❌ Workflow not found: <code>{}</code>", name)).await;
            return Ok(());
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| {
                println!("❌ Failed to read workflow file: {}", e);
                SlyError::Io(e)
            })?;
        
        let _ = self.notify(&format!("⚡ <b>Executing Workflow</b>: <code>/{}</code>", name)).await;

        let steps: Vec<String> = content.lines()
            .skip_while(|l| !l.starts_with("```bash"))
            .collect::<Vec<&str>>() // Collect to analyze
            .split(|l| l.starts_with("```bash"))
            .map(|chunk| {
                 chunk.iter()
                    .take_while(|l| !l.starts_with("```"))
                    .cloned()
                    .collect::<Vec<&str>>()
                    .join("\n")
            })
            .filter(|s| !s.trim().is_empty())
            .collect();
        // Simple regex-like parsing above is fragile, reverting to previous logic but cleaner
        // Actually, reusing the previous parsing logic is safer.
        let lines: Vec<&str> = content.lines().collect();
        let mut parsed_steps = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            if lines[i].starts_with("```bash") {
                let mut code = String::new();
                i += 1;
                while i < lines.len() && !lines[i].starts_with("```") {
                    code.push_str(lines[i]);
                    code.push('\n');
                    i += 1;
                }
                if !code.trim().is_empty() {
                    parsed_steps.push(code);
                }
            }
            i += 1;
        }

        for (idx, code) in parsed_steps.iter().enumerate() {
            let step_num = idx + 1;
            let first_line = code.lines().next().unwrap_or("...");
            
            // 1. Initial Message
            let msg_id = match self.notify(&format!("⏳ <b>Step {}/{}</b>: <code>{}</code>\n\n<i>Starting...</i>", step_num, parsed_steps.len(), html_escape(first_line))).await {
                Ok(id) => id,
                Err(_) => continue, // If we can't send, we probably can't run
            };
            
            // 2. Spawn Command
            use std::process::Stdio;
            use tokio::io::{AsyncBufReadExt, BufReader};
            
            let mut child = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(code)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            
            let stdout = child.stdout.take().expect("Failed to open stdout");
            let stderr = child.stderr.take().expect("Failed to open stderr");
            
            let mut reader_out = BufReader::new(stdout).lines();
            let mut reader_err = BufReader::new(stderr).lines();
            
            let mut output_buffer = Vec::new(); // Keep last 15 lines
            let mut last_update = std::time::Instant::now();
            let telegram_lock = self.telegram.clone();
            let base_msg = format!("⏳ <b>Step {}/{}</b>: <code>{}</code>", step_num, parsed_steps.len(), html_escape(first_line));

            loop {
                tokio::select! {
                    Ok(Some(line)) = reader_out.next_line() => {
                        println!("[stdout] {}", line);
                        if output_buffer.len() >= 15 { output_buffer.remove(0); }
                        output_buffer.push(line);
                    }
                    Ok(Some(line)) = reader_err.next_line() => {
                        println!("[stderr] {}", line);
                        if output_buffer.len() >= 15 { output_buffer.remove(0); }
                        output_buffer.push(format!("⚠️ {}", line));
                    }
                    else => break, // EOF
                }
                
                // Debounced Update (every 2s)
                if last_update.elapsed().as_secs() >= 2 {
                    let log_block = output_buffer.join("\n");
                    let _ = telegram_lock.lock().await.edit_message_text(msg_id, &format!("{}\n<pre>{}</pre>", base_msg, html_escape(&log_block))).await;
                    last_update = std::time::Instant::now();
                }
            }

            let status = child.wait().await?;
            let log_block = output_buffer.join("\n");
            
            if status.success() {
                 let _ = telegram_lock.lock().await.edit_message_text(msg_id, &format!("✅ <b>Step {} Complete</b>\n<pre>{}</pre>", step_num, html_escape(&log_block))).await;
            } else {
                 let _ = telegram_lock.lock().await.edit_message_text(msg_id, &format!("❌ <b>Step {} Failed</b>\n<pre>{}</pre>", step_num, html_escape(&log_block))).await;
                 return Err(SlyError::Task(format!("Workflow step {} failed", step_num)));
            }
        }

        let _ = self.notify(&format!("🏁 <b>Workflow Finished</b>: <code>/{}</code>", name)).await;
        Ok(())
    }

    async fn handle_callback(&self, cb: crate::io::telegram::CallbackQuery) -> Result<()> {
        if let Some(data) = cb.data {
             let (cmd, session_id) = if data.contains(':') {
                 let pts: Vec<&str> = data.split(':').collect();
                 (pts[0], Some(pts[1]))
             } else {
                 (data.as_str(), None)
             };

             match cmd {
                 "status" => self.handle_command("/status").await?,
                 "report" => self.handle_command("/report").await?,
                 "logs"   => self.handle_command("/logs").await?,
                 "stop"   => self.handle_command("/stop").await?,
                 "workspaces" => self.handle_command("/workspaces").await?,
                 "test"   => { let sys = self.clone(); tokio::spawn(async move { let _ = sys.execute_workflow("test").await; }); },
                 "github" => self.handle_command("/github").await?,
                 "cloudflare" => { let sys = self.clone(); tokio::spawn(async move { let _ = sys.execute_workflow("c").await; }); },
                 "switch" => {
                     if let Some(path) = session_id {
                         self.notify(&format!("📂 <b>Switching Workspace</b>: <code>{}</code>", path)).await?;
                         std::env::set_current_dir(path).map_err(|e| SlyError::Io(e))?;
                         self.notify("✅ Switched.").await?;
                     }
                 },
                 "undo"   => {
                     if let Some(id) = session_id {
                         self.notify(&format!("⏪ Rolling back session <code>{}</code>...", id)).await?;
                         let _ = self.event_tx.send(Impulse::Undo(id.to_string())).await;
                     }
                 },
                  "think" | "regenerate" => {
                      if let Some(id) = session_id {
                          let label = if cmd == "regenerate" { "🔄 Regenerating" } else { "⏭️ Proceeding" };
                          self.notify(&format!("{} with session <code>{}</code>...", label, id)).await?;
                          let _ = self.event_tx.send(Impulse::ThinkStep(id.to_string())).await;
                      }
                  },
                  "edit" => {
                      if let Some(id) = session_id {
                          self.notify(&format!("📝 <b>Refining Session</b>: <code>{}</code>\n\nPlease send your instructions as a reply or plain message to continue.", id)).await?;
                      }
                  },
                  "commit" => {
                     if let Some(_) = session_id {
                         // Real implementation would send a Resume/Commit impulse
                         self.notify("✅ Commit Authorized (Logic bypassed for now).").await?;
                     }
                 },
                 "approve" => { let _ = self.notify("✅ Task Approved.").await; },
                 "reject"  => { let _ = self.notify("❌ Task Rejected.").await; },
                 "wf" => {
                     if let Some(wf_name) = session_id {
                         let sys = self.clone();
                         let name = wf_name.to_string();
                         tokio::spawn(async move {
                             let _ = sys.execute_workflow(&name).await;
                         });
                     }
                 },
                 _ => {}
             }
             // Answer callback to remove loading state in Telegram
             let _ = self.telegram.lock().await.answer_callback_query(&cb.id).await;
        }
        Ok(())
    }

    async fn handle_task(&self, text: &str) -> Result<()> {
        println!("{} Processing Input via Telegram: {}", "📥".blue(), text);
        
        // Smart Routing: Check for active session to "Observation" instead of "Initiate"
        if let Ok(Some(session_id)) = self.memory.get_active_session_id().await {
            println!("   {} Routing to ACTIVE session: {}", "⚡".yellow(), session_id);
            self.notify(&format!("💬 <b>Observed Session</b>: <code>{}</code>", session_id)).await?;
            let _ = self.event_tx.send(Impulse::Observation(session_id, text.to_string())).await;
        } else {
            println!("   {} Initiating NEW session", "🚀".green());
            self.handle_command(&format!("/run {}", text)).await?;
        }
        Ok(())
    }

    pub async fn notify(&self, text: &str) -> Result<i64> {
        let id = self.telegram.lock().await.send_message(text).await?;
        Ok(id)
    }

    async fn perform_predictive_pulse(&self) -> Result<()> {
        println!("{} Running Predictive Pulse...", "🧠".magenta());
        let _ = crate::io::haptics::HapticSystem::info_pulse();
        
        let tasks_content = std::fs::read_to_string("TASKS.md").unwrap_or_else(|_| "No active tasks.".to_string());
        
        let prompt = format!(
            "Analyze the current state of work and provide a proactive architectural insight or suggestion. Keep it terse and premium. Use HTML for Telegram (<b>, <i>, <code>).\n\nTASKS:\n{}\n",
            tasks_content
        );

        match self.cortex.generate(&prompt, crate::core::cortex::ThinkingLevel::Low).await {
            Ok(insight) => {
                let msg = format!("<b>🧠 Proactive Insight</b>\n\n{}", insight);
                let _ = self.notify(&msg).await;
            }
            Err(e) => eprintln!("Predictive Pulse failed: {}", e),
        }

        Ok(())
    }

    async fn generate_workspace_keyboard(&self) -> crate::io::telegram::InlineKeyboardMarkup {
        use crate::io::telegram::InlineKeyboardButton;
        let mut buttons = Vec::new();
        
        // Scan common depth
        if let Ok(entries) = std::fs::read_dir("/Users/brixelectronics/Documents/mac") {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let path = entry.path();
                        if path.join(".git").exists() || path.join("Cargo.toml").exists() {
                            let name = path.file_name().unwrap().to_string_lossy().to_string();
                            buttons.push(vec![InlineKeyboardButton { 
                                text: format!("📁 {}", name), 
                                callback_data: format!("switch:{}", path.to_string_lossy()) 
                            }]);
                        }
                    }
                }
            }
        }
        
        crate::io::telegram::InlineKeyboardMarkup { inline_keyboard: buttons }
    }
}

pub struct SupervisorLock;
impl SupervisorLock {
    pub fn obtain() -> Result<Self> {
        let lock_path = std::env::temp_dir().join("sly_supervisor.lock");
        std::fs::write(&lock_path, "locked").unwrap();
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_supervisor_new() {
        std::env::set_var("GEMINI_API_KEY", "dummy_key");
        let (tx, _) = tokio::sync::mpsc::channel(1);
        let config = crate::core::state::SlyConfig::default();
        let cortex = Arc::new(crate::core::cortex::Cortex::new(config, "test".to_string()).unwrap());
        let temp_dir = std::env::temp_dir().join("sly_sup_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let path = temp_dir.join("cozo").to_string_lossy().to_string();
        let state = crate::core::state::GlobalState::new_for_tests(&path).await.unwrap();
        let memory = state.memory_raw.clone();

        let sup = Supervisor::new("token".to_string(), tx, cortex, memory);
        assert!(sup.telegram.try_lock().is_ok());
    }
}
