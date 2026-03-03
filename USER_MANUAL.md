# Sly User Manual

## 1. Introduction
Sly is a high-performance autonomous coding agent. It is designed to be "Decomplected"—minimizing technical debt and resource overhead by relying on the standard library and OS-level primitives.

## 2. Configuration
Sly uses minimalist configuration. Environment variables are managed via `.env` or system environment.

## 3. Usage

### Starting the Agent
```bash
cargo run -p sly-core
```
The agent will start the OODA loop (Observe, Orient, Decide, Act), using its internal Rich Hickey persona to guide its decisions.

### Monitoring Output
```bash
cargo run -p sly-monitor
```
The monitor polls project logs and provides color-coded terminal output to track the agent's thoughts, actions, and any errors.

## 4. Safety & Sandboxing
- **Path Guarding**: The agent is restricted to the current workspace. Attempting to write outside the root will be blocked.
- **Verification Loop**: Before any permanent change is committed, the agent runs validation scripts (e.g., `cargo check`).

## 5. Troubleshooting
- **Slow Responses**: Ensure `curl` is accessible and your `GEMINI_API_KEY` is valid.
- **Build Failures**: Check `supervisor.err` for compilation logs.
