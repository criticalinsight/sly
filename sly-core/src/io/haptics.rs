use crate::error::Result;
use std::process::Command;

pub enum SoundEffect {
    Tink,
    Glass,
    Hero,
    Basso,
    Sosumi,
}

impl SoundEffect {
    fn path(&self) -> &str {
        match self {
            SoundEffect::Tink => "/System/Library/Sounds/Tink.aiff",
            SoundEffect::Glass => "/System/Library/Sounds/Glass.aiff",
            SoundEffect::Hero => "/System/Library/Sounds/Hero.aiff",
            SoundEffect::Basso => "/System/Library/Sounds/Basso.aiff",
            SoundEffect::Sosumi => "/System/Library/Sounds/Sosumi.aiff",
        }
    }
}

pub struct HapticSystem;

impl HapticSystem {
    pub fn play_sound(effect: SoundEffect) -> Result<()> {
        let script = format!(
            "do shell script \"afplay {}\"",
            effect.path()
        );
        
        let _ = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .spawn();
            
        Ok(())
    }

    pub fn success_pulse() -> Result<()> {
        Self::play_sound(SoundEffect::Glass)
    }

    pub fn failure_pulse() -> Result<()> {
        Self::play_sound(SoundEffect::Basso)
    }

    pub fn info_pulse() -> Result<()> {
        Self::play_sound(SoundEffect::Tink)
    }
}
