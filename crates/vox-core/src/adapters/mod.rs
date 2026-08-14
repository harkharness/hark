//! Adapters: thin imperative shells implementing the ports.

pub mod claude_cli;
pub mod git_collect;
pub mod jsonl_scan;
pub mod live_sessions;
pub mod memory_files;
pub mod sqlite_store;
pub mod state_file;
pub mod worker;
