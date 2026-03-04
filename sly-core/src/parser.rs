//! Zero-Serde JSON action extraction.
//!
//! Parses LLM responses into [`AgentAction`] variants without any
//! serialization library. Handles well-formed markdown JSON blocks,
//! truncated blocks (parser hardening), and raw JSON fallback.

use crate::error::Result;

/// An atomic action the agent can take.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentAction {
    /// Write content to a file path via the overlay.
    WriteFile { path: String, content: String },
    /// Read a file from the scratchpad.
    ReadFile { path: String },
    /// Execute a shell command via `sh -c`.
    ExecShell { command: String },
    /// Signal task completion with a title and summary.
    FinalResponse { title: String, summary: String },
    /// Return a plain-text answer to the user.
    Answer { text: String },
    /// Invalid JSON — parsed but missing `directive` key.
    InvalidJson { raw: String },
}

/// Extract [`AgentAction`]s from an LLM response string.
///
/// Tries three strategies in order:
/// 1. Parse `` ```json ... ``` `` fenced blocks.
/// 2. Parse truncated blocks (no closing fence — parser hardening).
/// 3. Treat the entire response as raw JSON.
/// 4. Fall back to [`AgentAction::Answer`] with the raw text.
pub fn parse_action(response: &str) -> Result<Vec<AgentAction>> {
    let mut actions = Vec::new();
    let mut start_idx = 0;

    while let Some(start) = response[start_idx..].find("```json") {
        let abs_start = start_idx + start + 7;
        if let Some(end) = response[abs_start..].find("```") {
            let abs_end = abs_start + end;
            let json_str = &response[abs_start..abs_end].trim();
            
            if let Some(action) = manual_parse_json_action(json_str) {
                actions.push(action);
            }
            start_idx = abs_end + 3;
        } else {
            // Parser Hardening: No closing block, parse remaining
            let json_str = &response[abs_start..].trim();
            if let Some(action) = manual_parse_json_action(json_str) {
                actions.push(action);
            }
            break;
        }
    }
    
    // Parser Hardening: Fallback if raw JSON or simple dict was sent
    if actions.is_empty() {
        if let Some(action) = manual_parse_json_action(response) {
            actions.push(action);
            Ok(actions)
        } else if serde_json::from_str::<serde_json::Value>(response).is_ok() {
            // Valid JSON but no directive — signal retry
            Ok(vec![AgentAction::InvalidJson { raw: response.to_string() }])
        } else {
            Ok(vec![AgentAction::Answer { text: response.to_string() }])
        }
    } else {
        Ok(actions)
    }
}

fn manual_parse_json_action(json_str: &str) -> Option<AgentAction> {
    let data: serde_json::Value = serde_json::from_str(json_str).ok()?;
    
    let directive = data.get("directive")?.as_str()?;
    
    match directive {
        "WriteFile" => {
            let path = data.get("path")?.as_str()?.to_string();
            let content = data.get("content")?.as_str()?.to_string();
            Some(AgentAction::WriteFile { path, content })
        },
        "ExecShell" => {
            let command = data.get("command")?.as_str()?.to_string();
            Some(AgentAction::ExecShell { command })
        },
        "Answer" => {
            let text = data.get("text")?.as_str()?.to_string();
            Some(AgentAction::Answer { text })
        },
        "ReadFile" => {
            let path = data.get("path")?.as_str()?.to_string();
            Some(AgentAction::ReadFile { path })
        },
        "FinalResponse" => {
            let title = data.get("title")?.as_str()?.to_string();
            let summary = data.get("summary")?.as_str()?.to_string();
            Some(AgentAction::FinalResponse { title, summary })
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_action ---

    #[test]
    fn test_parse_exec_shell() {
        let response = "Sure, let me run that.\n```json\n{\"directive\": \"ExecShell\", \"command\": \"cargo build\"}\n```";
        let actions = parse_action(response).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], AgentAction::ExecShell { command: "cargo build".to_string() });
    }

    #[test]
    fn test_parse_write_file() {
        let response = "```json\n{\"directive\": \"WriteFile\", \"path\": \"main.rs\", \"content\": \"fn main() {}\"}\n```";
        let actions = parse_action(response).unwrap();
        assert_eq!(actions[0], AgentAction::WriteFile { path: "main.rs".to_string(), content: "fn main() {}".to_string() });
    }

    #[test]
    fn test_parse_final_response() {
        let response = "```json\n{\"directive\": \"FinalResponse\", \"title\": \"Done\", \"summary\": \"All good\"}\n```";
        let actions = parse_action(response).unwrap();
        assert_eq!(actions[0], AgentAction::FinalResponse { title: "Done".to_string(), summary: "All good".to_string() });
    }

    #[test]
    fn test_parse_plain_text_fallback() {
        let response = "I don't know how to do that.";
        let actions = parse_action(response).unwrap();
        assert_eq!(actions[0], AgentAction::Answer { text: response.to_string() });
    }

    #[test]
    fn test_parse_truncated_json_block() {
        // Parser Hardening: no closing ```
        let response = "```json\n{\"directive\": \"ExecShell\", \"command\": \"echo hi\"}";
        let actions = parse_action(response).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], AgentAction::ExecShell { command: "echo hi".to_string() });
    }

    #[test]
    fn test_parse_raw_json_fallback() {
        // Parser Hardening: raw JSON without markdown block
        let response = r#"{"directive": "Answer", "text": "42"}"#;
        let actions = parse_action(response).unwrap();
        assert_eq!(actions[0], AgentAction::Answer { text: "42".to_string() });
    }

    #[test]
    fn test_parse_multiple_actions() {
        let response = "```json\n{\"directive\": \"ExecShell\", \"command\": \"ls\"}\n```\nOk now:\n```json\n{\"directive\": \"ExecShell\", \"command\": \"pwd\"}\n```";
        let actions = parse_action(response).unwrap();
        assert_eq!(actions.len(), 2);
    }
}
