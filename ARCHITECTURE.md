# Sly Architecture: The Flat Logic Tree 🧙🏾‍♂️

"Simplicity is the absence of complecting." — Rich Hickey

Sly follows the **Hickey Strategic Doctrine**. It is a **Zero-Lib**, **Synchronous**, and **Linearized** engine designed for bare-metal performance on Apple Silicon.

## 🏛 The Strategic Pillar: Linear Logic
Unlike traditional agentic systems that use complex "Command Protocols" (Impulses) and "State Objects" (Sessions), Sly uses a **Direct Stream**.

### 1. Flat Root Structure
All modules reside in `src/`. No nesting. Absolute visibility.
- `control.rs`: The single logical pulse (OODA loop).
- `cortex.rs`: The LLM interface (Orient phase).
- `memory.rs`: The Data-at-Rest.
- `parser.rs`: Action extraction without Serde.
- `io.rs`: Standard I/O handler.
- `safety.rs`: Transactional overlay filesystem.

### 2. Protocol-less Execution
We have excised the `Impulse` message layer and command patterns.
- **Observe**: Read raw `String` from `io.rs`.
- **Orient**: `Cortex` passes the context strings to the LLM via `curl`.
- **Decide/Act**: `Parser` extracts JSON actions directly; `Control` executes them.
- No intermediate command enums. No "message bus." Just blocking function calls.

### 3. Mortal Data-at-Rest
We deleted `session.rs` to stop state mirroring.
- There is no `AgentSession` object lifecycle to manage.
- A session is simply a `Vec<String>` stored in `memory.rs`.
- We operate on values, not objects with state-management logic.

## 🛠 Component Breakdown
- **Control Loop**: Single-threaded, synchronous loop in `control.rs`.
- **Zero-Lib**: Replaced `tokio`, `serde`, `reqwest`, and `axum` with the Rust Standard Library (`std::fs`, `std::process`, `std::net`).
- **OS-as-a-Library**: Direct use of `sh` and `curl`.

## 🔋 Performance Metrics (v0.3.5)
- **Release Build Time**: ~0.59s
- **Binary Size**: 522KB
- **Compiler Warnings**: 0
- **Execution**: Predictable vertical flow. No race conditions. Zero async complection.
