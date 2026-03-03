# Sly Architecture: The Flat Logic Tree 🧙🏾‍♂️

"Simplicity is the absence of complecting."

Sly follows the **Rich Hickey Strategic Doctrine**. It is a **Zero-Lib**, **Synchronous**, and **Linearized** engine designed for Apple Silicon.

## 🏛 The Strategic Pillar: Linear Logic
Unlike traditional agentic systems that use complex "Command Protocols" (Impulses) and "State Objects" (Sessions), Sly uses a **Direct Stream**.

### 1. Flat Root Structure
All modules reside in `src/`. No nesting. Absolute visibility.
- [control.rs](file:///Users/brixelectronics/Downloads/sly/sly-core/src/control.rs): The single logical pulse.
- [cortex.rs](file:///Users/brixelectronics/Downloads/sly/sly-core/src/cortex.rs): The Orient phase.
- [memory.rs](file:///Users/brixelectronics/Downloads/sly/sly-core/src/memory.rs): The Data-at-Rest.

### 2. Protocol-less Execution
We have excised the `Impulse` layer.
- **Observe**: Read `String` from `io.rs`.
- **Orient**: `Cortex` generates a plan.
- **Decide/Act**: `Parser` extracts actions; `Control` executes them.
- No intermediate command enums. No "message bus." Just function calls.

### 3. Mortal Data-at-Rest
We deleted `session.rs`. 
- There is no `AgentSession` object.
- A session is simply a `Vec<String>` stored in [memory.rs](file:///Users/brixelectronics/Downloads/sly/sly-core/src/memory.rs).
- We operate on values, not objects with state-management logic.

## 🛠 Component Breakdown
- **Control Loop**: Single-threaded, blocking loop in `control.rs`.
- **Parser**: Robust, manual escape-aware string extraction in `parser.rs`.
- **OS-as-a-Library**: Direct use of `sh` and `curl` via standard `std::process`.

## 🔋 Performance & Reliability
- **Build Time**: ~0.15s (sub-second).
- **Execution**: Predictable vertical flow. No race conditions. Zero async complection.
