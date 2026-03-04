#!/usr/bin/env bash
# Sly Benchmark: Qwen3:8b vs Gemini 2.5 Flash
set -euo pipefail

SLY_BIN="/Users/brixelectronics/Downloads/sly/target/release/sly"
BENCH_DIR="/tmp/sly_benchmark"
PROMPT='Create a calculator web app with index.html, style.css, app.js. Dark theme, grid layout for buttons 0-9 and +-*/=. Display shows input and result. Then FinalResponse.'

rm -rf "$BENCH_DIR"
mkdir -p "$BENCH_DIR/qwen" "$BENCH_DIR/gemini"

echo "════════════════════════════════════════════"
echo "  SLY BENCHMARK: Code Generation"
echo "════════════════════════════════════════════"
echo ""
echo "Prompt: $PROMPT"
echo ""

# ── Run 1: Qwen3:8b (local Ollama) ──────────
echo "━━━ [1/2] Qwen3:8b (Ollama local) ━━━"
QWEN_START=$(date +%s)

printf '%s\n' "$PROMPT" | \
  SLY_MODEL="qwen3:8b" \
  SLY_OPENAI_URL="http://localhost:11434/v1/chat/completions" \
  OPENAI_API_KEY="ollama" \
  "$SLY_BIN" 2>&1 | tee "$BENCH_DIR/qwen/log.txt"

QWEN_END=$(date +%s)
QWEN_TIME=$((QWEN_END - QWEN_START))

# Copy committed files
cp /Users/brixelectronics/Downloads/sly-playground/*.html "$BENCH_DIR/qwen/" 2>/dev/null || true
cp /Users/brixelectronics/Downloads/sly-playground/*.css "$BENCH_DIR/qwen/" 2>/dev/null || true
cp /Users/brixelectronics/Downloads/sly-playground/*.js "$BENCH_DIR/qwen/" 2>/dev/null || true

# Clean playground for gemini run
rm -rf /Users/brixelectronics/Downloads/sly-playground/*

echo ""
echo "━━━ [2/2] Gemini 2.5 Flash (API) ━━━"
GEMINI_START=$(date +%s)

printf '%s\n' "$PROMPT" | \
  SLY_MODEL="gemini-2.5-flash" \
  "$SLY_BIN" 2>&1 | tee "$BENCH_DIR/gemini/log.txt"

GEMINI_END=$(date +%s)
GEMINI_TIME=$((GEMINI_END - GEMINI_START))

# Copy committed files
cp /Users/brixelectronics/Downloads/sly-playground/*.html "$BENCH_DIR/gemini/" 2>/dev/null || true
cp /Users/brixelectronics/Downloads/sly-playground/*.css "$BENCH_DIR/gemini/" 2>/dev/null || true
cp /Users/brixelectronics/Downloads/sly-playground/*.js "$BENCH_DIR/gemini/" 2>/dev/null || true

# ── Results ──────────────────────────────────
echo ""
echo "════════════════════════════════════════════"
echo "  BENCHMARK RESULTS"
echo "════════════════════════════════════════════"
echo ""

QWEN_STEPS=$(grep -c "Thinking\.\.\." "$BENCH_DIR/qwen/log.txt" 2>/dev/null || echo "0")
GEMINI_STEPS=$(grep -c "Thinking\.\.\." "$BENCH_DIR/gemini/log.txt" 2>/dev/null || echo "0")

QWEN_FILES=$(find "$BENCH_DIR/qwen" -name "*.html" -o -name "*.css" -o -name "*.js" | grep -v log | wc -l | tr -d ' ')
GEMINI_FILES=$(find "$BENCH_DIR/gemini" -name "*.html" -o -name "*.css" -o -name "*.js" | grep -v log | wc -l | tr -d ' ')

QWEN_BYTES=$(find "$BENCH_DIR/qwen" \( -name "*.html" -o -name "*.css" -o -name "*.js" \) -exec cat {} + 2>/dev/null | wc -c | tr -d ' ')
GEMINI_BYTES=$(find "$BENCH_DIR/gemini" \( -name "*.html" -o -name "*.css" -o -name "*.js" \) -exec cat {} + 2>/dev/null | wc -c | tr -d ' ')

QWEN_HTML_OK="❌"
GEMINI_HTML_OK="❌"
if grep -q "<html" "$BENCH_DIR/qwen/index.html" 2>/dev/null; then QWEN_HTML_OK="✅"; fi
if grep -q "<html" "$BENCH_DIR/gemini/index.html" 2>/dev/null; then GEMINI_HTML_OK="✅"; fi

QWEN_COMPLETE="❌"
GEMINI_COMPLETE="❌"
if grep -q "Auto-committed" "$BENCH_DIR/qwen/log.txt" 2>/dev/null; then QWEN_COMPLETE="✅"; fi
if grep -q "Auto-committed" "$BENCH_DIR/gemini/log.txt" 2>/dev/null; then GEMINI_COMPLETE="✅"; fi

printf "%-20s %-20s %-20s\n" "Metric" "Qwen3:8b" "Gemini 2.5 Flash"
printf "%-20s %-20s %-20s\n" "──────────────────" "──────────────────" "──────────────────"
printf "%-20s %-20s %-20s\n" "Time" "${QWEN_TIME}s" "${GEMINI_TIME}s"
printf "%-20s %-20s %-20s\n" "Steps" "$QWEN_STEPS" "$GEMINI_STEPS"
printf "%-20s %-20s %-20s\n" "Files generated" "$QWEN_FILES" "$GEMINI_FILES"
printf "%-20s %-20s %-20s\n" "Total bytes" "$QWEN_BYTES" "$GEMINI_BYTES"
printf "%-20s %-20s %-20s\n" "Valid HTML" "$QWEN_HTML_OK" "$GEMINI_HTML_OK"
printf "%-20s %-20s %-20s\n" "Auto-committed" "$QWEN_COMPLETE" "$GEMINI_COMPLETE"
printf "%-20s %-20s %-20s\n" "Inference" "Local (MLX)" "Cloud API"

echo ""
echo "Logs: $BENCH_DIR/"
