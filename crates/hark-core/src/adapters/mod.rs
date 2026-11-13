//! Adapters: thin imperative shells implementing the ports.

pub mod agent_detect;
pub mod artifacts_fs;
pub mod cpal_audio;
pub mod dash_sources;
pub mod fs_files;
pub mod git_collect;
pub mod log_read;
pub mod memory_files;
pub mod model_fetch;
pub mod diag_log;
pub mod recorder;
pub mod registry_fetch;
pub mod say_tts;
pub mod shell_env;
pub mod spd_tts;
pub mod sqlite_store;
pub mod state_file;
pub mod whisper_stt;

/// The voice this desktop has: `say` on macOS, Speech Dispatcher
/// elsewhere. One type, so the app and the CLI hold it by name.
#[cfg(target_os = "macos")]
pub type OsTts = say_tts::SayTts;
#[cfg(not(target_os = "macos"))]
pub type OsTts = spd_tts::SpdTts;

/// The desktop's voice, set up from the config: the chosen voice, and the
/// language the voice speaks (`language`, or the interface's when the
/// mic detects per utterance).
pub fn os_tts(config: &crate::config::Config) -> OsTts {
    #[cfg(target_os = "macos")]
    {
        let _ = &config.language;
        say_tts::SayTts {
            voice: config.voice.clone(),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        spd_tts::SpdTts {
            voice: config.voice.clone(),
            lang: crate::domain::lang::tts_code(&config.language, &config.ui_language),
        }
    }
}

/// The voices this desktop offers, as (name, locale), Portuguese first.
pub fn os_tts_voices() -> anyhow::Result<Vec<(String, String)>> {
    #[cfg(target_os = "macos")]
    let mut voices = say_tts::voices()?;
    #[cfg(not(target_os = "macos"))]
    let mut voices = spd_tts::voices()?;
    voices.sort_by_key(|(name, lang)| (!lang.starts_with("pt"), name.clone()));
    voices.dedup();
    Ok(voices)
}
