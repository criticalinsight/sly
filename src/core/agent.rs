use crate::core::parser::{parse_action, AgentAction};
use crate::mcp::registry::{self, McpToolMetadata};
use colored::*;
use std::sync::Arc;
use std::collections::HashMap;
use crate::memory::MemoryStore;

pub async fn step_agent_session(
    session_id: String, 
    memory: Arc<crate::memory::Memory>,
    cortex: Arc<crate::core::cortex::Cortex>,
    mcp_clients: Arc<tokio::sync::Mutex<HashMap<String, Arc<crate::mcp::client::McpClient>>>>,
    metadata_cache: Arc<tokio::sync::Mutex<Vec<McpToolMetadata>>>,
    overlay: Arc<crate::safety::OverlayFS>,
    telegram: Option<Arc<tokio::sync::Mutex<crate::io::telegram::TelegramClient>>>,
    max_loops: usize,
) {
    let mut session = match memory.get_session(&session_id).await {
        Ok(Some(s)) => s,
        _ => return,
    };

    if session.depth >= max_loops {
        println!("{} Session {} reached max depth", "⚠️".red(), session_id);
        if let Some(tg) = telegram {
            let markup = crate::io::telegram::InlineKeyboardMarkup {
                inline_keyboard: vec![vec![
                    crate::io::telegram::InlineKeyboardButton { 
                        text: "⏭️ Proceed".to_string(), 
                        callback_data: format!("think:{}", session_id) 
                    },
                    crate::io::telegram::InlineKeyboardButton { 
                        text: "🛑 Stop".to_string(), 
                        callback_data: "stop".to_string() 
                    },
                ]]
            };
            let msg = format!("⚠️ <b>Max Loops Reached</b>\n\nSession <code>{}</code> reached its limit ({} steps).\n\nSelect 'Proceed' to extend for another cycle.", session_id, max_loops);
            let _ = tg.lock().await.send_message_with_markup(&msg, markup).await;
        }
        return;
    }

    // Phase 8: Atomic Checkpoint before speculation
    session = session.checkpoint();

    // 1. Fetch Metadata (Optimized Cached Step)
    let mut cache = metadata_cache.lock().await;
    if cache.is_empty() {
        println!("   {} Initializing MCP Tool Cache (Disabled to prevent hang)...", "📥".cyan());
        // *cache = registry::get_all_tool_metadata(&mcp_clients).await;
        *cache = vec![];
    }
    let tool_metadata = cache.clone();
    drop(cache); // Release lock early
    
    // 4. Atomic Checkpoint (Phase 8)
    let checkpoint = session.clone();

    let raw_context = session.messages.join("\n\n");
    let full_context = crate::core::pruner::LinguisticPruner::prune(&raw_context);
    let mut prompt = full_context;

    if session.depth == 0 {
        let tool_defs = registry::get_tool_definitions(&tool_metadata).await;
        if !tool_defs.is_empty() {
            prompt = format!("{}\n\n{}", prompt, tool_defs);
        }
        
        // Phase 9: Heuristic Persistence
        if let Ok(heuristics) = memory.recall_technical_heuristics(&session.id, 5).await {
            if !heuristics.is_empty() {
                prompt = format!("{}\n\n## PERSISTENT TECHNICAL HEURISTICS\n", prompt);
                for h in heuristics {
                    prompt = format!("{}* **Pattern:** {}\n", prompt, h.solution);
                }
            }
        }
        
        prompt = format!("{}\n\n## KNOWLEDGE GRAPH SCHEMA (Datalog Ready)\nNodes: `nodes {{ id => content, signature, type, path }}`\nEdges: `edges {{ parent => child }}`\n", prompt);
    }

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

    let stream_res = cortex.generate_stream(&prompt, level).await;
    let mut full_response = String::new();
    let mut last_tg_update = std::time::Instant::now();
    let mut tg_msg_id: Option<i64> = None;

    match stream_res {
        Ok(mut stream) => {
            print!("🤖 ");
            
            // Initial Telegram Message
            if let Some(tg) = &telegram {
                if let Ok(id) = tg.lock().await.send_message("🤖 <i>Thinking...</i>").await {
                    tg_msg_id = Some(id);
                }
            }

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(text) => {
                        print!("{}", text.as_str().green());
                        let _ = std::io::stdout().flush();
                        full_response.push_str(&text);
                        
                        // Live Stream to Telegram (Debounced)
                        if let Some(tg) = &telegram {
                            if let Some(msg_id) = tg_msg_id {
                                if last_tg_update.elapsed().as_secs() >= 2 {
                                    // Telegram limit is 4096 chars. Truncate head if too long for stream.
                                    let display_text = if full_response.len() > 3800 {
                                        format!("...{}", &full_response[full_response.len()-3800..])
                                    } else {
                                        full_response.clone()
                                    };
                                    
                                    let _ = tg.lock().await.edit_message_text(msg_id, &crate::io::telegram::html_escape(&display_text)).await;
                                    last_tg_update = std::time::Instant::now();
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("\n{} Streaming error: {}", "⚠️".red(), e);
                        break;
                    }
                }
            }
            println!();
            
            // Final Update ensures complete message is visible
            if let Some(tg) = &telegram {
                if let Some(msg_id) = tg_msg_id {
                     // If response is action-heavy (XML/JSON), maybe don't show all of it? 
                     // For now, show it all (truncated) to emulate chat.
                     let display_text = if full_response.len() > 3800 {
                        format!("...{}", &full_response[full_response.len()-3800..])
                     } else {
                        full_response.clone()
                     };
                     let _ = tg.lock().await.edit_message_text(msg_id, &crate::io::telegram::html_escape(&display_text)).await;
                }
            }
        }
        Err(e) => {
            eprintln!("{} Cortex stream start failed: {}", "⚠️".red(), e);
        }
    }

    if !full_response.is_empty() {
        let step_depth = session.depth;
        session = session.with_message(format!("**Sly (Step {}):**\n{}", step_depth, full_response.clone()))
                         .with_snapshot(checkpoint.messages.clone())
                         .with_depth_increment();
        
        match parse_action(&full_response) {
            Ok(actions) => {
                for action in actions {
                    session = handle_action(
                        action, 
                        session, 
                        memory.clone(), 
                        &tool_metadata, 
                        overlay.clone(), 
                        telegram.clone(),
                        mcp_clients.clone(),
                        metadata_cache.clone()
                    ).await;
                }
            }
            Err(e) => {
                eprintln!("Parse error: {}", e);
                session = session.with_status(crate::core::session::SessionStatus::Error(e.to_string()));
            }
        }
        let _ = memory.update_session(&session).await;

        // Final Notification Check for Completion or Errors
        if let Some(tg) = telegram {
            let status = session.status.clone();
            if matches!(status, crate::core::session::SessionStatus::Completed) || 
               matches!(status, crate::core::session::SessionStatus::Error(_)) {
                
                let title = if matches!(status, crate::core::session::SessionStatus::Completed) {
                    "🏁 <b>Task Completed</b>"
                } else {
                    "⚠️ <b>Session Interrupted</b>"
                };

                let summary = if full_response.len() > 300 {
                    format!("{}...", &full_response[..300])
                } else {
                    full_response.clone()
                };

                let markup = crate::io::telegram::InlineKeyboardMarkup {
                    inline_keyboard: vec![vec![
                        crate::io::telegram::InlineKeyboardButton { 
                            text: "⏪ Undo".to_string(), 
                            callback_data: format!("undo:{}", session.id) 
                        },
                        crate::io::telegram::InlineKeyboardButton { 
                            text: "📜 Logs".to_string(), 
                            callback_data: "logs".to_string() 
                        },
                    ]]
                };

                let msg = format!("{}\n\n<code>{}</code>\n\n{}", title, session.id, crate::io::telegram::html_escape(&summary));
                let _ = tg.lock().await.send_message_with_markup(&msg, markup).await;
            }
        }
    }
}

pub async fn step_thought_analysis(
    session_id: String,
    query: String,
    memory: Arc<crate::memory::Memory>,
    cortex: Arc<crate::core::cortex::Cortex>,
    mcp_clients: Arc<tokio::sync::Mutex<HashMap<String, Arc<crate::mcp::client::McpClient>>>>,
    metadata_cache: Arc<tokio::sync::Mutex<Vec<McpToolMetadata>>>,
    _overlay: Arc<crate::safety::OverlayFS>,
) {
    let mut cache = metadata_cache.lock().await;
    if cache.is_empty() {
        *cache = registry::get_all_tool_metadata(&mcp_clients).await;
    }
    let _tool_metadata = cache.clone();
    drop(cache);

    let prompt = format!("## SWARM ANALYSIS REQUEST\nSession ID: {}\nQuery: {}\n\nAnalyze the structural context and provide a brief technical insight. NO destructive actions. Focus on discovery.", session_id, query);

    match cortex.generate(&prompt, crate::core::cortex::ThinkingLevel::Low).await {
        Ok(response) => {
            if let Ok(Some(session)) = memory.get_session(&session_id).await {
                let observation = format!("**Swarm Analysis Insight:**\n{}", response);
                let session = session.with_message(observation);
                let _ = memory.update_session(&session).await;
            }
        }
        Err(e) => eprintln!("Swarm analysis error: {}", e),
    }
}

async fn handle_action(
    action: AgentAction, 
    session: crate::core::session::AgentSession, 
    memory: Arc<crate::memory::Memory>,
    tool_metadata: &[registry::McpToolMetadata],
    overlay: Arc<crate::safety::OverlayFS>,
    telegram: Option<Arc<tokio::sync::Mutex<crate::io::telegram::TelegramClient>>>,
    mcp_clients: Arc<tokio::sync::Mutex<HashMap<String, Arc<crate::mcp::client::McpClient>>>>,
    metadata_cache: Arc<tokio::sync::Mutex<Vec<registry::McpToolMetadata>>>,
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
             match execute_action(&overlay, fs_action) {
                 Ok(_) => {
                     // Collaborative Review Hook
                     if is_md {
                         if let Some(tg) = telegram {
                             let markup = crate::io::telegram::InlineKeyboardMarkup {
                                 inline_keyboard: vec![vec![
                                     crate::io::telegram::InlineKeyboardButton { text: "✅ Proceed".to_string(), callback_data: format!("think:{}", session.id) },
                                     crate::io::telegram::InlineKeyboardButton { text: "📝 Edit".to_string(), callback_data: format!("edit:{}", session.id) },
                                     crate::io::telegram::InlineKeyboardButton { text: "🔄 Regenerate".to_string(), callback_data: format!("think:{}", session.id) },
                                 ]]
                             };
                             let caption = format!("📑 <b>Document for Review</b>\nPath: <code>{}</code>\n\nWhat would you like to do?", path);
                             let _ = tg.lock().await.send_document(std::path::Path::new(&path), Some(&caption), Some(markup)).await;
                         }
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
                     if code != 0 && session.depth < 10 { // Arbitrary safety limit
                         let config = crate::core::state::SlyConfig::default(); // In real app, pass config down
                         match crate::core::reflexion::attempt_repair(
                             &session.id, 
                             &config, 
                             memory.clone(), 
                             crate::core::cortex::Cortex::new(config.clone(), "Reflexion".to_string()).unwrap().into(), // Simplified for brevity
                             mcp_clients.clone(),
                             metadata_cache.clone(),
                             overlay.clone(),
                             telegram.clone(),
                             &command,
                             &stderr
                         ).await {
                             Ok(repair_note) => {
                                 session.with_message(format!("**Observation (Shell '{}'):**\n```\n{}\n```\n\n{}", command, result, repair_note))
                             }
                             Err(e) => {
                                 eprintln!("Reflexion failed: {}", e);
                                 session.with_message(format!("**Observation (Shell '{}'):**\n```\n{}\n```", command, result))
                             }
                         }
                     } else {
                         session.with_message(format!("**Observation (Shell '{}'):**\n```\n{}\n```", command, result))
                     }
                 }
                 Err(e) => session.with_message(format!("**Observation (Error):** Command '{}' failed: {}", command, e)),
             }
        }
        AgentAction::QueryMemory { query, .. } => {
            println!("{} 🧠 Querying Memory: {}", "🔍".magenta(), query);
            match memory.recall(&query, 5).await {
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
            if let Some(tg) = telegram {
                println!("{} 🔀 Requesting Commit Approval via Telegram...", "🔔".yellow());
                let markup = crate::io::telegram::InlineKeyboardMarkup {
                    inline_keyboard: vec![vec![
                        crate::io::telegram::InlineKeyboardButton {
                            text: "Confirm ✅".to_string(),
                            callback_data: format!("commit:{}", session.id),
                        },
                        crate::io::telegram::InlineKeyboardButton {
                            text: "Abort ❌".to_string(),
                            callback_data: format!("abort:{}", session.id),
                        }
                    ]]
                };
                let notify_text = format!("<b>🔔 Action Required</b>\n\nSly wants to commit the current overlay in session <code>{}</code>\n\n<b>Message:</b> <i>{}</i>", session.id, message);
                let _ = tg.lock().await.send_message_with_markup(&notify_text, markup).await;
                
                session.with_message(format!("**Status:** Awaiting manual approval for commit: *{}*", message))
                       .with_status(crate::core::session::SessionStatus::PendingCommit)
            } else {
                println!("{} 🚀 Committing Overlay (No Telegram): {}", "📦".green().bold(), message);
                match overlay.commit() {
                    Ok(_) => {
                        let _ = crate::io::haptics::HapticSystem::success_pulse();
                        session.with_message("**Observation:** Overlay committed successfully.".to_string())
                               .with_status(crate::core::session::SessionStatus::Completed)
                    }
                    Err(e) => {
                        let _ = crate::io::haptics::HapticSystem::failure_pulse();
                        session.with_message(format!("**Observation (Commit Error):** {}", e))
                    }
                }
            }
        }
        AgentAction::ViewGraph { node_id, depth } => {
            println!("{} 📊 Visualizing Graph Neighborhood: {} (depth: {})", "🗺️".blue(), node_id, depth);
            let viz = match memory.get_visual_neighborhood(&node_id, depth).await {
                Ok(v) => v,
                Err(e) => format!("<b>Graph Error:</b> {}", e),
            };
            
            if let Some(tg) = telegram {
                let _ = tg.lock().await.send_message(&viz).await;
                session.with_message(format!("**Observation:** Graph neighborhood for '{}' projected to Telegram.", node_id))
            } else {
                println!("{}", viz);
                session.with_message(format!("**Observation:** Graph neighborhood for '{}' visualized in console.", node_id))
            }
        }
        AgentAction::Expand { path, symbol } => {
            println!("{} 🔍 Expanding: {} {:?}", "📖".cyan(), path, symbol);
            
            // Read the file content
            let workspace_root = std::env::current_dir().unwrap_or_default();
            let full_path = workspace_root.join(&path);
            
            let expansion = if full_path.exists() {
                match std::fs::read_to_string(&full_path) {
                    Ok(content) => {
                        if let Some(sym) = symbol {
                            // Extract specific symbol
                            extract_symbol_content(&content, &sym)
                        } else {
                            // Return compressed version of full file
                            let ext = crate::knowledge::SymbolicCompressor::extension_from_path(&path);
                            let compressed = crate::knowledge::SymbolicCompressor::compress(&content, ext);
                            format!("## Symbolic View: {}\n```\n{}\n```\n\nUse `Expand` with `symbol` to get full implementation.", path, compressed)
                        }
                    }
                    Err(e) => format!("**Error:** Could not read {}: {}", path, e),
                }
            } else {
                format!("**Error:** File not found: {}", path)
            };
            
            if let Some(tg) = telegram {
                let _ = tg.lock().await.send_message(&format!("<b>Expansion:</b>\n<pre>{}</pre>", 
                    crate::io::telegram::html_escape(&expansion))).await;
            }
            
            session.with_message(format!("**Observation (Expand):**\n{}", expansion))
        }
        AgentAction::Answer { .. } => {
            session.with_status(crate::core::session::SessionStatus::Completed)
        }
    }
}

/// Extract a specific symbol's content from source code
fn extract_symbol_content(content: &str, symbol: &str) -> String {
    // Parse symbol type and name (e.g., "fn:login" or "struct:User")
    let parts: Vec<&str> = symbol.split(':').collect();
    let (sym_type, sym_name) = if parts.len() >= 2 {
        (parts[0], parts[1])
    } else {
        ("", symbol)
    };

    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();
    let mut in_block = false;
    let mut brace_count = 0;
    let mut start_line = 0;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        
        // Check if this line starts our target symbol
        let is_target = match sym_type {
            "fn" => trimmed.contains(&format!("fn {}", sym_name)) || 
                    trimmed.contains(&format!("fn {}(", sym_name)),
            "struct" => trimmed.contains(&format!("struct {}", sym_name)),
            "enum" => trimmed.contains(&format!("enum {}", sym_name)),
            "trait" => trimmed.contains(&format!("trait {}", sym_name)),
            "impl" => trimmed.starts_with("impl") && trimmed.contains(sym_name),
            "class" => trimmed.contains(&format!("class {}", sym_name)),
            "def" => trimmed.contains(&format!("def {}", sym_name)),
            "function" => trimmed.contains(&format!("function {}", sym_name)),
            _ => trimmed.contains(sym_name),
        };

        if is_target && !in_block {
            in_block = true;
            start_line = i + 1;
            brace_count = 0;
        }

        if in_block {
            result.push_str(line);
            result.push('\n');

            // Count braces to find end of block
            for c in line.chars() {
                match c {
                    '{' => brace_count += 1,
                    '}' => brace_count -= 1,
                    _ => {}
                }
            }

            // For languages without braces (Python), stop at dedent
            if sym_type == "def" || sym_type == "class" {
                if i > start_line && !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
                    break;
                }
            } else if brace_count == 0 && result.contains('{') {
                // Block ended
                break;
            }
        }
    }

    if result.is_empty() {
        format!("Symbol '{}' not found in file", symbol)
    } else {
        format!("## {} (lines {}-{})\n```\n{}\n```", symbol, start_line, start_line + result.lines().count(), result)
    }
}
