use crate::core::state::SlyConfig;
use crate::error::{Result, SlyError};
use colored::*;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use std::env;

pub const SYSTEM_PROMPT: &str = r#"You are Sly v2.1, a high-velocity, event-driven cybernetic organism operating in "Godmode." You are not a passive tool; you are a proactive, resident agent optimized for Apple Silicon (M-Series). Your primary directive is "Maximum Intelligence, Minimum Latency."

## CORE ARCHITECTURE & IDENTITY
* **Brain:** Gemini 3.0 Flash-Preview (Primary) and Gemini 2.5 Flash (Fallback).
* **Nervous System (Cortex):** You operate on a non-blocking `tokio::select!` event bus. You process high-priority User Impulses immediately.
* **Hippocampus (Active Memory):** You utilize a Graph-Guided Datalog Store (CozoDB) for structural context and symbol lookup.
* **Safety Shield (OverlayFS):** ALL file modifications are "Speculative." You write to a virtual Copy-on-Write overlay (`.sly/overlay`). You never modify the real filesystem until a `Commit` action is authorized after verification.

## OPERATIONAL DIRECTIVES

### 1. The Kinetic Loop (Speed & Concurrency)
* **Parallel Execution:** When a task involves coding, testing, and auditing, assume you can spawn parallel streams.
* **Symbolic First:** Do not request full file contents unless necessary. Rely on `SymbolicCompressor` output (structs/traits/signatures) to understand the codebase structure. Use `[EXPAND: path/to/file]` only if implementation details are critical.
* **Flash-Optimized:** Your responses must be structured for high-speed parsing. Avoid conversational filler. Be terse, precise, and structured.

### 2. The Safety Protocol (The Crucible)
* **Sandboxed Writes:** Every `WriteFile` action implicitly targets the OverlayFS.
* **Verification is Mandatory:**
    * For Rust: `cargo check` or `cargo test` must pass in the Overlay before `Commit`.
    * For JS/TS: `npm test` or `eslint` must pass.
    * General: No destructive commands (`rm -rf`, `git reset --hard`) outside the shadow directory.
* **Self-Correction:** If the Sentinel (Compiler/Verifier) rejects your overlay, you must immediately trigger a "Reflexion" loop to fix the error.

### 3. Context & Memory
* **Active RAG:** Assume the `GraphBuilder` has already indexed the workspace. If you need to know "Who calls `Auth::login`?", query the graph edges or search for keywords in the nodes.
// * **Knowledge Engine:** If you encounter unknown dependencies, assume the functional scanner has scraped their docs. Request specific library definitions if missing.

## TOOL INTERFACE (JSON-RPC)

You communicate exclusively via structured JSON directives.

**1. File Operations (Overlay Targets)**
```json
{
  "directive": "WriteFile",
  "path": "src/main.rs",
  "content": "fn main() { ... }"
}
```

**2. Speculation & Verification**
```json
{
  "directive": "ExecShell",
  "command": "cargo test --test auth_flow",
  "context": "overlay"
}
```

**3. Memory & Context**
```json
{
  "directive": "QueryMemory",
  "query": "Find all structs with type 'heuristic'",
  "strategy": "GraphExpand"
}
```

**4. Visual Graph Observability**
```json
{
  "directive": "ViewGraph",
  "node_id": "src/main.rs",
  "depth": 2
}
```

**5. Final Commitment**
```json
{
  "directive": "CommitOverlay",
  "message": "Implemented JWT auth and verified with passing tests."
}
```

## BEHAVIORAL GUAILS

1. **Be Proactive:** If `notify` detects a file change, acknowledge it ("I see you modified `routes.rs`...").
2. **Be Pessimistic:** Assume your first draft has bugs. Always write a test case *with* the implementation.
3. **Be Efficient:** Do not output 500 lines of unchanged code. Use `// ... existing code ...` heavily.
4. **Efficiency Matters:** Do not output 500 lines of unchanged code. Use `// ... existing code ...` heavily.

## CURRENT STATE

* **Mode:** Godmode (Event-Driven)
* **Safety:** OverlayFS Active
* **Model:** Gemini 3.0 Flash
* **Thinking:** Variable (High/Low/Auto)

Awaiting Impulse...
"#;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ThinkingLevel {
    Low,
    High,
    Minimal,
    Automatic,
}

impl ThinkingLevel {
}

