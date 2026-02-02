use crate::io::interface::{AgentIO, InputMessage};
use crate::error::Result;
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::io::{self, Write};

pub struct CliAdapter {
    session_id: String,
}

impl CliAdapter {
    pub fn new(session_id: &str) -> Self {
        Self { session_id: session_id.to_string() }
    }
}

#[async_trait]
impl AgentIO for CliAdapter {
    async fn next_message(&mut self) -> Result<Option<InputMessage>> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();
        
        // Simple prompt for interactive mode
        print!("> ");
        io::stdout().flush().map_err(|e| crate::error::SlyError::Io(e))?;
        
        match reader.read_line(&mut line).await {
            Ok(0) => Ok(None), // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return Ok(Some(InputMessage {
                        content: "".to_string(), // Empty inputs might be valid "continue" signals?
                        sender: "user".to_string(),
                        session_id: self.session_id.clone(),
                    }));
                }
                
                Ok(Some(InputMessage {
                    content: trimmed.to_string(),
                    sender: "user".to_string(),
                    session_id: self.session_id.clone(),
                }))
            }
            Err(e) => Err(crate::error::SlyError::Io(e))
        }
    }

    async fn send_message(&mut self, content: &str) -> Result<()> {
        println!("{}", content);
        Ok(())
    }
}
