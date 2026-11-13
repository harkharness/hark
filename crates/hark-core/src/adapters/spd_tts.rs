//! Text-to-speech through Speech Dispatcher (`spd-say`), the desktop's own
//! speech service on Linux, and cue sounds from the freedesktop sound
//! theme. The macOS twin is `say_tts`; `adapters::os_tts` picks one.

use crate::ports::{Cue, Tts};

pub struct SpdTts {
    /// A synthesis voice as `spd-say -L` lists it; empty = the service's
    /// default for the language.
    pub voice: String,
    /// ISO code, `pt-BR` style.
    pub lang: String,
}

/// How `spd-say` gets called: its arguments, and what goes on its stdin.
#[derive(Debug, PartialEq, Eq)]
struct SpeakPlan {
    args: Vec<String>,
    stdin: Option<String>,
}

/// The text goes on stdin (pipe mode), never in argv: spoken replies are
/// agent output, and `spd-say` parses its arguments. `-w` waits until it
/// has been spoken, so the "speaking" state ends when the voice does.
fn plan(voice: &str, lang: &str, text: &str) -> SpeakPlan {
    let mut args = vec!["-w".to_string(), "-e".to_string(), "-l".to_string(), lang.to_string()];
    if !voice.is_empty() {
        args.push("-y".to_string());
        args.push(voice.to_string());
    }
    SpeakPlan {
        args,
        stdin: Some(text.to_string()),
    }
}

/// `spd-say -L`: a header line, then NAME LANGUAGE VARIANT columns.
pub fn voices_from(listing: &str) -> Vec<(String, String)> {
    listing
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut cols = line.split_whitespace();
            let name = cols.next()?;
            let lang = cols.next()?;
            Some((name.to_string(), lang.to_string()))
        })
        .collect()
}

/// The voices the service offers right now.
pub fn voices() -> anyhow::Result<Vec<(String, String)>> {
    let out = std::process::Command::new("spd-say").arg("-L").output()?;
    Ok(voices_from(&String::from_utf8_lossy(&out.stdout)))
}

/// The cue players a desktop may have, first one found wins. Fire and
/// forget: a desktop with none of them still hears the voice.
fn cue_players(sound: &str) -> Vec<(String, Vec<String>)> {
    let file = format!("/usr/share/sounds/freedesktop/stereo/{sound}.oga");
    vec![
        ("paplay".into(), vec![file.clone()]),
        ("pw-play".into(), vec![file]),
        ("canberra-gtk-play".into(), vec!["-i".into(), sound.into()]),
    ]
}

impl Tts for SpdTts {
    fn speak(&self, text: &str) -> anyhow::Result<()> {
        let plan = plan(&self.voice, &self.lang, text);
        let mut child = std::process::Command::new("spd-say")
            .args(&plan.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()?;
        if let (Some(input), Some(mut stdin)) = (plan.stdin, child.stdin.take()) {
            use std::io::Write;
            stdin.write_all(input.as_bytes())?;
        }
        let status = child.wait()?;
        anyhow::ensure!(status.success(), "spd-say exited with {status}");
        Ok(())
    }

    fn stop(&self) {
        let _ = std::process::Command::new("spd-say").arg("-C").status();
    }

    fn beep(&self, kind: Cue) {
        let sound = match kind {
            Cue::Listening => "message-new-instant",
            Cue::Captured => "complete",
            Cue::Error => "dialog-error",
        };
        for (program, args) in cue_players(sound) {
            let spawned = std::process::Command::new(&program)
                .args(&args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            if spawned.is_ok() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spoken_text_is_never_an_argument() {
        let p = plan("", "pt-BR", "--output-module=x");
        assert_eq!(p.args, vec!["-w", "-e", "-l", "pt-BR"]);
        assert_eq!(p.stdin.as_deref(), Some("--output-module=x"));
    }

    #[test]
    fn a_chosen_voice_rides_along_and_an_empty_one_leaves_the_default() {
        assert!(plan("Portuguese_(Brazil)", "pt-BR", "oi").args.ends_with(&["-y".to_string(), "Portuguese_(Brazil)".to_string()]));
        assert!(!plan("", "pt-BR", "oi").args.contains(&"-y".to_string()));
    }

    #[test]
    fn the_voice_listing_skips_its_header_and_keeps_name_and_language() {
        let listing = "\
                     NAME                 LANGUAGE                  VARIANT
                Afrikaans                       af                     none
           Afrikaans+Adam                       af                     Adam
      Portuguese_(Brazil)                    pt-BR                     none
";
        assert_eq!(
            voices_from(listing),
            vec![
                ("Afrikaans".to_string(), "af".to_string()),
                ("Afrikaans+Adam".to_string(), "af".to_string()),
                ("Portuguese_(Brazil)".to_string(), "pt-BR".to_string()),
            ]
        );
    }

    #[test]
    fn cues_come_from_the_freedesktop_theme_by_name() {
        let players = cue_players("complete");
        assert_eq!(players[0].1, vec!["/usr/share/sounds/freedesktop/stereo/complete.oga"]);
        assert_eq!(players.last().unwrap().1, vec!["-i", "complete"]);
    }
}
