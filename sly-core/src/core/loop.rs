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
            let msg_opt = {
                let mut io = io_state.io.lock().await;
                io.next_message().await
            };

            match msg_opt {
                Ok(Some(msg)) => {
                    if msg.content.starts_with('/') {
                         if msg.content == "/stop" {
                             let _ = io_state.bus.publish(crate::core::bus::SlyEvent::Impulse(Impulse::SystemInterrupt)).await;
                         } else {
                            let _ = io_state.bus.publish(crate::core::bus::SlyEvent::Impulse(Impulse::InitiateSession(msg.content))).await;
                         }
                    } else {
                        let _ = io_state.bus.publish(crate::core::bus::SlyEvent::Impulse(Impulse::InitiateSession(msg.content))).await;
                    }
                }
                Ok(None) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
                Err(e) => {
                    eprintln!("Input Error: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }
    });

    // Spawn Reactive Fact Listener (Temporal Reflexion)
    let reactive_state = state.clone();
    tokio::spawn(async move {
        println!("🚀 Reactive Fact Listener: ONLINE");
        // We wait a bit for the server to launch if it was just started
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        match reactive_state.memory_raw.subscribe_to_failures().await {
            Ok(mut rx) => {
                while let Some(_update) = rx.recv().await {
                    println!("{} Fact Triggered: {}", "🔔".yellow(), "TEST FAILURE".red().bold());
                    let _ = reactive_state.bus.publish(crate::core::bus::SlyEvent::Impulse(
                        Impulse::InitiateSession("A critical test failed in the codebase! You must perform Temporal Reflexion, analyze the GleamDB facts about the failure, and repair the root cause immediately.".to_string())
                    )).await;
                }
            }
            Err(e) => eprintln!("{} Reactive listener failed to start: {}", "⚠️".red(), e),
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
