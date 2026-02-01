use notify::Event;

#[derive(Debug)]
pub enum Impulse {
    InitiateSession(String),
    ThinkStep(String),
    Observation(String, String),
    FileSystemEvent(Event),
    ThoughtStream(String, String),
    Undo(String), // session_id
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
