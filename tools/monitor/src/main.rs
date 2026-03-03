use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::time::{Duration};
use std::sync::{Arc, Mutex};
use std::thread;

struct DailyStats {
    executions: u64,
    errors: u64,
}

impl DailyStats {
    fn new() -> Self {
        Self {
            executions: 0,
            errors: 0,
        }
    }
}

fn main() -> std::io::Result<()> {
    // Define files to watch
    let file_paths = vec![
        "/Users/brixelectronics/Documents/mac/amkabot/server.log",
        "/Users/brixelectronics/Documents/mac/criticalinsight_repos/content-refinery/debug_refinery_final.log",
        "/tmp/sly_supervisor.err",
        "/tmp/sly_supervisor.out",
    ];

    println!("\x1b[1;34m--- Sly Real-Time Monitor (Rust Poller) ---\x1b[0m");
    println!("Waiting for activity...");

    let mut file_positions: HashMap<String, u64> = HashMap::new();
    let stats = Arc::new(Mutex::new(DailyStats::new()));

    // Initial seek to end
    for path in &file_paths {
        if let Ok(file) = File::open(path) {
             if let Ok(metadata) = file.metadata() {
                 file_positions.insert(path.to_string(), metadata.len());
                 println!("\x1b[32mWatching:\x1b[0m {}", path);
             }
        } else {
             println!("\x1b[33mWaiting for:\x1b[0m {} (File not found yet)", path);
             file_positions.insert(path.to_string(), 0);
        }
    }
    
    loop {
        for path in &file_paths {
            let mut current_pos = *file_positions.get(*path).unwrap_or(&0);

            if let Ok(mut file) = File::open(path) {
                if let Ok(metadata) = file.metadata() {
                    let len = metadata.len();

                    if len < current_pos { current_pos = 0; } // Truncated

                    if len > current_pos {
                        if file.seek(SeekFrom::Start(current_pos)).is_err() { continue; }

                        let mut buffer = vec![0; (len - current_pos) as usize];
                        if file.read_exact(&mut buffer).is_ok() {
                             let content = String::from_utf8_lossy(&buffer);
                             for line in content.lines() {
                                 if line.trim().is_empty() { continue; }

                                 let mut s = stats.lock().unwrap();
                                 let file_name = std::path::Path::new(path).file_name().unwrap_or_default().to_string_lossy();

                                 // Manual regex replacement
                                 let line_lower = line.to_lowercase();
                                 let is_error = line_lower.contains("error") || line_lower.contains("exception") || line_lower.contains("fatal") || line_lower.contains("fail");
                                 let is_exec = line_lower.contains("executing") || line_lower.contains("exec:") || line_lower.contains("running command");
                                 let is_think = line_lower.contains("thinking") || line_lower.contains("thought") || line_lower.contains("plan");

                                 let color_code = if is_error {
                                     s.errors += 1;
                                     "\x1b[31m" // red
                                 } else if is_exec {
                                     s.executions += 1;
                                     "\x1b[32m" // green
                                 } else if is_think {
                                     "\x1b[34m" // blue
                                 } else {
                                     "\x1b[0m" // reset
                                 };

                                 println!("[{}] {}{}\x1b[0m", file_name, color_code, line);
                                 
                                 if s.executions > 0 && s.executions.is_multiple_of(10) && is_exec {
                                     println!("--- Stats: Execs: {}, Errors: {} ---", s.executions, s.errors);
                                 }
                             }
                        }
                        file_positions.insert(path.to_string(), len);
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(500));
    }
}
