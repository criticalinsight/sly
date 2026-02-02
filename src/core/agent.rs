use crate::core::parser::{parse_action, AgentAction};
use crate::mcp::registry;
use colored::*;
use std::sync::Arc;
use crate::memory::MemoryStore;

pub async fn step_agent_session(
    session_id: String, 
    state: Arc<crate::core::state::GlobalState>,
    max_loops: usize,
) {
    let mut session = match state.memory_raw.get_session(&session_id).await {
        Ok(Some(s)) => s,
        _ => return,
    };

    if session.depth >= max_loops {
        println!("{} Session {} reached max depth", "⚠️".red(), session_id);
        use crate::core::bus::SlyEvent;
        let _ = state.bus.publish(SlyEvent::Error(session_id.clone(), format!("Max loops reached ({} steps). Please extend if needed.", max_loops))).await;
        return;
    }

    // Phase 8: Atomic Checkpoint before speculation
    let _ = state.memory_raw.checkpoint_session(&session).await;

    // 1. Fetch Metadata (Optimized Cached Step)
    let mut cache = state.metadata_cache.lock().await;
    if cache.is_empty() {
        println!("   {} Initializing MCP Tool Cache (Disabled to prevent hang)...", "📥".cyan());
        // *cache = registry::get_all_tool_metadata(&state.mcp_clients).await;
        *cache = vec![];
    }
    let tool_metadata = cache.clone();
    drop(cache); // Release lock early
    
    // 4. Prepare Context with Sliding Window
    let _checkpoint = session.clone();
    let context_limit = 10;
    let pruned_messages = if session.messages.len() > context_limit {
        let first = session.messages.first().cloned().unwrap_or_default();
        let recent = session.messages.iter().skip(session.messages.len() - 6).cloned().collect::<Vec<_>>();
        let mut m = vec![first];
        m.push(format!("--- Context Folded ({} steps omitted) ---", session.messages.len() - 7));
        m.extend(recent);
        m
    } else {
        session.messages.clone()
    };

    // 5. Build Technical Context (for step 0)
    let mut technical_context = String::new();
    if session.depth == 0 {
        let tool_defs = registry::get_tool_definitions(&tool_metadata).await;
        let heuristics = if let Ok(h) = state.memory_raw.recall_technical_heuristics(&session.id, 5).await {
            h.iter().map(|it| format!("* **Pattern:** {}\n", it.solution)).collect::<String>()
        } else {
            String::new()
        };
        technical_context = format!("\n\n## TOOLS\n{}\n\n## HEURISTICS\n{}\n\n## GRAPH SCHEMA\nNodes: `nodes {{ id => content, signature, type, path }}`\nEdges: `edges {{ parent => child }}`\n", 
           tool_defs, heuristics);
    }

    // 6. Build Structured History (Differential/Turn-based)
    // 6. Build Structured History (Differential/Turn-based)
    let history: Vec<serde_json::Value> = pruned_messages.iter().enumerate().map(|(i, m)| {
        let role = if i % 2 == 0 { "user" } else { "model" };
        let mut text = m.clone();
        
        // At step 0, we must inject tool definitions and heuristics into the first USER message
        if i == 0 && session.depth == 0 {
             text = format!("{}{}", text, technical_context);
        }

        // Apply Deduplication and Pruning
        let deduped = crate::knowledge::deduplicator::SemanticDeduplicator::deduplicate(&text);
        let pruned = crate::core::pruner::LinguisticPruner::prune(&deduped);

        serde_json::json!({
            "role": role,
            "parts": [{ "text": pruned }]
        })
    }).collect();

    let last_msg = session.messages.last().map(|m| m.to_lowercase()).unwrap_or_default();
    let level = if last_msg.contains("error") || last_msg.contains("failed") || last_msg.contains("not found") {
        println!("   {} [Bot Mode] Auto-Escalating to High Reasoning...", "🚀".magenta());
        crate::core::cortex::ThinkingLevel::High
    } else {
        crate::core::cortex::ThinkingLevel::Low
    };

    println!("{} [Session {}] Thinking ({:?})...", "🤔".magenta(), session_id, level);
    
    use futures::StreamExt;
    use colored::Colorize;
    use std::io::Write;

    let stream_res = state.cortex.generate_stream(history, level, session.cache_id.clone()).await;
    let mut full_response = String::new();
    let _last_tg_update = std::time::Instant::now();
    let _tg_msg_id: Option<i64> = None;

    match stream_res {
        Ok(mut stream) => {
            print!("🤖 ");
            
            // Initial Output (Optional)
            
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(text) => {
                        let text_str = text.as_str();
                        print!("{}", text_str.green());
                        let _ = std::io::stdout().flush();
                        full_response.push_str(text_str);
                        
                        // Live Stream to Bus (Debounced) - DISABLED for Simplification (Batch Send at End)
                        // If we want streaming, we need io.update_status() or similar.
                        /*
                        if last_tg_update.elapsed().as_secs() >= 2 {
                             let _ = state.bus.publish(SlyEvent::ThoughtStream(session_id.clone(), full_response.clone())).await;
                             last_tg_update = std::time::Instant::now();
                        }
                        */
                    }
                    Err(e) => {
                        eprintln!("\n{} Streaming error: {}", "⚠️".red(), e);
                        break;
                    }
                }
            }
            println!();
            
            // Final Update ensures complete message is visible
            {
                let mut io = state.io.lock().await;
                let _ = io.send_message(&full_response).await;
            }
        }
        Err(e) => {
            eprintln!("{} Cortex stream start failed: {}", "⚠️".red(), e);
        }
    }

    if !full_response.is_empty() {
        let step_depth = session.depth;
        session = session.with_message(format!("**Sly (Step {}):**\n{}", step_depth + 1, full_response.clone()))
                               .with_depth_increment();
        
        match parse_action(&full_response) {
            Ok(actions) => {
                for action in actions {
                    session = handle_action(
                        action, 
                        session, 
                        state.clone(),
                        &tool_metadata, 
                    ).await;
                }
            }
            Err(e) => {
                eprintln!("Parse error: {}", e);
                session = session.with_status(crate::core::session::SessionStatus::Error(e.to_string()));
            }
        }
        let _ = state.memory_raw.update_session(&session).await;

        // Final Notification Check for Completion or Errors
        let status = session.status.clone();
        if matches!(status, crate::core::session::SessionStatus::Completed) || 
           matches!(status, crate::core::session::SessionStatus::Error(_)) {
            
            let _ = state.bus.publish(crate::core::bus::SlyEvent::Thought(session_id.clone(), full_response.clone())).await;
        }
    }
}

