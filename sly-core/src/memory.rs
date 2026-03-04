//! Session persistence with mortal memory compression.
//!
//! Messages are stored delimited by `---MSG---` to avoid
//! corruption from newlines within messages (e.g. stderr).
//! When the history exceeds 20 entries, older context is
//! excised via a rolling window ("mortal memory").

use crate::error::Result;
use std::fs;
use std::path::Path;

/// Callback type for summarizing discarded messages during compression.
pub type SummarizeFn<'a> = &'a dyn Fn(&[String]) -> String;

/// File-backed session memory.
///
/// Each session is a plain text file: one message per line.
pub struct Memory {
    pub(crate) base_path: String,
}

impl Memory {
    /// Create or open a memory store at the given directory.
    pub fn new(path: &str) -> Result<Self> {
        let base = Path::new(path);
        if !base.exists() {
            fs::create_dir_all(base).ok();
        }
        Ok(Self { base_path: path.to_string() })
    }

    fn get_session_path(&self, id: &str) -> String {
        format!("{}/session_{}.txt", self.base_path, id)
    }

    /// Retrieve raw message list for a session.
    pub fn get_messages(&self, session_id: &str) -> Result<Vec<String>> {
        let path = self.get_session_path(session_id);
        if let Ok(content) = fs::read_to_string(path) {
            if content.is_empty() {
                return Ok(vec![]);
            }
            Ok(content.split("\n---MSG---\n")
                .map(|s| s.to_string())
                .collect())
        } else {
            Ok(vec![])
        }
    }

    /// Update session messages with Mortal Memory Compression.
    ///
    /// If `summarize_fn` is provided, discarded messages are passed to it
    /// to produce a dense summary. Otherwise a static marker is used.
    pub fn update_messages(
        &self,
        session_id: &str,
        messages: &[String],
        max_history: usize,
        summarize_fn: Option<SummarizeFn<'_>>,
    ) -> Result<()> {
        let mut final_messages = messages.to_vec();
        
        // Mortal Memory Compression (Rolling Window)
        if messages.len() > max_history {
            println!("🗜️ Memory threshold reached ({}). Compacting...", max_history);
            let keep_count = max_history - 1; // leave room for summary
            let discard_end = messages.len() - keep_count;
            let discarded = &messages[1..discard_end]; // skip first (original user query)
            let remainder = &messages[discard_end..];
            let first = messages[0].clone();
            
            let summary = if let Some(f) = summarize_fn {
                f(discarded)
            } else {
                "... [Older context mortality excised] ...".to_string()
            };

            final_messages = vec![first, summary];
            final_messages.extend_from_slice(remainder);
        }

        let path = self.get_session_path(session_id);
        fs::write(path, final_messages.join("\n---MSG---\n"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_memory(label: &str) -> Memory {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = format!("/tmp/sly_test_{}_{}", label, id);
        let _ = fs::remove_dir_all(&dir);
        Memory::new(&dir).unwrap()
    }

    #[test]
    fn test_memory_new_creates_dir() {
        let mem = temp_memory("new");
        assert!(Path::new(&mem.base_path).exists());
        fs::remove_dir_all(&mem.base_path).ok();
    }

    #[test]
    fn test_empty_session_returns_empty() {
        let mem = temp_memory("empty");
        let msgs = mem.get_messages("nonexistent").unwrap();
        assert!(msgs.is_empty());
        fs::remove_dir_all(&mem.base_path).ok();
    }

    #[test]
    fn test_write_and_read_messages() {
        let mem = temp_memory("rw");
        let msgs = vec!["hello".to_string(), "world".to_string()];
        mem.update_messages("rw1", &msgs, 20, None).unwrap();
        let retrieved = mem.get_messages("rw1").unwrap();
        assert_eq!(retrieved, msgs);
        fs::remove_dir_all(&mem.base_path).ok();
    }

    #[test]
    fn test_mortal_memory_compression() {
        let mem = temp_memory("compress");
        // Create 25 single-word messages (no newlines) to exceed threshold of 20
        let msgs: Vec<String> = (0..25).map(|i| format!("m{}", i)).collect();
        mem.update_messages("cmp", &msgs, 20, None).unwrap();
        let retrieved = mem.get_messages("cmp").unwrap();
        // Should be: first msg + marker + last 19 = 21 total
        assert_eq!(retrieved.len(), 21);
        assert_eq!(retrieved[0], "m0");
        assert_eq!(retrieved[1], "... [Older context mortality excised] ...");
        assert_eq!(*retrieved.last().unwrap(), "m24");
        fs::remove_dir_all(&mem.base_path).ok();
    }

    #[test]
    fn test_mortal_memory_with_summary_fn() {
        let mem = temp_memory("summary");
        let msgs: Vec<String> = (0..25).map(|i| format!("m{}", i)).collect();
        let summarizer = |discarded: &[String]| -> String {
            format!("SUMMARY: {} messages compressed", discarded.len())
        };
        mem.update_messages("sfn", &msgs, 20, Some(&summarizer)).unwrap();
        let retrieved = mem.get_messages("sfn").unwrap();
        assert_eq!(retrieved[0], "m0");
        assert!(retrieved[1].starts_with("SUMMARY:"));
        assert_eq!(*retrieved.last().unwrap(), "m24");
        fs::remove_dir_all(&mem.base_path).ok();
    }
}
