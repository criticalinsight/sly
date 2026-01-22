use tokio::sync::mpsc::Receiver;
use crate::io::events::Impulse;
use crate::core::state::GlobalState;
use crate::core::directives::Directive;
use crate::core::interpreter::DirectiveInterpreter;
use std::sync::Arc;
use colored::*;

pub async fn cortex_loop(
    mut priority_rx: Receiver<Impulse>,
    mut background_rx: Receiver<Impulse>,
    state: Arc<GlobalState>
) {
    println!("{}", "🧠 Cortex Event Bus: ONLINE (QoS Enabled)".green().bold());
    
    loop {
        let impulse = tokio::select! {
            biased;

            Some(imp) = priority_rx.recv() => Some((imp, "FAST")),
            Some(imp) = background_rx.recv() => Some((imp, "SLOW")),
            else => None,
        };

        if let Some((imp, lane)) = impulse {
            let directives = route_impulse(imp, lane);
            let mut should_shutdown = false;

            for directive in directives {
                if directive.type_name == "shutdown" {
                    should_shutdown = true;
                }
                DirectiveInterpreter::interpret(directive, state.clone()).await;
            }

            if should_shutdown {
                println!("{}", "👋 Graceful shutdown complete.".green());
                break;
            }
        } else {
            break;
        }
    }
}

fn route_impulse(impulse: Impulse, lane: &str) -> Vec<Directive> {
    let lane_tag = if lane == "FAST" { "⚡".yellow() } else { "🐢".blue() };
    use serde_json::json;
    
    match impulse {
        Impulse::InitiateSession(input) => {
            println!("{} [InitiateSession] {}", lane_tag, input);
            vec![Directive::new("initiate_session", json!({ "input": input }))]
        }
        Impulse::ThinkStep(session_id) => {
            vec![Directive::new("think", json!({ "session_id": session_id }))]
        }
        Impulse::Observation(session_id, obs) => {
            vec![Directive::new("observe", json!({ "session_id": session_id, "observation": obs }))]
        }
        Impulse::FileSystemEvent(event) => {
            // Convert notify::Event to a value. For now just paths.
            let paths: Vec<String> = event.paths.iter().map(|p| p.to_string_lossy().to_string()).collect();
            vec![Directive::new("fs_batch", json!({ "paths": paths }))]
        }
        Impulse::SwarmSignal(id, status) => {
            println!("{} [Swarm] Worker {} reported: {}", lane_tag, id, status);
            vec![] // Noop for now
        }
        Impulse::BootstrapSkills => {
            vec![Directive::new("bootstrap_skills", json!({}))]
        }
        Impulse::JanitorWakeup => {
            vec![Directive::new("maintenance", json!({}))]
        }
        Impulse::SystemInterrupt => {
            vec![Directive::new("shutdown", json!({}))]
        }
        Impulse::Error(e) => {
            vec![Directive::new("error", json!({ "message": e }))]
        }
    }
}
