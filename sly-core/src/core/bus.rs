use tokio::sync::broadcast;
use std::sync::Arc;
use crate::io::events::Impulse;
use crate::error::Result;

#[derive(Clone, Debug)]
pub enum SlyEvent {
    // Core Signals
    Impulse(Impulse),
    
    // Outcome Signals
    Thought(String, String),       // session_id, content (complete)
    ThoughtStream(String, String), // session_id, partial/delta content
    Action(String, String),        // session_id, description
    Error(String, String),         // session_id, message
    
    // System Status
    SystemStatus(String),
}

/// The central nervous system of Sly.
/// Decouples the reasoning engine from I/O mediums (Telegram, CLI, etc.)
pub struct EventBus {
    tx: broadcast::Sender<SlyEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1000);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SlyEvent> {
        self.tx.subscribe()
    }

    pub async fn publish(&self, event: SlyEvent) -> Result<()> {
        let _ = self.tx.send(event);
        Ok(())
    }

    /// Bridge for legacy Impulse channels
    pub async fn bridge_impulse(&self, mut rx: tokio::sync::mpsc::Receiver<crate::io::events::Impulse>) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            while let Some(imp) = rx.recv().await {
                let _ = tx.send(SlyEvent::Impulse(imp));
            }
        });
    }
}

pub type ArcBus = Arc<EventBus>;
