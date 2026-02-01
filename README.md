# Sly 🦀

> **Maximum Intelligence, Minimum RAM. Persistent Autonomy.**

Sly is a high-performance, single-binary autonomous coding agent written in **Rust**. It is designed specifically for **Apple Silicon** to provide a lightning-fast, native AI pair programmer experience without the bloat of Python, Node.js, or Docker.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-macos-lightgrey.svg)](https://www.apple.com/macos/)

*   **Ephemeral Velocity**: Support for `--ephemeral` (in-memory) engines. Treat session data as transient **Values** instead of persistent **Places**, enabling zero-friction parallel execution.
*   **Dynamic Workflows**: Orchestrate complex multi-step routines (e.g., `/fix`, `/test`, `/docs`) via hot-swappable markdown templates in `.agent/workflows/`.
*   **Recursive Reflexion**: Autonomous self-healing OODA loops. If a shell command fails, Sly automatically spawns a sub-agent to analyze and fix the error without user intervention.
*   **Sovereign MCP Ecosystem**: 
    - **Auto-Discovery**: Zero-config detection of MCP servers in `~/.sly/mcp/`.
    - **Universal Knowledge Retrieval (UKR)**: Unified search layer across all disparate providers.
    - **Tool Chaining**: Persistent session results enable complex "piped" workflows between tools.
*   **Active Memory**: Graph-Guided Datalog Store via **CozoDB**. **Cross-session Heuristic Persistence** for cumulative technical learning.
*   **Cortex**: Powered by **Gemini 2.5/3.0** with high-speed OODA loops and architectural reasoning.

## 🛠️ Quick Start

### 1. Installation
```bash
git clone https://github.com/criticalinsight/sly.git
cd sly
cargo install --path .
```
> [!TIP]
> Ensure `~/.cargo/bin` is in your `$PATH`. You can then run the agent simply by typing `sly`.

### 2. Initialize a Workspace
Go to your project directory and run:
```bash
sly init
```
This creates the isolated `.sly` directory and a default `config.toml`.

### 3. Setup Environment
Ensure your `.env` file in the project directory has your API key:
```bash
GEMINI_API_KEY=your_key_here
```

### 4. Ignite the Brain
```bash
sly
```

### 5. Headless Operation (Godmode)
To keep Sly running in the background with remote Telegram control:
```bash
sly supervisor install
launchctl load ~/Library/LaunchAgents/com.brixelectronics.sly.plist
```
Now you can step away from your machine and manage your sessions via Telegram!

### 6. Telegram Bot Setup
To enable remote control:
1.  Message [@BotFather](https://t.me/botfather) on Telegram.
2.  Run `/newbot` and follow the instructions to get your **Bot Token**.
3.  Add the token to your `.env`: `TELEGRAM_BOT_TOKEN=your_token_here`.
4.  Message your new bot to auto-detect your `Chat ID`.

## 🎮 Usage

Once running, you maintain a conversation with Sly or add tasks to `TASKS.md`.

### Configuration (`.sly/config.toml`)
```toml
project_name = "my-awesome-app"
autonomous_mode = true          # Set to true for headless operation
max_autonomous_loops = 50       # Circuit breaker for API spend
primary_model = "gemini-2.5-flash"
```

### Slash Commands & Workflows
- `/run <task>`: (Or plain text) Start a persistent session.
- `/ask <query>`: Direct reasoning without filesystem side-effects.
- `/fix`: **Self-Healing Loop**. Captures compile errors as facts and derives a fix.
- `/test`: Trigger test suites and receive summarized failure diffs.
- `/docs`: Re-analyze codebase and sync documentation.
- `/roadmap`: Implement the next prioritized item from `ROADMAP.md`.
- `--ephemeral`: Flag to run any command without a database lock.

## 🤝 Contributing

Contributions are welcome! Please check the [ROADMAP.md](ROADMAP.md) for current goals.

## ⚠️ Safety Notice

Sly can execute shell commands and modify files.
- The **OverlayFS Safety Shield** ensures all edits are transactional and speculative.
- **The Governor** prevents dangerous autonomous actions like `git push --force`.
- **Singleton Lock**: Prevents multiple agent instances from corrupting the same codebase.
- **Always** commit your work before letting an agent modify your codebase.

---
*Built with ❤️ in Rust*
