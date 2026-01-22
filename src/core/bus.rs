use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::Value;
use anyhow::Result;
use crate::core::state::GlobalState;
use async_trait::async_trait;

#[async_trait]
pub trait DirectiveHandler: Send + Sync {
    async fn handle(&self, data: Value, state: Arc<GlobalState>) -> Result<()>;
}

pub struct DirectiveBus {
    handlers: RwLock<HashMap<String, Box<dyn DirectiveHandler>>>,
}

impl DirectiveBus {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    pub async fn register<H>(&self, name: &str, handler: H)
    where
        H: DirectiveHandler + 'static,
    {
        let mut handlers = self.handlers.write().await;
        handlers.insert(name.to_string(), Box::new(handler));
    }

    pub async fn dispatch(&self, name: &str, data: Value, state: Arc<GlobalState>) -> Result<()> {
        let handlers = self.handlers.read().await;
        if let Some(handler) = handlers.get(name) {
            handler.handle(data, state).await
        } else {
            Err(anyhow::anyhow!("No handler registered for directive: {}", name))
        }
    }
}
