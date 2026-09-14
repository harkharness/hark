//! The schema-constrained one-shot (voice ask, dispatch gate, intent
//! router) over an agent that has no schema mode.
//!
//! Claude's CLI takes `--json-schema` and answers JSON, period. ACP has
//! nothing of the kind, so the schema goes INTO the prompt and the answer
//! comes back the way models answer: an object, a fenced block, a
//! sentence and then the object. `extract_lenient` finds it; when nothing
//! parses, the prose is the answer and `reply` stays empty — the caller
//! (the gate above all) decides what an unstructured answer means.
//!
//! One fresh session per question, closed right after: the ask is
//! stateless by design, and a session left open is an agent left running.

use crate::session::{connect, Opening, Wire};
use hark_agent::{AgentEvent, TurnResult};
use hark_core::ports::{AgentRunner, TurnRequest};
use std::time::Duration;

/// A one-shot that takes longer than this is an agent that hung.
const ASK_TIMEOUT: Duration = Duration::from_secs(180);

/// The one-shot runner for one ACP registry entry.
pub struct AcpRunner {
    pub id: String,
    pub cmd: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    /// Neutral cwd for the process — the data dir, never a project.
    pub work_dir: std::path::PathBuf,
}

/// System prompt, the question, then the schema as an instruction the
/// model can follow without a schema mode.
pub fn compose(request: &TurnRequest) -> String {
    // The persona is NOT here: it travels as `Opening::lean`, a real system
    // prompt for the agent that takes one and a prefix for the rest.
    format!(
        "{prompt}\n\nResponda SOMENTE com um objeto JSON válido que siga este schema, \
         sem nenhum texto antes ou depois do JSON:\n{schema}",
        prompt = request.prompt.trim(),
        schema = request.schema.trim(),
    )
}

