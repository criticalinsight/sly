//! Unified error type for the Sly engine.
//!
//! A single `SlyError` enum covers all failure modes.
//! Implements `Display`, `Error`, and `From<std::io::Error>`.

use std::fmt;

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, SlyError>;

/// Every error the engine can produce.
#[derive(Debug)]
pub enum SlyError {
    /// Filesystem or I/O failure.
    Io(std::io::Error),
    /// LLM inference failure.
    Cortex(String),
    /// Overlay filesystem violation.
    Overlay(String),
    /// Missing or invalid configuration.
    Config(String),
    /// JSON parsing failure (Zero-Serde).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_cortex() {
        let e = SlyError::Cortex("model failed".to_string());
        assert_eq!(format!("{}", e), "Cortex Error: model failed");
    }

    #[test]
    fn test_error_display_all_variants() {
        assert!(format!("{}", SlyError::Overlay("x".into())).contains("Overlay"));
        assert!(format!("{}", SlyError::Config("x".into())).contains("Config"));
        assert!(format!("{}", SlyError::Json("x".into())).contains("JSON"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let sly_err = SlyError::from(io_err);
        assert!(format!("{}", sly_err).contains("IO Error"));
    }
}
