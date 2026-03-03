use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "directive", rename_all = "PascalCase")]
pub enum AgentAction {
    WriteFile { path: String, content: String },
    ExecShell { command: String, context: String },
    QueryMemory { query: String, strategy: Option<String> },
    SearchCode { query: String, path_filter: Option<String> },

    CommitOverlay { message: String },
    CallTool { tool_name: String, arguments: Value },
    ViewGraph { node_id: String, depth: usize },
    
    /// Expand a symbolic reference to full implementation
    /// Use when detailed implementation context is needed
    Expand { path: String, symbol: Option<String> },
    
    FinalResponse { title: String, summary: String },
    Answer { text: String },
}


pub fn parse_action(response: &str) -> Result<Vec<AgentAction>> {
    let mut actions = Vec::new();
    let mut start_idx = 0;

    // A. Scan for multiple ```json blocks
    while let Some(start) = response[start_idx..].find("```json") {
        let abs_start = start_idx + start + 7;
        if let Some(end) = response[abs_start..].find("```") {
            let abs_end = abs_start + end;
            let json_str = &response[abs_start..abs_end].trim();
            
            // Try parse as Vector
            if let Ok(mut acts) = serde_json::from_str::<Vec<AgentAction>>(json_str) {
                actions.append(&mut acts);
            } 
            // Try parse as Single Object
            else if let Ok(act) = serde_json::from_str::<AgentAction>(json_str) {
                actions.push(act);
            }
            
            start_idx = abs_end + 3;
        } else {
            break;
        }
    }
    
    // B. Scan for generic ``` blocks if no json blocks found
    if actions.is_empty() {
        start_idx = 0;
        while let Some(start) = response[start_idx..].find("```") {
            // Check if it's already handled (json) - primitive check
            let snippet = &response[start_idx + start..];
            if snippet.starts_with("```json") {
                 start_idx += start + 7; // Skip
                 continue; 
            }
            
            let abs_start = start_idx + start + 3;
            if let Some(end) = response[abs_start..].find("```") {
                let abs_end = abs_start + end;
                let content = &response[abs_start..abs_end].trim();
                // Try parse
                if let Ok(act) = serde_json::from_str::<AgentAction>(content) {
                    actions.push(act);
                } else if let Ok(mut acts) = serde_json::from_str::<Vec<AgentAction>>(content) {
                    actions.append(&mut acts);
                }
                start_idx = abs_end + 3;
            } else {
                break;
            }
        }
    }

    if !actions.is_empty() {
        return Ok(actions);
    }
    
    // C. Fallback to whole text if looking like JSON
    let trimmed = response.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
         if let Ok(act) = serde_json::from_str::<AgentAction>(trimmed) {
             return Ok(vec![act]);
         }
         if let Ok(acts) = serde_json::from_str::<Vec<AgentAction>>(trimmed) {
             return Ok(acts);
         }
         // Error if it looks like JSON but matches nothing
         // return Err(anyhow!("Failed to parse structured directive"));
    }

    Ok(vec![AgentAction::Answer { text: response.to_string() }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_write_file() {
        let resp = r#"
Sure, here is the file:
```json
{
  "directive": "WriteFile",
  "path": "src/main.rs",
  "content": "fn main() {}"
}
```
        "#;
        let actions = parse_action(resp).unwrap();
        match &actions[0] {
            AgentAction::WriteFile { path, content } => {
                assert_eq!(path, "src/main.rs");
                assert_eq!(content, "fn main() {}");
            },
            _ => panic!("Wrong type"),
        }
    }

    #[test]
    fn test_parse_mcp_call() {
        let resp = r#"```json
{
  "directive": "CallTool",
  "tool_name": "list_files",
  "arguments": { "path": "." }
}
```"#;
        let actions = parse_action(resp).unwrap();
        match &actions[0] {
            AgentAction::CallTool { tool_name, arguments } => {
                assert_eq!(tool_name, "list_files");
                assert_eq!(arguments["path"], ".");
            },
            _ => panic!("Wrong type"),
        }
    }

    #[test]
    fn test_parse_multiple_actions() {
        let resp = r#"
Here are two actions:
```json
[
  {
    "directive": "WriteFile",
    "path": "test.txt",
    "content": "hello"
  },
  {
    "directive": "ExecShell",
    "command": "ls",
    "context": "testing"
  }
]
```
        "#;
        let actions = parse_action(resp).unwrap();
        assert_eq!(actions.len(), 2);
        if let AgentAction::WriteFile { path, .. } = &actions[0] {
            assert_eq!(path, "test.txt");
        } else {
            panic!("First action should be WriteFile");
        }
        if let AgentAction::ExecShell { command, .. } = &actions[1] {
            assert_eq!(command, "ls");
        } else {
            panic!("Second action should be ExecShell");
        }
    }

    #[test]
    fn test_parse_multiple_blocks() {
        let resp = r#"
First block:
```json
{
  "directive": "Answer",
  "text": "Hello"
}
```
Second block:
```json
{
  "directive": "CommitOverlay",
  "message": "fix: bug"
}
```
        "#;
        let actions = parse_action(resp).unwrap();
        assert_eq!(actions.len(), 2);
        match (&actions[0], &actions[1]) {
            (AgentAction::Answer { .. }, AgentAction::CommitOverlay { .. }) => {},
            _ => panic!("Wrong actions: {:?}", actions),
        }
    }

    #[test]
    fn test_parse_plain_text_fallback() {
        let resp = "This is just some text, not JSON.";
        let actions = parse_action(resp).unwrap();
        assert_eq!(actions.len(), 1);
        if let AgentAction::Answer { text } = &actions[0] {
            assert_eq!(text, resp);
        } else {
            panic!("Should fallback to Answer");
        }
    }

    #[test]
    fn test_parse_expand_directive() {
        let resp = r#"```json
{
  "directive": "Expand",
  "path": "src/auth.rs",
  "symbol": "fn:login"
}
```"#;
        let actions = parse_action(resp).unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            AgentAction::Expand { path, symbol } => {
                assert_eq!(path, "src/auth.rs");
                assert_eq!(symbol, &Some("fn:login".to_string()));
            },
            _ => panic!("Should parse as Expand"),
        }
    }

    #[test]
    fn test_parse_expand_without_symbol() {
        let resp = r#"```json
{
  "directive": "Expand",
  "path": "src/models.rs"
}
```"#;
        let actions = parse_action(resp).unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            AgentAction::Expand { path, symbol } => {
                assert_eq!(path, "src/models.rs");
                assert!(symbol.is_none());
            },
            _ => panic!("Should parse as Expand"),
        }
    }
}