pub async fn step_thought_analysis(
    session_id: String,
    query: String,
    state: Arc<crate::core::state::GlobalState>,
) {
    let cache = state.metadata_cache.lock().await;
    if cache.is_empty() {
        // *cache = registry::get_all_tool_metadata(&state.mcp_clients).await;
    }
    let _tool_metadata = cache.clone();
    drop(cache);

    let prompt = format!("## SWARM ANALYSIS REQUEST\nSession ID: {}\nQuery: {}\n\nAnalyze the structural context and provide a brief technical insight. NO destructive actions. Focus on discovery.", session_id, query);

    let history = vec![serde_json::json!({
        "role": "user",
        "parts": [{ "text": prompt }]
    })];

    match state.cortex.generate(history, crate::core::cortex::ThinkingLevel::Low, None).await {
        Ok(response) => {
            if let Ok(Some(session)) = state.memory_raw.get_session(&session_id).await {
                let observation = format!("**Swarm Analysis Insight:**\n{}", response);
                let session = session.with_message(observation);
                let _ = state.memory_raw.update_session(&session).await;
            }
        }
        Err(e) => eprintln!("Swarm analysis error: {}", e),
    }
}


async fn handle_action(
    action: AgentAction, 
    session: crate::core::session::AgentSession, 
    state: Arc<crate::core::state::GlobalState>,
    tool_metadata: &[registry::McpToolMetadata],
) -> crate::core::session::AgentSession {
    match action {
        AgentAction::CallTool { tool_name, arguments } => {
            println!("{} 🛠️  Calling Tool: {}...", "⚙️".cyan(), tool_name);
            match registry::call_mcp_tool(tool_metadata, &tool_name, arguments).await {
                Ok(tool_output) => {
                    let mut s = session.with_message(format!("**Observation (Tool '{}'):**\n```json\n{}\n```", tool_name, tool_output));
                    s.last_action_result = Some(tool_output);
                    s
                }
                Err(e) => {
                    session.with_message(format!("**Observation (Error from '{}'):**\n{}", tool_name, e))
                }
            }
        }
        AgentAction::WriteFile { path, content } => {
             use crate::core::fs::{FileSystemAction, execute_action};
             let is_md = path.ends_with(".md");
             let fs_action = FileSystemAction::Write { 
                 path: std::path::PathBuf::from(&path), 
                 content: content.clone()
             };
             println!("{} 📝 FileSystemAction: {:?}", "💾".blue(), fs_action);
             match execute_action(&state.overlay, fs_action) {
                 Ok(_) => {
                     // Collaborative Review Hook
                     if is_md {
                          let mut io = state.io.lock().await;
                          let _ = io.send_message(&format!("📝 **FileSystem Action**: Wrote file `{}`", path)).await;
                     }
                     session.with_message(format!("**Observation:** Action successfully executed in OverlayFS."))
                 }
                 Err(e) => {
                     eprintln!("     {} Action Failed: {}", "⚠️".red(), e);
                     session.with_message(format!("**Observation (Error):** Failed to execute action: {}", e))
                 }
             }
        }
        AgentAction::ExecShell { command, .. } => {
             println!("{} 🐚 ExecShell: {}", "💻".blue(), command);
             match tokio::process::Command::new("sh").arg("-c").arg(&command).output().await {
                 Ok(output) => {
                     let code = output.status.code().unwrap_or(-1);
                     let stdout = String::from_utf8_lossy(&output.stdout);
                     let stderr = String::from_utf8_lossy(&output.stderr);
                     
                     let result = format!("Exit Code: {}\nSTDOUT:\n{}\nSTDERR:\n{}", code, stdout, stderr);
                     
                     // Reflexion Hook: If failure (non-zero) AND sufficient depth allowance
                     if code != 0 && session.depth < (state.config.max_autonomous_loops - 5) {
                         match crate::core::reflexion::attempt_repair(
                             &session.id, 
                             state.clone(),
                             &command,
                             &stderr
                         ).await {
                             Ok(repair_obs) => {
                                 println!("   {} Reflexion Repair Successful.", "🩹".green());
                                 return session.with_message(format!("**Observation (Shell ERROR & Reflexion):**\n{}\n\n**Reflexion Patch:**\n{}", result, repair_obs));
                             }
                             Err(e) => eprintln!("   {} Reflexion Failed: {}", "⚠️".red(), e),
                         }
                     }
                     
                     session.with_message(format!("**Observation (Shell Result):**\n{}", result))
                 }
                 Err(e) => session.with_message(format!("**Observation (Shell Error):** Failed to start process: {}", e)),
             }
        }
        AgentAction::QueryMemory { query, .. } => {
            println!("{} 🧠 Querying Memory: {}", "🔍".magenta(), query);
            match state.memory_raw.recall(&query, 5).await {
                Ok(results) => {
                     let response = if results.is_empty() {
                         "No related documents found.".to_string()
                     } else {
                         results.join("\n\n---\n\n")
                     };
                     session.with_message(format!("**Observation (Memory Query):**\n{}", response))
                }
                Err(e) => {
                     session.with_message(format!("**Observation (Memory Error):** {}", e))
                }
            }
        }
        AgentAction::CommitOverlay { message } => {
            println!("{} 📦 Requesting Commit Approval: {}", "📦".yellow(), message);
            let mut io = state.io.lock().await;
            let _ = io.send_message(&format!("📦 **Commit Requested**: {}", message)).await;
            
            session.with_message(format!("**Status:** Awaiting manual approval for commit: *{}*", message))
                   .with_status(crate::core::session::SessionStatus::PendingCommit)
        }
        AgentAction::ViewGraph { node_id, depth } => {
            println!("{} 📊 Visualizing Graph Neighborhood: {} (depth: {})", "🗺️".blue(), node_id, depth);
            let viz = match state.memory_raw.get_visual_neighborhood(&node_id, depth).await {
                Ok(v) => v,
                Err(e) => format!("<b>Graph Error:</b> {}", e),
            };
            
            use crate::core::bus::SlyEvent;
            let _ = state.bus.publish(SlyEvent::Thought(session.id.clone(), viz)).await;
            session.with_message(format!("**Observation:** Graph neighborhood for '{}' visualized.", node_id))
        }
        AgentAction::Expand { path, symbol } => {
            println!("{} 📖 Expand: path='{}', symbol={:?}", "📖".blue(), path, symbol);
            match crate::knowledge::expand_symbol(&state.memory_raw, &path, symbol.as_deref()).await {
                Ok(content) => {
                    session.with_message(format!("**Observation (Symbol Expansion for '{}'):**\n```\n{}\n```", path, content))
                }
                Err(e) => session.with_message(format!("**Observation (Expansion Error):** {}", e))
            }
        }
        AgentAction::SearchCode { query, path_filter } => {
            println!("{} 🔍 SearchCode: '{}' (filter: {:?})", "🔍".blue(), query, path_filter);
            match state.memory_raw.recall(&query, 10).await {
                Ok(results) => {
                     session.with_message(format!("**Observation (Code Search for '{}'):**\n{}", query, results.join("\n---\n")))
                }
                Err(e) => session.with_message(format!("**Observation (Search Error):** {}", e))
            }
        }
        AgentAction::FinalResponse { title, summary } => {
            println!("{} 🏁 FinalResponse: {}", "🏁".green(), title);
            session.with_message(format!("**{}**\n\n{}", title, summary))
                   .with_status(crate::core::session::SessionStatus::Completed)
        }
        AgentAction::Answer { .. } => {
            session.with_status(crate::core::session::SessionStatus::Completed)
        }
    }
}
