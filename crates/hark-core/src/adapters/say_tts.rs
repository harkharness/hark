//! Text-to-speech via macOS `say` and cue sounds via `afplay`.

use crate::ports::{Cue, Tts};

pub struct SayTts {
    pub voice: String,
}

impl Tts for SayTts {
    fn speak(&self, text: &str) -> anyhow::Result<()> {
        let status = std::process::Command::new("/usr/bin/say")
            .args(["-v", &self.voice, text])
            .status()?;
        anyhow::ensure!(status.success(), "say exited with {status}");
        Ok(())
    }

    fn beep(&self, kind: Cue) {
        let sound = match kind {
            Cue::Listening => "/System/Library/Sounds/Tink.aiff",
            Cue::Captured => "/System/Library/Sounds/Pop.aiff",
            Cue::Error => "/System/Library/Sounds/Basso.aiff",
        };
        // Fire and forget; a missing sound must never break the flow.
        let _ = std::process::Command::new("/usr/bin/afplay")
            .arg(sound)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}
