use regex::Regex;

pub struct LinguisticPruner;

impl LinguisticPruner {
    pub fn prune(content: &str) -> String {
        // 1. Strip Single Line Comments (// ...)
        let re_single = Regex::new(r"//.*").unwrap();
        let content = re_single.replace_all(content, "");

        // 2. Strip Multi-line Comments (/* ... */)
        let re_multi = Regex::new(r"(?s)/\*.*?\*/").unwrap();
        let content = re_multi.replace_all(&content, "");

        // 3. Strip redundant whitespace and empty lines
        let mut result = String::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                result.push_str(trimmed);
                result.push('\n');
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prune_rust() {
        let code = r#"
            // This is a comment
            fn main() {
                /* block
                   comment */
                println!("hello"); // inline
            }
        "#;
        let pruned = LinguisticPruner::prune(code);
        assert!(!pruned.contains("comment"));
        assert!(pruned.contains("fn main()"));
        assert!(pruned.contains("println!(\"hello\");"));
    }
}