/// Run one question over an already-open wire: connect, read the turn,
/// hang up, hand back the structured answer if there was one.
pub fn ask_over(
    wire: Wire,
    child: Option<std::process::Child>,
    stderr: Option<std::process::ChildStderr>,
    agent: &str,
    cwd: &std::path::Path,
    request: &TurnRequest,
    on_event: &mut dyn FnMut(&AgentEvent),
) -> anyhow::Result<TurnResult> {
    let composed = compose(request);
    // The cheap lane's knobs, for an agent that offers them: the light
    // model and low effort make a one-shot question cheap over ACP too.
    // Manual mode on purpose: a one-shot question runs no tools, and the
    // Claude adapter otherwise inherits the user's mode — recorded: with
    // "auto" in force and haiku picked, it opened the answer with an
    // "Auto mode unavailable" notice.
    let knobs = hark_core::domain::directives::Directives {
        mode: Some(hark_core::domain::directives::Mode::Manual),
        effort: hark_core::domain::directives::Effort::from_flag(request.effort),
        model: (!request.model.is_empty()).then(|| request.model.to_string()),
    };
    // One question has no budget of its own beyond its single turn; the
    // lean opening is what keeps it cheap.
    let no_limits = hark_agent::SpawnLimits::default();
    let opening = Opening {
        agent,
        cwd,
        session_id: "",
        instruction: &composed,
        images: request.images,
        memory_file: None,
        directives: &knobs,
        limits: &no_limits,
        lean: Some(crate::session::LeanAsk { system_prompt: request.system_prompt }),
    };
    let connected = connect(wire, child, stderr, &opening)?;
    let mut prose: Vec<String> = Vec::new();
    let outcome = loop {
        match connected.events.recv_timeout(ASK_TIMEOUT) {
            Ok(event) => {
                on_event(&event);
                match event {
                    AgentEvent::AssistantText(text) => prose.push(text),
                    AgentEvent::Result(turn) => break Ok(turn),
                    // Nobody is here to click: a one-shot has no tools to
                    // run, so a permission ask is answered no and the
                    // turn goes on.
                    AgentEvent::PermissionRequest { request_id, .. } => {
                        let _ = connected
                            .session
                            .respond_permission(&request_id, hark_agent::PermissionDecision::Deny);
                    }
                    _ => {}
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                break Err(anyhow::anyhow!(
                    "agent_failed: {agent}: sem resposta em {}s",
                    ASK_TIMEOUT.as_secs()
                ))
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break Err(anyhow::anyhow!("agent_failed: {agent}: a sessão fechou antes da resposta"))
            }
        }
    };
    // Stateless by design: the session dies with the answer.
    connected.session.shutdown();
    let turn = outcome?;
    if turn.is_error {
        // `raw` already carries the agent's own words.
        return Ok(TurnResult { reply: None, ..turn });
    }
    let text = prose.join("\n");
    let reply = hark_core::domain::reply::extract_lenient(&text);
    // Callers without a typed reply parse `raw`: give them the JSON when
    // there is one, the prose when there is not.
    let raw = reply.as_ref().map(|v| v.to_string()).unwrap_or(text);
    Ok(TurnResult { reply, raw, ..turn })
}

impl AgentRunner for AcpRunner {
    fn ask(
        &self,
        request: &TurnRequest,
        on_event: &mut dyn FnMut(&AgentEvent),
    ) -> anyhow::Result<TurnResult> {
        let mut child = std::process::Command::new(&self.cmd)
            .args(&self.args)
            .current_dir(&self.work_dir)
            .envs(self.env.iter().cloned())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("{}", crate::backend::spawn_error(&self.cmd, &e)))?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take();
        ask_over(
            Wire { reader: Box::new(stdout), writer: Box::new(stdin) },
            Some(child),
            stderr,
            &self.id,
            &self.work_dir,
            request,
            on_event,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::Incoming;
    use crate::session::fake::{chunk, end_turn, fake_agent, gemini_like, update};
    use serde_json::json;

    const SCHEMA: &str = r#"{"type":"object","properties":{"fala":{"type":"string"}},"required":["fala"]}"#;

    fn request<'a>(images: &'a [(String, String)]) -> TurnRequest<'a> {
        TurnRequest {
            prompt: "quais as pendências de hoje?",
            images,
            model: "light",
            system_prompt: "Você é o Hark, responda em uma frase.",
            schema: SCHEMA,
            effort: "low",
        }
    }

    fn first_prompt_text(seen: &crate::session::fake::Seen) -> String {
        let sent = seen.lock().unwrap();
        let params = sent
            .iter()
            .find_map(|m| match m {
                Incoming::Request { method, params, .. } if method == "session/prompt" => Some(params.clone()),
                _ => None,
            })
            .expect("a prompt was sent");
        params["prompt"][0]["text"].as_str().unwrap_or_default().to_string()
    }

    #[test]
    fn compose_carries_the_question_and_the_schema_and_leaves_the_persona_to_the_opening() {
        // The system prompt travels separately (Opening.lean): the Claude
        // adapter takes it as a real system prompt, everyone else gets it
        // prepended by the handshake. compose() itself never repeats it.
        let text = compose(&request(&[]));
        assert!(!text.contains("Você é o Hark"), "{text}");
        let question = text.find("quais as pendências").expect("the question first");
        let schema = text.find(r#""required":["fala"]"#).expect("then the schema, verbatim");
        assert!(question < schema, "{text}");
        assert!(text.contains("JSON"), "the model must be told the shape is JSON: {text}");
    }

    #[test]
    fn an_ask_over_acp_extracts_the_json_the_agent_wrote_in_prose() {
        let (wire, seen) = fake_agent(gemini_like(|id, _p, say| {
            say(update(chunk("Claro! ```json\n{\"fala\": \"duas pendências\"}\n```")));
            say(end_turn(id));
        }));
        let mut events = Vec::new();
        let turn = ask_over(wire, None, None, "gemini", std::path::Path::new("/tmp"), &request(&[]), &mut |e| {
            events.push(e.clone());
        })
        .expect("answers");
        assert!(!turn.is_error);
        assert_eq!(turn.reply, Some(json!({ "fala": "duas pendências" })));
        // `raw` is what the callers without a typed reply parse: the JSON,
        // not the prose around it.
        assert_eq!(turn.raw, r#"{"fala":"duas pendências"}"#);
        assert_eq!(turn.model.as_deref(), Some("gemini"));
        assert!(events.iter().any(|e| matches!(e, AgentEvent::AssistantText(_))), "events were streamed");

        let text = first_prompt_text(&seen);
        assert!(text.contains("Você é o Hark") && text.contains(r#""required":["fala"]"#), "{text}");
    }

    #[test]
    fn a_reply_without_json_keeps_the_prose_and_has_no_structured_reply() {
        let (wire, _seen) = fake_agent(gemini_like(|id, _p, say| {
            say(update(chunk("não consigo responder isso")));
            say(end_turn(id));
        }));
        let turn = ask_over(wire, None, None, "gemini", std::path::Path::new("/tmp"), &request(&[]), &mut |_| {})
            .expect("answers");
        assert_eq!(turn.reply, None);
        assert_eq!(turn.raw, "não consigo responder isso");
    }

    #[test]
    fn a_failed_turn_is_an_error_with_the_agents_words() {
        let (wire, _seen) = fake_agent(gemini_like(|id, _p, say| {
            say(crate::rpc::error_response(&json!(id), -32603, "quota exceeded"));
        }));
        let turn = ask_over(wire, None, None, "gemini", std::path::Path::new("/tmp"), &request(&[]), &mut |_| {})
            .expect("a failed turn is still a turn");
        assert!(turn.is_error);
        assert!(turn.raw.contains("quota exceeded"));
        assert_eq!(turn.reply, None);
    }

    #[test]
    fn a_pasted_screenshot_rides_on_the_opening_prompt() {
        let images = vec![("image/png".to_string(), "AAAA".to_string())];
        let (wire, seen) = fake_agent(gemini_like(|id, _p, say| say(end_turn(id))));
        let _ = ask_over(wire, None, None, "gemini", std::path::Path::new("/tmp"), &request(&images), &mut |_| {})
            .expect("answers");
        let sent = seen.lock().unwrap();
        let prompt = sent
            .iter()
            .find_map(|m| match m {
                Incoming::Request { method, params, .. } if method == "session/prompt" => Some(params["prompt"].clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(prompt[1], json!({ "type": "image", "data": "AAAA", "mimeType": "image/png" }));
    }

    #[test]
    fn a_missing_binary_is_reported_as_missing() {
        let runner = AcpRunner {
            id: "ghost".into(),
            cmd: "hark-no-such-agent-xyz".into(),
            args: vec![],
            env: vec![],
            work_dir: std::env::temp_dir(),
        };
        let err = runner.ask(&request(&[]), &mut |_| {}).expect_err("cannot spawn").to_string();
        assert!(err.starts_with("agent_missing: hark-no-such-agent-xyz"), "{err}");
    }
}
