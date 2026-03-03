use thiserror::Error;

#[derive(Error, Debug)]
pub enum SlyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("Context error: {0}")]
    Cortex(String),

    #[error("Overlay error: {0}")]
    Overlay(String),

    #[error("Task error: {0}")]
    Task(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Watch error: {0}")]
    Watch(#[from] notify::Error),

    #[error("Generic error: {0}")]
    Generic(String),
}

pub type Result<T> = std::result::Result<T, SlyError>;
