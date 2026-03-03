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
    /// Execute a shell command via `sh -c`.
    ExecShell { command: String },
    /// Signal task completion with a title and summary.
    FinalResponse { title: String, summary: String },
    /// Return a plain-text answer to the user.
    Answer { text: String },
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
        } else {
            Ok(vec![AgentAction::Answer { text: response.to_string() }])
        }
    } else {
        Ok(actions)
    }
}

fn manual_parse_json_action(json_str: &str) -> Option<AgentAction> {
    let directive = find_json_val(json_str, "directive")?;
    
    match directive.as_str() {
        "WriteFile" => {
            let path = find_json_val(json_str, "path")?;
            let content = find_json_val(json_str, "content")?;
            Some(AgentAction::WriteFile { path, content })
        },
        "ExecShell" => {
            let command = find_json_val(json_str, "command")?;
            Some(AgentAction::ExecShell { command })
        },
        "Answer" => {
            let text = find_json_val(json_str, "text")?;
            Some(AgentAction::Answer { text })
        },
        "FinalResponse" => {
            let title = find_json_val(json_str, "title")?;
            let summary = find_json_val(json_str, "summary")?;
            Some(AgentAction::FinalResponse { title, summary })
        },
        _ => None,
    }
}

/// Extract the string value for `key` from a JSON-like string.
///
/// This is the Zero-Serde core: a simple `"key": "value"` scanner.
pub fn find_json_val(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    if let Some(idx) = json.find(&pattern) {
        let after = &json[idx + pattern.len()..];
        if let Some(colon) = after.find(':') {
            let val_part = &after[colon + 1..].trim();
            if val_part.starts_with('"') {
                let (val, _) = extract_json_string(&val_part[1..]);
                return Some(val);
            }
        }
    }
    None
}

/// Extract a JSON string literal, handling escape sequences.
///
/// Returns `(decoded_string, bytes_consumed)`. Stops at the
/// closing `"` or end-of-input.
pub fn extract_json_string(s: &str) -> (String, usize) {
    let mut result = String::new();
    let mut escaped = false;
    let mut bytes_read = 0;
    for (i, c) in s.char_indices() {
        if escaped {
            match c {
                'n' => result.push('\n'),
                'r' => result.push('\r'),
                't' => result.push('\t'),
                '\\' => result.push('\\'),
                '"' => result.push('"'),
                _ => result.push(c),
            }
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            return (result, i + 1);
        } else {
            result.push(c);
        }
        bytes_read = i + c.len_utf8();
    }
    (result, bytes_read)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- extract_json_string ---

    #[test]
    fn test_extract_simple_string() {
        let (val, len) = extract_json_string(r#"hello""#);
        assert_eq!(val, "hello");
        assert_eq!(len, 6);
    }

    #[test]
    fn test_extract_escaped_newline() {
        let (val, _) = extract_json_string(r#"line1\nline2""#);
        assert_eq!(val, "line1\nline2");
    }

    #[test]
    fn test_extract_escaped_quote() {
        let (val, _) = extract_json_string(r#"say \"hi\"""#);
        assert_eq!(val, "say \"hi\"");
    }

    #[test]
    fn test_extract_escaped_backslash() {
        let (val, _) = extract_json_string(r#"path\\to\\file""#);
        assert_eq!(val, "path\\to\\file");
    }

    #[test]
    fn test_extract_empty_string() {
        let (val, len) = extract_json_string(r#"""#);
        assert_eq!(val, "");
        assert_eq!(len, 1);
    }

    // --- find_json_val ---

    #[test]
    fn test_find_json_val_basic() {
        let json = r#"{"directive": "ExecShell", "command": "ls -la"}"#;
        assert_eq!(find_json_val(json, "directive"), Some("ExecShell".to_string()));
        assert_eq!(find_json_val(json, "command"), Some("ls -la".to_string()));
    }

    #[test]
    fn test_find_json_val_missing_key() {
        let json = r#"{"directive": "ExecShell"}"#;
        assert_eq!(find_json_val(json, "nonexistent"), None);
    }

    #[test]
    fn test_find_json_val_with_spaces() {
        let json = r#"{ "directive" : "Answer" , "text" : "hello world" }"#;
        assert_eq!(find_json_val(json, "directive"), Some("Answer".to_string()));
        assert_eq!(find_json_val(json, "text"), Some("hello world".to_string()));
    }

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
