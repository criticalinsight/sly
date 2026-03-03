use crate::state::GlobalState;
use crate::parser::{parse_action, AgentAction};
use crate::error::Result;
use colored::*;

/// The main OODA (Observe-Orient-Decide-Act) heartbeat.
pub fn cortex_loop(state: &mut GlobalState) {
    println!("{}", "🧠 Cortex Core: ONLINE (Zero-Library Mode)".green().bold());
    
    loop {
        match state.io.next_input() {
            Ok(Some(input)) => {
                if input == "/stop" || input == "/exit" {
                    println!("{}", "👋 Graceful shutdown complete.".green());
                    break;
                }

                if input == "/undo" {
                    println!("{} Rollback", "⏪".yellow());
                    let _ = state.overlay.rollback();
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

/// A simplified reasoning cycle that operates directly on memory.
fn run_reasoning_cycle(user_input: String, state: &mut GlobalState) -> Result<()> {
    let session_id = format!("sess_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
    
    let mut messages = state.memory.get_messages(&session_id)?;
    messages.push(user_input);

    for i in 0..state.config.max_autonomous_loops {
        println!("{} [Step {}] Thinking...", "🤔".magenta(), i + 1);
        
        let prompt = format!("History: {:?}\n\nTask: {}", messages, messages.last().unwrap());
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
                     let output = std::process::Command::new("sh").arg("-c").arg(&command).output();
                     let obs = match output {
                         Ok(o) => format!("Exit Code: {}\nStdout: {}\nStderr: {}", 
                             o.status.code().unwrap_or(-1),
                             String::from_utf8_lossy(&o.stdout),
                             String::from_utf8_lossy(&o.stderr)),
                         Err(e) => format!("System Error: {}", e),
                     };
                     messages.push(format!("Observation: {}", obs));
                }
                AgentAction::FinalResponse { title, summary } => {
                    println!("🏁 Done: {} - {}", title.bold(), summary);
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
