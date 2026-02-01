use tokio::sync::mpsc::Receiver;
use crate::io::events::Impulse;
use crate::core::state::GlobalState;
use crate::core::interpreter::ImpulseInterpreter;
use std::sync::Arc;
use colored::*;

pub async fn cortex_loop(
    mut priority_rx: Receiver<Impulse>,
    mut background_rx: Receiver<Impulse>,
    state: Arc<GlobalState>
) {
    println!("{}", "🧠 Cortex Event Bus: ONLINE (Godmode Static)".green().bold());
    
    loop {
        let impulse = tokio::select! {
            biased;

            Some(imp) = priority_rx.recv() => Some(imp),
            Some(imp) = background_rx.recv() => Some(imp),
            else => None,
        };

        if let Some(imp) = impulse {
            let mut should_shutdown = false;
            if matches!(imp, Impulse::Terminate | Impulse::SystemInterrupt) {
                should_shutdown = true;
            }
            
            ImpulseInterpreter::interpret(imp, state.clone()).await;

            if should_shutdown {
                println!("{}", "👋 Graceful shutdown complete.".green());
                break;
            }
        } else {
            break;
        }
    }
}
