use crate::knowledge::scanner::FileValue;
use crate::memory::GraphNode;
use crate::knowledge::parser;
use regex::Regex;

pub struct Extractor;

impl Extractor {
    pub fn extract_symbols(file: &FileValue) -> Vec<GraphNode> {
        let path_str = file.path.to_str().unwrap_or_default();
        let ext = &file.extension;
        let content = &file.content;

        if ext == "rs" {
            match parser::parse_rust(content) {
                Ok(nodes) => {
                     nodes.into_iter().map(|n| GraphNode {
                         id: n.id,
                         content: n.content,
                         node_type: n.kind,
                         path: path_str.to_string(),
                         edges: n.edges,
                     }).collect()
                },
                Err(e) => {
                    eprintln!("AST Parse failed for {}, falling back to Regex: {}", path_str, e);
                    Self::extract_regex(content, ext, path_str)
                }
            }
        } else {
            Self::extract_regex(content, ext, path_str)
        }
    }

    fn extract_regex(content: &str, ext: &str, path_str: &str) -> Vec<GraphNode> {
        let mut nodes = Vec::new();

        // Regex logic moved from knowledge.rs
        match ext {
            "rs" => {
                let re_fn = Regex::new(r"pub\s+fn\s+([a-zA-Z0-9_]+)").unwrap();
                let re_struct = Regex::new(r"pub\s+struct\s+([a-zA-Z0-9_]+)").unwrap();

                for cap in re_fn.captures_iter(content) {
                    nodes.push(GraphNode {
                        id: format!("fn:{}", &cap[1]),
                        content: format!("Function definition: {}", &cap[0]),
                        node_type: "fn".to_string(),
                        path: path_str.to_string(),
                        edges: Vec::new(),
                    });
                }
                for cap in re_struct.captures_iter(content) {
                    nodes.push(GraphNode {
                        id: format!("struct:{}", &cap[1]),
                        content: format!("Struct definition: {}", &cap[0]),
                        node_type: "struct".to_string(),
                        path: path_str.to_string(),
                        edges: Vec::new(),
                    });
                }
            }
            "md" => {
                let re_h1 = Regex::new(r"(?m)^#\s+(.+)$").unwrap();
                for cap in re_h1.captures_iter(content) {
                    nodes.push(GraphNode {
                        id: format!("doc:{}", &cap[1].to_lowercase().replace(" ", "_")),
                        content: cap[0].to_string(),
                        node_type: "markdown_heading".to_string(),
                        path: path_str.to_string(),
                        edges: Vec::new(),
                    });
                }
            }
            _ => {
                // Generic node for unknown files
                nodes.push(GraphNode {
                    id: format!("file:{}", path_str),
                    content: content.chars().take(200).collect(),
                    node_type: "file".to_string(),
                    path: path_str.to_string(),
                    edges: Vec::new(),
                });
            }
        }
        nodes
    }
}
