use crate::error::Result;
use std::fs;
use std::path::Path;

pub struct Memory {
    base_path: String,
}

impl Memory {
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
            Ok(content.lines().map(|s| s.to_string()).collect())
        } else {
            Ok(vec![])
        }
    }

    /// Update session messages directly.
    pub fn update_messages(&self, session_id: &str, messages: &[String]) -> Result<()> {
        let path = self.get_session_path(session_id);
        fs::write(path, messages.join("\n"))?;
        Ok(())
    }
}
