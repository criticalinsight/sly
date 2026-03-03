use notify::Event;

#[derive(Debug, Clone)]
pub enum Impulse {
    InitiateSession(String),
    VisualInput(String, String), // path, prompt
    ThinkStep(String),
    Observation(String, String),
    FileSystemEvent(Event),
    ThoughtStream(String, String),
    Undo(String), // session_id
    ExecuteWorkflow(String),
    Terminate,
    SystemInterrupt,
    Error(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_impulse_debug() {
        let impulse = Impulse::SystemInterrupt;
        assert!(format!("{:?}", impulse).contains("SystemInterrupt"));
    }
}
