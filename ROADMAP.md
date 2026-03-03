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

## Phase 5: Decomplecting I/O ✅

*We reject the complexity of async buses. We embrace Traits and Pipes.*

- [x] **AgentIO Trait**: `modality()`, `send_structured()`, `supports_streaming()`
- [x] **CliAdapter**: stdin/stdout + pipe mode for IDE integration
- [x] **IoModality Enum**: Cli, CliPipe, Telegram, McpServer

## Phase 6: Data-Oriented Mind ✅

- [x] **The Immutable Ledger**: Session snapshots in CozoDB
- [x] **Time-travel debugging**: `checkpoint` and `rollback`
- [x] **Semantic Deduplication**: Collapse redundant error logs
- [x] **Adaptive Pruning**: Heuristic relevance scoring

## Phase 7: Generative Simplicity ✅

*The system creates its own extensions.*

- [x] **Sub-Process Composition**: `spawn_child_agent()`, `spawn_parallel_agents()`
- [x] CLI flags: `--ephemeral`, `--pipe`, `--persona`

## Phase 8: IDE Native Integration (MCP Server) ✅

- [x] **MCP Server Mode**: `sly --mcp-server`
- [x] **JSON-RPC Protocol**: `initialize`, `tools/list`, `tools/call`
- [x] **Tools**: `sly_task`, `sly_query`

## Phase 9: Distributed Swarm ✅

*A Vim for agents must become a Tmux for agents.*

- [x] **SwarmSupervisor**: Parallel orchestration, conflict detection
- [x] **SwarmWorker**: Ephemeral subprocess execution
- [x] **SwarmTask/SwarmResult**: Pure data structures

### Swarm v2 Enhancements ✅
- [x] **merge.rs**: 3-way conflict resolution via git merge-file
- [x] **context.rs**: Shared file index + dependency graph
- [x] **partition.rs**: PerFile, PerModule, BySCC strategies
- [x] **overlay.rs**: Worker isolation + atomic commit/rollback
- [x] **events.rs**: Real-time SwarmEvent streaming
- [x] **cache.rs**: Content-addressed memoization

---

## Phase 10: Standardizing Datalog & Ergonomics ✅

*Errere humanum est, ergonomicum divinum est.*

- [x] **Associative Bindings**: Refactored `gleamdb` to return `Dict(String, Value)` instead of positional lists.
- [x] **Generic Storage Protocol**: Decoupled engine from physical storage with `StorageAdapter`.
- [x] **Dogfooding in Amkabot**: Verified SQLite persistence in a production-grade market bot.

## Phase 11: Sovereign Knowledge Service ✅

*The mind is not a part of the body; it is a neighbor to the body.*

- [x] **De-complecting Memory**: Migrated `sly`'s memory from embedded CozoDB to a standalone Gleam service.
- [x] **sly_knowledge**: A sovereign BEAM service for code symbol graphs.
- [x] **Deterministic Hashing**: Unified cross-platform entity identification.
- [x] **Recursive Dependency Queries**: High-performance transitive closure reasoning over codebases.

## Phase 12: Performance & Search Optimizations ✅

*Stability is the absence of change in the face of pressure.*

- [x] **Atomic Batch Ingestion**: ~55x speedup via the `persist_batch` protocol.
- [x] **Native FTS5 Integration**: Sub-second semantic search via SQLite trigram indexing.
- [x] **De-complecting Search**: Formal separation of full-text queries from relational Datalog facts.

---

## CLI (v0.6.0)

```bash
sly init                           # Initialize workspace
sly session <query>                # One-shot session
sly swarm <task> --workers N       # Parallel execution
sly --mcp-server                   # IDE integration
sly --pipe --persona hickey        # Subprocess mode
```

---

> "It is better to have 100 functions operate on one data structure than 10 functions on 10 structures."
