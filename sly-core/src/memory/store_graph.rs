use crate::memory::backend_cozo::CozoBackend;
use crate::memory::backend_gleam::GleamBackend;
use crate::memory::MemoryStore;
use crate::error::{Result, SlyError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use cozo::{DataValue, ScriptMutability};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraphNode {
    pub id: String,
    pub content: String,
    pub signature: String,
    pub node_type: String, // struct, fn, impl, file
    pub path: String,
    pub edges: Vec<String>, // IDs of related nodes
}

pub type LibraryEntry = (String, String, String, String, String, String);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TechnicalHeuristic {
    pub id: String,
    pub context: String, // mapped to 'tag'
    pub solution: String, // mapped to 'val'
    pub success_weight: f64,
}

pub struct Memory {
    backend: CozoBackend,
    gleam_backend: GleamBackend,
}

impl Memory {
    pub async fn new(path: &str, read_only: bool) -> Result<Self> {
        let backend = CozoBackend::new(path, read_only)?;
        let gleam_backend = GleamBackend::new(path)?;
        Ok(Self { backend, gleam_backend })
    }

    pub async fn new_light(path: &str, read_only: bool) -> Result<Self> {
        let backend = CozoBackend::new(path, read_only)?;
        let gleam_backend = GleamBackend::new(path)?;
        Ok(Self { backend, gleam_backend })
    }

    pub async fn new_transient() -> Result<Self> {
        let backend = CozoBackend::new(":memory:", false)?;
        let gleam_backend = GleamBackend::new(":memory:")?;
        Ok(Self { backend, gleam_backend })
    }

    pub fn record_event(&self, op: &str, data: Value) -> Result<()> {
        self.backend.record_event(op, data)
    }

    // --- Graph Node Logic ---

    pub async fn add_node(&self, node: &GraphNode) -> Result<()> {
        self.batch_add_nodes(vec![node.clone()]).await
    }

    pub async fn batch_add_nodes(&self, nodes: Vec<GraphNode>) -> Result<()> {
        if nodes.is_empty() {
            return Ok(());
        }

        // Parallel storage in GleamDB
        self.gleam_backend.batch_add_nodes(nodes.clone()).await?;

        // Keep Cozo for legacy/fallback for now
        let node_script = "
            ?[id, content, signature, type, path] <- $nodes
            :put nodes { id => content, signature, type, path }
        ";

        let mut node_rows = Vec::new();
        let mut edge_rows = Vec::new();

        for node in nodes.iter() {
            node_rows.push(DataValue::List(vec![
                DataValue::from(node.id.clone()),
                DataValue::from(node.content.clone()),
                DataValue::from(node.signature.clone()),
                DataValue::from(node.node_type.clone()),
                DataValue::from(node.path.clone()),
            ]));

            for target in &node.edges {
                edge_rows.push(DataValue::List(vec![
                    DataValue::from(node.id.clone()),
                    DataValue::from(target.clone()),
                    DataValue::from("related"),
                ]));
            }
        }

        let mut node_params = BTreeMap::new();
        node_params.insert("nodes".to_string(), DataValue::List(node_rows));

        self.backend.run_script(node_script, node_params, ScriptMutability::Mutable)
            .map_err(|e| SlyError::Database(format!("Failed to batch add nodes (nodes part): {}", e)))?;

        if !edge_rows.is_empty() {
            let edge_script = "
                ?[from, to, rel_type] <- $edges
                :put edges { from, to => rel_type }
            ";
            let mut edge_params = BTreeMap::new();
            edge_params.insert("edges".to_string(), DataValue::List(edge_rows));

            self.backend.run_script(edge_script, edge_params, ScriptMutability::Mutable)
                .map_err(|e| SlyError::Database(format!("Failed to batch add nodes (edges part): {}", e)))?;
        }

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
            signature: String::new(),
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
            signature: String::new(),
            node_type: "heuristic".to_string(),
            path: "global".to_string(),
            edges: vec![],
        })
        .await
    }

    pub async fn persist_technical_heuristic(&self, h: &TechnicalHeuristic) -> Result<()> {
        let content = serde_json::to_string(h).map_err(|e| SlyError::Task(e.to_string()))?;
        self.add_node(&GraphNode {
            id: format!("h:{}", h.id),
            content,
            signature: String::new(),
            node_type: "heuristic".to_string(),
            path: "global".to_string(),
            edges: vec![],
        }).await
    }

    pub async fn recall_technical_heuristics(&self, query_text: &str, limit: usize) -> Result<Vec<TechnicalHeuristic>> {
        let script = "
            ?[c] := *nodes{id, content: c}, str_includes(id, 'h:'), str_includes(c, $query)
            :limit $limit
        ";
        let mut params = BTreeMap::new();
        params.insert("query".to_string(), DataValue::from(query_text.to_string()));
        params.insert("limit".to_string(), DataValue::from(limit as i64));

        let res = self.backend.run_script(script, params, ScriptMutability::Immutable)?;
        let mut heuristics = Vec::new();
        for row in res.rows {
            if let Some(DataValue::Str(json)) = row.first() {
                if let Ok(h) = serde_json::from_str::<TechnicalHeuristic>(json) {
                    heuristics.push(h);
                }
            }
        }
        Ok(heuristics)
    }

    pub async fn find_related(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        // Fallback to simple keyword match since semantic search is removed
        let query_script = "
            ?[content] := *nodes{content}, content.contains($query)
            :limit $limit
        ";

        let mut params = BTreeMap::new();
        params.insert("query".to_string(), DataValue::from(query.to_string()));
        params.insert("limit".to_string(), DataValue::from(limit as i64));

        let result = self.backend.run_script(query_script, params, ScriptMutability::Immutable)?;

        let mut results = Vec::new();
        for row in result.rows {
            if let Some(DataValue::Str(s)) = row.first() {
                results.push(s.to_string());
            }
        }

        Ok(results)
    }

    /// Get all symbols from a specific file path
    pub async fn get_symbols_for_path(&self, path: &str) -> Result<Vec<GraphNode>> {
        // Query Gleam Knowledge Service
        match self.gleam_backend.get_symbols_for_path(path).await {
            Ok(nodes) if !nodes.is_empty() => return Ok(nodes),
            _ => {}
        }

        // Fallback to Cozo
        let query_script = "
            ?[id, content, signature, type, path] := *nodes{id, content, signature, type, path}, 
                str_includes(path, $path)
        ";

        let mut params = BTreeMap::new();
        params.insert("path".to_string(), DataValue::from(path.to_string()));

        let result = self.backend.run_script(query_script, params, ScriptMutability::Immutable)?;

        let mut nodes = Vec::new();
        for row in result.rows {
            if row.len() >= 5 {
                let id = match &row[0] { DataValue::Str(s) => s.to_string(), _ => continue };
                let content = match &row[1] { DataValue::Str(s) => s.to_string(), _ => String::new() };
                let signature = match &row[2] { DataValue::Str(s) => s.to_string(), _ => String::new() };
                let node_type = match &row[3] { DataValue::Str(s) => s.to_string(), _ => String::new() };
                let path = match &row[4] { DataValue::Str(s) => s.to_string(), _ => String::new() };

                nodes.push(GraphNode {
                    id,
                    content,
                    signature,
                    node_type,
                    path,
                    edges: Vec::new(),
                });
            }
        }

        Ok(nodes)
    }

    pub async fn batch_add_library_entries(&self, entries: Vec<LibraryEntry>) -> Result<()> {

        if entries.is_empty() {
            return Ok(());
        }
        let entries_len = entries.len();
        let library_names: Vec<String> = entries.iter().map(|e| e.1.clone()).collect();

        let query_script = "
            ?[id, name, version, content, language, chunk_type] <- $data
            :put library { id => name, version, content, language, chunk_type }
        ";

        let mut data_rows = Vec::new();

        for (id, name, version, content, language, chunk_type) in entries {
            let row = vec![
                DataValue::from(id),
                DataValue::from(name),
                DataValue::from(version),
                DataValue::from(content),
                DataValue::from(language),
                DataValue::from(chunk_type),
            ];
            data_rows.push(DataValue::List(row));
        }

        let mut params = BTreeMap::new();
        params.insert("data".to_string(), DataValue::List(data_rows));

        self.backend.run_script(query_script, params, ScriptMutability::Mutable)
            .map_err(|e| SlyError::Database(format!("Failed to bulk add library entries: {}", e)))?;

        self.backend.record_event("batch_add_library", serde_json::json!({
            "count": entries_len,
            "library_names": library_names
        }))?;

        Ok(())
    }

    pub async fn search_library(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        let query_script = "
            ?[content] := *library{content}, content.contains($query)
            :limit $limit
        ";

        let mut params = BTreeMap::new();
        params.insert("query".to_string(), DataValue::from(query.to_string()));
        params.insert("limit".to_string(), DataValue::from(limit as i64));

        let result = self.backend.run_script(query_script, params, ScriptMutability::Immutable)
            .map_err(|e| SlyError::Database(format!("Library search failed: {}", e)))?;

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

    pub async fn get_neighborhood(&self, node_id: &str, depth: usize) -> Result<Vec<String>> {
        // Multi-hop expansion via Datalog recursion
        let script = "
            visited[id, 0] := id = $id
            visited[id, d+1] := visited[prev_id, d], *edges{from: prev_id, to: id}, d < $depth
            visited[id, d+1] := visited[prev_id, d], *edges{from: id, to: prev_id}, d < $depth
            
            ?[content] := visited[id, d], *nodes{id, content}
        ";
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::from(node_id.to_string()));
        params.insert("depth".to_string(), DataValue::from(depth as i64));

        let res = self.backend.run_script(script, params, ScriptMutability::Immutable)?;

        let mut results = Vec::new();
        for row in res.rows {
            if let Some(DataValue::Str(s)) = row.first() {
                results.push(s.to_string());
            }
        }
        Ok(results)
    }

    pub async fn expand_context(&self, initial_ids: Vec<String>, depth: usize) -> Result<Vec<String>> {
        if initial_ids.is_empty() { return Ok(vec![]); }
        
        let script = "
            visited[id, 0] <- $initial_ids
            visited[id, d+1] := visited[prev_id, d], *edges{from: prev_id, to: id}, d < $depth
            visited[id, d+1] := visited[prev_id, d], *edges{from: id, to: prev_id}, d < $depth
            
            ?[content] := visited[id, d], *nodes{id, content}
        ";
        let mut params = BTreeMap::new();
        params.insert("initial_ids".to_string(), DataValue::List(initial_ids.into_iter().map(DataValue::from).collect()));
        params.insert("depth".to_string(), DataValue::from(depth as i64));

        let res = self.backend.run_script(script, params, ScriptMutability::Immutable)?;

        let mut results = Vec::new();
        for row in res.rows {
            if let Some(DataValue::Str(s)) = row.first() {
                results.push(s.to_string());
            }
        }
        Ok(results)
    }

    pub async fn get_visual_neighborhood(&self, node_id: &str, depth: usize) -> Result<String> {
        let nodes = self.get_neighborhood(node_id, depth).await?;
        if nodes.is_empty() { return Ok("   (Empty Neighborhood)".to_string()); }

        let mut output = format!("<b>Graph Neighborhood for <code>{}</code>:</b>\n", node_id);
        for (i, content) in nodes.iter().enumerate().take(15) {
            let prefix = if i == nodes.len() - 1 { "└── " } else { "├── " };
            let lines: Vec<&str> = content.lines().collect();
            let first_line = lines.first().unwrap_or(&"").chars().take(60).collect::<String>();
            output.push_str(&format!("<code>{}</code>{} ...\n", prefix, crate::io::telegram::html_escape(&first_line)));
        }
        if nodes.len() > 15 {
             output.push_str(&format!("<i>... and {} more nodes</i>", nodes.len() - 15));
        }
        Ok(output)
    }

    pub async fn check_sync_status(&self, path: &str) -> Result<Option<(i64, String)>> {
        let script = "?[last_ingested, content_hash] := *sync_log { path: $path, last_ingested, content_hash }";
        let mut params = BTreeMap::new();
        params.insert("path".to_string(), DataValue::from(path.to_string()));

        let res = self.backend.run_script(script, params, ScriptMutability::Immutable)?;

        if let Some(row) = res.rows.first() {
            let ts = match row.first() {
                Some(DataValue::Num(n)) => {
                    format!("{:?}", n).parse::<f64>().unwrap_or(0.0) as i64
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

    pub async fn create_session(&self, session: &crate::core::session::AgentSession) -> Result<()> {
        let last_result = session.last_action_result.clone().unwrap_or(serde_json::Value::Null);
        let script = "
            ?[id, status, depth, input, last_result, cache_id, metadata, created_at] <- [[$id, $status, $depth, $input, $last_result, $cache_id, $metadata, $now]]
            :put sessions_v6 { id => status, depth, input, last_result, cache_id, metadata, created_at }
        ";
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::from(session.id.clone()));
        params.insert("status".to_string(), DataValue::from(format!("{:?}", session.status)));
        params.insert("depth".to_string(), DataValue::from(session.depth as i64));
        params.insert("input".to_string(), DataValue::from(session.messages.first().cloned().unwrap_or_default()));
        params.insert("last_result".to_string(), DataValue::from(serde_json::to_string(&last_result).unwrap_or_default()));
        let cid_dv = match session.cache_id.clone() {
            Some(cid) => DataValue::from(cid),
            None => DataValue::Null,
        };
        params.insert("cache_id".to_string(), cid_dv);
        params.insert("metadata".to_string(), DataValue::from(serde_json::to_string(&session.metadata).unwrap_or_default()));
        params.insert("now".to_string(), DataValue::from(chrono::Utc::now().timestamp()));

        self.backend.run_script(script, params, ScriptMutability::Mutable)?;

        for (i, msg) in session.messages.iter().enumerate() {
            self.add_session_message(&session.id, i, msg).await?;
        }
        Ok(())
    }

    pub async fn update_session(&self, session: &crate::core::session::AgentSession) -> Result<()> {
        let last_result = session.last_action_result.clone().unwrap_or(serde_json::Value::Null);
        let script = "
            ?[id, status, depth, input, last_result, cache_id, metadata, created_at] := *sessions_v6{id, input, created_at}, 
                id = $id, status = $status, depth = $depth, last_result = $last_result, cache_id = $cache_id, metadata = $metadata
            :put sessions_v6 { id => status, depth, input, last_result, cache_id, metadata, created_at }
        ";
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::from(session.id.clone()));
        params.insert("status".to_string(), DataValue::from(format!("{:?}", session.status)));
        params.insert("depth".to_string(), DataValue::from(session.depth as i64));
        params.insert("last_result".to_string(), DataValue::from(serde_json::to_string(&last_result).unwrap_or_default()));
        let cid_dv = match session.cache_id.clone() {
            Some(cid) => DataValue::from(cid),
            None => DataValue::Null,
        };
        params.insert("cache_id".to_string(), cid_dv);
        params.insert("metadata".to_string(), DataValue::from(serde_json::to_string(&session.metadata).unwrap_or_default()));

        self.backend.run_script(script, params, ScriptMutability::Mutable)?;

        for (i, msg) in session.messages.iter().enumerate() {
            self.add_session_message(&session.id, i, msg).await?;
        }

        Ok(())
    }

    pub async fn checkpoint_session(&self, session: &crate::core::session::AgentSession) -> Result<()> {
        let script = "?[max_idx] := *session_snapshots{session_id: $id, snapshot_index: max_idx} :limit 1";
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::from(session.id.clone()));
        let res = self.backend.run_script(script, params, ScriptMutability::Immutable)?;
        
        let next_idx = if let Some(row) = res.rows.first() {
            if let Some(DataValue::Num(n)) = row.first() {
                let current_idx: i64 = format!("{:?}", n).parse().unwrap_or(0);
                current_idx + 1
            } else { 0 }
        } else { 0 };

        let snap_script = "
            ?[session_id, snapshot_index, history] <- [[$session_id, $snapshot_index, $history]]
            :create session_snapshots { session_id, snapshot_index => history }
        ";
        let mut snap_params = BTreeMap::new();
        snap_params.insert("session_id".to_string(), DataValue::from(session.id.clone()));
        snap_params.insert("snapshot_index".to_string(), DataValue::from(next_idx));
        snap_params.insert("history".to_string(), DataValue::from(serde_json::to_string(&session.messages).unwrap_or_default()));
        self.backend.run_script(snap_script, snap_params, ScriptMutability::Mutable)?;
        Ok(())
    }

    pub async fn rollback_session(&self, session_id: &str) -> Result<Option<crate::core::session::AgentSession>> {
        let mut session = match self.get_session(session_id).await? {
            Some(s) => s,
            None => return Ok(None),
        };

        let script = "?[idx, history] := *session_snapshots{session_id: $id, snapshot_index: idx, history} :sort idx desc :limit 1";
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::from(session_id.to_string()));
        let res = self.backend.run_script(script, params, ScriptMutability::Immutable)?;
        
        if let Some(row) = res.rows.first() {
            if let Some(DataValue::Json(h)) = row.get(1) {
                if let Ok(history) = serde_json::from_value::<Vec<String>>(h.0.clone()) {
                    session.messages = history;
                    session.status = crate::core::session::SessionStatus::Idle;
                    
                    // Remove the snapshot we just rolled back to (optional, but consistent with 'pop')
                    if let Some(DataValue::Num(idx)) = row.first() {
                        let del_script = "?[session_id, snapshot_index] := *session_snapshots{session_id, snapshot_index}, session_id = $id, snapshot_index = $idx :rm session_snapshots { session_id, snapshot_index }";
                        let mut del_params = BTreeMap::new();
                        del_params.insert("id".to_string(), DataValue::from(session_id.to_string()));
                        del_params.insert("idx".to_string(), DataValue::Num(idx.clone()));
                        self.backend.run_script(del_script, del_params, ScriptMutability::Mutable)?;
                    }
                    
                    self.update_session(&session).await?;
                    return Ok(Some(session));
                }
            }
        }
        Ok(None)
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
        let script = "?[status, depth, input, last_result, cache_id, metadata] := *sessions_v6{id: $id, status, depth, input, last_result, cache_id, metadata}";
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::from(id.to_string()));

        let res = self.backend.run_script(script, params, ScriptMutability::Immutable)?;
        if let Some(row) = res.rows.first() {
            let (status_str, depth) = match (row.first(), row.get(1)) {
                (Some(DataValue::Str(s)), Some(DataValue::Num(n))) => {
                    let d_str = format!("{:?}", n);
                    (s.as_str(), d_str.parse::<usize>().unwrap_or(0))
                }
                _ => ("Idle", 0),
            };
            let last_action_result = match row.get(3) {
                Some(DataValue::Str(s)) => {
                    serde_json::from_str(s).ok()
                }
                _ => None,
            };
            let cache_id = match row.get(4) {
                Some(DataValue::Str(s)) => Some(s.to_string()),
                _ => None,
            };
            let metadata = match row.get(5) {
                Some(DataValue::Str(s)) => serde_json::from_str(s).unwrap_or_default(),
                _ => std::collections::HashMap::new(),
            };
            let status = match status_str {
                "Thinking" => crate::core::session::SessionStatus::Thinking,
                "AwaitingObservation" => crate::core::session::SessionStatus::AwaitingObservation,
                "Completed" => crate::core::session::SessionStatus::Completed,
                s if s.starts_with("Error") => crate::core::session::SessionStatus::Error(s.replace("Error(\"", "").replace("\")", "")),
                _ => crate::core::session::SessionStatus::Idle,
            };

            let msg_script = "?[msg_index, content] := *session_messages{session_id: $id, msg_index, content} :sort msg_index";
            let mut msg_params = BTreeMap::new();
            msg_params.insert("id".to_string(), DataValue::from(id.to_string()));
            let msg_res = self.backend.run_script(msg_script, msg_params, ScriptMutability::Immutable)?;
            
            let mut messages = Vec::new();
            for m_row in msg_res.rows {
                if let Some(DataValue::Str(c)) = m_row.get(1) {
                    messages.push(c.to_string());
                }
            }

            Ok(Some(crate::core::session::AgentSession {
                id: id.to_string(),
                messages,
                depth,
                status,
                last_action_result,
                cache_id,
                metadata,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_active_session_id(&self) -> Result<Option<String>> {
        let script = "?[id] := *sessions{id, status}, status = 'Thinking' :limit 1";
        let res = self.backend.run_script(script, Default::default(), ScriptMutability::Immutable)?;
        if let Some(row) = res.rows.first() {
            if let Some(DataValue::Str(id)) = row.first() {
                return Ok(Some(id.to_string()));
            }
        }
        
        // If none is 'Thinking', check for 'AwaitingObservation' or 'PendingCommit'
        let script_alt = "?[id] := *sessions{id, status}, (status = 'AwaitingObservation' or status = 'PendingCommit') :sort id desc :limit 1";
        let res_alt = self.backend.run_script(script_alt, Default::default(), ScriptMutability::Immutable)?;
        if let Some(row) = res_alt.rows.first() {
            if let Some(DataValue::Str(id)) = row.first() {
                return Ok(Some(id.to_string()));
            }
        }

        Ok(None)
    }

    pub async fn get_latest_session_id(&self) -> Result<Option<String>> {
        let script = "?[id] := *sessions{id, created_at} :sort created_at desc :limit 1";
        let res = self.backend.run_script(script, Default::default(), ScriptMutability::Immutable)?;
        if let Some(row) = res.rows.first() {
            if let Some(DataValue::Str(id)) = row.first() {
                return Ok(Some(id.to_string()));
            }
        }
        Ok(None)
    }

    pub async fn subscribe_to_failures(&self) -> Result<tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>> {
        self.gleam_backend.subscribe_to_failures().await
    }

    pub async fn subscribe_to_clauses(&self, clauses: serde_json::Value) -> Result<tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>> {
        self.gleam_backend.subscribe(clauses).await
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

    async fn recall_as_of(&self, clauses: Value, tx_id: u64) -> Result<Vec<GraphNode>> {
        self.gleam_backend.query_as_of(clauses, tx_id).await
    }

    async fn count_nodes(&self) -> Result<usize> {
        let script = "?[id] := *nodes{id}";
        let res = self.backend.run_script(script, Default::default(), ScriptMutability::Immutable)?;
        Ok(res.rows.len())
    }

    async fn register_skill(&self, _name: &str, _code: &str, _desc: &str, _signature: &str) -> Result<()> {
        Ok(())
    }

    async fn get_skill(&self, _name: &str) -> Result<Option<String>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    async fn setup_memory(name: &str) -> (Memory, PathBuf) {
        let temp_dir = std::env::temp_dir().join(format!("sly_mem_test_{}_v3", name));
        if temp_dir.exists() { let _ = fs::remove_dir_all(&temp_dir); }
        fs::create_dir_all(&temp_dir).unwrap();
        
        let path = temp_dir.join("cozo").to_string_lossy().to_string();
        let mem = Memory::new(&path, false).await.expect("Failed to create memory");
        (mem, temp_dir)
    }

    #[tokio::test]
    async fn test_memory_record_event() -> Result<()> {
        let (mem, _tmp) = setup_memory("record_event").await;
        mem.record_event("test_op", serde_json::json!({"key": "value"}))?;
        Ok(())
    }

    #[tokio::test]
    async fn test_memory_add_node() -> Result<()> {
        let (mem, _tmp) = setup_memory("add_node").await;
        let node = GraphNode {
            id: "test:1".to_string(),
            content: "Some content".to_string(),
            signature: "sig".to_string(),
            node_type: "test".to_string(),
            path: "test.rs".to_string(),
            edges: vec![],
        };
        mem.add_node(&node).await?;
        
        let count = mem.count_nodes().await?;
        assert_eq!(count, 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_memory_session() -> Result<()> {
        let (mem, _tmp) = setup_memory("session").await;
        let session = crate::core::session::AgentSession {
            id: "sess_1".to_string(),
            messages: vec!["Hello".to_string()],
            depth: 1,
            status: crate::core::session::SessionStatus::Idle,
            last_action_result: None,
            cache_id: None,
            metadata: std::collections::HashMap::new(),
        };
        
        mem.create_session(&session).await?;
        let loaded = mem.get_session("sess_1").await?;
        assert!(loaded.is_some());
        let loaded_val = loaded.unwrap();
        assert_eq!(loaded_val.id, "sess_1");
        assert_eq!(loaded_val.messages[0], "Hello");
        Ok(())
    }

    #[tokio::test]
    async fn test_technical_heuristics() -> Result<()> {
        let (mem, _tmp) = setup_memory("heuristics").await;
        let h = TechnicalHeuristic {
            id: "h1".to_string(),
            context: "rust, auth, jwt".to_string(),
            solution: "Use jsonwebtoken with HS256".to_string(),
            success_weight: 1.0,
        };
        
        mem.persist_technical_heuristic(&h).await?;
        
        let recalled = mem.recall_technical_heuristics("auth", 5).await?;
        assert_eq!(recalled.len(), 1);
        assert_eq!(recalled[0].id, "h1");
        assert_eq!(recalled[0].solution, "Use jsonwebtoken with HS256");
        
        let none = mem.recall_technical_heuristics("javascript", 5).await?;
        assert_eq!(none.len(), 0);
        
        Ok(())
    }
}
