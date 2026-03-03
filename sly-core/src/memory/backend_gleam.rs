use crate::error::{Result, SlyError};
use crate::memory::GraphNode;
use serde_json::{Value, json};
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct GleamBackend {
    pub db_path: String,
    ws_conn: Arc<Mutex<Option<GleamConnection>>>,
}

#[derive(Clone)]
struct GleamConnection {
    sender: futures::channel::mpsc::UnboundedSender<ControlMessage>,
}

enum ControlMessage {
    Request(Value, tokio::sync::oneshot::Sender<Result<Value>>),
    Subscribe(tokio::sync::mpsc::UnboundedSender<Value>),
}

impl GleamBackend {
    pub fn new(path: &str) -> Result<Self> {
        let db_path = if path == ":memory:" {
            ":memory:".to_string()
        } else {
            let p = std::path::Path::new(path);
            if p.is_absolute() {
                p.join("gleam.db").to_string_lossy().to_string()
            } else {
                std::env::current_dir().unwrap().join(p).join("gleam.db").to_string_lossy().to_string()
            }
        };
        Ok(Self { 
            db_path,
            ws_conn: Arc::new(Mutex::new(None)),
        })
    }

    async fn ensure_connection(&self) -> Result<GleamConnection> {
        let mut conn_lock = self.ws_conn.lock().await;
        if let Some(ref conn) = *conn_lock {
            return Ok(conn.clone());
        }

        let url = "ws://localhost:4000/ws";
        let (ws_stream, _) = connect_async(url).await
            .map_err(|e| SlyError::Database(format!("Failed to connect to Gleam server: {}", e)))?;
        
        let (mut write, mut read) = ws_stream.split();
        let (ctrl_tx, mut ctrl_rx) = futures::channel::mpsc::unbounded::<ControlMessage>();
        
        let mut pending_requests = std::collections::HashMap::new();
        let mut request_counter: u64 = 0;
        let mut subscribers = Vec::new();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(ctrl) = ctrl_rx.next() => {
                        match ctrl {
                            ControlMessage::Request(mut req, reply) => {
                                request_counter += 1;
                                let id = request_counter;
                                req["id"] = json!(id);
                                pending_requests.insert(id, reply);
                                if let Ok(json) = serde_json::to_string(&req) {
                                    let _ = write.send(Message::Text(json.into())).await;
                                }
                            }
                            ControlMessage::Subscribe(tx) => {
                                subscribers.push(tx);
                                let sub_msg = json!({ "method": "subscribe", "id": 0 });
                                if let Ok(json) = serde_json::to_string(&sub_msg) {
                                    let _ = write.send(Message::Text(json.into())).await;
                                }
                            }
                        }
                    }
                    Some(msg) = read.next() => {
                        match msg {
                            Ok(Message::Text(text)) => {
                                if let Ok(resp) = serde_json::from_str::<Value>(&text) {
                                    if let Some(id) = resp.get("id").and_then(|v| v.as_u64()) {
                                        if let Some(reply) = pending_requests.remove(&id) {
                                            let _ = reply.send(Ok(resp));
                                        }
                                    } else if let Some(msg_type) = resp.get("type").and_then(|v| v.as_str()) {
                                        if msg_type == "reactive_update" {
                                            subscribers.retain(|tx| {
                                                tx.send(resp.clone()).is_ok()
                                            });
                                        }
                                    }
                                }
                            }
                            Ok(Message::Close(_)) | Err(_) => break,
                            _ => {}
                        }
                    }
                    else => break,
                }
            }
        });

        let conn = GleamConnection { sender: ctrl_tx };
        *conn_lock = Some(conn.clone());
        Ok(conn)
    }

    pub async fn batch_add_nodes(&self, nodes: Vec<GraphNode>) -> Result<()> {
        let conn = self.ensure_connection().await?;
        for node in nodes {
            let req = json!({
                "method": "index",
                "params": [
                    &self.db_path,
                    &node.path,
                    &node.id,
                    &node.node_type,
                    0, // line
                    &node.content,
                    &node.signature
                ]
            });
            let (tx, rx) = tokio::sync::oneshot::channel();
            let _ = conn.sender.unbounded_send(ControlMessage::Request(req, tx));
            let _ = rx.await.map_err(|_| SlyError::Database("Gleam request failed".to_string()))??;
        }
        Ok(())
    }

    pub async fn get_symbols_for_path(&self, path: &str) -> Result<Vec<GraphNode>> {
        let req = json!({
            "method": "path-query",
            "params": [&self.db_path, path]
        });
        self.query_gleam(req).await
    }

    pub async fn search(&self, query: &str) -> Result<Vec<GraphNode>> {
        let req = json!({
            "method": "search",
            "params": [&self.db_path, query]
        });
        self.query_gleam(req).await
    }

    pub async fn query_as_of(&self, clauses: Value, tx_id: u64) -> Result<Vec<GraphNode>> {
        let req = json!({
            "method": "query",
            "clauses": clauses,
            "as_of": tx_id
        });
        self.query_gleam(req).await
    }

    async fn query_gleam(&self, req: Value) -> Result<Vec<GraphNode>> {
        let conn = self.ensure_connection().await?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = conn.sender.unbounded_send(ControlMessage::Request(req, tx));
        let resp = rx.await.map_err(|_| SlyError::Database("Gleam query failed".to_string()))??;
        
        let mut nodes = Vec::new();
        // GleamDB v2.0 returns {status: "ok", result: {rows: [], metadata: {}}}
        if let Some(result) = resp.get("result") {
            if let Some(rows) = result.get("rows").and_then(|v| v.as_array()) {
                for item in rows {
                    if let Some(obj) = item.as_object() {
                        nodes.push(GraphNode {
                            id: obj.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            content: obj.get("content").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            signature: obj.get("signature").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            node_type: obj.get("kind").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            path: obj.get("path").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            edges: Vec::new(),
                        });
                    }
                }
            }
        }
        Ok(nodes)
    }

    pub async fn speculative_soul(&self, facts: Value) -> Result<Value> {
        let req = json!({
            "method": "with_facts",
            "params": [&self.db_path, facts]
        });
        let conn = self.ensure_connection().await?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = conn.sender.unbounded_send(ControlMessage::Request(req, tx));
        let resp = rx.await.map_err(|_| SlyError::Database("Speculative execution failed".to_string()))??;
        Ok(resp)
    }

    pub async fn get_scc(&self, attr: &str) -> Result<Vec<Vec<u64>>> {
        let req = json!({
            "method": "graph",
            "algo": "scc",
            "attribute": attr
        });
        let conn = self.ensure_connection().await?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = conn.sender.unbounded_send(ControlMessage::Request(req, tx));
        let resp = rx.await.map_err(|_| SlyError::Database("SCC algorithm failed".to_string()))??;
        
        let mut components = Vec::new();
        if let Some(comps) = resp.get("components").and_then(|v| v.as_array()) {
            for comp in comps {
                if let Some(nodes) = comp.as_array() {
                    components.push(nodes.iter().filter_map(|n| n.as_u64()).collect());
                }
            }
        }
        Ok(components)
    }

    pub async fn subscribe_to_failures(&self) -> Result<tokio::sync::mpsc::UnboundedReceiver<Value>> {
        let clauses = json!([
            {
                "type": "pos",
                "clause": {
                    "e": {"kind": "var", "name": "e"},
                    "a": "test/status",
                    "v": {"kind": "val", "value": {"type": "str", "value": "failed"}}
                }
            }
        ]);
        self.subscribe(clauses).await
    }

    pub async fn subscribe(&self, clauses: Value) -> Result<tokio::sync::mpsc::UnboundedReceiver<Value>> {
        let conn = self.ensure_connection().await?;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        
        let sub_msg = json!({
            "method": "subscribe",
            "clauses": clauses
        });
        
        // We use a dummy oneshot since subscribe doesn't wait for a response ID in the current Gleam implementation
        // but the ControlMessage enum expects a reply.
        // Wait, looking at handle_ws_message in Gleam, it returns a JSON response immediately for "subscribe"
        let (reply_tx, _reply_rx) = tokio::sync::oneshot::channel();
        let _ = conn.sender.unbounded_send(ControlMessage::Request(sub_msg, reply_tx));
        let _ = conn.sender.unbounded_send(ControlMessage::Subscribe(tx));
        Ok(rx)
    }

    pub fn record_event(&self, _op: &str, _data: Value) -> Result<()> {
        Ok(())
    }
}
