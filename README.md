# Sly: The Zero-Lib Engine 🧙🏾‍♂️

Sly is a high-performance, single-binary autonomous coding agent designed for **Apple Silicon**.

Following the **Rich Hickey Strategic Doctrine**, Sly has been "De-complecting" into a minimalist, synchronous engine that relies on the **Standard Library** and **OS as a Library**, achieving near-zero dependencies and sub-second compilation.

## 🚀 Key Features

- **Zero-Lib Architecture**: Removed heavy frameworks (Tokio, Axum, Serde, Ratatui).
- **Synchronous Loop**: A linear, predictable OODA loop (Observe, Orient, Decide, Act) in `control.rs`.
- **Flat Root Structure**: No directory nesting in `src/` for absolute visibility.
- **Transactional Safety**: All file edits are guarded by path-based sandboxing.
- **Mortal State**: Plain data structures instead of `Arc`/`Mutex` for simple reasoning.

## 🛠 Development

### Prerequisites
- Rust (latest stable)
- `curl` (system-wide)

### Running Sly
```bash
# Run the core agent
cargo run -p sly-core
```

### Build & Check
```bash
# Workspace-wide check (completes in ~0.08s)
cargo check --workspace
```

## 📜 Philosophy
> "Simplicity is the absence of complecting." — Rich Hickey

Sly is built to be "Simple" (one fold) rather than "Easy" (near to hand but tangled). We prioritize:
1. **Values over Objects**: Data-oriented enums and structs.
2. **De-complecting**: Separating concerns physically and logically.
3. **Linearity**: Vertical pillars of execution over trees of abstraction.

## License
Apache License, Version 2.0.
