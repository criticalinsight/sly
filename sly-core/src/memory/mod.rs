pub mod backend_cozo;
pub mod backend_gleam;
pub mod store_graph;

pub use store_graph::{Memory, GraphNode, LibraryEntry};

use async_trait::async_trait;
use crate::error::Result;
use serde_json::Value;

#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn recall(&self, query: &str, limit: usize) -> Result<Vec<String>>;
    async fn recall_facts(&self, query: &str) -> Result<Vec<String>>;
    async fn search_library(&self, query: &str, limit: usize) -> Result<Vec<String>>;
    async fn store(&self, content: &str, metadata: Option<Value>) -> Result<String>;
    async fn recall_as_of(&self, clauses: Value, tx_id: u64) -> Result<Vec<GraphNode>>;
    async fn count_nodes(&self) -> Result<usize>;
    
    // Skills (WASM) - Note: WASM logic was partially decommissioned but traits might remain for compat
    async fn register_skill(&self, name: &str, code: &str, desc: &str, signature: &str) -> Result<()>;
    async fn get_skill(&self, name: &str) -> Result<Option<String>>;
}
