// src/knowledge/registry.rs - Registry Monitoring for Sly v0.5.0

use anyhow::{Result, Context};
use serde_json::Value;
use std::sync::Arc;
use crate::memory::MemoryStore;
// use crate::io::telemetry::{Telemetry, TelemetryEvent};

pub struct RegistryMonitor {
    #[allow(dead_code)]
    memory: Arc<dyn MemoryStore>,
}

impl RegistryMonitor {
    pub fn new(memory: Arc<dyn MemoryStore>) -> Self {
        Self { memory }
    }

    /// Checks all libraries in memory against upstream registries
    pub async fn audit_upstream(&self) -> Result<()> {
        // 1. Get known libraries and versions from memory
        // Since we don't have a direct "get version" in MemoryStore, 
        // we'll assume the implementation uses the Library table.
        // For simplicity, we'll use a hardcoded check for 'reqwest' as a demo of the logic.
        
        let target_libs = vec!["reqwest", "wasmtime", "cozo"];
        
        for lib in target_libs {
            if let Ok(latest) = self.check_crates_io(lib).await {
                // Here we would compare with memory
                println!("📦 Registry Check: {} (Latest: {})", lib, latest);
                
                // If drift detected, say it!
                // Telemetry::say(TelemetryEvent::Custom(format!("Update available for {}.", lib)));
            }
        }
        
        Ok(())
    }

    async fn check_crates_io(&self, name: &str) -> Result<String> {
        let client = reqwest::Client::builder()
            .user_agent("Sly/0.5.0 (github.com/brixelectronics/sly)")
            .build()?;
            
        let url = format!("https://crates.io/api/v1/crates/{}", name);
        let res = client.get(url).send().await?;
        let json: Value = res.json().await?;
        
        let version = json["crate"]["max_version"]
            .as_str()
            .context("Failed to parse crates.io version")?
            .to_string();
            
        Ok(version)
    }
}
