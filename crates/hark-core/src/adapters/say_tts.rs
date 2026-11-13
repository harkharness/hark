//! Text-to-speech via macOS `say` and cue sounds via `afplay`. The Linux
//! twin is `spd_tts`; `adapters::os_tts` picks one.

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

/// `say -v ?`: one voice per line, `Name   locale   # sample`. The name
/// may hold spaces and parentheses; the locale is the first token with an
/// underscore.
pub fn voices_from(listing: &str) -> Vec<(String, String)> {
    listing
        .lines()
        .filter_map(|line| {
            let lang_at = line.find('_').map(|i| i.saturating_sub(2))?;
            let name = line[..lang_at].trim();
            let lang = line[lang_at..].split_whitespace().next()?;
            (!name.is_empty()).then(|| (name.to_string(), lang.to_string()))
        })
        .collect()
}

/// The voices the OS engine offers right now.
pub fn voices() -> anyhow::Result<Vec<(String, String)>> {
    let out = std::process::Command::new("/usr/bin/say").args(["-v", "?"]).output()?;
    Ok(voices_from(&String::from_utf8_lossy(&out.stdout)))
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

    fn stop(&self) {
        let _ = std::process::Command::new("killall").arg("say").status();
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
    fn the_voice_listing_yields_name_and_locale_per_line() {
        let listing = "\
Luciana             pt_BR    # Olá! Meu nome é Luciana.
Samantha            en_US    # Hello! My name is Samantha.
Eddy (Português (Brasil)) pt_BR    # Olá! Meu nome é Eddy.
";
        assert_eq!(
            voices_from(listing),
            vec![
                ("Luciana".to_string(), "pt_BR".to_string()),
                ("Samantha".to_string(), "en_US".to_string()),
                ("Eddy (Português (Brasil))".to_string(), "pt_BR".to_string()),
            ]
        );
    }

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
