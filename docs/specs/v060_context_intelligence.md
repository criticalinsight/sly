# Sly v0.6.0 — Context Intelligence Release

## User Story

**As a** developer using Sly for multi-step autonomous tasks,
**I want** the agent to maintain coherent context across long reasoning chains,
**so that** it doesn't lose its plan, repeat itself, or waste tokens on verbose observations.

## Problem Statement

Qwen 3.5 benchmark (19 steps) revealed: mortal memory compression at step 5 discards the original plan. Stdout observations from `ls -la` or `cat` flood context with 10KB+ of raw text. The model has no awareness of which files it already created. No way to estimate if the prompt is approaching the model's context limit.

---

## Features

### F1: Observation Truncation
**Given** an `ExecShell` produces stdout > 500 chars,
**When** the observation is added to the message history,
**Then** stdout is truncated to 500 chars with `... [truncated, N bytes total]`.
Stderr is capped at 300 chars. `ReadFile` already truncates at 2KB — keep as-is.

### F2: File Manifest Header
**Given** the agent has written 1+ files via `WriteFile`,
**When** the next LLM call is made,
**Then** a `[FILES: path1, path2, ...]` header is prepended to the user message so the model always knows what exists.

### F3: Sliding Summary Compression
**Given** the message history exceeds `max_memory_window`,
**When** mortal memory fires,
**Then** instead of raw truncation marker `"... [Older context mortality excised] ..."`, the system concatenates the discarded messages and replaces them with a dense `SYSTEM SUMMARY: <key facts>` message produced by the LLM itself.

### F4: Token Budget Warning
**Given** the total message history exceeds an estimated token threshold (default: 4000 chars ≈ 1000 tokens),
**When** the next LLM call is about to fire,
**Then** print `⚠️ Token budget: ~N/M tokens used` to stderr. No blocking — informational only.

---

## Technical Constraints

- **Zero new dependencies**. All features use stdlib + existing `serde_json`.
- **No async**. Everything stays synchronous and single-threaded.
- **No new modules**. Changes go into existing `memory.rs`, `control.rs`, `state.rs`.
- **Rich Hickey**: Each feature is a pure function. No new traits. No generics.

## Acceptance Criteria

| Feature | Test |
|:--------|:-----|
| F1: Truncation | `cargo test` — new test: 1000-char stdout → truncated to 500 |
| F2: File Manifest | `cargo test` — WriteFile path tracked, header injected |
| F3: Summary | `cargo test` — mortal memory produces summary string |
| F4: Token Warning | Visual: `⚠️ Token budget` printed during reasoning |

## Priority Order

1. **F1** (Observation Truncation) — simplest, highest ROI
2. **F2** (File Manifest) — small, complements F1
3. **F4** (Token Warning) — informational, no behavior change
4. **F3** (Sliding Summary) — most complex, requires LLM call during compression
