//! The three-layer answer planner — the single biggest token saver.
//!
//! Layer 1 (Local): deterministic facts (board, running workers, measured
//! spend) answered from the index with pt-BR templates. Zero tokens.
//! Layer 2 (MiniFormat): the model only WRITES over data Vox already has
//! (light model, minimal context) instead of digesting the full snapshot.
//! Layer 3 (FullAsk): real reasoning, routed as before.
//!
//! Quality guard: anything uncertain falls through to FullAsk. Never
//! answer locally on a guess.

use crate::domain::board::{Task, TaskStatus};
use crate::domain::claude_event::VoiceReply;
use crate::domain::memory::{WorkerRecord, WorkerStatus};

#[derive(Debug, Clone, PartialEq)]
pub enum AnswerPlan {
    /// Answered right here, zero tokens.
    Local(VoiceReply),
    /// The model only phrases this pre-built minimal context.
    MiniFormat { context: String },
    /// Full snapshot + routed model (the current pipeline).
    FullAsk,
}

/// Everything the local layers may use — all free, already on disk.
pub struct LocalFacts<'a> {
    pub board: &'a [Task],
    pub workers: &'a [WorkerRecord],
    pub spend_day_usd: Option<f64>,
    pub spend_week_usd: Option<f64>,
}

/// Phrases that force the full pipeline no matter what ("pensa melhor").
const FORCE_FULL: &[&str] = &[
    "pensa melhor", "pensa bem", "analisa", "investiga", "por que", "porque",
    "como resolvo", "como faço", "detalha", "explica",
];

/// Decide the cheapest layer that still answers WELL.
pub fn plan_answer(question: &str, facts: &LocalFacts) -> AnswerPlan {
    let q = normalize(question);
    if FORCE_FULL.iter().any(|p| q.contains(p)) {
        return AnswerPlan::FullAsk;
    }

    // Layer 1: deterministic questions with deterministic answers.
    if asks_running(&q) {
        return AnswerPlan::Local(running_reply(facts.workers));
    }
    if asks_board(&q) {
        return AnswerPlan::Local(board_reply(facts.board));
    }
    if asks_spend(&q) {
        return AnswerPlan::Local(spend_reply(&q, facts));
    }

    // Layer 2: pure phrasing over data Vox already holds.
    if asks_mini_summary(&q) {
        return AnswerPlan::MiniFormat {
            context: mini_context(facts),
        };
    }

    AnswerPlan::FullAsk
}

/// Prompt for the mini-format layer: minimal context + the question,
/// hundreds of tokens instead of the multi-thousand snapshot.
pub fn mini_prompt(context: &str, question: &str) -> String {
    format!(
        "Dados locais do orquestrador (completos para esta pergunta):\n\n{context}\n\nPergunta do usuario (por voz):\n{question}"
    )
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' => 'u',
            'ç' => 'c',
            _ => c,
        })
        .collect()
}

fn asks_running(q: &str) -> bool {
    q.contains("rodando")
        || q.contains("workers ativos")
        || q.contains("worker ativo")
        || (q.contains("executando") && (q.contains("task") || q.contains("tarefa") || q.contains("o que")))
}

fn asks_board(q: &str) -> bool {
    let about_board = q.contains("board") || q.contains("quadro");
    (about_board && (q.starts_with("o que") || q.contains("status") || q.contains("como esta") || q.contains("tem no")))
        || q.contains("quantas tasks")
        || q.contains("quantas tarefas")
}

fn asks_spend(q: &str) -> bool {
    q.contains("quanto gastei")
        || q.contains("quanto gastamos")
        || q.contains("quanto custou o dia")
        || q.contains("qual o gasto")
        || q.contains("quanto ja gastei")
}

fn asks_mini_summary(q: &str) -> bool {
    (q.starts_with("resume") || q.starts_with("resumo") || q.starts_with("me resume"))
        && (q.contains("board") || q.contains("quadro") || q.contains("dia")
            || q.contains("task") || q.contains("tarefa") || q.contains("semana"))
}

fn running(workers: &[WorkerRecord]) -> Vec<&WorkerRecord> {
    workers
        .iter()
        .filter(|w| matches!(w.status, WorkerStatus::Running))
        .collect()
}

