//! Adapters: thin imperative shells implementing the ports.

pub mod claude_cli;
pub mod cpal_audio;
pub mod fs_files;
pub mod git_collect;
pub mod jsonl_scan;
pub mod live_sessions;
pub mod memory_files;
pub mod say_tts;
pub mod sqlite_store;
pub mod state_file;
pub mod whisper_stt;
pub mod worker;
