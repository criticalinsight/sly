use std::fmt;

pub type Result<T> = std::result::Result<T, SlyError>;

#[derive(Debug)]
pub enum SlyError {
    Io(std::io::Error),
    Cortex(String),
    Overlay(String),
    Config(String),
    Json(String),
}

impl std::error::Error for SlyError {}

impl fmt::Display for SlyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlyError::Io(e) => write!(f, "IO Error: {}", e),
            SlyError::Cortex(e) => write!(f, "Cortex Error: {}", e),
            SlyError::Overlay(e) => write!(f, "Overlay Error: {}", e),
            SlyError::Config(e) => write!(f, "Config Error: {}", e),
            SlyError::Json(e) => write!(f, "JSON Error: {}", e),
        }
    }
}

impl From<std::io::Error> for SlyError {
    fn from(error: std::io::Error) -> Self {
        SlyError::Io(error)
    }
}
