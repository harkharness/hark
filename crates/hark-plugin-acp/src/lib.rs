//! ACP backend: one plugin for every agent that speaks the Agent Client
//! Protocol (jsonrpc 2.0 over stdio) — gemini, claude-code-acp, codex.
//!
//! This crate is being built in the order the plan set: the PURE parts
//! first (capability negotiation, event translation), driven by traffic
//! recorded from a real agent, and the runtime after.

pub mod caps;
pub mod translate;
