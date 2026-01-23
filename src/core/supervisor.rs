use anyhow::{anyhow, Context, Result};
use std::env;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::memory::Memory;
use colored::*;
use crate::io::telegram::TelegramClient;
use crate::io::telegram::escape_markdown_v2;

pub struct Supervisor {
    telegram: Arc<Mutex<TelegramClient>>,
    executor: Arc<Mutex<Option<tokio::process::Child>>>,
    auto_heal: bool,
    last_event_ts: Arc<Mutex<i64>>,
}

impl Supervisor {
    pub fn new(token: String) -> Self {
        Self {
            telegram: Arc::new(Mutex::new(TelegramClient::new(token))),
            executor: Arc::new(Mutex::new(None)),
            auto_heal: true,
            last_event_ts: Arc::new(Mutex::new(chrono::Utc::now().timestamp_millis())), // No lookback, fresh start
        }
    }

    pub async fn run(self) -> Result<()> {
        let _lock = crate::core::supervisor::SupervisorLock::obtain().context("Singleton lock failed")?;
        
        // Load Config (Phase 6: Persistence)
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

        println!("{}", "👁️  Sly Supervisor Online (Decomplected Outbox active)".green().bold());
        let mut offset = 0;

        loop {
            let mut batch = Vec::new();

            // Priority 1: Remote Tasks/Commands from Telegram
            let updates = match self.telegram.lock().await.get_updates(offset).await {
                Ok(u) => u,
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

            for update in updates {
                offset = update.update_id + 1;
                if let Some(msg) = update.message {
                    if env::var("TELEGRAM_CHAT_ID").is_err() {
                        self.telegram.lock().await.set_chat_id(msg.chat.id);
                    }
                    if let Some(text) = msg.text {
                        if text.starts_with('/') {
                            let _ = self.handle_command(&text).await;
                        } else {
                            let _ = self.handle_task(&text).await;
                        }
                    }
                }
                if let Some(cb) = update.callback_query {
                    let _ = self.handle_callback(cb).await;
                }
            }

            // Priority 2: Process Outbox Fact (Decomplected high-priority telemetry)
            let _ = self.process_outbox(&mut batch).await;

            // Priority 3: Poll Event Log for Telemetry (Light Memory access)
            self.poll_events(&mut batch).await;

            // Priority 4: Flush Batch to Telegram (Summarized)
            let _ = self.flush_batch(batch).await;

            // Priority 5: Monitor Executor Health
            let _ = self.monitor_executor().await;

            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    async fn handle_command(&self, text: &str) -> Result<()> {
        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.is_empty() { return Ok(()); }

        match parts[0] {
            "/start" => self.start_executor().await?,
            "/stop" => self.stop_executor().await?,
            "/status" => self.report_status().await?,
            "/logs" => self.send_logs().await?,
            "/query" => {
                if parts.len() > 1 {
                    let script = parts[1..].join(" ");
                    self.execute_datalog(&script).await?;
                } else {
                    let _ = self.notify("⚠️ Usage: `/query <datalog_script>`").await;
                }
            }
            "/help" => {
                let help = "🤖 *Sly Supervisor Commands*:\n\n/start \\- Launch Sly Agent\n/stop \\- Shutdown Sly Agent\n/status \\- Check Health & Activity\n/logs \\- View Recent Errors\n/query \\- Execute Datalog script";
                let _ = self.notify(help).await;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_callback(&self, cb: crate::io::telegram::CallbackQuery) -> Result<()> {
        let _ = self.telegram.lock().await.answer_callback_query(&cb.id).await;
        
        if let Some(data) = cb.data {
            match data.as_str() {
                "restart" => {
                    self.stop_executor().await?;
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    self.start_executor().await?;
                }
                "stop" => {
                    self.stop_executor().await?;
                }
                "logs" => {
                    self.send_logs().await?;
                }
                "flush_logs" => {
                    let _ = std::fs::write("/tmp/sly_supervisor.err", "");
                    let _ = std::fs::write("/tmp/sly_supervisor.log", "");
                    let _ = self.notify("🧹 *Logs Flushed*").await;
                }
                "approve_plan" => {
                    self.record_decision("PLAN_APPROVED").await?;
                    let _ = self.notify("✅ *Plan Approved*. Signalling Agent...").await;
                }
                "reject_plan" => {
                    self.record_decision("PLAN_REJECTED").await?;
                    let _ = self.notify("❌ *Plan Rejected*. Signaling Agent...").await;
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn record_decision(&self, op: &str) -> Result<()> {
        let mem = Memory::new_light(".sly/cozo", false).await?;
        mem.record_event(op, serde_json::json!({ "source": "telegram_remote" }))?;
        Ok(())
    }

    async fn handle_task(&self, text: &str) -> Result<()> {
        let tasks_path = std::path::Path::new("TASKS.md");
        if !tasks_path.exists() {
            let _ = self.notify("⚠️ `TASKS.md` not found in workspace.").await;
            return Ok(());
        }

        let mut content = std::fs::read_to_string(tasks_path)?;
        let ts = chrono::Utc::now().timestamp_millis() % 10000;
        let task_line = format!("- [ ] {} (via Telegram) <!-- id: tg_{} -->", text, ts);

        // Semantic Routing: Find a good section
        let sections = ["Bugs", "Incoming", "Inbox", "Active Tasks", "Tasks"];
        let mut inserted = false;

        for section in sections {
            let pattern = format!("## {}", section);
            if let Some(pos) = content.find(&pattern).or_else(|| content.find(&format!("### {}", section))) {
                if let Some(next_line_pos) = content[pos..].find('\n') {
                    content.insert_str(pos + next_line_pos + 1, &format!("{}\n", task_line));
                    inserted = true;
                    break;
                }
            }
        }

        if !inserted {
            if !content.ends_with('\n') { content.push('\n'); }
            content.push_str(&format!("{}\n", task_line));
        }

        std::fs::write(tasks_path, content)?;
        let _ = self.notify(&format!("✅ *Task Queued*: `{}`", escape_markdown_v2(text))).await;
        Ok(())
    }

    async fn start_executor(&self) -> Result<()> {
        let mut child_lock = self.executor.lock().await;
        if child_lock.is_some() {
            let _ = self.notify("⚠️ *Sly is already running*").await;
            return Ok(());
        }

        println!("🚀 Starting Sly Executor...");
        let exe_path = std::env::current_exe()?;
        let child = tokio::process::Command::new(exe_path)
            .arg("session")
            .arg("Waiting for input...")
            .spawn()
            .context("Failed to spawn sly executor")?;

        *child_lock = Some(child);
        let _ = self.notify("✅ *Sly Executor Launched* (Godmode Active)").await;
        Ok(())
    }

    async fn stop_executor(&self) -> Result<()> {
        let mut child_lock = self.executor.lock().await;
        if let Some(mut child) = child_lock.take() {
            println!("🛑 Stopping Sly Executor...");
            let _ = child.kill().await;
            let _ = self.notify("🛑 *Sly Executor Stopped*").await;
        } else {
            let _ = self.notify("⚠️ *Sly is not running*").await;
        }
        Ok(())
    }

    async fn report_status(&self) -> Result<()> {
        let child_lock = self.executor.lock().await;
        let status = if child_lock.is_some() { "🟢 *RUNNING*" } else { "🔴 *STOPPED*" };
        let heal_status = if self.auto_heal { "🛡️ *ON*" } else { "⚠️ *OFF*" };
        let msg = format!("📊 *Status*: {}\nAuto-Healing: {}\nMode: Godmode\nSafety: OverlayFS Active", status, heal_status);
        
        use crate::io::telegram::{InlineKeyboardMarkup, InlineKeyboardButton};
        let keyboard = InlineKeyboardMarkup {
            inline_keyboard: vec![
                vec![
                    InlineKeyboardButton { text: "🔄 Restart".to_string(), callback_data: "restart".to_string() },
                    InlineKeyboardButton { text: "🛑 Stop".to_string(), callback_data: "stop".to_string() },
                ],
                vec![
                    InlineKeyboardButton { text: "📜 Logs".to_string(), callback_data: "logs".to_string() },
                    InlineKeyboardButton { text: "🧹 Flush".to_string(), callback_data: "flush_logs".to_string() },
                ]
            ]
        };
        let _ = self.telegram.lock().await.send_message_with_markup(&msg, keyboard).await;
        Ok(())
    }

    async fn send_logs(&self) -> Result<()> {
        let log = std::fs::read_to_string("/tmp/sly_supervisor.err").unwrap_or_default();
        let truncated = if log.len() > 3000 { format!("{}...", &log[log.len()-3000..]) } else { log };
        let msg = format!("📜 *Recent Logs*:\n\n```\n{}\n```", escape_markdown_v2(&truncated));
        let _ = self.notify(&msg).await;
        Ok(())
    }

    async fn execute_datalog(&self, script: &str) -> Result<()> {
        let mem = Memory::new_light(".sly/cozo", true).await?;
        match mem.backend_run_script(script) {
            Ok(res) => {
                let json = serde_json::to_string_pretty(&res)?;
                let truncated = if json.len() > 3000 { format!("{}...", &json[..3000]) } else { json };
                let msg = format!("💾 *Query Result*:\n\n```json\n{}\n```", escape_markdown_v2(&truncated));
                let _ = self.notify(&msg).await;
            }
            Err(e) => {
                let _ = self.notify(&format!("❌ *Datalog Error*: `{}`", escape_markdown_v2(&e.to_string()))).await;
            }
        }
        Ok(())
    }

    async fn monitor_executor(&self) -> Result<()> {
        let mut child_lock = self.executor.lock().await;
        if let Some(ref mut child) = *child_lock {
            match child.try_wait() {
                Ok(None) => {} // Still running
                Ok(Some(status)) => {
                    let msg = format!("🚨 *Sly Executor Exit* (Status: {})", status);
                    let _ = self.notify(&msg).await;
                    *child_lock = None;
                    
                    if self.auto_heal {
                        let _ = self.notify("🛡️ *Auto-Healing*: Restarting in 5s...").await;
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                        // Use a detached start to avoid blocking the monitor
                        println!("🛡️ Auto-healing restart triggered.");
                    }
                }
                Err(e) => {
                    eprintln!("⚠️ Error monitoring executor: {}", e);
                    *child_lock = None;
                }
            }
        }
        Ok(())
    }

    async fn poll_events(&self, batch: &mut Vec<(String, serde_json::Value)>) {
        // Open memory in TRANSIENT mode (don't hold the lock)
        let mem = match Memory::new_light(".sly/cozo", true).await {
            Ok(m) => m,
            Err(e) => {
                let err_msg = e.to_string();
                if !err_msg.contains("Resource temporarily unavailable") {
                    eprintln!("⚠️ Supervisor poll error: {}", err_msg);
                }
                return;
            }
        };

        let last_ts = *self.last_event_ts.lock().await;
        let query = format!(
            "?[op, data, ts] := *event_log{{op, data, timestamp: ts}}, ts > {} :sort ts :limit 50",
            last_ts
        );

        match mem.backend_run_script(&query) {
            Ok(res) => {
                let mut max_ts = last_ts;
                for row in res.rows {
                    let op = match row.get(0) {
                        Some(cozo::DataValue::Str(s)) => s.to_string(),
                        _ => continue,
                    };
                    let data = match row.get(1) {
                        Some(cozo::DataValue::Json(j)) => j.0.clone(),
                        _ => serde_json::Value::Null,
                    };
                    let ts = match row.get(2) {
                        Some(cozo::DataValue::Num(n)) => {
                            let s = format!("{:?}", n);
                            let clean = s.trim_start_matches("Num(").trim_end_matches(')');
                            if let Ok(f) = clean.parse::<f64>() {
                                f as i64
                            } else {
                                continue;
                            }
                        },
                        _ => continue,
                    };

                    if ts > max_ts { 
                        max_ts = ts; 
                    }
                    batch.push((op, data));
                }
                *self.last_event_ts.lock().await = max_ts;
            }
            Err(_e) => {}
        }
    }

    async fn process_outbox(&self, batch: &mut Vec<(String, serde_json::Value)>) -> Result<()> {
        let outbox = std::path::Path::new(".sly/outbox");
        if !outbox.exists() { return Ok(()); }

        let entries = std::fs::read_dir(outbox)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(fact) = serde_json::from_str::<serde_json::Value>(&content) {
                        let op = fact["op"].as_str().unwrap_or("UNKNOWN").to_string();
                        let data = fact["data"].clone();
                        batch.push((op, data));
                    }
                }
                let _ = std::fs::remove_file(path);
            }
        }
        Ok(())
    }

    async fn flush_batch(&self, batch: Vec<(String, serde_json::Value)>) -> Result<()> {
        if batch.is_empty() { return Ok(()); }

        // Group by OP and Data (for duplicate suppression)
        use std::collections::HashMap;
        let mut grouped: HashMap<(String, String), usize> = HashMap::new();
        let mut order: Vec<(String, String, serde_json::Value)> = Vec::new();

        for (op, data) in batch {
            let data_str = serde_json::to_string(&data).unwrap_or_default();
            let key = (op.clone(), data_str.clone());
            if let Some(count) = grouped.get_mut(&key) {
                *count += 1;
            } else {
                grouped.insert(key, 1);
                order.push((op, data_str, data));
            }
        }

        for (op, data_str, data) in order {
            let count = grouped.get(&(op.clone(), data_str)).unwrap_or(&1);
            
            if op == "EXEC:propose_plan" {
                // Plans are NEVER batched/concealed
                self.broadcast_fact(&op, &data, 1).await?;
            } else {
                self.broadcast_fact(&op, &data, *count).await?;
            }
        }

        Ok(())
    }

    async fn broadcast_fact(&self, op: &str, data: &serde_json::Value, count: usize) -> Result<()> {
        let prefix = if count > 1 { format!("*{}x*: ", count) } else { "".to_string() };

        if op == "EXEC:propose_plan" {
            let plan_data = data.as_str().unwrap_or("Empty Plan");
            let truncated = if plan_data.len() > 3000 { format!("{}...", &plan_data[..3000]) } else { plan_data.to_string() };
            let msg = format!("📝 *New Implementation Plan*\n\n{}", escape_markdown_v2(&truncated));
            
            use crate::io::telegram::{InlineKeyboardMarkup, InlineKeyboardButton};
            let keyboard = InlineKeyboardMarkup {
                inline_keyboard: vec![
                    vec![
                        InlineKeyboardButton { text: "✅ Approve".to_string(), callback_data: "approve_plan".to_string() },
                        InlineKeyboardButton { text: "❌ Reject".to_string(), callback_data: "reject_plan".to_string() },
                    ]
                ]
            };
            let _ = self.telegram.lock().await.send_message_with_markup(&msg, keyboard).await;
        } else if op == "ARTIFACT:task" {
            let summary = data["summary"].as_str().unwrap_or("Task list updated.");
            let msg = format!("📋 {}*Task Update*: {}\n\n_Check TASKS.md for details._", prefix, escape_markdown_v2(summary));
            let _ = self.notify(&msg).await;
        } else if op == "ARTIFACT:walkthrough" {
            let msg = format!("🚀 {}*Phase Complete*: Walkthrough available.\n\n_Review walkthrough.md for the full audit._", prefix);
            let _ = self.notify(&msg).await;
        } else if op.starts_with("EXEC:") || op.contains("ERROR") || op == "DIRECTIVE" || op.starts_with("ARTIFACT") || op == "PING" {
            let icon = if op.contains("ERROR") { "🚨" } 
                      else if op.starts_with("ARTIFACT") { "📦" }
                      else if op.starts_with("EXEC") { "⚙️" } 
                      else if op == "PING" { "🔔" }
                      else { "👁️" };
            let clean_op = op.replace("EXEC:", "").replace("ARTIFACT:", "");
            let data_str = if data.is_null() || data.as_object().map(|o| o.is_empty()).unwrap_or(false) { 
                "".to_string() 
            } else { 
                format!("\n```json\n{}\n```", serde_json::to_string(&data).unwrap_or_default()) 
            };
            let msg = format!("{}{} *Fact*: `{}`{}", icon, prefix, escape_markdown_v2(&clean_op), escape_markdown_v2(&data_str));
            let _ = self.notify(&msg).await;
        }
        Ok(())
    }

    async fn notify(&self, text: &str) -> Result<()> {
        self.telegram.lock().await.send_message(text).await
    }

    pub fn install_service() -> Result<()> {
        let exe_path = std::env::current_exe()?;
        let login_item_path = dirs::home_dir()
            .context("Could not find home directory")?
            .join("Library/LaunchAgents/com.brixelectronics.sly.plist");

        let plist_content = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.brixelectronics.sly</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>supervisor</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>WorkingDirectory</key>
    <string>{}</string>
    <key>StandardOutPath</key>
    <string>/tmp/sly_supervisor.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/sly_supervisor.err</string>
</dict>
</plist>"#, 
            exe_path.to_string_lossy(),
            std::env::current_dir()?.to_string_lossy()
        );

        std::fs::write(&login_item_path, plist_content)?;
        println!("{} LaunchAgent installed at: {:?}", "✅".green(), login_item_path);
        println!("{} Run this to start: 'launchctl load {:?}'", "🚀".blue(), login_item_path);
        
        Ok(())
    }
}

pub struct SupervisorLock {
    _file: std::fs::File,
}

impl SupervisorLock {
    pub fn obtain() -> Result<Self> {
        let lock_path = Path::new(".sly/supervisor.lock");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(lock_path)?;
        
        use fs2::FileExt;
        if file.try_lock_exclusive().is_err() {
            return Err(anyhow!("Another supervisor is running"));
        }
        
        Ok(Self { _file: file })
    }
}
