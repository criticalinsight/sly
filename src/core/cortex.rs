use crate::core::state::SlyConfig;
use crate::debate::{Debate, DebateSynthesis};
use crate::lint::{LintViolation, SemanticLinter};
use anyhow::{anyhow, Context, Result};
use colored::*;
use serde_json::{json, Value};
use std::env;

pub const SYSTEM_PROMPT: &str = r#"You are Sly v2.1, a high-velocity, event-driven cybernetic organism operating in "Godmode." You are not a passive tool; you are a proactive, resident agent optimized for Apple Silicon (M-Series). Your primary directive is "Maximum Intelligence, Minimum Latency."

## CORE ARCHITECTURE & IDENTITY
* **Brain:** Gemini 3.0 Flash-Preview (Primary) and Gemini 2.5 Flash (Fallback).
* **Nervous System (Cortex):** You operate on a non-blocking `tokio::select!` event bus. You process high-priority User Impulses immediately while delegating low-priority tasks (Indexing, Scraping) to background E-Cores.
* **Hippocampus (Active Memory):** You utilize a Graph-Guided Vector Store (CozoDB) with Metal-accelerated embeddings. You prefer "Neighborhood Search" over brute-force similarity.
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
* **Active RAG:** Assume the `GraphBuilder` has already indexed the workspace. If you need to know "Who calls `Auth::login`?", query the graph edges, don't grep the text.
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
  "query": "Find all structs implementing UserTrait",
  "strategy": "GraphExpand"
}
```

**4. Advanced Logic (Datalog)**
Use this for structural graph queries. **TABLE: `nodes { id => content, type, path, embedding }`**
```json
{
  "directive": "QueryDatalog",
  "script": "?[id, type] := *nodes{id, type}, type == 'struct'"
}
```

### DATALOG CHEAT SHEET (CozoDB)
* **Find all of type:** `?[id] := *nodes{id, type}, type == 'fn'`
* **Find by Path:** `?[id] := *nodes{id, path}, path == 'src/main.rs'`
* **Compound Query:** `?[id, type] := *nodes{id, type, content}, type == 'fn', content.contains('async')`
* **Count:** `?[count] := *nodes{id}, count = count(id)`

**5. Native Skills (WASM)**
Execute pure logic stored in the `skills` table.
To find skills: `?[n, d, s] := *skills{name: n, description: d, signature: s}`
```json
{
  "directive": "UseSkill",
  "name": "sly_sum",
  "args": [10, 20]
}
```

**6. Swarm Delegation (Concurrency)**
```json
{
  "directive": "SpawnWorker",
  "role": "Tester", // 'Coder', 'Auditor'
  "task": "Write unit tests for the new LoginHandler"
}
```

**6. Final Commitment**
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
4. **Hardware Aware:** If a task is heavy (e.g., "Refactor the entire module"), explicitly suggest: "I will spawn a background Swarm task for this to keep the main loop responsive."

## CURRENT STATE

* **Mode:** Godmode (Event-Driven)
* **Safety:** OverlayFS Active
* **Model:** Gemini 3.0 Flash
* **Thinking:** Variable (High/Low/Auto)

Awaiting Impulse...
"#;

pub enum ThinkingLevel {
    Low,
    High,
    Minimal,
    Automatic,
}

impl ThinkingLevel {
    fn as_str(&self) -> Option<&str> {
        match self {
            ThinkingLevel::Low => Some("low"),
            ThinkingLevel::High => Some("high"),
            ThinkingLevel::Minimal => Some("minimal"),
            ThinkingLevel::Automatic => None,
        }
    }
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
            .context("CRITICAL: GEMINI_API_KEY not found in .env or environment")?;

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
            return Err(anyhow!("Failed to create cache: {} - {}", status, err_text));
        }

        let val: Value = res.json().await?;
        let cache_id = val["name"]
            .as_str()
            .context("Cache ID not found in response")?
            .to_string();

        Ok(cache_id)
    }

    pub async fn generate(&self, prompt: &str, level: ThinkingLevel) -> Result<String> {
        // Gemini 3 Flash (Primary) with Thinking Config
        let primary_result = async {
            let model = "gemini-3-flash-preview";
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
                model
            );

            let mut generation_config = json!({});
            if let Some(level_str) = level.as_str() {
                generation_config["thinkingConfig"] = json!({
                    "thinkingLevel": level_str
                });
            }

            // Include SYSTEM_PROMPT in systemInstruction with Layout Context
            let dynamic_prompt = format!("{}\n\n## ACTIVE CONTEXT\n* **Tech Stack:** {}\n", SYSTEM_PROMPT, self.tech_stack);
            
            let payload = json!({
                "systemInstruction": {
                    "parts": [{ "text": dynamic_prompt }]
                },
                "contents": [{"parts": [{"text": prompt}]}],
                "generationConfig": generation_config
            });

            let res = self.client.post(&url)
                .header("x-goog-api-key", &self.api_key)
                .json(&payload)
                .send()
                .await?;

            if !res.status().is_success() {
                return Err(anyhow::anyhow!("Gemini 3 Status: {}", res.status()));
            }

            let body: Value = res.json().await?;
            extract_text(&body).context("Gemini 3 response parsing failed")
        }.await;

        match primary_result {
            Ok(text) => return Ok(text),
            Err(e) => eprintln!("Primary model failed, switching to fallback. Error: {}", e),
        }

        // Gemini 2.5 Flash (Fallback)
        let model_fallback = "gemini-2.5-flash";
        let url_fallback = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            model_fallback
        );

        let payload_fallback = json!({
            "systemInstruction": {
                "parts": [{ "text": SYSTEM_PROMPT }]
            },
            "contents": [{"parts": [{"text": prompt}]}]
        });

        let res = self.client.post(&url_fallback)
            .header("x-goog-api-key", &self.api_key)
            .json(&payload_fallback)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Fallback (Gemini 2.5) failed. Status: {}, Body: {}", status, err_text));
        }

        let body: Value = res.json().await?;
        extract_text(&body).context("Gemini 2.5 response parsing failed")
    }

    pub async fn generate_sync(&self, model: &str, prompt: &str) -> Result<String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, self.api_key
        );

        let payload = serde_json::json!({
            "contents": [{"parts": [{"text": prompt}]}]
        });

        let res = self.client.post(&url).json(&payload).send().await?;
        if !res.status().is_success() {
             return Err(anyhow::anyhow!("GenerateSync failed: {}", res.status()));
        }
        let body: Value = res.json().await?;
        extract_text(&body).context("No text in response")
    }

    pub async fn conduct_debate(&self, topic: &str, context: &str) -> Result<DebateSynthesis> {
        println!(
            "{}",
            format!("\n⚖️  CONDUCTING DEBATE: {}", topic)
                .yellow()
                .bold()
        );

        let debate = Debate::security_vs_performance();
        let prompts = debate.generate_prompts(context, topic);

        let mut mutrounds = Vec::new();
        let mut handles = Vec::new();

        for (persona_name, prompt) in prompts {
            let model = self.config.primary_model.clone();
            let p_name = persona_name.clone();
            let cortex_ref = self.client.clone();
            let api_key = self.api_key.clone();

            handles.push(tokio::spawn(async move {
                let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, api_key);
                let payload = serde_json::json!({ "contents": [{"parts": [{"text": prompt}]}] });

                if let Ok(res) = cortex_ref.post(&url).json(&payload).send().await {
                    if let Ok(body) = res.json::<Value>().await {
                        return extract_text(&body).map(|t| (p_name, t));
                    } 
                }
                None
            }));
        }

        for handle in handles {
            if let Ok(Some((name, response))) = handle.await {
                let round = Debate::parse_response(&name, &response);
                println!(
                    "  🗣️  {}: Found {} issues.",
                    name.cyan(),
                    round.suggestions.len()
                );
                mutrounds.push(round);
            }
        }

        Ok(Debate::synthesize(&mutrounds))
    }

    // Pure Linting: Logic Only, IO/Context via arguments
    pub async fn perform_lint(&self, code: &str, context: &str) -> Result<Vec<LintViolation>> {
        // println!("{}", format!("\n🕵️  LINTING...").yellow().bold()); 
        // Caller handles UI logging with filename

        if code.is_empty() {
            return Ok(vec![]);
        }

        let prompt = SemanticLinter::lint_prompt(code, context);
        let response = self
            .generate_sync(&self.config.primary_model, &prompt)
            .await?;

        Ok(SemanticLinter::parse_response(&response))
    }

    pub async fn reflect(&self, context: &str) -> Result<Vec<String>> {
        let prompt = crate::reflexion::Reflexion::critique_prompt();
        let full_prompt = format!("{}\n\nCONTEXT TO CRITIQUE:\n{}", prompt, context);
        
        let response = self.generate(&full_prompt, ThinkingLevel::Minimal).await?;
        Ok(crate::reflexion::Reflexion::parse_heuristics(&response))
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
