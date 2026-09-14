//! ACP backend: one plugin for every agent that speaks the Agent Client
//! Protocol (jsonrpc 2.0 over stdio) — gemini, codex, the DeepSeek
//! harness, kiro, copilot. One registry line per agent, one crate for all.
//!
//! Built in the order the plan set: the PURE parts first (capability
//! negotiation, event translation), driven by traffic recorded from a
//! real agent; then the runtime — framing (`rpc`), the live session
//! (`session`) and the backend seam (`backend`).

pub mod ask;
pub mod backend;
pub mod caps;
pub mod directives;
pub mod rpc;
pub mod session;
pub mod translate;

pub use backend::AcpBackend;
