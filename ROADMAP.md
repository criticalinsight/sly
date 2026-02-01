# Sly Roadmap: The Pursuit of Simplicity

> "Simplicity is prerequisite for reliability." — Edsger W. Dijkstra (Channelled by Rich Hickey)

We reject the path of "Easy". We do not complect mechanism with policy, nor state with identity. Sly is not a bot; Sly is a coordination value.

## The Foundation (Completed)
*We have built the loom. Now we must weave.*

- [x] **Rust Migration**: Shedding the incidental complexity of Python.
- [x] **Singleton Enforcement**: Identity requires singular existence.
- [x] **Native Observability**: `sly-monitor` as a separate, orthogonal eye.
- [x] **Recursive Reflexion**: Errors are just data to be reasoned about.
- [x] **MCP Sovereignty**: Tools are capabilities, not code.

---

## Phase 5: Decomplecting I/O (The Next Epoch)
*Currently, Sly is braided with Telegram. This is limiting.*

- [ ] **The Event Bus (Pure Signal)**:
    - Eliminate hard-coded Telegram dependencies from the Core.
    - Implementing a pure `Event` schema (CloudEvents or simple Maps).
    - *Why*: The Core should not know about "Chat IDs", only "Destinations".
- [ ] **The Adapter Pattern (I/O Plugins)**:
    - `sly-telegram`: A completely separate process that translates `Events` <=> `Telegram API`.
    - `sly-cli`: A separate process for stdin/stdout interaction.
    - *Why*: Changing the medium should not require recompiling the mind.

## Phase 6: Data-Oriented Mind
*State is not an object. State is a value at a point in time.*

- [ ] **The Immutable Ledger**:
    - Move all memory from `struct` fields to `CozoDB`.
    - Every "thought", "action", and "result" is a fact tuple `[entity, attribute, value, tx, added]`.
    - *Why*: We gain time-travel debugging and "Resume-Anywhere" capabilities for free.
- [ ] **Semantic Specs**:
    - Define tool capabilities as `edn` (or JSON) data, not Rust traits.
    - Validation via data rules, not compiler checks (Runtime flexibility).

## Phase 7: Generative Simplicity
*The system creates its own extensions.*

- [ ] **Self-Synthesized Adapters**:
    - Sly writes the code for a new I/O adapter (e.g., Discord) and launches it.
    - *Why*: The ultimate automation is the automation of the automaton's interface.

---

> "It is better to have 100 functions operate on one data structure than 10 functions on 10 structures."
