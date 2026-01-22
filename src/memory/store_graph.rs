use anyhow::{anyhow, Result};
use cozo::{DataValue, ScriptMutability};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;


use super::backend_cozo::{CozoBackend, vec_to_datavalue};
use super::engine_candle::EmbeddingEngine;
use super::MemoryStore;
use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraphNode {
    pub id: String,
    pub content: String,
    pub node_type: String, // struct, fn, impl, file
    pub path: String,
    pub edges: Vec<String>, // IDs of related nodes
}

pub type LibraryEntry = (String, String, String, String, String, String, Vec<f32>);

pub struct Memory {
    backend: CozoBackend,
    engine: EmbeddingEngine,
}

impl Memory {
    pub async fn new(path: &str) -> Result<Self> {
        let backend = CozoBackend::new(path)?;
        let engine = EmbeddingEngine::new()?;

        Ok(Self { backend, engine })
    }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.engine.embed(text)
    }

    pub fn batch_embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        self.engine.batch_embed(texts)
    }

    pub fn record_event(&self, op: &str, data: Value) -> Result<()> {
        self.backend.record_event(op, data)
    }

    // --- Caching Logic ---

    pub async fn check_cache(&self, query: &str) -> Result<Option<String>> {
        let embedding = self.embed(query)?;

        let query_script = "
            ?[response] := ~cache:idx {
                response |
                query: $query_vec,
                k: 1,
                bind_distance: dist,
                ef: 100
            },
            dist < 0.1
        ";

        let mut params = BTreeMap::new();
        params.insert("query_vec".to_string(), vec_to_datavalue(embedding));

        let result = self.backend.run_script(query_script, params, ScriptMutability::Immutable)?;

        if let Some(DataValue::Str(s)) = result.rows.first().and_then(|r| r.first()) {
            return Ok(Some(s.to_string()));
        }

        Ok(None)
    }

    pub async fn store_cache(&self, query: &str, response: &str) -> Result<()> {
        let embedding = self.embed(query)?;
        let id = uuid::Uuid::new_v4().to_string();

        let query_script = "
            ?[id, query, response, embedding] <- [[$id, $query, $response, $embedding]]
            :put cache { id => query, response, embedding }
        ";

        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::from(id));
        params.insert("query".to_string(), DataValue::from(query));
        params.insert("response".to_string(), DataValue::from(response));
        params.insert("embedding".to_string(), vec_to_datavalue(embedding));

        self.backend.run_script(query_script, params, ScriptMutability::Mutable)?;

        // Record event
        self.backend.record_event("store_cache", serde_json::json!({
            "query": query,
            "response": response
        }))?;
        Ok(())
    }

    // --- Graph Node Logic ---

    pub async fn add_node(&self, node: &GraphNode) -> Result<()> {
        self.batch_add_nodes(vec![node.clone()]).await
    }

    pub async fn batch_add_nodes(&self, nodes: Vec<GraphNode>) -> Result<()> {
        if nodes.is_empty() {
            return Ok(());
        }

        let mut contents = Vec::new();
        for node in &nodes {
            contents.push(node.content.clone());
        }

        let embeddings = self.batch_embed(&contents)?;

        let query_script = "
            ?[id, content, type, path, embedding] <- $nodes
            :put nodes { id => content, type, path, embedding }

            ?[from, to, rel_type] <- $edges
            :put edges { from, to => rel_type }
        ";

        let mut node_rows = Vec::new();
        let mut edge_rows = Vec::new();

        for (i, node) in nodes.iter().enumerate() {
            node_rows.push(DataValue::List(vec![
                DataValue::from(node.id.clone()),
                DataValue::from(node.content.clone()),
                DataValue::from(node.node_type.clone()),
                DataValue::from(node.path.clone()),
                vec_to_datavalue(embeddings[i].clone()),
            ]));

            for target in &node.edges {
                edge_rows.push(DataValue::List(vec![
                    DataValue::from(node.id.clone()),
                    DataValue::from(target.clone()),
                    DataValue::from("related"),
                ]));
            }
        }

        let mut params = BTreeMap::new();
        params.insert("nodes".to_string(), DataValue::List(node_rows));
        params.insert("edges".to_string(), DataValue::List(edge_rows));

        self.backend.run_script(query_script, params, ScriptMutability::Mutable)
            .map_err(|e| anyhow!("Failed to batch add nodes: {}", e))?;

        // Record event (once for the whole batch)
        self.backend.record_event("batch_add_nodes", serde_json::json!({
            "count": nodes.len(),
            "paths": nodes.iter().map(|n| n.path.clone()).collect::<Vec<_>>()
        }))?;

        Ok(())
    }

    pub async fn store_lesson(&self, lesson: &str) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        self.add_node(&GraphNode {
            id,
            content: lesson.to_string(),
            node_type: "lesson".to_string(),
            path: "global".to_string(),
            edges: vec![],
        })
        .await
    }

    pub async fn store_heuristic(&self, heuristic: &str) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        self.add_node(&GraphNode {
            id,
            content: heuristic.to_string(),
            node_type: "heuristic".to_string(),
            path: "global".to_string(),
            edges: vec![],
        })
        .await
    }

    pub async fn find_related(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        let embedding = self.embed(query)?;

        // Hybrid Search: Vector similarity + Graph traversal (placeholder logic for now)
        let query_script = format!(
            "
            ?[content, dist] := ~nodes:idx {{
                content |
                query: $query_vec,
                k: {},
                bind_distance: dist,
                ef: 100
            }}
            :sort dist
        ",
            limit
        );

        let mut params = BTreeMap::new();
        params.insert("query_vec".to_string(), vec_to_datavalue(embedding));

        let result = self.backend.run_script(&query_script, params, ScriptMutability::Immutable)?;

        let mut results = Vec::new();
        for row in result.rows {
            if let Some(DataValue::Str(s)) = row.first() {
                results.push(s.to_string());
            }
        }

        Ok(results)
    }

    // --- Library / Autodidact Logic ---

    // High-performance batch insertion
    pub async fn batch_add_library_entries(&self, entries: Vec<LibraryEntry>) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let entries_len = entries.len();
        let library_names: Vec<String> = entries.iter().map(|e| e.1.clone()).collect();

        let query_script = "
            ?[id, name, version, content, language, chunk_type, embedding] <- $data
            :put library { id => name, version, content, language, chunk_type, embedding }
        ";

        let mut data_rows = Vec::new();

        for (id, name, version, content, language, chunk_type, embedding_vec) in entries {
            let row = vec![
                DataValue::from(id),
                DataValue::from(name),
                DataValue::from(version),
                DataValue::from(content),
                DataValue::from(language),
                DataValue::from(chunk_type),
                vec_to_datavalue(embedding_vec),
            ];
            data_rows.push(DataValue::List(row));
        }

        let mut params = BTreeMap::new();
        params.insert("data".to_string(), DataValue::List(data_rows));

        self.backend.run_script(query_script, params, ScriptMutability::Mutable)
            .map_err(|e| anyhow!("Failed to bulk add library entries: {}", e))?;

        // Record event
        self.backend.record_event("batch_add_library", serde_json::json!({
            "count": entries_len,
            "library_names": library_names
        }))?;

        Ok(())
    }

    pub async fn search_library(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        let embedding = self.embed(query)?;

        let query_script = format!(
            "
            ?[content, type, dist] := ~library:idx {{
                content, chunk_type: type |
                query: $query_vec,
                k: {},
                bind_distance: dist,
                ef: 100
            }}
            // Apply weight: boost definitions
            ?[content, score] := ?[content, type, dist],
                weight = if type == \"definition\" {{ 0.8 }} else {{ 1.0 }},
                score = dist * weight
            :sort score
            :limit {}
        ",
            limit * 2,
            limit
        );

        let mut params = BTreeMap::new();
        params.insert("query_vec".to_string(), vec_to_datavalue(embedding));

        let result = self.backend.run_script(&query_script, params, ScriptMutability::Immutable)
            .map_err(|e| anyhow!("Library search failed: {}", e))?;

        let mut results = Vec::new();
        for row in result.rows {
            if let Some(DataValue::Str(s)) = row.first() {
                results.push(s.to_string());
            }
        }
        Ok(results)
    }

    pub async fn get_known_libraries(&self) -> Result<Vec<String>> {
        let script = "?[name] := *library{name} :group by name";
        let result = self.backend.run_script(script, Default::default(), ScriptMutability::Immutable)?;

        let mut names = Vec::new();
        for row in result.rows {
            if let Some(DataValue::Str(s)) = row.first() {
                names.push(s.to_string());
            }
        }
        names.sort();
        names.dedup();
        Ok(names)
    }

    pub async fn get_known_libraries_with_versions(&self) -> Result<Vec<(String, String)>> {
        let script = "?[name, version] := *library{name, version}";
        let result = self.backend.run_script(script, Default::default(), ScriptMutability::Immutable)?;

        let mut libs = Vec::new();
        for row in result.rows {
            if let (Some(DataValue::Str(name)), Some(DataValue::Str(version))) = (row.first(), row.get(1)) {
                libs.push((name.to_string(), version.to_string()));
            }
        }
        libs.sort();
        libs.dedup();
        Ok(libs)
    }

    pub async fn register_library(&self, name: &str, version: &str) -> Result<()> {
        let script = "
            ?[id, name, version, content, language, chunk_type, embedding] := 
                id = $id, name = $name, version = $version, 
                content = \"\", language = \"\", chunk_type = \"metadata\",
                embedding = $empty_vec
            :put library { id => name, version, content, language, chunk_type, embedding }
        ";
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::from(format!("{}_{}", name, version)));
        params.insert("name".to_string(), DataValue::from(name.to_string()));
        params.insert("version".to_string(), DataValue::from(version.to_string()));
        params.insert("empty_vec".to_string(), vec_to_datavalue(vec![0.0; 384]));
        
        self.backend.run_script(script, params, ScriptMutability::Mutable)
            .map_err(|e| anyhow!("Failed to register library {}: {}", name, e))?;
        Ok(())
    }

    pub async fn get_neighborhood(&self, path: &str) -> Result<Vec<String>> {
        let script = "
            ?[content] := *nodes{path, content}, path = $path
            ?[content] := *nodes{id, content}, *edges{from: $path, to: id}
            ?[content] := *nodes{id, content}, *edges{from: id, to: $path}
        ";
        let mut params = BTreeMap::new();
        params.insert("path".to_string(), DataValue::from(path.to_string()));

        let res = self.backend.run_script(script, params, ScriptMutability::Immutable)?;

        let mut results = Vec::new();
        for row in res.rows {
            if let Some(DataValue::Str(s)) = row.first() {
                results.push(s.to_string());
            }
        }
        Ok(results)
    }

    // --- KV / Sync Logic ---

    pub async fn get_kv_cache(&self, hash: &str) -> Result<Option<String>> {
        let script = "?[cache_id] := kv_cache { hash: $hash, cache_id }";
        let mut params = BTreeMap::new();
        params.insert("hash".to_string(), DataValue::from(hash.to_string()));

        let res = self.backend.run_script(script, params, ScriptMutability::Immutable)?;
        if let Some(row) = res.rows.first() {
            if let Some(DataValue::Str(s)) = row.first() {
                return Ok(Some(s.to_string()));
            }
        }
        Ok(None)
    }

    pub async fn set_kv_cache(&self, hash: &str, cache_id: &str) -> Result<()> {
        let script = "
            ?[hash, cache_id, created_at] <- [[$hash, $cache_id, $now]]
            :put kv_cache { hash => cache_id, created_at }
        ";
        let now = chrono::Utc::now().timestamp();
        let mut params = BTreeMap::new();
        params.insert("hash".to_string(), DataValue::from(hash.to_string()));
        params.insert("cache_id".to_string(), DataValue::from(cache_id.to_string()));
        params.insert("now".to_string(), DataValue::from(now));

        self.backend.run_script(script, params, ScriptMutability::Mutable)?;
        Ok(())
    }

    pub async fn check_sync_status(&self, path: &str) -> Result<Option<(i64, String)>> {
        let script = "?[last_ingested, content_hash] := sync_log { path: $path, last_ingested, content_hash }";
        let mut params = BTreeMap::new();
        params.insert("path".to_string(), DataValue::from(path.to_string()));

        let res = self.backend.run_script(script, params, ScriptMutability::Immutable)?;

        if let Some(row) = res.rows.first() {
            let ts = match row.first() {
                Some(DataValue::Num(n)) => {
                    let s = format!("{:?}", n);
                    s.parse::<i64>().unwrap_or(0)
                }
                _ => 0,
            };
            let hash = match row.get(1) {
                Some(DataValue::Str(h)) => h.to_string(),
                _ => String::new(),
            };
            if !hash.is_empty() {
                return Ok(Some((ts, hash)));
            }
        }
        Ok(None)
    }

    pub async fn update_sync_status(&self, path: &str, hash: &str) -> Result<()> {
        let script = "
            ?[path, last_ingested, content_hash] <- [[$path, $now, $hash]]
            :put sync_log { path => last_ingested, content_hash }
        ";
        let now = chrono::Utc::now().timestamp();
        let mut params = BTreeMap::new();
        params.insert("path".to_string(), DataValue::from(path.to_string()));
        params.insert("hash".to_string(), DataValue::from(hash.to_string()));
        params.insert("now".to_string(), DataValue::from(now));

        self.backend.run_script(script, params, ScriptMutability::Mutable)?;
        Ok(())
    }

    pub fn backend_run_script(&self, script: &str) -> Result<cozo::NamedRows> {
        self.backend.run_script(script, BTreeMap::new(), ScriptMutability::Immutable)
    }

    pub async fn register_skill(&self, name: &str, code: &str, desc: &str, signature: &str) -> Result<()> {
        let script = "
            ?[name, code, description, signature] <- [[$name, $code, $desc, $sig]]
            :put skills { name, code, description, signature }
        ";
        let mut params = BTreeMap::new();
        params.insert("name".to_string(), DataValue::from(name.to_string()));
        params.insert("code".to_string(), DataValue::from(code.to_string()));
        params.insert("desc".to_string(), DataValue::from(desc.to_string()));
        params.insert("sig".to_string(), DataValue::from(signature.to_string()));

        self.backend.run_script(script, params, ScriptMutability::Mutable)?;
        Ok(())
    }

    pub async fn get_skill(&self, name: &str) -> Result<Option<String>> {
        let script = "?[code] := *skills{name: $name, code}";
        let mut params = BTreeMap::new();
        params.insert("name".to_string(), DataValue::from(name.to_string()));

        let res = self.backend.run_script(script, params, ScriptMutability::Immutable)?;
        if let Some(row) = res.rows.first() {
            if let Some(DataValue::Str(code)) = row.first() {
                return Ok(Some(code.to_string()));
            }
        }
        Ok(None)
    }

    // --- Session Persistence (Phase 5) ---

    pub async fn create_session(&self, session: &crate::core::session::AgentSession) -> Result<()> {
        let script = "
            ?[id, status, depth, input, created_at] <- [[$id, $status, $depth, $input, $now]]
            :put sessions { id => status, depth, input, created_at }
        ";
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::from(session.id.clone()));
        params.insert("status".to_string(), DataValue::from(format!("{:?}", session.status)));
        params.insert("depth".to_string(), DataValue::from(session.depth as i64));
        params.insert("input".to_string(), DataValue::from(session.messages.first().cloned().unwrap_or_default()));
        params.insert("now".to_string(), DataValue::from(chrono::Utc::now().timestamp()));

        self.backend.run_script(script, params, ScriptMutability::Mutable)?;

        // Store initial messages
        for (i, msg) in session.messages.iter().enumerate() {
            self.add_session_message(&session.id, i, msg).await?;
        }
        Ok(())
    }

    pub async fn update_session(&self, session: &crate::core::session::AgentSession) -> Result<()> {
        let script = "
            ?[id, status, depth, input, created_at] := *sessions{id, input, created_at}, 
                id = $id, status = $status, depth = $depth
            :put sessions { id => status, depth, input, created_at }
        ";
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::from(session.id.clone()));
        params.insert("status".to_string(), DataValue::from(format!("{:?}", session.status)));
        params.insert("depth".to_string(), DataValue::from(session.depth as i64));

        self.backend.run_script(script, params, ScriptMutability::Mutable)?;

        // Only add the NEWEST message to avoid re-writing everything? 
        // For simplicity and to match the 'Fact' approach, we just ensure 
        // the current list matches. In Datalog, we can just :put the messages.
        for (i, msg) in session.messages.iter().enumerate() {
            self.add_session_message(&session.id, i, msg).await?;
        }
        Ok(())
    }

    async fn add_session_message(&self, session_id: &str, index: usize, content: &str) -> Result<()> {
        let script = "
            ?[session_id, msg_index, content] <- [[$session_id, $index, $content]]
            :put session_messages { session_id, msg_index => content }
        ";
        let mut params = BTreeMap::new();
        params.insert("session_id".to_string(), DataValue::from(session_id.to_string()));
        params.insert("index".to_string(), DataValue::from(index as i64));
        params.insert("content".to_string(), DataValue::from(content.to_string()));

        self.backend.run_script(script, params, ScriptMutability::Mutable)?;
        Ok(())
    }

    pub async fn get_session(&self, id: &str) -> Result<Option<crate::core::session::AgentSession>> {
        let script = "?[status, depth, input] := *sessions{id: $id, status, depth, input}";
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::from(id.to_string()));

        let res = self.backend.run_script(script, params, ScriptMutability::Immutable)?;
        if let Some(row) = res.rows.first() {
            let status_str = match row.first() {
                Some(DataValue::Str(s)) => s.as_str(),
                _ => "Idle",
            };
            let depth = match row.get(1) {
                Some(DataValue::Num(n)) => {
                    let s = format!("{:?}", n);
                    s.parse::<usize>().unwrap_or(0)
                }
                _ => 0,
            };
            let status = match status_str {
                "Thinking" => crate::core::session::SessionStatus::Thinking,
                "AwaitingObservation" => crate::core::session::SessionStatus::AwaitingObservation,
                "Completed" => crate::core::session::SessionStatus::Completed,
                s if s.starts_with("Error") => crate::core::session::SessionStatus::Error(s.replace("Error(\"", "").replace("\")", "")),
                _ => crate::core::session::SessionStatus::Idle,
            };

            // Fetch messages
            let msg_script = "?[content] := *session_messages{session_id: $id, msg_index, content} :sort msg_index";
            let mut msg_params = BTreeMap::new();
            msg_params.insert("id".to_string(), DataValue::from(id.to_string()));
            let msg_res = self.backend.run_script(msg_script, msg_params, ScriptMutability::Immutable)?;
            
            let mut messages = Vec::new();
            for m_row in msg_res.rows {
                if let Some(DataValue::Str(c)) = m_row.first() {
                    messages.push(c.to_string());
                }
            }

            Ok(Some(crate::core::session::AgentSession {
                id: id.to_string(),
                messages,
                depth,
                status,
            }))
        } else {
            Ok(None)
        }
    }
}

