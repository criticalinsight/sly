use anyhow::{anyhow, Context, Result};
use cozo::{DataValue, DbInstance, ScriptMutability, Vector};
use ndarray::Array1;
use std::collections::BTreeMap;
use std::path::Path;
use uuid::Uuid;
use chrono::Utc;

pub struct CozoBackend {
    pub db: DbInstance,
}

impl CozoBackend {
    pub fn new(path: &str, read_only: bool) -> Result<Self> {
        let db_path = Path::new(path).join("cozo.db");
        let db_path_str = db_path.to_str().context("Invalid UTF-8 in database path")?;
        
        let mut retries = 0;
        let max_retries = 10;
        let db = loop {
            let options = if read_only {
                r#"{"read_only": true}"#
            } else {
                "{}"
            };

            match DbInstance::new("rocksdb", db_path_str, options) {
                Ok(db) => break db,
                Err(e) if !read_only && retries < max_retries && e.to_string().contains("Resource temporarily unavailable") => {
                    retries += 1;
                    eprintln!("⚠️ Database is locked. Retry {}/{}...", retries, max_retries);
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                }
                Err(e) if read_only && retries < max_retries && e.to_string().contains("Resource temporarily unavailable") => {
                    retries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                Err(e) => return Err(anyhow!("Failed to open CozoDB at {}: {}", db_path_str, e)),
            }
        };

        let backend = Self { db };
        if !read_only {
            backend.initialize_schema()?;
        }
        Ok(backend)
    }

    fn initialize_schema(&self) -> Result<()> {
        // Initialize Schema
        let create_cache = "
            :create cache {
                id: String
                =>
                query: String,
                response: String,
                embedding: <F32; 384>
            }
        ";
        self.run_schema_script(create_cache, "cache")?;

        let create_index = "
            ::hnsw create cache:idx {
                dim: 384,
                dtype: F32,
                fields: [embedding],
                distance: Cosine,
                m: 50,
                ef_construction: 200
            }
        ";
        self.run_schema_script(create_index, "cache:idx")?;

        // Updated Schema for Active RAG: includes embedding in nodes
        let create_nodes = "
            :create nodes {
                id: String
                =>
                content: String,
                type: String,
                path: String,
                embedding: <F32; 384>
            }
        ";
        self.run_schema_script(create_nodes, "nodes")?;

        // Schema Migration: Check if `nodes` has `embedding` column
        let check_schema = "?[id, embedding] := *nodes{id, embedding} :limit 1";
        if let Err(_) = self.db.run_script(check_schema, Default::default(), ScriptMutability::Immutable) {
            println!("Migrating `nodes` schema: Adding `embedding` column...");
            
            // 0. Cleanup partial state
            let _ = self.db.run_script("::remove nodes_old", Default::default(), ScriptMutability::Mutable);

            // 1. Rename old table (backup)
            if let Err(e) = self.db.run_script("::rename nodes nodes_old", Default::default(), ScriptMutability::Mutable) {
                eprintln!("Failed to rename nodes->nodes_old: {}", e);
            }
            
            // 1.5 Explicit remove
            let _ = self.db.run_script("::remove nodes", Default::default(), ScriptMutability::Mutable);

            // 2. Create new table
            if let Err(e) = self.db.run_script(create_nodes, Default::default(), ScriptMutability::Mutable) {
                eprintln!("Failed to recreate nodes table: {}", e);
            }
            
            // 3. Migrate data
            let migrate_data = "
                ?[id, content, type, path, embedding] := *nodes_old{id, content, type, path}, embedding = $empty_vec
                :put nodes { id => content, type, path, embedding }
            ";
            let mut params = BTreeMap::new();
            params.insert("empty_vec".to_string(), vec_to_datavalue(vec![0.0; 384]));

            if let Err(e) = self.db.run_script(migrate_data, params, ScriptMutability::Mutable) {
                eprintln!("Failed to migrate nodes data: {}", e);
            }
            
            // 4. Drop old table
            let _ = self.db.run_script("::remove nodes_old", Default::default(), ScriptMutability::Mutable);
            println!("`nodes` verification/migration complete.");
        }

        let create_nodes_idx = "
            ::hnsw create nodes:idx {
                dim: 384,
                dtype: F32,
                fields: [embedding],
                distance: Cosine,
                m: 50,
                ef_construction: 200
            }
        ";
        self.run_schema_script(create_nodes_idx, "nodes:idx")?;

        let create_edges = "
            :create edges {
                from: String,
                to: String
                =>
                rel_type: String
            }
        ";
        self.run_schema_script(create_edges, "edges")?;

        // Initialize Library Table for Docs
        let create_library = "
            :create library {
                id: String
                =>
                name: String,
                version: String,
                content: String,
                language: String,
                chunk_type: String,
                embedding: <F32; 384>
            }
        ";
        self.run_schema_script(create_library, "library")?;

        // Schema Migration: Check if `library` has `embedding` column
        let check_lib_schema = "?[id, embedding] := *library{id, embedding} :limit 1";
        if let Err(_) = self.db.run_script(check_lib_schema, Default::default(), ScriptMutability::Immutable) {
            println!("Migrating `library` schema: Adding `embedding` column...");
            
            let _ = self.db.run_script("::remove library_old", Default::default(), ScriptMutability::Mutable);
            
            if let Err(e) = self.db.run_script("::rename library library_old", Default::default(), ScriptMutability::Mutable) {
                eprintln!("Failed to rename library->library_old: {}", e);
            }

            let _ = self.db.run_script("::remove library", Default::default(), ScriptMutability::Mutable);
            
            if let Err(e) = self.db.run_script(create_library, Default::default(), ScriptMutability::Mutable) {
                eprintln!("Failed to recreate library table: {}", e);
            }
            
            // Migrate data
            let migrate_lib_data = "
                ?[id, name, version, content, language, chunk_type, embedding] := *library_old{id, name, version, content, language, chunk_type}, embedding = $empty_vec
                :put library { id => name, version, content, language, chunk_type, embedding }
            ";
            let mut params = BTreeMap::new();
            params.insert("empty_vec".to_string(), vec_to_datavalue(vec![0.0; 384]));

            if let Err(e) = self.db.run_script(migrate_lib_data, params, ScriptMutability::Mutable) {
                eprintln!("Failed to migrate library data: {}", e);
            }
            
            let _ = self.db.run_script("::remove library_old", Default::default(), ScriptMutability::Mutable);
            println!("`library` verification/migration complete.");
        }

        let create_kv = "
            :create kv_cache {
                hash: String
                =>
                cache_id: String,
                created_at: Int
            }
        ";
        self.run_schema_script(create_kv, "kv_cache")?;

        let create_sync = "
            :create sync_log {
                path: String
                =>
                last_ingested: Float,
                content_hash: String
            }
        ";
        // Initialize sync_log table
        self.run_schema_script(create_sync, "sync_log")?;

        let create_event_log = "
            :create event_log {
                id: String
                =>
                op: String,
                data: Json,
                timestamp: Int,
                version: Int
            }
        ";
        self.run_schema_script(create_event_log, "event_log")?;

        let create_skills = "
            :create skills {
                name: String
                =>
                code: String,
                description: String,
                signature: String
            }
        ";
        self.run_schema_script(create_skills, "skills")?;

        let create_lib_index = "
            ::hnsw create library:idx {
                dim: 384,
                dtype: F32,
                fields: [embedding],
                distance: Cosine,
                m: 50,
                ef_construction: 200
            }
        ";
        self.run_schema_script(create_lib_index, "library:idx")?;

        let create_sessions = "
            :create sessions {
                id: String
                =>
                status: String,
                depth: Int,
                input: String,
                created_at: Int
            }
        ";
        self.run_schema_script(create_sessions, "sessions")?;

        let create_messages = "
            :create session_messages {
                session_id: String,
                msg_index: Int
                =>
                content: String
            }
        ";
        self.run_schema_script(create_messages, "session_messages")?;

        Ok(())
    }

    fn run_schema_script(&self, script: &str, name: &str) -> Result<()> {
        if let Err(e) = self.db.run_script(script, Default::default(), ScriptMutability::Mutable) {
            let msg = e.to_string();
            if !msg.contains("conflicts with an existing one") && !msg.contains("already exists") && !msg.contains("non-existent field") {
                eprintln!("Database initialization error ({}): {}", name, e);
            }
        }
        Ok(())
    }

    pub fn run_script(&self, script: &str, params: BTreeMap<String, DataValue>, mutability: ScriptMutability) -> Result<cozo::NamedRows> {
        self.db.run_script(script, params, mutability)
            .map_err(|e| anyhow!("CozoDB Error: {}", e))
    }

    /// Records an atomic fact (Event) in the immutable log.
    /// This is the 'Hickey Solution' to decoupled state.
    pub fn record_event(&self, op: &str, data: serde_json::Value) -> Result<()> {
        let script = "
            ?[id, op, data, timestamp, version] <- [[$id, $op, $data, $timestamp, $version]]
            :put event_log { id => op, data, timestamp, version }
        ";
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::from(Uuid::new_v4().to_string()));
        params.insert("op".to_string(), DataValue::from(op));
        
        // Correct construction for Cozo JSON
        params.insert("data".to_string(), DataValue::from(data));
        
        params.insert("timestamp".to_string(), DataValue::from(Utc::now().timestamp_millis()));
        params.insert("version".to_string(), DataValue::from(1));

        self.run_script(script, params, ScriptMutability::Mutable)?;
        Ok(())
    }
}

// Convert Rust Vec<f32> to Cozo DataValue::Vec
pub fn vec_to_datavalue(v: Vec<f32>) -> DataValue {
    DataValue::Vec(Vector::F32(Array1::from_vec(v)))
}
