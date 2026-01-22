use anyhow::{Context, Result};
use syn::{visit::Visit, ItemFn, ItemStruct, ItemEnum, ItemImpl, ItemTrait, Type};
use quote::ToTokens;

#[derive(Debug, Clone)]
pub struct ExtractedNode {
    pub id: String,
    pub kind: String, // "fn", "struct", "enum", "impl", "trait"
    pub signature: String,
    pub content: String,
    pub edges: Vec<String>, // "Referenced" identifiers
}

pub struct RustVisitor {
    pub nodes: Vec<ExtractedNode>,
    pub current_scope: Vec<String>, // Stack of scopes: ["module", "struct_name"]
}

impl RustVisitor {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            current_scope: Vec::new(),
        }
    }

    fn generate_id(&self, name: &str, kind: &str) -> String {
        // e.g., "fn:my_func" or "struct:MyStruct"
        // Ideally should include module path, but for single file parse, simple ID is okay
        // If we have scope, prepend it?
        if self.current_scope.is_empty() {
            format!("{}:{}", kind, name)
        } else {
             format!("{}:{}:{}", kind, self.current_scope.join("::"), name)
        }
    }
}

impl<'ast> Visit<'ast> for RustVisitor {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let name = node.sig.ident.to_string();
        let id = self.generate_id(&name, "fn");
        
        let signature = node.sig.to_token_stream().to_string();
        
        // Naive content extraction: formatting the whole item
        // Note: For large functions this might be big.
        let content = node.to_token_stream().to_string();

        let mut extracted = ExtractedNode {
            id: id.clone(),
            kind: "fn".to_string(),
            signature,
            content,
            edges: Vec::new(),
        };

        // Heuristic Edge Detection: Scan body for identifiers
        // This is "loose" detection.
        let body_str = node.block.to_token_stream().to_string();
        extracted.edges = extract_potential_edges(&body_str);

        self.nodes.push(extracted);

        // Recurse? Fn inside Fn?
        // self.current_scope.push(name);
        // syn::visit::visit_item_fn(self, node);
        // self.current_scope.pop();
    }

    fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
        let name = node.ident.to_string();
        let id = self.generate_id(&name, "struct");
        
        let content = node.to_token_stream().to_string();
        
        self.nodes.push(ExtractedNode {
            id,
            kind: "struct".to_string(),
            signature: format!("struct {}", name),
            content,
            edges: Vec::new(),
        });
    }

    fn visit_item_enum(&mut self, node: &'ast ItemEnum) {
        let name = node.ident.to_string();
        let id = self.generate_id(&name, "enum");
        
        let content = node.to_token_stream().to_string();
        
        self.nodes.push(ExtractedNode {
            id,
            kind: "enum".to_string(),
            signature: format!("enum {}", name),
            content,
            edges: Vec::new(),
        });
    }

    fn visit_item_trait(&mut self, node: &'ast ItemTrait) {
        let name = node.ident.to_string();
        let id = self.generate_id(&name, "trait");
        
        let content = node.to_token_stream().to_string();
        
        self.nodes.push(ExtractedNode {
            id,
            kind: "trait".to_string(),
            signature: format!("trait {}", name),
            content,
            edges: Vec::new(),
        });
    }
    
    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        // Impl blocks are tricky. They don't have a single "name".
        // They are "impl Type" or "impl Trait for Type".
        
        let type_name = match &*node.self_ty {
            Type::Path(type_path) => type_path.path.segments.last().map(|s| s.ident.to_string()).unwrap_or("Unknown".to_string()),
            _ => "Unknown".to_string(),
        };
        
        let trait_name = if let Some((_, path, _)) = &node.trait_ {
            path.segments.last().map(|s| s.ident.to_string())
        } else {
            None
        };
        
        let (name, kind) = if let Some(t) = trait_name {
            (format!("{} for {}", t, type_name), "impl_trait")
        } else {
            (type_name.clone(), "impl")
        };
        
        let id = format!("{}:{}", kind, name.replace(" ", "_"));
        
        // Capture the Impl block itself
        self.nodes.push(ExtractedNode {
            id: id.clone(),
            kind: kind.to_string(),
            signature: format!("impl {}", name),
            content: format!("impl {} {{ ... }}", name),
            edges: vec![format!("struct:{}", type_name)],
        });

        // We want to capture the methods inside the impl as related to this Type
        self.current_scope.push(name.clone());
        
        // Manual recursion to visit items inside impl
        for item in &node.items {
            if let syn::ImplItem::Fn(method) = item {
                let method_name = method.sig.ident.to_string();
                let method_id = format!("fn:{}:{}", type_name, method_name); // Simplified ID for methods: fn:Struct:method
                
                 let signature = method.sig.to_token_stream().to_string();
                 let content = method.to_token_stream().to_string();
                 
                 let mut extracted = ExtractedNode {
                    id: method_id,
                    kind: "method".to_string(),
                    signature,
                    content,
                    edges: vec![format!("struct:{}", type_name)], // Edge back to struct
                };
                
                // Edge detection in body
                let body_str = method.block.to_token_stream().to_string();
                extracted.edges.extend(extract_potential_edges(&body_str));
                
                self.nodes.push(extracted);
            }
        }
        
        self.current_scope.pop();
    }
}