#[async_trait]
impl MemoryStore for Memory {
    async fn recall(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        self.find_related(query, limit).await
    }

    async fn recall_facts(&self, query: &str) -> Result<Vec<String>> {
        self.find_related(query, 5).await
    }

    async fn search_library(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        self.search_library(query, limit).await
    }

    async fn store(&self, content: &str, _metadata: Option<Value>) -> Result<String> {
        self.store_heuristic(content).await?;
        Ok("stored".to_string())
    }

    async fn forget(&self, _id: &str) -> Result<()> {
        Ok(())
    }

    async fn count_nodes(&self) -> Result<usize> {
        let script = "?[count] := *nodes{id}, count = count(id)";
        let res = self.backend.run_script(script, Default::default(), ScriptMutability::Immutable)?;
        
        if let Some(row) = res.rows.first() {
             // Handle Cozo Num (can be f64 or similar)
            match row.first() {
                Some(DataValue::Num(n)) => {
                     // Check if it has a sensible integer representation or string
                     let s = format!("{:?}", n);
                     Ok(s.parse::<f64>().unwrap_or(0.0) as usize)
                },
                _ => Ok(0),
            }
        } else {
            Ok(0)
        }
    }

    async fn register_skill(&self, name: &str, code: &str, desc: &str, signature: &str) -> Result<()> {
        self.register_skill(name, code, desc, signature).await
    }

    async fn get_skill(&self, name: &str) -> Result<Option<String>> {
        self.get_skill(name).await
    }
}
