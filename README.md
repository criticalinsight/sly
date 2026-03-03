# Sly: The Zero-Lib Engine 🧙🏾‍♂️

Sly is an ultra-minimalist, high-performance autonomous coding agent. Designed specifically for individuals who value execution over abstraction, Sly is built as a single, rapidly-compiled binary with near-zero dependencies.

Following the **Rich Hickey Strategic Doctrine**, Sly has been thoroughly "de-complected." We stripped out asynchronous runtimes, message protocols, and thick abstraction layers to reveal the absolute core of autonomous logic.

## 🚀 Key Achievements (v0.3.5)
- **Release Compilation**: `0.59s`
- **Binary Footprint**: `522KB`
- **Zero-Lib Architecture**: Removed `tokio`, `axum`, `serde`, and `reqwest`.
- **Standard Library Only**: Relies exclusively on `std` and system-native tools (`curl`, `sh`).
- **Synchronous Pillar**: A linear, predictable OODA loop in a single thread. No race conditions.

## 🛠 Environment & Setup
Sly operates by treating your **Operating System as a Library**.

### Prerequisites
- Rust (latest stable)
- `curl` (system-wide)
- `GEMINI_API_KEY` exported in your environment.

### Getting Started
```bash
# Build the production release
cargo build --release --workspace

# Run the core agent
./target/release/sly
```

## 📜 Philosophy: The "Flat Tree"
> "Simplicity is the absence of complecting." — Rich Hickey

Sly is built to be "Simple" (one fold) rather than "Easy" (near to hand but tangled). We prioritize:
1. **Values over Objects**: Pure state transitions (e.g., `Vec<String>`) over complex state-management classes.
2. **De-complecting**: Absolutely no "Protocol" or "Command" indirection layers between standard I/O and execution.
3. **Linearity**: Vertical pillars of execution (sync function calls) over horizontal trees of abstraction (message buses, event loops).

## License
Apache License, Version 2.0.