/// Helper to scan for CamelCase (Structs/Types) and snake_case (functions) usage in a string
fn extract_potential_edges(body: &str) -> Vec<String> {
    let mut edges = Vec::new();
    
    // Very naive: Split by non-alphanumeric
    // Filter for things that look like identifiers
    // This is "noisy" but recalls facts well for RAG.
    
    let re = regex::Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]*").unwrap();
    for cap in re.captures_iter(body) {
        let ident = &cap[0];
        if ident.len() > 3 && !is_keyword(ident) {
             edges.push(ident.to_string());
        }
    }
    
    edges.sort();
    edges.dedup();
    edges
}

fn is_keyword(s: &str) -> bool {
    matches!(s, 
        "fn" | "let" | "mut" | "if" | "else" | "match" | "return" | "pub" | "use" | "mod" | 
        "struct" | "enum" | "impl" | "for" | "while" | "loop" | "break" | "continue" | 
        "true" | "false" | "String" | "Vec" | "Option" | "Result" | "Ok" | "Err" | "Some" | "None" |
        "self" | "super" | "crate" | "async" | "await" | "move" | "path" | "str" | "i32" | "u32" | 
        "i64" | "u64" | "bool" | "usize" | "println" | "format" | "unwrap"
    )
}

pub fn parse_rust(code: &str) -> Result<Vec<ExtractedNode>> {
    let syntax = syn::parse_file(code).context("Failed to parse Rust code")?;
    let mut visitor = RustVisitor::new();
    visitor.visit_file(&syntax);
    Ok(visitor.nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let code = r#"
            pub struct User {
                username: String,
            }

            pub fn create_user(username: String) -> User {
                User { username }
            }
        "#;
        let nodes = parse_rust(code).expect("Parse failed");
        
        // Find struct
        let struct_node = nodes.iter().find(|n| n.kind == "struct").expect("Struct missing");
        assert_eq!(struct_node.id, "struct:User");
        
        // Find function
        let fn_node = nodes.iter().find(|n| n.kind == "fn").expect("Fn missing");
        assert_eq!(fn_node.id, "fn:create_user");
        
        // Edge check: "User" should be detected in body
        assert!(fn_node.edges.contains(&"User".to_string()));
    }

    #[test]
    fn test_impl_block() {
        let code = r#"
            struct Car;
            
            impl Car {
                pub fn drive(&self) {
                    println!("Vrum");
                }
            }
        "#;
        let nodes = parse_rust(code).expect("Parse failed");
        
        // Impl node
        let impl_node = nodes.iter().find(|n| n.kind == "impl").expect("Impl node missing");
        assert_eq!(impl_node.id, "impl:Car");
        
        // Method node
        let method = nodes.iter().find(|n| n.kind == "method").expect("Method missing");
        assert_eq!(method.id, "fn:Car:drive");
        assert_eq!(method.edges[0], "struct:Car");
    }
}
