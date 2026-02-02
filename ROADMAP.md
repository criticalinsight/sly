# Sly Roadmap: The Pursuit of Simplicity

> "Simplicity is prerequisite for reliability." — Edsger W. Dijkstra (Channelled by Rich Hickey)

We reject the path of "Easy". We do not complect mechanism with policy, nor state with identity. Sly is not a bot; Sly is a coordination value.

## The Foundation (Completed)

*We have built the loom. Now we must weave.*

- [x] **Rust Migration**: Shedding the incidental complexity of Python.
- [x] **Singleton Enforcement**: Identity requires singular existence.
- [x] **Native Observability**: `sly-monitor` as a separate, orthogonal eye.
- [x] **Recursive Reflexion**: Errors are just data to be reasoned about.
- [x] **MCP Client Sovereignty**: Capabilities as tools.

## Phase 4: Token Optimization (Completed)

*Intelligence is a function of signal-to-noise ratio.*

- [x] **Linguistic Pruner**: Regex-based semantic compression (~90% reduction).
- [x] **Gemini Context Caching**: Native API integration for system prompts.
- [x] **Heuristic Persistence**: Cross-session memory in CozoDB.
- [x] **Incremental Context**: Structural caching of MCP tool definitions.

---

## Phase 5: Decomplecting I/O (The Simplification)

*We reject the complexity of async buses and stateful servers. We embrace Traits and Pipes.*

- [ ] **The I/O Trait (Dependency Injection)**:
  - Define `AgentIO` trait for abstracting input/output.
  - Refactor `Cortex` to accept `Box<dyn AgentIO>` instead of hardcoded Telegram.
  - *Why*: Direct function calls are simpler than message queues.
- [ ] **CLI-First Architecture (Sly as Pipe)**:
  - Implement `sly --mcp` to map Stdin/Stdout to the internal dispatcher.
  - Treat the OS process boundary as the state boundary.
  - *Why*: Stateless processes are easier to manage than stateful servers.

## Phase 6: Data-Oriented Mind

*State is not an object. State is a value at a point in time.*

- [x] **The Immutable Ledger** (Core Implementation):
  - Session snapshots in `CozoDB`.
  - Time-travel debugging via `checkpoint` and `rollback`.
- [x] **Data-Driven Logic**:
  - [x] **Semantic Deduplication**: Collapse redundant error logs.
  - [x] **Adaptive Pruning**: Heuristic relevance scoring.
  - [x] **Differential Context Updates**: Delta-only context.

## Phase 7: Generative Simplicity

*The system creates its own extensions.*

- [ ] **Sub-Process Composition**:
  - Implement recursive tools via `Command::new("sly")`.
  - Child agents run as isolated ephemeral processes.
- [ ] **Self-Synthesized Adapters**:
  - Sly writes code for new I/O adapters and compiles them.

---

> "It is better to have 100 functions operate on one data structure than 10 functions on 10 structures."
