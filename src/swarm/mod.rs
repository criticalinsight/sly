//! Distributed Swarm Module
//! 
//! Phase 9: Multi-agent parallelism for large-scale refactors.
//! The Supervisor orchestrates Worker agents for distributed execution.
//!
//! ## v2 Enhancements
//! - Conflict Resolution (3-way merge + LLM fallback)
//! - Shared Context (pre-analyzed codebase)
//! - Dependency-Aware Partitioning (SCC clustering)
//! - Transactional Rollback (overlay per worker)
//! - Progress Streaming (real-time events)
//! - Memoized Analysis (content-addressed cache)

pub mod supervisor;
pub mod worker;
pub mod task;
pub mod merge;
pub mod context;
pub mod partition;
pub mod overlay;
pub mod events;
pub mod cache;

pub use supervisor::SwarmSupervisor;
pub use worker::SwarmWorker;
pub use task::{SwarmTask, SwarmResult};
pub use merge::{MergeEngine, MergeResult};
pub use context::SwarmContext;
pub use partition::{TaskPartitioner, PartitionStrategy};
pub use overlay::{WorkerOverlay, CommitCoordinator};
pub use events::{SwarmEvent, event_channel};
pub use cache::{ContentCache, AnalysisCache};
