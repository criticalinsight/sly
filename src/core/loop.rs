use crate::io::events::Impulse;
use crate::core::state::GlobalState;
use crate::core::interpreter::ImpulseInterpreter;
use std::sync::Arc;
use colored::*;

pub async fn cortex_loop(
    state: Arc<GlobalState>
) {
    println!("{}", "🧠 Cortex Event Bus: ONLINE".green().bold());
    
    let mut rx = state.bus.subscribe();

    // Spawn Input Poller
    let io_state = state.clone();
    tokio::spawn(async move {
        println!("👂 Input Loop Active");
        loop {
            // Scope the lock to avoid holding it across await (if next_message is long running? No, we await inside)
            // Actually next_message IS async, so we must hold the lock while awaiting it?
            // Yes, Mutex<Box<dyn AgentIO>>.
            // Ideally AgentIO::next_message is cancellation safe.
            
            let msg_opt = {
                let mut io = io_state.io.lock().await;
                io.next_message().await
            };

            match msg_opt {
                Ok(Some(msg)) => {
                    // Convert InputMessage to Impulse
                    if msg.content.starts_with('/') {
                         // Command parsing (primitive for now)
                         // TODO: Better command parser
                         if msg.content == "/stop" {
                             let _ = io_state.bus.publish(crate::core::bus::SlyEvent::Impulse(Impulse::SystemInterrupt)).await;
                         } else {
                            // Treat as session initiation
                            let _ = io_state.bus.publish(crate::core::bus::SlyEvent::Impulse(Impulse::InitiateSession(msg.content))).await;
                         }
                    } else {
                        // Default to chat/session
                        let _ = io_state.bus.publish(crate::core::bus::SlyEvent::Impulse(Impulse::InitiateSession(msg.content))).await;
                    }
                }
                Ok(None) => {
                    // Start of stream / End of stream?
                    // CLI returns None on EOF.
                    // Should we break/shutdown?
                    // For CLI, yes. For Telegram, maybe not? Telegram returns Ok(None) on empty poll (my implementation returns None if empty updates?)
                    // My telegram impl returned Ok(None) if updates empty. We should sleep and retry.
                    // CLI returns Ok(None) on EOF (ctrl+d).
                    
                    // Optimization: Sleep 100ms to avoid busy loop if IO returns None immediately
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
                Err(e) => {
                    eprintln!("Input Error: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }
    });
    
    loop {
        match rx.recv().await {
            Ok(event) => {
                if let crate::core::bus::SlyEvent::Impulse(imp) = event {
                    let mut should_shutdown = false;
                    if matches!(imp, Impulse::Terminate | Impulse::SystemInterrupt) {
                        should_shutdown = true;
                    }
                    
                    ImpulseInterpreter::interpret(imp, state.clone()).await;

                    if should_shutdown {
                        println!("{}", "👋 Graceful shutdown complete.".green());
                        break;
                    }
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                eprintln!("Cortex Loop Lagged: skipped {} messages", n);
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}
