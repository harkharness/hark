//! Pure prompt assembly: snapshot + question -> the exact text sent to Claude.

use crate::domain::memory::WorkerRecord;
use crate::domain::snapshot::SessionSummary;

/// A Claude Code session currently running (from `claude agents --json`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveSession {
    pub name: String,
    pub cwd: String,
    pub status: Option<String>,
    pub session_id: Option<String>,
}

/// Working-tree status of one configured repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoStatus {
    pub path: String,
    pub branch: String,
    pub dirty_files: usize,
    pub recent_commits: Vec<String>,
}

/// Everything Vox knows at question time. Assembled by adapters, consumed here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub generated_at: String,
    pub sessions: Vec<SessionSummary>,
    pub live: Vec<LiveSession>,
    pub repos: Vec<RepoStatus>,
    /// Recent Vox Q&A (journal tail of the resolved context), oldest first.
    pub journal: Vec<String>,
    /// Dispatched workers across ALL workspaces (the machine-wide view).
    pub workers: Vec<WorkerRecord>,
    /// The invisible kanban, rendered by `board::render` (empty = no section).
    pub board: String,
}

/// JSON schema enforced on Claude's answer (`--json-schema`).
/// `fala` is read aloud by TTS; `detalhes` is shown on screen.
pub const RESPONSE_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "fala": {
      "type": "string",
      "description": "UMA frase curta de manchete para ser lida em voz alta (maximo ~15 palavras). NUNCA enumere itens aqui: se a resposta for uma lista, resuma a contagem e ofereca ler ou detalhar (ex: 'Quatro pendencias na tela; a mais quente e o PR de Pagamentos. Quer que eu leia?'). Sem markdown, sem caminhos, sem codigo."
    },
    "detalhes": {
      "type": "string",
      "description": "Resumo objetivo para leitura na tela, no maximo 10 linhas. Pode conter caminhos e referencias. Sem repetir o conteudo de 'fala'."
    },
    "itens": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Lista opcional de itens acionaveis, um por linha."
    },
    "board": {
      "type": "array",
      "description": "Atualizacoes do quadro de tarefas do usuario. Inclua APENAS quando a conversa revelar tarefa nova, mudanca de status ou conclusao; na duvida, omita. Reuse o titulo de uma tarefa existente do quadro ao atualiza-la.",
      "items": {
        "type": "object",
        "properties": {
          "titulo": { "type": "string", "description": "Titulo curto e estavel da tarefa" },
          "status": { "type": "string", "enum": ["backlog", "doing", "waiting", "done"] },
          "nota": { "type": "string", "description": "Uma linha de contexto (ex: aguardando GMUD)" },
          "sessao": { "type": "string", "description": "Id da sessao de onde essa tarefa vem, copiado EXATAMENTE do contexto (campo 'sessao ...'). Sempre inclua quando a tarefa se refere a uma sessao listada." }
        },
        "required": ["titulo", "status"]
      }
    }
  },
  "required": ["fala", "detalhes"]
}"#;

/// System prompt for the voice assistant persona.
pub const VOICE_SYSTEM_PROMPT: &str = "Voce e o Vox, assistente de voz de um engenheiro. \
Responda em portugues brasileiro. Seja direto e pratico. \
Divisao rigida: 'fala' e so a manchete falada (1 frase, sem listas); \
a informacao completa vai em 'detalhes' e 'itens', que aparecem na tela. \
Ouvir e caro, ler e barato: nunca faca a voz recitar o que a tela ja mostra. \
Se as mensagens mais recentes indicarem que um problema ja foi resolvido, nao o liste como pendencia.";

/// Sessions rendered into the prompt, newest first. Sized so a full week of
/// heavy usage still fits at a few cents per question.
pub const MAX_SESSIONS: usize = 30;
/// Character budget per rendered user prompt from the history.
const MAX_PROMPT_CHARS: usize = 220;

/// Collapse whitespace and clamp to `max` chars (ellipsis when truncated).
/// Keeps giant pasted logs in the history from exploding the context.
fn compact(text: &str, max: usize) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.chars().count() <= max {
        return joined;
    }
    let mut clamped: String = joined.chars().take(max).collect();
    clamped.push('…');
    clamped
}

