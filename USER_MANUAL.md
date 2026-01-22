# Sly User Manual (v0.6.0)

Welcome to **Sly**, a high-performance, single-binary autonomous coding agent written in Rust. Sly is designed for Apple Silicon, combining deep relational awareness with haptic telemetry for a state-of-the-art coding experience.

## 🚀 Getting Started

### 1. Installation
Install Sly globally using `cargo`:
```bash
git clone https://github.com/criticalinsight/sly.git
cd sly
cargo install --path .
```

### 2. Initialization
Navigate to any project workspace and run:
```bash
sly init
```
This creates a `.sly/` directory structure containing your local memory (CozoDB) and configuration.

### 3. API Configuration
Ensure your `.env` file contains your Gemini API key:
```bash
GEMINI_API_KEY=your_key_here
```

---

## 🧠 Core Features

### 1. Hybrid Relational Recall
Sly uses an AST-based **Code Graph** (powered by `syn`) to understand not just snippets of text, but the relationships between your structs, traits, and functions. 
- **Auto-Recall**: Before every "Thinking" step, Sly automatically queries its memory for relevant code symbols and documentation.
- **Graph Expansion**: Vector hits are expanded to their graph neighbors, ensuring full context.

### 2. High Autonomy (Auto-Pilot)
As of v0.6.0, Sly runs in **High Autonomy** mode by default.
- **Persistent OODA**: Sly will execute up to 50 loops autonomously to solve complex tasks.
- **Governor Approval**: The agent "self-approves" verified changes using its internal Safety Governor.

### 3. Haptic Telemetry
Sly provides real-time progress feedback via macOS system audio.
- **Thinking**: You will hear "Thinking" when Sly starts a cognition cycle.
- **Task Complete**: "Task complete" signals when the agent has fulfilled your objective or checked off all items in `TASKS.md`.

---

## ⚙️ Configuration (`.sly/config.toml`)

Customize your agent's behavior:
```toml
project_name = "sly-engine"
autonomous_mode = true          # Enable/Disable autonomous execution
max_autonomous_loops = 50       # Safety circuit breaker
primary_model = "gemini-3-flash" # Primary cognition model
```

---

## 🧹 Maintenance (The Janitor)

Sly features a background **Custodial Loop** that runs every 5 minutes:
1.  **Registry Auditing**: Checks for documentation drift against upstream authorities (Crates.io).
2.  **Autonomous Learning**: Re-scans your workspace for tech stack changes or new dependencies.
3.  **Haptic Sync**: Announces relevant maintenance events.

---

## 🛡️ Safety and Control

- **OverlayFS Shield**: All code modifications are speculative until committed. If a build fails, the overlay is discarded.
- **The Crucible**: Integrated Rust build-gate that prevents merging invalid code.
- **Command Lock**: Dangerous commands (e.g., `rm -rf /`, `git push --force`) are blocked by the Safety Governor.

---
*Manual generated on January 21, 2026 for Sly v0.6.0*
