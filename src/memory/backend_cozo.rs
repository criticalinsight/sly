use crate::error::{Result, SlyError};
use colored::*;
use cozo::{DataValue, DbInstance, ScriptMutability};
use std::collections::BTreeMap;
use std::path::Path;
use uuid::Uuid;
use chrono::Utc;

pub struct CozoBackend {
    pub db: DbInstance,
}

impl CozoBackend {
    pub fn new(path: &str, read_only: bool) -> Result<Self> {
        let is_ephemeral = path == ":memory:";
        
        let (engine, path_owned) = if is_ephemeral {
            ("mem", String::new())
        } else {
            let db_path = Path::new(path).join("cozo.db");
            ("rocksdb", db_path.to_str().ok_or_else(|| SlyError::Database("Invalid UTF-8 in database path".to_string()))?.to_string())
        };
        let path_str = path_owned.as_str();

        let mut retries = 0;
        let max_retries = if is_ephemeral { 0 } else { 10 };
        
        let db = loop {
            let options = if is_ephemeral {
                r#"{}"#
            } else if read_only {
                r#"{"read_only": true, "WAL": true}"#
            } else {
                r#"{"WAL": true}"#
            };

            match DbInstance::new(engine, path_str, options) {
                Ok(db) => break db,
                Err(e) if !is_ephemeral && !read_only && retries < max_retries && e.to_string().contains("Resource temporarily unavailable") => {
                    retries += 1;
                    eprintln!("⚠️ Database is locked. Retry {}/{}...", retries, max_retries);
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                }
                Err(e) if !is_ephemeral && read_only && retries < max_retries && e.to_string().contains("Resource temporarily unavailable") => {
                    retries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                Err(e) => return Err(SlyError::Database(format!("Failed to open CozoDB: {}", e))),
            }
        };

        let backend = Self { db };
        if !read_only {
            backend.initialize_schema()?;
        }
        Ok(backend)
    }

    fn initialize_schema(&self) -> Result<()> {
        // Initialize Schema (Vector-less)
        let create_nodes = "
            :create nodes {
                id: String
                =>
                content: String,
                signature: String,
                type: String,
                path: String
            }
        ";
        self.run_schema_script(create_nodes, "nodes")?;

        let create_edges = "
            :create edges {
                from: String,
                to: String
                =>
                rel_type: String
            }
        ";
        self.run_schema_script(create_edges, "edges")?;

        let create_library = "
            :create library {
                id: String
                =>
                name: String,
                version: String,
                content: String,
                language: String,
                chunk_type: String
            }
        ";
        self.run_schema_script(create_library, "library")?;

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
                last_ingested: Int,
                content_hash: String
            }
        ";
        self.run_schema_script(create_sync, "sync_log")?;

        let create_event_log = "
            :create event_log {
                id: String
                =>
                op: String,
                data: Json,
                timestamp: Int,
                version: Int,
                signature: String
            }
        ";
        self.run_schema_script(create_event_log, "event_log")?;

        let create_sessions = "
            :create sessions_v6 {
                id: String
                =>
                status: String,
                depth: Int,
                input: String,
                last_result: String,
                cache_id: String?,
                metadata: String,
                created_at: Int
            }
        ";
        self.run_schema_script(create_sessions, "sessions_v6")?;

        let create_messages = "
            :create session_messages {
                session_id: String,
                msg_index: Int
                =>
                content: String
            }
        ";
        self.run_schema_script(create_messages, "session_messages")?;

        let create_snapshots = "
            :create session_snapshots {
                session_id: String,
                snapshot_index: Int
                =>
                history: Json
            }
        ";
        self.run_schema_script(create_snapshots, "session_snapshots")?;

        Ok(())
    }

    fn run_schema_script(&self, script: &str, name: &str) -> Result<()> {
        match self.db.run_script(script, Default::default(), ScriptMutability::Mutable) {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("conflicts with an existing one") || msg.contains("already exists") {
                    Ok(())
                } else if msg.contains("non-existent field") || msg.contains("mismatch") {
                    println!("   {} Recreating schema for {}", "🔄".yellow(), name);
                    let _ = self.db.run_script(&format!("::remove {}", name), Default::default(), ScriptMutability::Mutable);
                    self.db.run_script(script, Default::default(), ScriptMutability::Mutable)
                        .map(|_| ())
                        .map_err(|e| SlyError::Database(format!("Failed to recreate {}: {}", name, e)))
                } else {
                    println!("   {} Schema error for {}: {}", "⚠️".red(), name, e);
                    Err(SlyError::Database(format!("Schema error {}: {}", name, e)))
                }
            }
        }
    }

    pub fn run_script(&self, script: &str, params: BTreeMap<String, DataValue>, mutability: ScriptMutability) -> Result<cozo::NamedRows> {
        self.db.run_script(script, params, mutability)
            .map_err(|e| SlyError::Database(format!("CozoDB Error: {}", e)))
    }

    pub fn record_event(&self, op: &str, data: serde_json::Value) -> Result<()> {
        use sha2::{Sha256, Digest};
        
        let id = Uuid::new_v4().to_string();
        let timestamp = Utc::now().timestamp_millis();
        let version = 1;

        // Create canonical string for signing
        let payload = format!("{}:{}:{}:{}", id, op, data, timestamp);
        let mut hasher = Sha256::new();
        hasher.update(payload.as_bytes());
        let signature = hex::encode(hasher.finalize());

        let script = "
            ?[id, op, data, timestamp, version, signature] <- [[$id, $op, $data, $timestamp, $version, $signature]]
            :put event_log { id => op, data, timestamp, version, signature }
        ";
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::from(id));
        params.insert("op".to_string(), DataValue::from(op));
        params.insert("data".to_string(), DataValue::from(data));
        params.insert("timestamp".to_string(), DataValue::from(timestamp));
        params.insert("version".to_string(), DataValue::from(version));
        params.insert("signature".to_string(), DataValue::from(signature));

        self.run_script(script, params, ScriptMutability::Mutable)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_event_signing() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("sly_test_signing");
        if temp_dir.exists() { let _ = std::fs::remove_dir_all(&temp_dir); }
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let path = temp_dir.to_str().unwrap();
        let backend = CozoBackend::new(path, false)?;
        
        backend.record_event("test_op", json!({"key": "value"}))?;
        
        // Correct Cozo query syntax: explicit field names for all fields
        let res = backend.db.run_script("?[id, op, sig] := *event_log{id, op, data, timestamp, version, signature: sig}", Default::default(), ScriptMutability::Immutable).unwrap();
        assert_eq!(res.rows.len(), 1);
        let sig = res.rows[0][2].clone();
        assert!(matches!(sig, cozo::DataValue::Str(_)));
        if let cozo::DataValue::Str(s) = sig {
            assert_eq!(s.len(), 64); // SHA-256 hex length
        }
        
        Ok(())
    }
}
