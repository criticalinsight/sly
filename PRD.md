# Zero-Lib PRD: Sly Engine

## 1. Executive Summary
**Sly** is a minimalist, single-binary autonomous coding agent.
**Philosophy**: "Maximum Intelligence, Zero Dependencies."

## 2. Problem Statement
Modern software (and agents) are "complected" with layers of frameworks, runtimes, and libraries that:
- Slow down startup and compilation.
- Obscure logic and syscalls.
- Create dependency hell.

## 3. Product Vision
To be the **Simple** engine. No async, no heavy frameworks, no unnecessary abstractions. A tool that leverages the OS and the standard library to achieve its goals with total transparency.

## 4. Technical Specifications
- **Synchronous Core**: Single-threaded blocking OODA loop.
- **Library Excision**: 0 dependencies for core operations (No Serde, No Tokio, No Axum).
- **Manual String Parsing**: Replaced complex JSON serialization with direct string manipulation.
- **OS as a Library**: Invokes `curl` for networking and `sh` for environment interaction.

## 5. Functional Goals
- **Autonomy**: Solve coding tasks via a Rich Hickey reasoning loop.
- **Speed**: < 100ms startup, < 1s compilation.
- **Transparency**: Every byte of the engine is understandable by a single human.

## 6. Success Metrics
- **Dependency Count**: Minimized to essential utility (e.g., `colored`, `dotenvy`).
- **Compilation Time**: Sub-second check.
- **Resource Footprint**: Minimal CPU/RAM overhead.