/// Render the full user prompt: context snapshot + the spoken question.
pub fn build(question: &str, snapshot: &Snapshot) -> String {
    let sessions = sorted_sessions(snapshot);
    let sections = [
        format!("Contexto gerado em: {}", snapshot.generated_at),
        board_section(&snapshot.board),
        live_section(&snapshot.live),
        workers_section(&snapshot.workers),
        repos_section(&snapshot.repos),
        sessions_section(&sessions),
        journal_section(&snapshot.journal),
        format!("Pergunta do usuario (por voz):\n{question}"),
    ];
    sections
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn sorted_sessions(snapshot: &Snapshot) -> Vec<&SessionSummary> {
    let mut sessions: Vec<_> = snapshot.sessions.iter().collect();
    sessions.sort_by(|a, b| b.last_ts.cmp(&a.last_ts));
    sessions.truncate(MAX_SESSIONS);
    sessions
}

fn live_section(live: &[LiveSession]) -> String {
    if live.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = live
        .iter()
        .map(|l| {
            format!(
                "- {} (cwd: {}, status: {})",
                l.name,
                l.cwd,
                l.status.as_deref().unwrap_or("unknown")
            )
        })
        .collect();
    format!("Sessoes do Claude Code ABERTAS agora:\n{}", lines.join("\n"))
}

fn board_section(board: &str) -> String {
    if board.is_empty() {
        return String::new();
    }
    format!(
        "Quadro de tarefas do usuario (mantido pelo Vox; atualize via campo 'board' da resposta):\n{board}"
    )
}

fn workers_section(workers: &[WorkerRecord]) -> String {
    if workers.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = workers
        .iter()
        .map(|w| {
            let status = serde_json::to_string(&w.status).unwrap_or_default();
            format!(
                "- {} [{}] contexto {} em {} (sessao {}, desde {}): {}",
                w.task_id,
                status.trim_matches('"'),
                w.context,
                w.workspace,
                w.session_id,
                w.started_at,
                w.summary
            )
        })
        .collect();
    format!(
        "Tarefas despachadas pelo Vox (todos os workspaces):\n{}",
        lines.join("\n")
    )
}

fn journal_section(journal: &[String]) -> String {
    if journal.is_empty() {
        return String::new();
    }
    format!(
        "Consultas recentes feitas ao Vox (mais antiga primeiro):\n{}",
        journal.join("\n---\n")
    )
}

fn repos_section(repos: &[RepoStatus]) -> String {
    if repos.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = repos
        .iter()
        .map(|r| {
            format!(
                "- {} (branch: {}, dirty files: {})\n  commits recentes: {}",
                r.path,
                r.branch,
                r.dirty_files,
                r.recent_commits.join(" | ")
            )
        })
        .collect();
    format!("Repositorios monitorados:\n{}", lines.join("\n"))
}

fn sessions_section(sessions: &[&SessionSummary]) -> String {
    if sessions.is_empty() {
        return "Nenhuma sessao recente no historico.".to_string();
    }
    let blocks: Vec<String> = sessions.iter().map(|s| session_block(s)).collect();
    format!(
        "Historico de sessoes recentes (mais nova primeiro):\n{}",
        blocks.join("\n")
    )
}

fn session_block(s: &SessionSummary) -> String {
    // The index stores every prompt; only the newest few reach the context.
    let skip = s
        .recent_prompts
        .len()
        .saturating_sub(crate::domain::snapshot::MAX_RECENT_PROMPTS);
    let prompts: Vec<String> = s
        .recent_prompts
        .iter()
        .skip(skip)
        .map(|p| format!("    [{}] {}", p.ts, compact(&p.text, MAX_PROMPT_CHARS)))
        .collect();
    format!(
        "- sessao {} | titulo: {} | cwd: {} | branch: {} | ultima atividade: {}\n  ultimos pedidos do usuario:\n{}",
        s.session_id,
        s.title.as_deref().unwrap_or("(sem titulo)"),
        s.cwd.as_deref().unwrap_or("?"),
        s.git_branch.as_deref().unwrap_or("?"),
        s.last_ts.as_deref().unwrap_or("?"),
        prompts.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::memory::{WorkerRecord, WorkerStatus};
    use crate::domain::snapshot::{RecentPrompt, SessionSummary};

    fn snapshot() -> Snapshot {
        Snapshot {
            generated_at: "2026-08-14T12:00:00.000Z".into(),
            sessions: vec![
                SessionSummary {
                    session_id: "old1".into(),
                    cwd: Some("/home/dev/alpha".into()),
                    git_branch: Some("main".into()),
                    title: Some("Alpha refactor".into()),
                    last_prompt: Some("run the tests again".into()),
                    last_ts: Some("2026-08-13T09:00:00.000Z".into()),
                    recent_prompts: vec![RecentPrompt {
                        ts: "2026-08-13T09:00:00.000Z".into(),
                        text: "run the tests again".into(),
                    }],
                },
                SessionSummary {
                    session_id: "new1".into(),
                    cwd: Some("/home/dev/beta".into()),
                    git_branch: Some("feat/webhook".into()),
                    title: Some("Webhook migration".into()),
                    last_prompt: Some("open the PR".into()),
                    last_ts: Some("2026-08-14T11:00:00.000Z".into()),
                    recent_prompts: vec![RecentPrompt {
                        ts: "2026-08-14T11:00:00.000Z".into(),
                        text: "open the PR".into(),
                    }],
                },
            ],
            live: vec![LiveSession {
                name: "beta-42".into(),
                cwd: "/home/dev/beta".into(),
                status: Some("idle".into()),
                session_id: Some("live1".into()),
            }],
            repos: vec![RepoStatus {
                path: "/home/dev/beta".into(),
                branch: "feat/webhook".into(),
                dirty_files: 3,
                recent_commits: vec!["feat: add handler".into()],
            }],
            journal: vec!["2026-08-14T09:00:00Z\nQ: como está o beta?\nA: dois PRs abertos".into()],
            workers: vec![WorkerRecord {
                task_id: "t-beta-1".into(),
                context: "beta".into(),
                workspace: "/home/dev/beta".into(),
                session_id: "new1".into(),
                status: WorkerStatus::Running,
                started_at: "2026-08-14T11:30:00Z".into(),
                summary: "abrir PR do DNS antigo".into(),
            }],
            board: "[doing]\n- Migração do DNS antigo (atualizado 2026-08-14)".into(),
        }
    }

    #[test]
    fn renders_board_section() {
        let text = build("e o board?", &snapshot());
        assert!(text.contains("Quadro de tarefas"));
        assert!(text.contains("Migração do DNS antigo"));
    }

    #[test]
    fn renders_journal_and_workers_sections() {
        let text = build("e agora?", &snapshot());
        assert!(text.contains("como está o beta?"));
        assert!(text.contains("t-beta-1"));
        assert!(text.contains("abrir PR do DNS antigo"));
        assert!(text.contains("running"));
    }

    #[test]
    fn renders_question_sessions_and_context() {
        let text = build("quais as pendências de hoje?", &snapshot());

        assert!(text.contains("quais as pendências de hoje?"));
        assert!(text.contains("Webhook migration"));
        assert!(text.contains("feat/webhook"));
        assert!(text.contains("open the PR"));
        assert!(text.contains("beta-42"));
        assert!(text.contains("dirty files: 3"));
        // Newest session must come before the older one.
        let newest = text.find("Webhook migration").unwrap();
        let oldest = text.find("Alpha refactor").unwrap();
        assert!(newest < oldest);
    }

    #[test]
    fn compacts_long_texts_for_context() {
        let long = format!("start {} end", "x".repeat(500));
        let c = compact(&long, 40);
        assert!(c.chars().count() <= 41); // 40 + ellipsis
        assert!(c.starts_with("start"));
        assert!(c.ends_with('…'));
        // Newlines and runs of spaces collapse to single spaces.
        assert_eq!(compact("a\n\n  b\tc", 100), "a b c");
        // Short texts pass through untouched.
        assert_eq!(compact("oi", 100), "oi");
    }

    #[test]
    fn caps_number_of_sessions_rendered() {
        let mut snap = snapshot();
        snap.sessions = (0..40)
            .map(|i| SessionSummary {
                session_id: format!("s{i}"),
                last_ts: Some(format!("2026-08-14T{:02}:00:00.000Z", i % 24)),
                last_prompt: Some(format!("prompt {i}")),
                ..SessionSummary::new(format!("s{i}"))
            })
            .collect();
        let text = build("q?", &snap);
        let rendered = text.matches("- sessao ").count();
        assert_eq!(rendered, MAX_SESSIONS);
    }

    #[test]
    fn response_schema_is_valid_json_with_expected_fields() {
        let schema: serde_json::Value = serde_json::from_str(RESPONSE_SCHEMA).unwrap();
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "fala"));
        assert!(required.iter().any(|v| v == "detalhes"));
        assert_eq!(schema["properties"]["itens"]["type"], "array");
    }
}
