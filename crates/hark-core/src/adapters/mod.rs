//! Adapters: thin imperative shells implementing the ports.

pub mod agent_detect;
pub mod cpal_audio;
pub mod fs_files;
pub mod git_collect;
pub mod memory_files;
pub mod model_fetch;
pub mod recorder;
pub mod registry_fetch;
pub mod say_tts;
pub mod shell_env;
pub mod sqlite_store;
pub mod state_file;
pub mod whisper_stt;
