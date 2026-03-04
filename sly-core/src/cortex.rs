//! LLM inference via OS-native `curl`.
//!
//! Supports Gemini (default) and any OpenAI-compatible endpoint
//! via the `SLY_OPENAI_URL` environment variable.

use crate::state::SlyConfig;
use crate::error::{Result, SlyError};
use std::process::Command;

// Removed handwritten escape_json in favor of serde_json

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
        let msgs: Vec<serde_json::Value> = self.messages.iter()
            .map(|m| serde_json::json!({ "role": m.role, "content": m.content }))
            .collect();
        serde_json::json!({
            "model": self.model,
            "messages": msgs
        }).to_string()
    }
}

struct GeminiRequest<'a> {
    system: &'a str,
    user: &'a str,
}

impl<'a> GeminiRequest<'a> {
    fn to_json(&self) -> String {
        serde_json::json!({
            "systemInstruction": { "parts": [{ "text": self.system }] },
            "generationConfig": { "responseMimeType": "application/json" },
            "contents": [{ "parts": [{ "text": self.user }] }]
        }).to_string()
    }
}

/// The default system prompt that teaches the LLM the action schema.
const AGENT_SYSTEM_PROMPT: &str = r#"You are Sly, an autonomous coding agent. You MUST respond with exactly ONE mathematically pure JSON object per turn. Do NOT wrap your response in ```json ticks or Markdown.

Every action MUST include a "thought" key to explain your reasoning, followed by the action directive.

Available actions:
1. Write a file:
{
  "thought": "I need to initialize the main entry point.",
  "directive": "WriteFile",
  "path": "relative/path.ext",
  "content": "full file contents"
}

2. Execute a shell command:
{
  "thought": "I need to compile the code.",
  "directive": "ExecShell",
  "command": "shell command here"
}

3. Give a text answer:
{
  "thought": "I need to explain the solution.",
  "directive": "Answer",
  "text": "your answer"
}

4. Read a file:
{
  "thought": "I need to inspect the existing code.",
  "directive": "ReadFile",
  "path": "relative/path.ext"
}

5. Signal task completion:
{
  "thought": "The task is fully complete.",
  "directive": "FinalResponse",
  "title": "Title",
  "summary": "What was done"
}

Rules:
- Output exactly ONE JSON object per turn.
- No markdown formatting like ```json.
- No conversational text outside the JSON.
- After executing actions you will receive Observations with results.
- When the task is fully complete, send FinalResponse.
- Persistence: Changes made via WriteFile are staged in an overlay and only committed to disk upon FinalResponse or Answer.
- Think step by step using the "thought" key."#;

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
        messages: &[String],
        _level: ThinkingLevel,
        _system_prompt: Option<String>,
    ) -> Result<String> {
        let sys = AGENT_SYSTEM_PROMPT;
        let openai_url_opt = std::env::var("SLY_OPENAI_URL").ok().filter(|s| !s.is_empty());

        let (url, data, auth_header) = if let Some(openai_url) = openai_url_opt {
            let mut api_msgs = vec![OpenAIMessage { role: "system", content: sys }];
            for msg in messages {
                if let Some(content) = msg.strip_prefix("MODEL: ") {
                    api_msgs.push(OpenAIMessage { role: "assistant", content });
                } else if let Some(content) = msg.strip_prefix("USER: ") {
                    api_msgs.push(OpenAIMessage { role: "user", content });
                } else {
                    api_msgs.push(OpenAIMessage { role: "user", content: msg });
                }
            }
            
            let req = OpenAIRequest {
                model: &self.config.primary_model,
                messages: api_msgs,
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
            
            // Gemini expects alternating user/model turns. For simplicity in zero-library mode,
            // we concatenate the history into a structured prompt, as building the complex
            // parts/contents arrays manually is highly error-prone.
            let formatted_history = messages.join("\n\n");
            
            let req = GeminiRequest { system: sys, user: &formatted_history };
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

        let json: serde_json::Value = serde_json::from_str(&res_body)
            .map_err(|_| SlyError::Json(format!("Failed to parse JSON response: {}", res_body)))?;

        // Extract content via JSON pointers based on API provider format
        if let Some(content) = json.pointer("/choices/0/message/content").and_then(|v| v.as_str()) {
            Ok(content.to_string())
        } else if let Some(text) = json.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()) {
            Ok(text.to_string())
        } else {
            Err(SlyError::Json(format!("Response missing expected text/content fields: {}", res_body)))
        }
    }
}
