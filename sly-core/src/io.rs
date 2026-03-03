use crate::error::Result;
use std::io::{self, BufRead, Write};

pub struct CliAdapter {
    pipe_mode: bool,
}

impl CliAdapter {
    pub fn new() -> Self {
        Self { 
            pipe_mode: false,
        }
    }

    /// Read next line from stdin. Direct and de-complected.
    pub fn next_input(&mut self) -> Result<Option<String>> {
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        let mut line = String::new();
        
        if !self.pipe_mode {
            print!("> ");
            io::stdout().flush().map_err(|e| crate::error::SlyError::Io(e))?;
        }
        
        match handle.read_line(&mut line) {
            Ok(0) => Ok(None), 
            Ok(_) => {
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() {
                    return self.next_input(); // Re-prompt on empty input
                }
                Ok(Some(trimmed))
            }
            Err(e) => Err(crate::error::SlyError::Io(e))
        }
    }
}
