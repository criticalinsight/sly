//! LLM inference via OS-native `curl`.
//!
//! Supports Gemini (default) and any OpenAI-compatible endpoint
//! via the `SLY_OPENAI_URL` environment variable.

use crate::state::SlyConfig;
use crate::error::{Result, SlyError};
use std::process::Command;

/// Barebones JSON escaping for strings
fn escape_json(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            '\n' => vec!['\\', 'n'],
            '\r' => vec!['\\', 'r'],
            '\t' => vec!['\\', 't'],
            c if c.is_control() => format!("\\u{:04x}", c as u32).chars().collect(),
            c => vec![c],
        })
        .collect()
}

// --- Data Structures for API Requests ---

struct OpenAIMessage<'a> {
    role: &'a str,
    content: &'a str,
}

struct OpenAIRequest<'a> {
    model: &'a str,
    messages: Vec<OpenAIMessage<'a>>,
}

impl<'a> OpenAIRequest<'a> {
    fn to_json(&self) -> String {
        let msgs: Vec<String> = self.messages.iter()
            .map(|m| format!(r#"{{"role": "{}", "content": "{}"}}"#, m.role, escape_json(m.content)))
            .collect();
        format!(r#"{{"model": "{}", "messages": [{}]}}"#, self.model, msgs.join(", "))
    }
}

struct GeminiRequest<'a> {
    system: &'a str,
    user: &'a str,
}

impl<'a> GeminiRequest<'a> {
    fn to_json(&self) -> String {
        let sys_json = format!(r#"{{"parts": [{{"text": "{}"}}]}}"#, escape_json(self.system));
        let user_json = format!(r#"{{"parts": [{{"text": "{}"}}]}}"#, escape_json(self.user));
        format!(r#"{{"systemInstruction": {}, "contents": [{}]}}"#, sys_json, user_json)
    }
}

/// The default system prompt that teaches the LLM the action schema.
const AGENT_SYSTEM_PROMPT: &str = r#"You are Sly, an autonomous coding agent. You MUST respond using JSON inside ```json code blocks.

Available actions:
1. Write a file:
```json
{"directive": "WriteFile", "path": "relative/path.ext", "content": "full file contents"}
```

2. Execute a shell command:
```json
{"directive": "ExecShell", "command": "shell command here"}
```

3. Give a text answer:
```json
{"directive": "Answer", "text": "your answer"}
```

4. Signal task completion:
```json
{"directive": "FinalResponse", "title": "Title", "summary": "What was done"}
```

Rules:
- Always use ```json code blocks for actions.
- You may include multiple action blocks in one response.
- After executing actions you will receive Observations with results.
- When the task is fully complete, send FinalResponse.
- Think step by step. Execute one action at a time when uncertain."#;

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
    /// A system prompt with the action schema is always injected.
    pub fn generate_sync(
        &self,
        prompt: String,
        _level: ThinkingLevel,
        _system_prompt: Option<String>,
    ) -> Result<String> {
        let sys = AGENT_SYSTEM_PROMPT;

        let (url, data, auth_header) = if let Ok(openai_url) = std::env::var("SLY_OPENAI_URL") {
            let req = OpenAIRequest {
                model: &self.config.primary_model,
                messages: vec![
                    OpenAIMessage { role: "system", content: sys },
                    OpenAIMessage { role: "user", content: &prompt },
                ],
            };
            let auth = std::env::var("OPENAI_API_KEY").unwrap_or_default();
            let auth_header = format!("Authorization: Bearer {}", auth);
            (openai_url, req.to_json(), auth_header)
        } else {
            let api_key = std::env::var("GEMINI_API_KEY")
                .map_err(|_| SlyError::Config("GEMINI_API_KEY (or SLY_OPENAI_URL) not set".into()))?;
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                self.config.primary_model, api_key
            );
            let req = GeminiRequest { system: sys, user: &prompt };
            (url, req.to_json(), String::new())
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
