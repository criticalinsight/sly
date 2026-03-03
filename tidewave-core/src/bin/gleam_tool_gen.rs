use std::env;
use std::fs;
use std::path::Path;
use serde_json::json;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: gleam_tool_gen <project_path>");
        std::process::exit(1);
    }

    let project_path = Path::new(&args[1]);
    let src_path = project_path.join("src");

    if !src_path.exists() {
        eprintln!("Source directory not found: {:?}", src_path);
        std::process::exit(1);
    }

    let mut tools = Vec::new();

    // Walk through src directory and find .gleam files
    if let Ok(entries) = fs::read_dir(src_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("gleam") {
                if let Ok(content) = fs::read_to_string(&path) {
                    extract_tools(&content, &mut tools);
                }
            }
        }
    }

    println!("{}", serde_json::to_string_pretty(&tools).unwrap());
}

fn extract_tools(content: &str, tools: &mut Vec<serde_json::Value>) {
    // Basic regex-free parsing for pub fn
    for line in content.lines() {
        if line.trim().starts_with("pub fn") {
            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            if parts.len() >= 3 {
                let name = parts[2].split('(').next().unwrap_or("");
                if !name.is_empty() {
                    tools.push(json!({
                        "name": name,
                        "description": format!("Gleam function: {}", name),
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    }));
                }
            }
        }
    }
}
