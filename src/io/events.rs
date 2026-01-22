use notify::Event;

#[derive(Debug)]
pub enum Impulse {
    InitiateSession(String),
    ThinkStep(String),
    Observation(String, String),
    FileSystemEvent(Event),
    SwarmSignal(u64, String), // WorkerId, Status
    BootstrapSkills,
    JanitorWakeup,
    SystemInterrupt,
    Error(String),
}
