use crate::state::SlyConfig;
use crate::error::{Result, SlyError};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ThinkingLevel {
    #[default]
    Low,
}

pub struct Cortex {
    config: SlyConfig,
    _api_key: String, 
}

impl Cortex {
    pub fn new(config: SlyConfig, api_key: String) -> Result<Self> {
        Ok(Self { config, _api_key: api_key })
    }

    pub fn generate_sync(
        &self, 
        prompt: String, 
        _level: ThinkingLevel,
        _system_prompt: Option<String>
    ) -> Result<String> {
        let api_key = std::env::var("GEMINI_API_KEY")
            .map_err(|_| SlyError::Config("GEMINI_API_KEY not set".into()))?;

        // Zero-Lib: Direct CURL execution for maximum simplicity & visibility
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", 
            self.config.primary_model, api_key);

        let data = format!(r#"{{"contents": [{{"parts":[{{"text": {:?}}}]}}]}}"#, prompt);
        
        let output = Command::new("curl")
            .arg("-s")
            .arg("-X")
            .arg("POST")
            .arg("-H")
            .arg("Content-Type: application/json")
            .arg("-d")
            .arg(&data)
            .arg(&url)
            .output()?;

        if !output.status.success() {
            return Err(SlyError::Cortex(format!("CURL failed: {}", String::from_utf8_lossy(&output.stderr))));
        }

        let res_body = String::from_utf8_lossy(&output.stdout);
        
        // Manual JSON extraction (Zero-Serde)
        if let Some(text) = crate::parser::find_json_val(&res_body, "text") {
            Ok(text)
        } else {
            Err(SlyError::Json(format!("Failed to parse response: {}", res_body)))
        }
    }
}
