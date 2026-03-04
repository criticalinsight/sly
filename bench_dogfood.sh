#!/bin/bash

# Configuration
export SLY_MODEL="gemini-2.5-flash"
export SLY_OPENAI_URL="" # We want to use Gemini API directly

# Define the target directory (Sly's own repo)
TARGET_DIR=$(pwd)
LOG_DIR="/tmp/sly_dogfood"
mkdir -p "$LOG_DIR"

PROMPT="Refactor the Sly codebase to improve separation of concerns based on Rich Hickey's principles. 
IMPORTANT: DO NOT hallucinates code.
0. Use ExecShell with 'ls -R src' and 'cat sly-core/src/cortex.rs' and 'cat sly-core/src/main.rs' to read the exact actual Rust codebase before you write anything.
1. Create a new file sly-core/src/utils.rs. 
2. Move the Escape Json function from sly-core/src/cortex.rs into sly-core/src/utils.rs as a public function. 
3. Update sly-core/src/cortex.rs to import and use the new utils::escape_json function. 
4. Update sly-core/src/main.rs to declare the mod utils. 
5. Run 'cargo test' as an ExecShell action to aggressively verify that your changes compile and pass tests. 
6. When tests pass, emit FinalResponse. Do not use markdown backticks around the json."

echo "════════════════════════════════════════════"
echo "  SLY BENCHMARK: Dogfooding (Self-Improvement)"
echo "════════════════════════════════════════════"
echo ""
echo "Prompt: $PROMPT"
echo ""
echo "━━━ Running Sly (Self-Modification) ━━━"

echo "$PROMPT" | cargo run --bin sly | tee "$LOG_DIR/log.txt"

echo ""
echo "════════════════════════════════════════════"
echo "  DOGFOOD BENCHMARK COMPLETE"
echo "════════════════════════════════════════════"
