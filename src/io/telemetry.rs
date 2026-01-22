// src/io/telemetry.rs - Haptic Telemetry for Sly v0.5.0

use std::process::Command;

pub enum TelemetryEvent {
    Thinking,
    Done,
    Error,
    Custom(String),
}

pub struct Telemetry;

impl Telemetry {
    pub fn say(event: TelemetryEvent) {
        let msg = match event {
            TelemetryEvent::Thinking => "Thinking.".to_string(),
            TelemetryEvent::Done => "Task complete.".to_string(),
            TelemetryEvent::Error => "Error encountered.".to_string(),
            TelemetryEvent::Custom(s) => s,
        };

        // We use spawn and don't wait to avoid blocking the agent loop
        let _ = Command::new("say")
            .arg("-v")
            .arg("Siri") // Clean voice for macOS
            .arg(&msg)
            .spawn();
    }
}
