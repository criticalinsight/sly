//! LLM inference via OS-native `curl`.
//!
//! Supports Gemini (default) and any OpenAI-compatible endpoint
//! via the `SLY_OPENAI_URL` environment variable.

use crate::state::SlyConfig;
use crate::error::{Result, SlyError};
use std::process::Command;

/// Controls the reasoning budget sent to the model.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ThinkingLevel {
    #[default]
    Low,
}

/// The inference gateway. Wraps a `curl` call to the configured LLM.
pub struct Cortex {
    config: SlyConfig,
    _api_key: String,
}

impl Cortex {
    /// Create a new Cortex bound to the given config and API key.
    pub fn new(config: SlyConfig, api_key: String) -> Result<Self> {
        Ok(Self { config, _api_key: api_key })
    }

    /// Perform a synchronous LLM generation.
    ///
    /// # Endpoint Selection
    ///
    /// 1. If `SLY_OPENAI_URL` is set → uses OpenAI-compatible chat format.
    /// 2. Otherwise → uses Gemini `generateContent` with `GEMINI_API_KEY`.
    ///
    /// Response text is extracted using Zero-Serde JSON helpers from
    /// [`crate::parser`].
    pub fn generate_sync(
        &self,
        prompt: String,
        _level: ThinkingLevel,
        _system_prompt: Option<String>,
    ) -> Result<String> {
        let (url, data, auth_header) = if let Ok(openai_url) = std::env::var("SLY_OPENAI_URL") {
            let data = format!(
                r#"{{"model": "{}", "messages": [{{"role": "user", "content": {:?}}}]}}"#,
                self.config.primary_model, prompt
            );
            let auth = std::env::var("OPENAI_API_KEY").unwrap_or_default();
            let auth_header = format!("Authorization: Bearer {}", auth);
            (openai_url, data, auth_header)
        } else {
            let api_key = std::env::var("GEMINI_API_KEY")
                .map_err(|_| SlyError::Config("GEMINI_API_KEY (or SLY_OPENAI_URL) not set".into()))?;
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                self.config.primary_model, api_key
            );
            let data = format!(r#"{{"contents": [{{"parts":[{{"text": {:?}}}]}}]}}"#, prompt);
            (url, data, String::new())
        };

        let mut cmd = Command::new("curl");
        cmd.arg("-s")
            .arg("-X").arg("POST")
            .arg("-H").arg("Content-Type: application/json");

        if !auth_header.is_empty() {
            cmd.arg("-H").arg(&auth_header);
        }

        let output = cmd.arg("-d").arg(&data).arg(&url).output()?;

        if !output.status.success() {
            return Err(SlyError::Cortex(
                format!("CURL failed: {}", String::from_utf8_lossy(&output.stderr)),
            ));
        }

        let res_body = String::from_utf8_lossy(&output.stdout);

        // Zero-Serde: extract "text" (Gemini) or "content" (OpenAI) field.
        if let Some(text) = crate::parser::find_json_val(&res_body, "text")
            .or_else(|| crate::parser::find_json_val(&res_body, "content"))
        {
            Ok(text)
        } else {
            Err(SlyError::Json(format!("Failed to parse response: {}", res_body)))
        }
    }
}
