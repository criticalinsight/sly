# Sly User Manual

## 1. Introduction
Sly is a high-performance autonomous coding agent. It is designed to be "Decomplected"—minimizing technical debt and resource overhead.

## 2. Configuration (`.sly/config.toml`)
Default configuration for maximum performance:
```toml
project_name = "my-project"
autonomous_mode = true
primary_model = "gemini-2.5-flash"
thinking_level = "auto"
```

## 4. Godmode & Remote Management
Sly can be managed remotely via Telegram. This is ideal for background tasks or checking progress while away from your Mac.

### Telegram Bot Setup
1.  **Create Bot**: Message [@BotFather](https://t.me/botfather) and use `/newbot`.
2.  **Get Token**: Copy the token provided (e.g., `123456:ABC-DEF`).
3.  **Config**: Add it to your `.env` in the project root:
    ```bash
    TELEGRAM_BOT_TOKEN="123456:ABC-DEF"
    ```
4.  **Detect ID**: Send any message to your bot. Sly will automatically detect your Chat ID and save it to `.sly/config.toml` for persistence.

### Telegram Commands
- `/start`: Manually launch the agent.
- `/stop`: Kill the current agent session.
- `/status`: Get health metrics and interactive buttons.
- `/logs`: Stream the last 20 lines of the system log.
- `/query <datalog>`: Run advanced graph queries directly against the agent's memory.
- `/help`: Show command reference.

### Interactive Dashboard (`/status`)
The `/status` command now returns an interactive keyboard for one-tap operations:
- **🔄 Restart**: Stop and immediately restart the agent session.
- **🛑 Stop**: Kill the current agent process.
- **📄 Logs**: View recent system logs (same as `/logs`).
- **🧹 Flush**: Clear `sly_supervisor.log` and `sly_supervisor.err` to reclaim space.

### Remote Plan Approval
When Sly proposes an implementation plan, you will receive it in Telegram with **[✅ Approve]** and **[❌ Reject]** buttons. Approving a plan signals the agent to begin execution immediately.

### Haptic Telemetry & Semantic Batching
The Supervisor will automatically push **Facts** to Telegram as they happen:
- ⚙️ **Tool Executions**: Real-time log of what files are being touched.
- 👁️ **Directives**: The agent's next planned steps.
- 🚨 **Errors**: Immediate alerts for OOM or runtime panics.
- 📊 **Batching**: Similar facts (e.g., repeating errors) are condensed into a single "xN" summary to prevent message flooding.

## 5. Operational Hardening
### Circuit Breaker
To prevent infinite crash loops, Sly includes an intelligent circuit breaker:
- If the agent crashes **3 times within 10 minutes**, auto-healing is suspended.
- You will receive a critical alert in Telegram.
- To reset, manually run `/start` from Telegram once you have addressed the underlying issue.

### Singleton Enforcement & Lock-Free Telemetry
Sly is optimized for concurrency:
- **Supervisor Lock**: PID-aware file locking (`.sly/supervisor.lock`) ensures only one background monitor runs.
- **Decomplected Outbox**: Fast, filesystem-based event queuing ensures your Telegram notifications work even if the agent is heavily writing to the database.

## 6. Troubleshooting
- **Database Locked**: Usually caused by multiple instances. Singleton enforcement now minimizes this.
- **Service Not Starting**: Check `/tmp/sly_supervisor.err` for logs. Use the **Flush** button if logs are too large.
