use crate::io::interface::{AgentIO, InputMessage, IoModality};
use crate::error::Result;
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::io::{self, Write};

pub struct CliAdapter {
    session_id: String,
    pipe_mode: bool,
}

impl CliAdapter {
    pub fn new(session_id: &str) -> Self {
        Self { 
            session_id: session_id.to_string(),
            pipe_mode: false,
        }
    }

    /// Create a CLI adapter in pipe mode (no prompts, JSON output)
    pub fn pipe(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            pipe_mode: true,
        }
    }
}

#[async_trait]
impl AgentIO for CliAdapter {
    fn modality(&self) -> IoModality {
        if self.pipe_mode {
            IoModality::CliPipe
        } else {
            IoModality::Cli
        }
    }

    async fn next_message(&mut self) -> Result<Option<InputMessage>> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();
        
        // Only show prompt in interactive mode
        if !self.pipe_mode {
            print!("> ");
            io::stdout().flush().map_err(|e| crate::error::SlyError::Io(e))?;
        }
        
        match reader.read_line(&mut line).await {
            Ok(0) => Ok(None), // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return Ok(Some(InputMessage {
                        content: "".to_string(),
                        sender: "user".to_string(),
                        session_id: self.session_id.clone(),
                        metadata: None,
                    }));
                }
                
                // In pipe mode, try to parse as JSON for metadata
                let metadata = if self.pipe_mode {
                    serde_json::from_str(trimmed).ok()
                } else {
                    None
                };
                
                Ok(Some(InputMessage {
                    content: trimmed.to_string(),
                    sender: "user".to_string(),
                    session_id: self.session_id.clone(),
                    metadata,
                }))
            }
            Err(e) => Err(crate::error::SlyError::Io(e))
        }
    }

    async fn send_message(&mut self, content: &str) -> Result<()> {
        println!("{}", content);
        Ok(())
    }

    fn supports_streaming(&self) -> bool {
        !self.pipe_mode // Only interactive mode supports streaming
    }
}
