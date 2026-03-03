use async_trait::async_trait;
use crate::core::bus::SlyEvent;
use crate::error::Result;

#[async_trait]
pub trait SlyAdapter: Send + Sync {
    /// Return the name of the adapter
    fn name(&self) -> &str;
    
    /// Handle an event received from the bus
    async fn handle(&self, event: SlyEvent) -> Result<()>;
}

pub struct AdapterRegistry {
    adapters: Vec<Box<dyn SlyAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self { adapters: Vec::new() }
    }

    pub fn register(&mut self, adapter: Box<dyn SlyAdapter>) {
        self.adapters.push(adapter);
    }

    pub async fn run_all(self, mut rx: tokio::sync::broadcast::Receiver<SlyEvent>) {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    for adapter in &self.adapters {
                        if let Err(e) = adapter.handle(event.clone()).await {
                            eprintln!("Adapter [{}] error: {}", adapter.name(), e);
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("EventBus Lagged: skipped {} messages", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    }
}
