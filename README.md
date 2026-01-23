# Sly 🦀

> **Maximum Intelligence, Minimum RAM. Persistent Autonomy.**

Sly is a high-performance, single-binary autonomous coding agent written in **Rust**. It is designed specifically for **Apple Silicon** to provide a lightning-fast, native AI pair programmer experience without the bloat of Python, Node.js, or Docker.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-macos-lightgrey.svg)](https://www.apple.com/macos/)

*   **Godmode & Decomplected**: Formal separation of source "Value" from transient build state, resulting in 95% CPU reduction during idle.
*   **Persistent Supervisor**: A dedicated background daemon (`sly supervisor`) that monitors and manages your coding sessions.
*   **Interactive Remote Control**: Manage your agent from anywhere via Telegram with interactive buttons, real-time log streaming (`/logs`), and remote plan approval.
*   **Operational Hardening**: Built-in **Circuit Breaker** to prevent crash loops and **PID-aware Singleton Enforcement** for multi-instance safety.
*   **Haptic Telemetry**: Real-time event streaming of agent facts (tool use, directives, errors) with **Semantic Batching** (grouping 50+ identical errors into one summary) via a **Decomplected Outbox**.
*   **Mac Native**: Installs as a native macOS LaunchAgent for automatic start on login.
*   **Auto-Healing**: Reliable recovery from crashes or OOM events.
*   **Active Memory**: Graph-Guided RAG via **CozoDB** and Metal-accelerated embeddings (**Candle/BGE**).
*   **Cortex**: Powered by **Gemini 2.5 Flash** with **Thinking Levels** (`High` / `Low`).

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

### Slash Commands
- `/path <path>`: Change the target codebase directory.
- `/status`: Show memory usage, session turns, and token counts.
- `/clear`: Wipe the current session history (RAM only).
- `/exit`: Quit the agent.

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
