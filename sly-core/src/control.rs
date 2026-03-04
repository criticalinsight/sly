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
    let session_id = format!("sess_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
    
    let mut messages = state.memory.get_messages(&session_id)?;
    messages.push(user_input);

    for i in 0..state.config.max_autonomous_loops {
        println!("{} [Step {}] Thinking...", "🤔".magenta(), i + 1);
        
        let last_msg = messages.last().cloned().unwrap_or_default();
        let prompt = format!("History: {:?}\n\nTask: {}", messages, last_msg);
        let response = state.cortex.generate_sync(prompt, crate::cortex::ThinkingLevel::Low, None)?;
        
        println!("🤖 {}", response.green());
        messages.push(response.clone());

        let actions = parse_action(&response)?;
        let mut completed = false;

        for action in actions {
            match action {
                AgentAction::WriteFile { path, content } => {
                     println!("{} Writing {}", "💾".blue(), path);
                     state.overlay.write_file(std::path::Path::new(&path), &content).ok();
                     messages.push(format!("Observation: Wrote {}", path));
                }
                AgentAction::ExecShell { command } => {
                     println!("{} Shell: {}", "💻".blue(), command);
                     let temp_out = format!("/tmp/sly_out_{}_{}", std::process::id(), i);
                     let temp_err = format!("/tmp/sly_err_{}_{}", std::process::id(), i);
                     let safe_cmd = format!("( {} ) > {} 2> {}", command, temp_out, temp_err);
                     
                     match std::process::Command::new("sh").arg("-c").arg(&safe_cmd).spawn() {
                         Ok(mut child) => {
                             let start = std::time::Instant::now();
                             let timeout_secs = 60;
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

                                         messages.push(format!("Observation:\n{}\nStdout: {}\nStderr: {}", primer, out, err));
                                         break;
                                     }
                                     Ok(None) => {
                                         if start.elapsed().as_secs() > timeout_secs {
                                             child.kill().ok();
                                             let out = std::fs::read_to_string(&temp_out).unwrap_or_default();
                                             let err = std::fs::read_to_string(&temp_err).unwrap_or_default();
                                             messages.push(format!("Observation: Timeout ({}s). Killed.\nStdout: {}\nStderr: {}", timeout_secs, out, err));
                                             break;
                                         }
                                         std::thread::sleep(std::time::Duration::from_millis(100));
                                     }
                                     Err(e) => {
                                         messages.push(format!("Observation: Wait Error: {}", e));
                                         break;
                                     }
                                 }
                             }
                             std::fs::remove_file(&temp_out).ok();
                             std::fs::remove_file(&temp_err).ok();
                         }
                         Err(e) => {
                             messages.push(format!("Observation: Spawn Error: {}", e));
                         }
                     }
                }
                AgentAction::FinalResponse { title, summary } => {
                    println!("🏁 Done: {} - {}", title.bold(), summary);
                    // Auto-commit overlay files on task completion
                    match state.overlay.commit() {
                        Ok(files) if !files.is_empty() => {
                            println!("{} Auto-committed {} file(s):", "✅".green(), files.len());
                            for f in &files {
                                println!("   📄 {}", f);
                            }
                        }
                        _ => {}
                    }
                    completed = true;
                }
                AgentAction::Answer { text } => {
                    println!("💬 Answer: {}", text);
                    completed = true;
                }
            }
        }

        state.memory.update_messages(&session_id, &messages)?;
        
        if completed { break; }
    }

    Ok(())
}
