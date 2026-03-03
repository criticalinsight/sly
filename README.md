# Sly: The Zero-Lib Engine 🧙🏾‍♂️

Sly is an ultra-minimalist autonomous coding agent.
Built as a single, rapidly-compiled binary with
near-zero dependencies.

> "Simplicity is the absence of complecting."
> — Rich Hickey

## Key Metrics (v0.3.5)

- **Release Build**: `0.59s`
- **Binary Size**: `522KB`
- **Warnings**: `0`
- **Test Suite**: `26 tests, 0 failures`

## Architecture

Zero-Lib. Standard Library only. OS-as-a-Library.

- Replaced `tokio`, `axum`, `serde`, `reqwest`
  with `std` and native `curl` / `sh`.
- Synchronous OODA loop. No race conditions.

## Features

- **Local Inference**: Set `SLY_OPENAI_URL` for
  any OpenAI-compatible endpoint.
- **Ralph Loop Reflexion**: Failed commands trigger
  a reflexion primer that forces error analysis.
- **Execution Timeouts**: 60-second kill switch
  on shell commands.
- **Mortal Memory**: Rolling 20-message window
  prevents unbounded token growth.
- **Parser Hardening**: Survives truncated JSON
  and missing markdown fences.

## Setup

### Prerequisites

- Rust (latest stable)
- `curl` (system-wide)
- `GEMINI_API_KEY` or `SLY_OPENAI_URL` exported.

### Build & Run

```bash
cargo build --release --workspace
./target/release/sly
```

### Run Tests

```bash
cargo test --workspace
```

## Module Map

| Module       | Role                                    |
|--------------|-----------------------------------------|
| `control.rs` | OODA heartbeat loop                    |
| `cortex.rs`  | LLM API (Gemini / OpenAI-compat)       |
| `memory.rs`  | Session persistence, mortal window      |
| `parser.rs`  | Zero-Serde JSON action extraction       |
| `safety.rs`  | Transactional overlay filesystem        |
| `io.rs`      | Stdin/stdout CLI adapter                |
| `state.rs`   | Global config and state bundle          |
| `error.rs`   | Unified error type                      |

## Philosophy

1. **Values over Objects**: `Vec<String>` over
   state-management classes.
2. **De-complecting**: No protocol or command
   indirection layers.
3. **Linearity**: Vertical function calls over
   horizontal message buses.

## License

Apache License, Version 2.0.
