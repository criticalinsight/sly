
- [x] **v0.2.4: The Supervisor Release** <!-- id: 20 -->
    - [x] MCP Integration & Godmode <!-- id: 21 -->
    - [x] Symbol Cannon (Regex-based) <!-- id: 22 -->

- [x] **v0.5.0: Relational Intelligence** <!-- id: 50 -->
    - [x] AST-based Code Graph (syn parser) <!-- id: 51 -->
    - [/] Hybrid Graph-Vector recall integration <!-- id: 52 -->
    - [ ] Haptic Telemetry (say/notify) <!-- id: 53 -->
    - [ ] Registry Monitoring <!-- id: 54 -->

- [ ] **v1.0.0: The "Pro" Release** <!-- id: 100 -->
    - [x] Worker Swarm Orchestration (src/swarm.rs) <!-- id: 101 -->
    - [ ] Predictive Context pre-fetching <!-- id: 102 -->
    - [ ] Multi-Agent Task Delegation <!-- id: 103 -->

- [ ] **v1.1.0: Context Hyper-Efficiency** <!-- id: 110 -->
    - [ ] Differential Symbol Sync <!-- id: 111 -->
    - [ ] Linguistic Pruning (AI-guided) <!-- id: 112 -->
    - [x] Local RAG Re-ranking (candle) <!-- id: 113 -->

- [ ] **v1.2.0: Verified Autonomy** <!-- id: 120 -->
    - [x] Self-Correcting Debate Loops (src/debate.rs) <!-- id: 121 -->
    - [ ] Rollback Snapshots (.sly/snapshots) <!-- id: 122 -->
    - [ ] Semantic Linting <!-- id: 123 -->

- [ ] **Technical Debt & Decomplection** <!-- id: 900 -->
    - [x] Migrate `memory_legacy.rs` to `src/memory/` <!-- id: 901 -->
    - [x] Resolve `#[allow(dead_code)]` in `swarm.rs` and `debate.rs` <!-- id: 902 -->
    - [x] **Phase 3: The Epochal Agent & Logic-Ready Knowledge**
    - [x] Implement `AgentSession` and `SessionStore` (Temporal Decomplection)
    - [x] Refactor `agent.rs` for step-based OODA turns
    - [x] Implement `QueryDatalog` for structured memory reasoning
    - [x] Update `cortex_loop` to handle session-based impulses
    - [x] **Verification & Refinement**
        - [x] **[VERIFICATION]** Verify `UseSkill` execution with robust parser (End-to-End Test)
    - [x] **[OPTIMIZATION]** Streamline Supervisor
      - [x] Remove Server, Janitor, Swarm components.
      - [-] Decomplect `sly-learn` (Disabled on request).
      - [x] Restored `BootstrapSkills` to `sly` binary (boot integrity).
      - [x] Verify lighter `sly` binary.
        - [x] Add more "Neighborhood" search logic to Datalog primitives
    - [x] Removed legacy `check_library_updates` (deprecated) <!-- id: 904 -->
    - [x] Fix "required column embedding not found" error in node insertion (likely schema mismatch) <!-- id: 905 -->
    - [x] **[RELEASE]** v0.6.0: Documentation & Compilation

- [ ] **Phase 4: The Hickey Decomplection (Rich's Way)** <!-- id: 1000 -->
    - [ ] **Explode `GlobalState`** (Replace with `Arc<Components>`) <!-- id: 1001 -->
    - [ ] **Dissolve `KnowledgeEngine`** (Replace with pure functions in `knowledge/`) <!-- id: 1002 -->
    - [ ] **Dissolve `Cortex`** (Replace with `llm::generate` + `rag::context`) <!-- id: 1003 -->
    - [ ] **Value-Oriented Sessions** (Replace `AgentSession` mutation with functional reduction) <!-- id: 1004 -->
    - [ ] **Data-Oriented Tools** (Replace `AgentAction` enum with EDN/JSON maps) <!-- id: 1005 -->

- [ ] **Phase 5: Reliability & Schema Fixes (Hickey Style)** <!-- id: 1100 -->
    - [x] Fix CozoDB `event_log` schema (`JSON` -> `Json` parser error) <!-- id: 1101 -->
    - [x] Decomplect Tool Registry (Fetch metadata once, use as immutable snapshot) <!-- id: 1102 -->
    - [x] Graceful, event-driven shutdown loop (no more aggressive `process::exit`) <!-- id: 1103 -->
    - [x] Immutable intent logging for every agent action (Directive Audit) <!-- id: 1104 -->
    - [x] Stable Ctrl+C handler for session state persistence on exit <!-- id: 1105 -->
