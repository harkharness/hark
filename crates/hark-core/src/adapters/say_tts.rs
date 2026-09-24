//! Text-to-speech via macOS `say` and cue sounds via `afplay`.

use crate::ports::{Cue, Tts};

pub struct SayTts {
    pub voice: String,
}

/// How `say` gets called: its arguments, and what goes on its stdin.
#[derive(Debug, PartialEq, Eq)]
struct SpeakPlan {
    args: Vec<String>,
    stdin: Option<String>,
}

/// The text goes on stdin, never in argv: `say` parses its arguments,
/// and spoken replies are agent output.
fn plan(voice: &str, text: &str) -> SpeakPlan {
    SpeakPlan {
        args: vec!["-v".into(), voice.into()],
        stdin: Some(text.into()),
    }
}

impl Tts for SayTts {
    fn speak(&self, text: &str) -> anyhow::Result<()> {
        let plan = plan(&self.voice, text);
        let mut child = std::process::Command::new("/usr/bin/say")
            .args(&plan.args)
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let (Some(input), Some(mut stdin)) = (plan.stdin, child.stdin.take()) {
            use std::io::Write;
            stdin.write_all(input.as_bytes())?;
        }
        let status = child.wait()?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spoken_text_is_never_an_argument() {
        // Replies are spoken automatically, and `say` parses its argv:
        // a reply of "--output-file=/Users/u/.zshrc" would overwrite that
        // file with audio. With no message argument, `say` reads stdin.
        let p = plan("Luciana", "--output-file=/tmp/x");
        assert_eq!(p.args, vec!["-v".to_string(), "Luciana".to_string()]);
        assert_eq!(p.stdin.as_deref(), Some("--output-file=/tmp/x"));
    }
}
