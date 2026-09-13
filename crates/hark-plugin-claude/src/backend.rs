//! The claude plugin as an `AgentBackend`: the driver stops naming
//! PersistentWorker/WorkerSpawn and speaks the neutral seam. Spawning
//! still runs the same process with the same flags — this module only
//! adds the translation and the stdout→event bridge.

use crate::worker::{PersistentWorker, WorkerSpawn};
use hark_agent::AgentEvent;
use hark_core::ports::{AgentBackend, AgentRunner, AgentSession, EventRx, SessionSpec};
use std::sync::Arc;

/// The native Claude Code backend: one instance per registry entry.
pub struct ClaudeBackend {
    pub bin: String,
    /// Neutral cwd for the one-shot runner (ask/gate) — the data dir.
    pub work_dir: std::path::PathBuf,
    /// The registry entry's base env. This is how one plugin serves two
    /// entries: the plain "claude" and a twin pointed at a company
    /// gateway through ANTHROPIC_BASE_URL / ANTHROPIC_AUTH_TOKEN. Applied
    /// to every process — workers AND the one-shot ask/gate — so nothing
    /// slips past the gateway.
    pub envs: Vec<(String, String)>,
}

impl ClaudeBackend {
    fn to_worker_spawn(&self, spec: &SessionSpec) -> WorkerSpawn {
        WorkerSpawn {
            claude_bin: self.bin.clone(),
            cwd: spec.cwd.clone(),
            session_id: spec.session_id.clone(),
            instruction: spec.instruction.clone(),
            directives: spec.directives.clone(),
            limits: spec.limits,
            // Entry env first, per-session (eco) env on top.
            envs: self.envs.iter().cloned().chain(spec.envs.iter().cloned()).collect(),
            fork: spec.fork,
        }
    }
}

impl AgentBackend for ClaudeBackend {
    fn id(&self) -> &str {
        "claude"
    }

    fn capabilities(&self) -> hark_agent::Capabilities {
        crate::capabilities()
    }

    fn spawn(&self, spec: &SessionSpec) -> anyhow::Result<(Arc<dyn AgentSession>, EventRx)> {
        let (worker, stdout) = PersistentWorker::spawn(&self.to_worker_spawn(spec))?;
        let (tx, rx) = std::sync::mpsc::channel();
        bridge_reader(stdout, tx);
        Ok((Arc::new(worker), rx))
    }

    fn runner(&self) -> Box<dyn AgentRunner + Send + Sync> {
        Box::new(crate::cli::ClaudeCli {
            claude_bin: self.bin.clone(),
            work_dir: self.work_dir.clone(),
            envs: self.envs.clone(),
        })
    }
}

/// The session trait is the worker's own API — a straight delegation.
impl AgentSession for PersistentWorker {
    fn send_text(&self, text: &str, images: &[(String, String)]) -> anyhow::Result<()> {
        PersistentWorker::send_text(self, text, images)
    }
    fn respond_permission(
        &self,
        request_id: &str,
        decision: hark_agent::PermissionDecision,
    ) -> anyhow::Result<()> {
        PersistentWorker::respond_permission(self, request_id, decision)
    }
    fn interrupt(&self) -> anyhow::Result<()> {
        PersistentWorker::interrupt(self)
    }
    fn shutdown(&self) {
        PersistentWorker::shutdown(self)
    }
    fn pid(&self) -> Option<u32> {
        Some(self.pid)
    }
    fn exit_report(&self) -> (Option<i32>, String) {
        PersistentWorker::exit_report(self)
    }
}

/// stdout → events: one parsed line per send; dropping the sender at EOF
/// closes the stream, which is how the reader loop learns the process
/// ended. HARK_DEBUG echoes raw lines, as the old in-loop echo did.
pub fn bridge_reader<R: std::io::Read + Send + 'static>(
    reader: R,
    tx: std::sync::mpsc::Sender<AgentEvent>,
) {
    std::thread::spawn(move || {
        use std::io::BufRead;
        let debug = std::env::var("HARK_DEBUG").is_ok();
        for line in std::io::BufReader::new(reader).lines().map_while(Result::ok) {
            if debug {
                eprintln!("[hark worker] {line}");
            }
            if tx.send(crate::stream::parse(&line)).is_err() {
                break; // receiver gone: the driver dropped the session
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use hark_agent::AgentEvent;

    #[test]
    fn the_bridge_turns_stream_json_lines_into_events_and_closes() {
        // A canned worker transcript: init, prose, a result — then EOF.
        let script = concat!(
            r#"{"type":"system","subtype":"init","session_id":"s-1","slash_commands":["/compact"]}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"olá"}]}}"#,
            "\n",
            r#"{"type":"result","subtype":"success","is_error":false,"result":"fim","total_cost_usd":0.01,"duration_ms":5,"modelUsage":{}}"#,
            "\n",
        );
        let (tx, rx) = std::sync::mpsc::channel();
        bridge_reader(std::io::Cursor::new(script.to_string()), tx);

        let events: Vec<AgentEvent> = rx.iter().collect(); // iter ends on close
        assert!(
            matches!(&events[0], AgentEvent::SessionStarted { session_id, .. } if session_id == "s-1"),
            "first event announces the session, got {:?}",
            events.first()
        );
        assert!(events
            .iter()
            .any(|e| matches!(e, AgentEvent::AssistantText(t) if t.contains("olá"))));
        assert!(
            matches!(events.last(), Some(AgentEvent::Result(r)) if !r.is_error),
            "stream ends with the turn result"
        );
        // rx.iter() completing IS the close assertion: the sender dropped.
    }
}
