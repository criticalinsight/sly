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

        // 3. Collapse long JSON blobs (heuristic)
        let re_json = Regex::new(r"\{(\s*.*?\s*)\}").unwrap();
        let content = re_json.replace_all(&content, |caps: &regex::Captures| {
            let inner = &caps[1];
            if inner.len() > 300 {
                format!("{{ /* Collapsed JSON payload ({} bytes) */ }}", inner.len())
            } else {
                caps[0].to_string()
            }
        });

        // 4. Collapse technical stack traces (Rust specific)
        let re_stack = Regex::new(r"(?m)^\s*at .*\n(\s+at .*\n)+").unwrap();
        let content = re_stack.replace_all(&content, "[... collapsed stack trace ...]\n");

        // 5. Strip redundant whitespace and empty lines
        let mut result = String::new();
        let mut prev_line = "";
        let mut repeat_count = 0;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }

            if trimmed == prev_line {
                repeat_count += 1;
                if repeat_count < 3 {
                    result.push_str(trimmed);
                    result.push('\n');
                } else if repeat_count == 3 {
                    result.push_str("... [repeated lines omitted] ...\n");
                }
            } else {
                repeat_count = 0;
                result.push_str(trimmed);
                result.push('\n');
                prev_line = trimmed;
            }
        }

        result
    }
}


pub trait ContextPruner {
    fn score_relevance(chunk: &str) -> f32;
}

pub struct HeuristicPruner;

impl ContextPruner for HeuristicPruner {
    fn score_relevance(chunk: &str) -> f32 {
        let chunk_lower = chunk.to_lowercase();
        let mut score = 0.5; // Base score

        // Boost for errors
        if chunk_lower.contains("error") || chunk_lower.contains("failed") || chunk_lower.contains("panic") {
            score += 0.4;
        }

        // Boost for recent timestamps (heuristic)
        // Checks for patterns like "2024-..." or [INFO]
        if chunk_lower.contains("202") || chunk_lower.contains("[info]") {
            score += 0.2;
        }

        // Penalize repetitive boilerplate
        if chunk.len() > 1000 && !chunk_lower.contains("error") {
            score -= 0.1;
        }

        let score_f32: f32 = score as f32;
        score_f32.clamp(0.0, 1.0)
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

    #[test]
    fn test_prune_json() {
        let large_json = format!("{{ \"data\": \"{}\" }}", "A".repeat(500));
        let pruned = LinguisticPruner::prune(&large_json);
        assert!(pruned.contains("Collapsed JSON payload"));
    }

    #[test]
    fn test_prune_repeats() {
        let text = "output\noutput\noutput\noutput\noutput\n";
        let pruned = LinguisticPruner::prune(text);
        assert!(pruned.contains("[repeated lines omitted]"));
    }

    #[test]
    fn test_heuristic_score() {
        let error_log = "Error: Something failed badly";
        let score = HeuristicPruner::score_relevance(error_log);
        assert!(score > 0.8);

        let info_log = "[INFO] System started 2024-01-01";
        let score2 = HeuristicPruner::score_relevance(info_log);
        assert!(score2 > 0.6);

        let junk = "Just some random text without keywords";
        let score3 = HeuristicPruner::score_relevance(junk);
        assert_eq!(score3, 0.5);
    }
}
