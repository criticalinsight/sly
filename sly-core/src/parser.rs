use crate::error::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum AgentAction {
    WriteFile { path: String, content: String },
    ExecShell { command: String },
    FinalResponse { title: String, summary: String },
    Answer { text: String },
}

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
            break;
        }
    }
    
    if actions.is_empty() {
        Ok(vec![AgentAction::Answer { text: response.to_string() }])
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
