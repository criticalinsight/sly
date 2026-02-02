
pub struct SemanticDeduplicator;

impl SemanticDeduplicator {
    /// Collapses repetitive content and error logs.
    pub fn deduplicate(content: &str) -> String {
        let mut result = String::new();
        let lines: Vec<&str> = content.lines().collect();
        
        if lines.is_empty() {
            return String::new();
        }

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();
            
            // Skip empty lines (handled by pruner usually, but safe here)
            if trimmed.is_empty() {
                result.push('\n');
                i += 1;
                continue;
            }

            // check for immediate repetition
            let mut count = 0;
            let mut j = i;
            while j < lines.len() && lines[j].trim() == trimmed {
                count += 1;
                j += 1;
            }

            if count > 2 {
                result.push_str(line);
                result.push('\n');
                result.push_str(&format!("... [Repeated {} times] ...\n", count - 1));
                i = j;
                continue;
            }
            
            // Check for Stack Trace / Error repetitions (Heuristic)
            // If line starts with "at " or "Error:", check if subsequent blocks look similar
            /* 
               Future improvement: sophisticated Levenshtein distance 
               For now, we trust strict equality + simple prefix matching
            */

             result.push_str(line);
             result.push('\n');
             i += 1;
        }

        result
    }
    
    /// Specifically targets error logs which might have slightly different timestamps but same message
    pub fn deduplicate_errors(content: &str) -> String {
        // Implementation for fuzzy matching errors could go here
        // For now, delegating to main deduplicate
        Self::deduplicate(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_exact_lines() {
        let input = "Error A\nError A\nError A\nError A\nNext line";
        let output = SemanticDeduplicator::deduplicate(input);
        assert!(output.contains("Error A"));
        assert!(output.contains("[Repeated 3 times]"));
        assert!(output.contains("Next line"));
    }
}
