# Sly Architecture: The Flat Logic Tree 🧙🏾‍♂️

> "Simplicity is the absence of complecting."
> — Rich Hickey

Sly is a **Zero-Lib**, **Synchronous**, and
**Linearized** engine for bare-metal performance.

## The Strategic Pillar: Linear Logic

Unlike traditional agentic systems that use complex
"Command Protocols" and "State Objects", Sly uses a
**Direct Stream**.

### 1. Flat Root Structure

All modules reside in `src/`. No nesting.

- `control.rs` — OODA loop with Ralph Loop
  Reflexion.
- `cortex.rs` — LLM interface via `curl`.
- `memory.rs` — Mortal memory with rolling window.
- `parser.rs` — Zero-Serde JSON extraction.
- `io.rs` — Standard I/O handler.
- `safety.rs` — Transactional overlay filesystem.
- `state.rs` — Global configuration bundle.
- `error.rs` — Unified error type.

### 2. Protocol-less Execution

No intermediate command enums. No message bus.

- **Observe**: Read raw `String` from `io.rs`.
- **Orient**: `cortex.rs` sends context via `curl`.
- **Decide**: `parser.rs` extracts JSON actions.
- **Act**: `control.rs` dispatches them.

### 3. Mortal Data-at-Rest

There is no `AgentSession` object lifecycle.
A session is a `Vec<String>` stored as a text file.
We operate on values, not objects.

### 4. Ralph Loop Reflexion

Failed shell commands inject a reflexion primer
into the observation stream, forcing the LLM to
analyze stderr objectively before retrying.

## Performance Metrics (v0.3.5)

- **Release Build Time**: ~0.59s
- **Binary Size**: 522KB
- **Compiler Warnings**: 0
- **Test Suite**: 26 tests, 0 failures
- **Execution**: Predictable vertical flow. Zero
  async complection.
