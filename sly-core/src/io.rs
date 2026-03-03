//! Standard I/O adapter for the CLI interface.
//!
//! Reads user input from stdin, line by line.
//! Empty lines are silently re-prompted.

use crate::error::Result;
use std::io::{self, BufRead, Write};

/// Minimal CLI adapter. Reads lines from stdin.
pub struct CliAdapter {
    pipe_mode: bool,
}

impl CliAdapter {
    /// Create a new interactive CLI adapter.
    pub fn new() -> Self {
        Self { pipe_mode: false }
    }

    /// Read the next non-empty line from stdin.
    ///
    /// Returns `Ok(None)` on EOF, `Ok(Some(line))` on valid input.
    /// Empty lines are silently skipped (re-prompt via loop, not
    /// recursion, to avoid stack overflow on piped input).
    pub fn next_input(&mut self) -> Result<Option<String>> {
        loop {
            let stdin = io::stdin();
            let mut handle = stdin.lock();
            let mut line = String::new();

            if !self.pipe_mode {
                print!("> ");
                io::stdout().flush().map_err(crate::error::SlyError::Io)?;
            }

            match handle.read_line(&mut line) {
                Ok(0) => return Ok(None),
                Ok(_) => {
                    let trimmed = line.trim().to_string();
                    if trimmed.is_empty() {
                        continue; // Loop instead of recursion
                    }
                    return Ok(Some(trimmed));
                }
                Err(e) => return Err(crate::error::SlyError::Io(e)),
            }
        }
    }
}