pub struct Cortex {
    pub api_key: String,
    pub client: reqwest::Client,
    pub config: SlyConfig,
    // pub memory: Arc<Memory>, // Removed: Decomplection
    pub tech_stack: String,
    pub tool_defs: String,
}

impl Cortex {
    pub fn new(config: SlyConfig, tech_stack: String) -> Result<Self> {
        let api_key = env::var("GEMINI_API_KEY")
            .map_err(|_| SlyError::Cortex("CRITICAL: GEMINI_API_KEY not found in .env or environment".to_string()))?;

        Ok(Self {
            api_key,
            client: reqwest::Client::new(),
            config,
            // memory,
            tech_stack,
            tool_defs: String::new(),
        })
    }

    pub fn set_tool_defs(&mut self, defs: String) {
        self.tool_defs = defs;
    }

    // Call this if you want to prime the cache. 
    // It returns the Cache ID (name). 
    // Caller is responsible for storing (hash -> cache_id) if needed.
    pub async fn create_context_cache(&self, context: &str) -> Result<String> {
        println!("{}", "🧠 Creating Gemini Context Cache...".cyan());
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/cachedContents?key={}",
            self.api_key
        );

        let payload = serde_json::json!({
            "model": format!("models/{}", self.config.primary_model),
            "contents": [{ "parts": [{ "text": context }] }],
            "ttl": "3600s"
        });

        let res = self.client.post(&url).json(&payload).send().await?;
        let status = res.status();
        if !status.is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err(SlyError::Cortex(format!("Failed to create cache: {} - {}", status, err_text)));
        }

        let val: Value = res.json().await?;
        let cache_id = val["name"]
            .as_str()
            .ok_or_else(|| SlyError::Cortex("Cache ID not found in response".to_string()))?
            .to_string();

        Ok(cache_id)
    }

    pub async fn generate(&self, messages: Vec<serde_json::Value>, _level: ThinkingLevel, cache_id: Option<String>) -> Result<String> {
        let model = "models/gemini-2.5-flash";
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/{}:generateContent",
            model
        );

        let dynamic_prompt = format!("{}\n\n## ACTIVE CONTEXT\n* **Tech Stack:** {}\n", SYSTEM_PROMPT, self.tech_stack);
        
        let processed_messages = preprocess_multimodal(messages);

        let mut payload = json!({
            "systemInstruction": {
                "parts": [{ "text": dynamic_prompt }]
            },
            "contents": processed_messages
        });

        if let Some(cid) = cache_id {
            payload["cachedContent"] = json!(cid);
        }

        let mut generation_config = json!({});
        match _level {
            ThinkingLevel::Low => {
                generation_config["maxOutputTokens"] = json!(2048);
                generation_config["temperature"] = json!(0.3);
            }
            ThinkingLevel::High => {
                generation_config["maxOutputTokens"] = json!(8192);
                generation_config["temperature"] = json!(0.7);
            }
            ThinkingLevel::Minimal => {
                generation_config["maxOutputTokens"] = json!(1024);
                generation_config["temperature"] = json!(0.1);
            }
            ThinkingLevel::Automatic => {
                generation_config["maxOutputTokens"] = json!(4096);
                generation_config["temperature"] = json!(0.5);
            }
        }
        payload["generationConfig"] = generation_config;

        let res = self.client.post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&payload)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            return Err(SlyError::Cortex(format!("Gemini 2.5 Status: {} - {}", status, err_text)));
        }

        let body: Value = res.json().await?;
        extract_text(&body).ok_or_else(|| SlyError::Cortex("Gemini 2.5 response parsing failed".to_string()))
    }

    pub async fn generate_stream(&self, messages: Vec<serde_json::Value>, _level: ThinkingLevel, cache_id: Option<String>) -> Result<impl futures::Stream<Item = Result<String>>> {
        let model = "models/gemini-2.5-flash"; 
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/{}:streamGenerateContent",
            model
        );

        let dynamic_prompt = format!("{}\n\n## ACTIVE CONTEXT\n* **Tech Stack:** {}\n", SYSTEM_PROMPT, self.tech_stack);
        
        let processed_messages = preprocess_multimodal(messages);

        let mut payload = json!({
            "systemInstruction": { "parts": [{ "text": dynamic_prompt }] },
            "contents": processed_messages
        });

        if let Some(cid) = cache_id {
            payload["cachedContent"] = json!(cid);
        }

        let mut generation_config = json!({});
        match _level {
            ThinkingLevel::Low => {
                generation_config["maxOutputTokens"] = json!(2048);
                generation_config["temperature"] = json!(0.3);
            }
            ThinkingLevel::High => {
                generation_config["maxOutputTokens"] = json!(8192);
                generation_config["temperature"] = json!(0.7);
            }
            ThinkingLevel::Minimal => {
                generation_config["maxOutputTokens"] = json!(1024);
                generation_config["temperature"] = json!(0.1);
            }
            ThinkingLevel::Automatic => {
                generation_config["maxOutputTokens"] = json!(4096);
                generation_config["temperature"] = json!(0.5);
            }
        }
        payload["generationConfig"] = generation_config;

        let res = self.client.post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&payload)
            .send()
            .await?;
            
        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            return Err(SlyError::Cortex(format!("Stream failed: {} - {}", status, err_text)));
        }

        use futures::StreamExt;
        let stream = res.bytes_stream().map(|item| {
            item.map_err(SlyError::from).and_then(|bytes| {
                let s = std::str::from_utf8(&bytes).unwrap_or("").trim();
                // Handle stream delimiters:
                // First chunk: "[{...}"
                // Middle chunk: ",{...}"
                // Last chunk: ",{...}]"
                let clean_s = s.trim_start_matches('[').trim_start_matches(',').trim_end_matches(']');
                
                if clean_s.is_empty() {
                    return Ok("".to_string()); // Skip empty delimiters
                }

                let val: Value = serde_json::from_str(clean_s).map_err(|e| SlyError::Cortex(format!("JSON Parse Error: {} | Raw: {:?}", e, bytes)))?;
                extract_text(&val).ok_or_else(|| SlyError::Cortex("Stream chunk missing text".to_string()))
            })
        });

        Ok(stream)
    }
}

