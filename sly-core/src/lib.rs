//! # Sly Core
//!
//! Zero-Lib autonomous coding agent engine.
//!
//! Sly is a minimalist, synchronous, single-threaded agent that uses
//! the Rust Standard Library exclusively. No `tokio`, `serde`, or
//! `reqwest`. LLM inference is performed via `curl`; shell execution
//! via `sh`.
//!
//! ## Module Map
//!
//! | Module     | Responsibility                          |
//! |------------|-----------------------------------------|
//! | `control`  | OODA heartbeat loop                     |
//! | `cortex`   | LLM API call (Gemini / OpenAI-compat)   |
//! | `memory`   | Session persistence with mortal window  |
//! | `parser`   | Zero-Serde JSON action extraction       |
//! | `safety`   | Transactional overlay filesystem        |
//! | `io`       | Stdin/stdout CLI adapter                |
//! | `state`    | Global configuration and state bundle   |
//! | `error`    | Unified error type                      |

pub mod state;
pub mod control;
pub mod cortex;
pub mod memory;
pub mod io;
pub mod parser;
pub mod safety;
pub mod error;
