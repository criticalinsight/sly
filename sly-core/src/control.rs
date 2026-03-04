//! The OODA (Observe-Orient-Decide-Act) control loop.
//!
//! This is the engine heartbeat. It reads user input, drives
//! reasoning cycles, and dispatches agent actions.
//!
//! ## Ralph Loop Reflexion
//!
//! When a shell command fails (non-zero exit code), the observation
//! is prefixed with a reflexion primer that forces the LLM to
//! analyze stderr rather than blindly retrying.

use crate::state::GlobalState;
use crate::parser::{parse_action, AgentAction};
use crate::error::Result;
use colored::*;

/// The main OODA heartbeat. Blocks on stdin, runs reasoning cycles.
pub fn cortex_loop(state: &mut GlobalState) {
    println!("{}", "🧠 Cortex Core: ONLINE (Zero-Library Mode)".green().bold());
    
    loop {
        match state.io.next_input() {
            Ok(Some(input)) => {
                if input.starts_with('/') {
                    if !handle_slash_command(&input, state) {
                        break;
                    }
                    continue;
                }

                // Direct execution of user query
                if let Err(e) = run_reasoning_cycle(input, state) {
                    eprintln!("{} Execution Error: {}", "⚠️".red(), e);
                }
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("Input Error: {}", e);
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}

/// Handles slash commands, returning true to continue the loop, false to exit.
fn handle_slash_command(input: &str, state: &mut GlobalState) -> bool {
    match input {
        "/stop" | "/exit" => {
            println!("{}", "👋 Graceful shutdown complete.".green());
            false
        }
        "/undo" => {
            println!("{} Rollback", "⏪".yellow());
            let _ = state.overlay.rollback();
            true
        }
        "/commit" => {
            match state.overlay.commit() {
                Ok(files) => {
                    println!("{} Committed {} file(s):", "✅".green(), files.len());
                    for f in &files {
                        println!("   📄 {}", f);
                    }
                }
                Err(e) => eprintln!("{} Commit Error: {}", "⚠️".red(), e),
            }
            true
        }
        "/files" => {
            let files = state.overlay.list_files();
            if files.is_empty() {
                println!("📂 Overlay is empty.");
            } else {
                println!("📂 Overlay ({} file(s)):", files.len());
                for f in &files {
                    println!("   📄 {}", f);
                }
            }
            true
        }
        "/status" => {
            if let Ok(msgs) = state.memory.get_messages(&state.session_id) {
                if msgs.is_empty() {
                    println!("📜 Memory is currently empty.");
                } else {
                    println!("📜 Message Trace ({} messages):", msgs.len());
                    for (i, m) in msgs.iter().enumerate() {
                        println!("\n---[{}]---\n{}", i, m.bright_black());
                    }
                }
            } else {
                println!("⚠️ No memory trace found.");
            }
            true
        }
        _ => {
            println!("{} Unknown command: {}", "⚠️".yellow(), input);
            true
        }
    }
}

/// Execute one full reasoning cycle for a user query.
///
/// Loops up to `max_autonomous_loops`, calling the LLM and
/// dispatching parsed actions until a `FinalResponse` or `Answer`
/// is emitted. Includes 60-second execution timeouts and Ralph
/// Loop reflexion on command failures.
fn run_reasoning_cycle(user_input: String, state: &mut GlobalState) -> Result<()> {
    let mut messages = state.memory.get_messages(&state.session_id)?;
    messages.push(format!("USER: {}", user_input));
    let cycle_start = std::time::Instant::now();
    let mut written_files: Vec<String> = Vec::new();

    for i in 0..state.config.max_autonomous_loops {
        let elapsed = cycle_start.elapsed().as_secs();
        println!("{} [Step {}/{}] Thinking... ({}s elapsed)", "🤔".magenta(), i + 1, state.config.max_autonomous_loops, elapsed);

        // F2: Inject file manifest header so model knows what exists
        if !written_files.is_empty() {
            let manifest = format!("[FILES: {}]", written_files.join(", "));
            // Update the first user message with the manifest
            if let Some(first) = messages.first_mut() {
                if !first.contains("[FILES:") {
                    *first = format!("{} {}", manifest, first);
                } else {
                    // Update existing manifest
                    if let Some(end) = first.find(']') {
                        *first = format!("{}{}", manifest, &first[end + 1..]);
                    }
                }
            }
        }

        // F4: Token budget warning
        let total_chars: usize = messages.iter().map(|m| m.len()).sum();
        let est_tokens = total_chars / 4;
        if est_tokens > state.config.token_budget_warning {
            eprintln!("{} Token budget: ~{}/{} tokens used", "⚠️".yellow(), est_tokens, state.config.token_budget_warning);
        }
        
        // Pass the explicit array structure to cortex for KV Cache hit
        let response = state.cortex.generate_sync(&messages, crate::cortex::ThinkingLevel::Low, None)?;
        
        println!("🤖 {}", response.green());
        messages.push(format!("MODEL: {}", response));

        let actions = parse_action(&response)?;
        let mut completed = false;

        for action in actions {
            match action {
                AgentAction::WriteFile { path, content } => {
                     println!("{} Writing {}", "💾".blue(), path);
                     state.overlay.write_file(std::path::Path::new(&path), &content).ok();
                     written_files.push(path.clone());
                     messages.push(format!("USER: Observation: Wrote {}", path));
                }
                AgentAction::ExecShell { command } => {
                     println!("{} Shell: {}", "💻".blue(), command);
                     let temp_out = format!("/tmp/sly_out_{}_{}", std::process::id(), i);
                     let temp_err = format!("/tmp/sly_err_{}_{}", std::process::id(), i);
                     let safe_cmd = format!("( {} ) > {} 2> {}", command, temp_out, temp_err);
                     
                     match std::process::Command::new("sh")
                         .arg("-c")
                         .arg(&safe_cmd)
                         .current_dir(&state.overlay.scratchpad_dir)
                         .spawn() {
                         Ok(mut child) => {
                             let start = std::time::Instant::now();
                             let timeout_secs = 3600; // Increased from 60 to allow compilation
                             loop {
                                 match child.try_wait() {
                                     Ok(Some(status)) => {
                                         let out = std::fs::read_to_string(&temp_out).unwrap_or_default();
                                         let err = std::fs::read_to_string(&temp_err).unwrap_or_default();
                                         
                                         let is_success = status.success();
                                         let status_code = status.code().unwrap_or(-1);

                                         let primer = if !is_success {
                                             format!("⚠️ COMMAND FAILED (Exit Code {})\nRalph Loop Reflexion: Analyze the Stderr. What caused the failure? What is the objective correction? Do not repeat the exact same command. ⚠️\n---", status_code)
                                         } else {
                                             String::new()
                                         };

                                         let out_trunc = truncate_output(&out, 500);
                                         let err_trunc = truncate_output(&err, 300);
                                         messages.push(format!("USER: Observation:\n{}\nStdout: {}\nStderr: {}", primer, out_trunc, err_trunc));
                                         break;
                                     }
                                     Ok(None) => {
                                         if start.elapsed().as_secs() > timeout_secs {
                                             child.kill().ok();
                                             let out = std::fs::read_to_string(&temp_out).unwrap_or_default();
                                             let err = std::fs::read_to_string(&temp_err).unwrap_or_default();
                                             let out_trunc = truncate_output(&out, 500);
                                             let err_trunc = truncate_output(&err, 300);
                                             messages.push(format!("USER: Observation: Timeout ({}s). Killed.\nStdout: {}\nStderr: {}", timeout_secs, out_trunc, err_trunc));
                                             break;
                                         }
                                         std::thread::sleep(std::time::Duration::from_millis(100));
                                     }
                                     Err(e) => {
                                         messages.push(format!("USER: Observation: Wait Error: {}", e));
                                         break;
                                     }
                                 }
                             }
                             std::fs::remove_file(&temp_out).ok();
                             std::fs::remove_file(&temp_err).ok();
                         }
                         Err(e) => {
                             messages.push(format!("USER: Observation: Spawn Error: {}", e));
                         }
                     }
                }
                AgentAction::ReadFile { path } => {
                     println!("{} Reading {}", "📖".blue(), path);
                     let read_path = state.overlay.scratchpad_dir.join(&path);
                     match std::fs::read_to_string(&read_path) {
                         Ok(content) => {
                             let truncated = if content.len() > 2000 {
                                 format!("{}\n... [truncated, {} bytes total]", &content[..2000], content.len())
                             } else {
                                 content
                             };
                             messages.push(format!("USER: Observation: Contents of {}:\n{}", path, truncated));
                         }
                         Err(e) => {
                             messages.push(format!("USER: Observation: Failed to read {}: {}", path, e));
                         }
                     }
                }
                AgentAction::FinalResponse { title, summary } => {
                    println!("🏁 Done: {} - {}", title.bold(), summary);
                    commit_overlay(state);
                    completed = true;
                }
                AgentAction::Answer { text } => {
                    println!("💬 Answer: {}", text);
                    commit_overlay(state);
                    completed = true;
                }
                AgentAction::InvalidJson { raw } => {
                    println!("{} Invalid JSON (no directive): {}", "⚠️".yellow(), &raw[..raw.len().min(80)]);
                    messages.push("USER: Observation: Your response was valid JSON but missing the required \"directive\" key. You MUST respond with one of: WriteFile, ReadFile, ExecShell, Answer, or FinalResponse. Retry.".to_string());
                }
            }
        }

        state.memory.update_messages(&state.session_id, &messages, state.config.max_memory_window, None)?;
        
        if completed { break; }
    }

    Ok(())
}

/// Helper to commit overlay changes and print summary.
fn commit_overlay(state: &mut GlobalState) {
    match state.overlay.commit() {
        Ok(files) if !files.is_empty() => {
            println!("{} Auto-committed {} file(s):", "✅".green(), files.len());
            for f in &files {
                println!("   📄 {}", f);
            }
        }
        _ => {}
    }
}

/// Truncate text to `max_chars`, appending a size marker if truncated.
fn truncate_output(text: &str, max_chars: usize) -> String {
    if text.len() > max_chars {
        format!("{}...\n[truncated, {} bytes total]", &text[..max_chars], text.len())
    } else {
        text.to_string()
    }
}
