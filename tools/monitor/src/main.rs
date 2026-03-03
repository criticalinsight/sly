use colored::*;
use regex::Regex;
use std::collections::HashMap;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::time::{self, Duration};
use serde::Serialize;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Local, Timelike};
use fs2::FileExt;

#[derive(Debug, Clone)]
struct DailyStats {
    executions: u64,
    errors: u64,
    thoughts: u64,
    last_report_date: String,
}

impl DailyStats {
    fn new() -> Self {
        Self {
            executions: 0,
            errors: 0,
            thoughts: 0,
            last_report_date: Local::now().format("%Y-%m-%d").to_string(),
        }
    }
    
    fn reset(&mut self) {
        self.executions = 0;
        self.errors = 0;
        self.thoughts = 0;
        self.last_report_date = Local::now().format("%Y-%m-%d").to_string();
    }
}

async fn send_telegram_report(token: &str, chat_id: &str, stats: &DailyStats) {
    let stability = if stats.errors == 0 { "High" } else if stats.errors < 5 { "Moderate" } else { "Degraded" };
    
    let message = format!(
        "<b>📊 Sly Daily Report</b>\n📅 Date: {}\n\n• <b>Executions</b>: {}\n• <b>Errors</b>: {} (Stability: {})\n• <b>Insights</b>: {}\n\n<i>System is healthy and operating within parameters.</i>",
        stats.last_report_date, stats.executions, stats.errors, stability, stats.thoughts
    );

    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    let client = reqwest::Client::new();
    let params = [("chat_id", chat_id), ("text", &message), ("parse_mode", "HTML")];

    match client.post(&url).form(&params).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("{} Sent daily report to Telegram.", "📡".magenta());
            } else {
                eprintln!("{} Failed to send Telegram report: Status {}", "⚠️".red(), resp.status());
            }
        },
        Err(e) => eprintln!("{} Failed to send Telegram report: {}", "⚠️".red(), e),
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Singleton Check
    let lock_file = std::fs::File::create("/tmp/sly_monitor.lock")?;
    if lock_file.try_lock_exclusive().is_err() {
        eprintln!("{} Error: Sly Monitor is already running.", "🛑".red());
        std::process::exit(1);
    }
    // Intentionally leak lock_file so it persists for the duration of the process
    // Rust would drop it at end of scope otherwise. However, since we are in main,
    // we can just keep it assigned.
    let _singleton_guard = lock_file;

    // Load .env
    dotenvy::dotenv().ok();
    let telegram_token = env::var("TELEGRAM_BOT_TOKEN").ok().map(|t| t.trim().to_string());
    let mut telegram_chat_id = env::var("TELEGRAM_CHAT_ID").ok();

    // Try to load chat_id from .sly/config.toml if not in env
    if telegram_chat_id.is_none() {
        if let Ok(config_content) = tokio::fs::read_to_string(".sly/config.toml").await {
            for line in config_content.lines() {
                if line.starts_with("telegram_chat_id") {
                    if let Some(val) = line.split('=').nth(1) {
                         telegram_chat_id = Some(val.trim().to_string());
                         println!("{} Loaded Chat ID from config: {}", "🔌".green(), val.trim());
                    }
                }
            }
        }
    }

    if telegram_token.is_none() {
        println!("{} TELEGRAM_BOT_TOKEN not found. Reporting disabled.", "⚠️".yellow());
    }

    // Define files to watch
    let file_paths = vec![
        "/Users/brixelectronics/Documents/mac/amkabot/server.log",
        "/Users/brixelectronics/Documents/mac/criticalinsight_repos/content-refinery/debug_refinery_final.log",
        "/tmp/sly_supervisor.err",
        "/tmp/sly_supervisor.out",
    ];

    println!("{}", "--- Sly Real-Time Monitor (Rust Poller + Telegram) ---".bold().blue());
    println!("{}", "Waiting for activity...".dimmed());

    // Track file positions: path -> offset
    let mut file_positions: HashMap<String, u64> = HashMap::new();
    let stats = Arc::new(Mutex::new(DailyStats::new()));

    // Regex patterns
    let re_error = Regex::new(r"(?i)(error|exception|fatal|fail)").unwrap();
    let re_exec = Regex::new(r"(?i)(executing|exec:|Running command)").unwrap();
    let re_think = Regex::new(r"(?i)(thinking|thought|plan)").unwrap();
    let re_gleam_db = Regex::new(r"(?i)\[GleamDB\]").unwrap();
    let re_latency = Regex::new(r"(\d+)ms").unwrap();

    // Initial seek to end
    for path in &file_paths {
        if let Ok(file) = File::open(path).await {
             if let Ok(metadata) = file.metadata().await {
                 file_positions.insert(path.to_string(), metadata.len());
                 println!("{} {}", "Watching:".green(), path);
             }
        } else {
             println!("{} {} (File not found yet)", "Waiting for:".yellow(), path);
             file_positions.insert(path.to_string(), 0);
        }
    }
    
    let stats_clone = stats.clone();
    let token_clone = telegram_token.clone();
    let chat_id_clone = telegram_chat_id.clone();
    
    // Background Reporting Task
    tokio::spawn(async move {
        // Build a robust daily schedule
        let mut interval = time::interval(Duration::from_secs(60)); // Check every minute
        loop {
            interval.tick().await;
            let now = Local::now();
            
            // Report once per day (checking against stored date)
            let mut s = stats_clone.lock().await;
            let today = now.format("%Y-%m-%d").to_string();
            
            if s.last_report_date != today {
                // Time to report!
                if let (Some(token), Some(chat_id)) = (&token_clone, &chat_id_clone) {
                     send_telegram_report(token, chat_id, &s).await;
                } else {
                    println!("{} Daily report skipped (Missing credentials). Execs: {}", "ℹ️".blue(), s.executions);
                }
                
                // Reset for the new day
                s.reset();
            }
        }
    });

    loop {
        for path in &file_paths {
            let mut current_pos = *file_positions.get(*path).unwrap_or(&0);

            if let Ok(mut file) = File::open(path).await {
                if let Ok(metadata) = file.metadata().await {
                    let len = metadata.len();

                    if len < current_pos { current_pos = 0; } // Truncated

                    if len > current_pos {
                        if let Err(_) = file.seek(std::io::SeekFrom::Start(current_pos)).await { continue; }

                        let mut buffer = vec![0; (len - current_pos) as usize];
                        if let Ok(_) = file.read_exact(&mut buffer).await {
                             let content = String::from_utf8_lossy(&buffer);
                             for line in content.lines() {
                                 if line.trim().is_empty() { continue; }

                                 // Update Stats
                                 let mut s = stats.lock().await;
                                 let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
                                 let file_name = std::path::Path::new(path).file_name().unwrap_or_default().to_string_lossy();

                                 let colored_line = if re_error.is_match(line) {
                                     s.errors += 1;
                                     line.red()
                                 } else if re_gleam_db.is_match(line) {
                                     if line.contains("Query") {
                                         line.yellow()
                                     } else {
                                         line.bright_magenta()
                                     }
                                 } else if re_exec.is_match(line) {
                                     s.executions += 1;
                                     line.green()
                                 } else if re_think.is_match(line) {
                                     s.thoughts += 1;
                                     line.blue()
                                 } else {
                                     line.normal()
                                 };

                                 println!("{} [{}] {}", timestamp.dimmed(), file_name.cyan(), colored_line);
                                 
                                 if s.executions > 0 && s.executions % 10 == 0 && re_exec.is_match(line) {
                                     println!("--- Stats: Execs: {}, Errors: {} ---", s.executions, s.errors);
                                 }
                             }
                        }
                        file_positions.insert(path.to_string(), len);
                    }
                }
            }
        }
        time::sleep(Duration::from_millis(500)).await;
    }
}
