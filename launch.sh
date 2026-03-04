#!/usr/bin/env bash
# Sly v0.4.0 Launch Script 🧙🏾‍♂️
# Zero-Lib Autonomous Coding Agent
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY="$SCRIPT_DIR/target/release/sly"

# ── Preflight ──────────────────────────────────────────
check_env() {
    if [ -z "${GEMINI_API_KEY:-}" ] && [ -z "${SLY_OPENAI_URL:-}" ]; then
        echo "⚠️  Neither GEMINI_API_KEY nor SLY_OPENAI_URL is set."
        echo "   export GEMINI_API_KEY=<your-key>"
        echo "   — or —"
        echo "   export SLY_OPENAI_URL=http://localhost:11434/v1/chat/completions"
        exit 1
    fi
}

build_if_needed() {
    if [ ! -f "$BINARY" ]; then
        echo "🔨 Building release binary..."
        cargo build --release --workspace --manifest-path "$SCRIPT_DIR/Cargo.toml"
    fi
}

# ── Main ───────────────────────────────────────────────
check_env
build_if_needed

echo "🧠 Launching Sly ($(du -h "$BINARY" | cut -f1 | xargs))"
exec "$BINARY" "$@"