fn running_reply(workers: &[WorkerRecord]) -> VoiceReply {
    let live = running(workers);
    let fala = match live.len() {
        0 => "Nenhuma task rodando agora.".to_string(),
        1 => format!("Uma task rodando: {}.", clip(&live[0].summary, 60)),
        n => format!("{n} tasks rodando agora."),
    };
    VoiceReply {
        fala,
        detalhes: if live.is_empty() {
            "Nenhum worker ativo no registro.".into()
        } else {
            "Workers ativos, mais recente primeiro.".into()
        },
        itens: live.iter().map(|w| clip(&w.summary, 90)).collect(),
        board: Vec::new(),
    }
}

fn board_reply(tasks: &[Task]) -> VoiceReply {
    let count = |s: TaskStatus| tasks.iter().filter(|t| t.status == s).count();
    let (doing, waiting, backlog, done) = (
        count(TaskStatus::Doing),
        count(TaskStatus::Waiting),
        count(TaskStatus::Backlog),
        count(TaskStatus::Done),
    );
    let fala = if tasks.is_empty() {
        "O board está vazio.".to_string()
    } else {
        format!("No board: {doing} em andamento, {waiting} esperando, {backlog} no backlog e {done} concluídas.")
    };
    VoiceReply {
        fala,
        detalhes: "Contagem direta do board local.".into(),
        itens: tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Doing || t.status == TaskStatus::Waiting)
            .map(|t| format!("{} [{}]", clip(&t.title, 70), status_label(t.status)))
            .collect(),
        board: Vec::new(),
    }
}

fn spend_reply(q: &str, facts: &LocalFacts) -> VoiceReply {
    let day = facts.spend_day_usd.unwrap_or(0.0);
    let week = facts.spend_week_usd.unwrap_or(0.0);
    let fala = if q.contains("semana") {
        format!("Na semana: {} em turnos medidos.", usd(week))
    } else {
        format!("Hoje: {}. Na semana: {}.", usd(day), usd(week))
    };
    VoiceReply {
        fala,
        detalhes: "Valores do ledger local (turnos do Vox medidos pelo CLI).".into(),
        itens: vec![format!("24h: {}", usd(day)), format!("7 dias: {}", usd(week))],
        board: Vec::new(),
    }
}

/// Minimal context for the phrasing layer: board + running workers.
fn mini_context(facts: &LocalFacts) -> String {
    let mut out = String::from("## Board\n");
    for t in facts.board.iter().filter(|t| t.status != TaskStatus::Done) {
        out.push_str(&format!(
            "- {} [{}]{}\n",
            clip(&t.title, 80),
            status_label(t.status),
            t.note.as_deref().map(|n| format!(" — {}", clip(n, 100))).unwrap_or_default(),
        ));
    }
    let live = running(facts.workers);
    if !live.is_empty() {
        out.push_str("\n## Rodando agora\n");
        for w in live {
            out.push_str(&format!("- {}\n", clip(&w.summary, 100)));
        }
    }
    if let (Some(day), Some(week)) = (facts.spend_day_usd, facts.spend_week_usd) {
        out.push_str(&format!("\n## Gasto medido\n- 24h: {}\n- 7d: {}\n", usd(day), usd(week)));
    }
    clip(&out, 1500)
}

fn status_label(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Doing => "em andamento",
        TaskStatus::Waiting => "esperando",
        TaskStatus::Backlog => "backlog",
        TaskStatus::Done => "concluída",
    }
}

