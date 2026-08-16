//! Pure domain logic. Nothing in this module performs I/O.

pub mod board;
pub mod claude_event;
pub mod context;
pub mod directives;
pub mod gate;
pub mod dispatch;
pub mod intent;
pub mod memory;
pub mod prompt;
pub mod session_log;
pub mod snapshot;
pub mod task_command;
pub mod transcript;
pub mod vad;
