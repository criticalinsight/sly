use crate::memory::GraphNode;
use regex::Regex;

pub struct Extractor;

impl Extractor {
    pub fn extract_symbols(content: &str, ext: &str, path_str: &str) -> Vec<GraphNode> {
        let mut all_nodes = Vec::new();

        // Always add a "file" node for the full content
        all_nodes.push(GraphNode {
            id: format!("file:{}", path_str),
            content: content.to_string(),
            signature: path_str.to_string(),
            node_type: "file".to_string(),
            path: path_str.to_string(),
            edges: Vec::new(),
        });

        // Use Regex for all extractions to remove AST dependency weight
        all_nodes.extend(Self::extract_regex(content, ext, path_str));

        all_nodes
    }

    fn extract_regex(content: &str, ext: &str, path_str: &str) -> Vec<GraphNode> {
        let mut nodes = Vec::new();

        match ext {
            "rs" => {
                let re_fn = Regex::new(r"(?m)^(?:pub(?:\(.*\))?\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)").unwrap();
                let re_struct = Regex::new(r"(?m)^(?:pub(?:\(.*\))?\s+)?struct\s+([a-zA-Z0-9_]+)").unwrap();
                let re_enum = Regex::new(r"(?m)^(?:pub(?:\(.*\))?\s+)?enum\s+([a-zA-Z0-9_]+)").unwrap();
                let re_trait = Regex::new(r"(?m)^(?:pub(?:\(.*\))?\s+)?trait\s+([a-zA-Z0-9_]+)").unwrap();
                let re_impl = Regex::new(r"(?m)^impl(?:\s+<.*>)?\s+([a-zA-Z0-9_:]+)(?:\s+for\s+([a-zA-Z0-9_:]+))?").unwrap();
                let re_use = Regex::new(r"(?m)^use\s+([a-zA-Z0-9_:]+)").unwrap();

                // For dependency tracking, we find all "CamelCase" or "PascalCase" words as potential type references
                let re_type_ref = Regex::new(r"\b([A-Z][a-zA-Z0-9_]+)\b").unwrap();

                for cap in re_fn.captures_iter(content) {
                    let mut edges = Vec::new();
                    // Basic internal dependency: find other symbols mentioned in the function body
                    for t_cap in re_type_ref.captures_iter(&cap[0]) {
                         edges.push(format!("struct:{}", &t_cap[1]));
                    }

                    nodes.push(GraphNode {
                        id: format!("fn:{}", &cap[1]),
                        content: format!("Function definition: {}", &cap[0]),
                        signature: cap[0].to_string(),
                        node_type: "fn".to_string(),
                        path: path_str.to_string(),
                        edges,
                    });
                }
                for cap in re_struct.captures_iter(content) {
                    nodes.push(GraphNode {
                        id: format!("struct:{}", &cap[1]),
                        content: format!("Struct definition: {}", &cap[0]),
                        signature: cap[0].to_string(),
                        node_type: "struct".to_string(),
                        path: path_str.to_string(),
                        edges: Vec::new(),
                    });
                }
                for cap in re_enum.captures_iter(content) {
                    nodes.push(GraphNode {
                        id: format!("enum:{}", &cap[1]),
                        content: format!("Enum definition: {}", &cap[0]),
                        signature: cap[0].to_string(),
                        node_type: "enum".to_string(),
                        path: path_str.to_string(),
                        edges: Vec::new(),
                    });
                }
                for cap in re_trait.captures_iter(content) {
                    nodes.push(GraphNode {
                        id: format!("trait:{}", &cap[1]),
                        content: format!("Trait definition: {}", &cap[0]),
                        signature: cap[0].to_string(),
                        node_type: "trait".to_string(),
                        path: path_str.to_string(),
                        edges: Vec::new(),
                    });
                }
                for cap in re_impl.captures_iter(content) {
                    let mut edges = Vec::new();
                    let name = if let Some(target) = cap.get(2) {
                        edges.push(format!("trait:{}", cap.get(1).map(|m| m.as_str()).unwrap_or("")));
                        edges.push(format!("struct:{}", target.as_str()));
                        format!("{} for {}", cap.get(1).map(|m| m.as_str()).unwrap_or(""), target.as_str())
                    } else {
                        edges.push(format!("struct:{}", cap.get(1).map(|m| m.as_str()).unwrap_or("")));
                        cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string()
                    };
                    nodes.push(GraphNode {
                        id: format!("impl:{}", name.replace(" ", "_")),
                        content: cap[0].to_string(),
                        signature: cap[0].to_string(),
                        node_type: "impl".to_string(),
                        path: path_str.to_string(),
                        edges,
                    });
                }
                // File level dependencies
                for cap in re_use.captures_iter(content) {
                    nodes.push(GraphNode {
                        id: format!("use:{}", &cap[1].replace("::", "_")),
                        content: cap[0].to_string(),
                        signature: cap[1].to_string(),
                        node_type: "import".to_string(),
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
                        signature: cap[1].to_string(),
                        node_type: "markdown_heading".to_string(),
                        path: path_str.to_string(),
                        edges: Vec::new(),
                    });
                }
            }
            _ => {
                nodes.push(GraphNode {
                    id: format!("file:{}", path_str),
                    content: content.chars().take(200).collect(),
                    signature: path_str.to_string(),
                    node_type: "file".to_string(),
                    path: path_str.to_string(),
                    edges: Vec::new(),
                });
            }
        }
        nodes
    }
}