fn usd(v: f64) -> String {
    format!("${v:.2}")
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        text.chars().take(max).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(title: &str, status: TaskStatus) -> Task {
        Task {
            title: title.into(),
            status,
            workspace: None,
            session_ids: Vec::new(),
            updated_at: "2026-08-17T10:00:00Z".into(),
            note: None,
            pinned: false,
        }
    }

    fn worker(summary: &str, status: WorkerStatus) -> WorkerRecord {
        WorkerRecord {
            task_id: "t-1".into(),
            context: String::new(),
            workspace: "/p/vox".into(),
            session_id: "s-1".into(),
            status,
            started_at: "2026-08-17T09:00:00Z".into(),
            summary: summary.into(),
        }
    }

    fn facts<'a>(board: &'a [Task], workers: &'a [WorkerRecord]) -> LocalFacts<'a> {
        LocalFacts {
            board,
            workers,
            spend_day_usd: Some(0.42),
            spend_week_usd: Some(3.10),
        }
    }

    #[test]
    fn running_questions_answer_locally() {
        let workers = vec![
            worker("migra o webhook", WorkerStatus::Running),
            worker("terminou ontem", WorkerStatus::Done),
        ];
        for q in [
            "o que tá rodando?",
            "tem task rodando agora?",
            "quais workers ativos?",
            "o que você está executando?",
        ] {
            let AnswerPlan::Local(reply) = plan_answer(q, &facts(&[], &workers)) else {
                panic!("{q} deveria ser local");
            };
            assert!(reply.fala.contains("Uma task rodando"), "{q} → {}", reply.fala);
            assert_eq!(reply.itens.len(), 1);
        }
    }

    #[test]
    fn board_questions_answer_locally_with_counts() {
        let board = vec![
            task("Migração Carteira", TaskStatus::Doing),
            task("Alertas", TaskStatus::Waiting),
            task("Velha", TaskStatus::Done),
        ];
        for q in ["o que tem no board?", "status do board", "quantas tasks temos?"] {
            let AnswerPlan::Local(reply) = plan_answer(q, &facts(&board, &[])) else {
                panic!("{q} deveria ser local");
            };
            assert!(reply.fala.contains("1 em andamento"), "{q} → {}", reply.fala);
        }
    }

    #[test]
    fn spend_questions_read_the_ledger() {
        let AnswerPlan::Local(reply) = plan_answer("quanto gastei hoje?", &facts(&[], &[])) else {
            panic!("gasto deveria ser local");
        };
        assert!(reply.fala.contains("$0.42"));
        let AnswerPlan::Local(reply) =
            plan_answer("quanto gastei nessa semana?", &facts(&[], &[]))
        else {
            panic!();
        };
        assert!(reply.fala.contains("$3.10"));
    }

    #[test]
    fn summaries_use_mini_format_with_minimal_context() {
        let board = vec![task("Migração Carteira", TaskStatus::Doing)];
        let workers = vec![worker("roda testes", WorkerStatus::Running)];
        let plan = plan_answer("resume o board pra mim", &facts(&board, &workers));
        let AnswerPlan::MiniFormat { context } = plan else {
            panic!("resumo deveria ser mini-format");
        };
        assert!(context.contains("Migração Carteira"));
        assert!(context.contains("roda testes"));
        assert!(context.len() <= 1600, "contexto mínimo, não snapshot");
        assert!(matches!(
            plan_answer("resumo do dia", &facts(&board, &workers)),
            AnswerPlan::MiniFormat { .. }
        ));
    }

    #[test]
    fn real_questions_fall_through_to_full_ask() {
        let board = vec![task("Migração Carteira", TaskStatus::Doing)];
        for q in [
            "quais as pendências de hoje?",
            "por que a task do webhook travou?",
            "o que ficou pendente no projeto nu?",
            "como faço pra migrar o DNS?",
            "qual PR está aberto?",
            "onde parei na migração ontem?",
            "resume o que aconteceu na sessão do Gladius", // sessão ≠ dado local
        ] {
            assert_eq!(
                plan_answer(q, &facts(&board, &[])),
                AnswerPlan::FullAsk,
                "{q} exige o pipeline completo"
            );
        }
    }

    #[test]
    fn escape_phrases_force_full_even_on_local_shapes() {
        assert_eq!(
            plan_answer("analisa o que tá rodando", &facts(&[], &[])),
            AnswerPlan::FullAsk
        );
        assert_eq!(
            plan_answer("pensa melhor: quanto gastei hoje?", &facts(&[], &[])),
            AnswerPlan::FullAsk
        );
    }

    #[test]
    fn empty_worlds_have_honest_answers() {
        let AnswerPlan::Local(reply) = plan_answer("o que tá rodando?", &facts(&[], &[])) else {
            panic!();
        };
        assert!(reply.fala.contains("Nenhuma"));
        let AnswerPlan::Local(reply) = plan_answer("o que tem no board?", &facts(&[], &[])) else {
            panic!();
        };
        assert!(reply.fala.contains("vazio"));
    }

    #[test]
    fn mini_prompt_carries_context_and_question() {
        let prompt = mini_prompt("## Board\n- X", "resume o board");
        assert!(prompt.contains("## Board"));
        assert!(prompt.contains("resume o board"));
        assert!(prompt.len() < 400);
    }
}