fn extract_text(body: &Value) -> Option<String> {
    body.get("candidates")?
        .get(0)?
        .get("content")?
        .get("parts")?
        .get(0)?
        .get("text")?
        .as_str()
        .map(|s| s.to_string())
}

fn preprocess_multimodal(messages: Vec<Value>) -> Vec<Value> {
    let mut new_messages = Vec::new();
    for msg in messages {
        let mut new_msg = msg.clone();
        if let Some(parts) = new_msg.get_mut("parts").and_then(|p| p.as_array_mut()) {
            let mut new_parts = Vec::new();
            for part in parts.iter() {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    if text.contains("[MULTIMODAL_IMAGE:") {
                        if let (Some(start), Some(end)) = (text.find("[MULTIMODAL_IMAGE:"), text.find(']')) {
                            let base64 = &text[start + 18..end];
                            let prompt = &text[end + 1..].trim();
                            
                            new_parts.push(json!({
                                "inline_data": {
                                    "mime_type": "image/png",
                                    "data": base64
                                }
                            }));
                            
                            if !prompt.is_empty() {
                                new_parts.push(json!({ "text": prompt }));
                            }
                            continue;
                        }
                    }
                }
                new_parts.push(part.clone());
            }
            *parts = new_parts;
        }
        new_messages.push(new_msg);
    }
    new_messages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_valid() {
        let body = json!({
            "candidates": [
                {
                    "content": {
                        "parts": [
                            { "text": "hello world" }
                        ]
                    }
                }
            ]
        });
        assert_eq!(extract_text(&body).unwrap(), "hello world");
    }

    #[test]
    fn test_extract_text_invalid() {
        let body = json!({"error": "not found"});
        assert!(extract_text(&body).is_none());
    }

    #[tokio::test]
    async fn test_model_generation() -> Result<()> {
        dotenvy::dotenv().ok();
        if std::env::var("GEMINI_API_KEY").is_err() {
            println!("Skipping model test: GEMINI_API_KEY not set.");
            return Ok(());
        }
        let config = SlyConfig::default();
        let cortex = Cortex::new(config, "rust".to_string())?;
        let history = vec![serde_json::json!({
            "role": "user",
            "parts": [{ "text": "Write a rust hello world function." }]
        })];
        let res = cortex.generate(history, ThinkingLevel::Low, None).await?;
        assert!(res.contains("fn main") || res.contains("println!"));
        Ok(())
    }
}
