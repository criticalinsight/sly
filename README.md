# Sly 🧠

**Zero-library autonomous coding agent.** Built entirely on the Rust standard library + `curl`. No `tokio`, no `reqwest`, no frameworks. The complexity budget goes to the LLM, not the engine.

## Architecture

```
User Input → OODA Loop → LLM (Gemini / OpenAI-compat) → JSON Actions → Overlay FS → Commit
```

| Module     | Responsibility                          |
|------------|-----------------------------------------|
| `control`  | OODA heartbeat loop                     |
| `cortex`   | LLM API call (Gemini / OpenAI-compat)   |
| `memory`   | Session persistence with mortal window  |
| `parser`   | JSON action extraction via `serde_json` |
| `safety`   | Transactional overlay filesystem        |
| `io`       | Stdin/stdout CLI adapter                |
| `state`    | Global configuration and state bundle   |
| `error`    | Unified error type                      |

## Quick Start

```bash
# Build
cargo build --release

# Run with Gemini
export GEMINI_API_KEY="your-key"
./target/release/sly

# Run with local Ollama
export SLY_MODEL="qwen3.5:latest"
export SLY_OPENAI_URL="http://localhost:11434/v1/chat/completions"
./target/release/sly

# Pipe mode (auto-detected)
echo "Create a hello.py that prints hello world" | ./target/release/sly
```

## Directives

The agent communicates via strict JSON actions:

| Directive | Description |
|:----------|:------------|
| `WriteFile` | Write content to a file (staged in overlay) |
| `ReadFile` | Read a file from the scratchpad |
| `ExecShell` | Execute a shell command |
| `Answer` | Return a text answer + commit overlay |
| `FinalResponse` | Signal task completion + commit overlay |

## Slash Commands

| Command | Action |
|:--------|:-------|
| `/commit` | Commit overlay to disk |
| `/undo` | Rollback overlay changes |
| `/files` | List overlay contents |
| `/status` | Print message trace |
| `/stop` | Graceful shutdown |

## Safety

All file writes go through a **transactional overlay**:
- `WriteFile` stages to temp dir + scratchpad
- `ExecShell` runs in the scratchpad (can see staged files)
- Files persist to real disk only on `Answer`, `FinalResponse`, or `/commit`
- `/undo` wipes all staged changes

## Configuration

| Env Var | Default | Description |
|:--------|:--------|:------------|
| `SLY_MODEL` | `qwen3:8b` | Model identifier |
| `SLY_OPENAI_URL` | — | OpenAI-compatible endpoint URL |
| `GEMINI_API_KEY` | — | Gemini API key (falls back to Gemini if no OpenAI URL) |

## Philosophy

> *"Simple made easy"* — Rich Hickey

Every feature is one function doing one thing. `commit()` copies files. `rollback()` deletes them. `parse_action()` extracts actions. No frameworks, no traits, no generics. The complexity budget goes to the LLM, not the engine.

## License

MIT
